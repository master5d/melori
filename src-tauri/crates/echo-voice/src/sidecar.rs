use std::collections::HashSet;
use std::sync::Mutex;

use crate::{SynthOpts, VoiceEngine, VoiceError, VoiceProfile};

pub struct SidecarConfig {
    pub base_url: String,
    pub timeout_secs: u64,
}

pub struct SidecarRemote {
    base_url: String,
    client: reqwest::Client,
    rt: Option<tokio::runtime::Runtime>,
    uploaded: Mutex<HashSet<String>>,
}

impl Drop for SidecarRemote {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            // Dropping a Runtime inside a tokio async context panics ("Cannot drop a runtime in a
            // context where blocking is not allowed"), so offload to a detached thread.
            if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::spawn(move || drop(rt));
            }
            // else: drop normally here
        }
    }
}

impl SidecarRemote {
    /// Run a future to completion synchronously, safe from any caller context.
    /// If we're already inside a tokio runtime, `Runtime::block_on` would panic
    /// ("Cannot start a runtime from within a runtime"), so we offload to a plain
    /// scoped thread that has no ambient runtime. Otherwise we block directly.
    fn run_blocking<F, T>(&self, fut: F) -> T
    where
        F: std::future::Future<Output = T> + Send,
        T: Send,
    {
        let rt = self.rt.as_ref().unwrap();
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::scope(|s| s.spawn(|| rt.block_on(fut)).join().unwrap())
        } else {
            rt.block_on(fut)
        }
    }

    pub fn new(cfg: SidecarConfig) -> Result<Self, VoiceError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
            .build()
            .map_err(|e| VoiceError::Engine(format!("http client: {e}")))?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| VoiceError::Engine(format!("runtime: {e}")))?;
        Ok(Self {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            client,
            rt: Some(rt),
            uploaded: Mutex::new(HashSet::new()),
        })
    }

    pub fn health(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        self.run_blocking(async {
            matches!(self.client.get(&url).send().await, Ok(r) if r.status().is_success())
        })
    }

    fn ensure_uploaded(&self, profile: &VoiceProfile) -> Result<(), VoiceError> {
        {
            let seen = self.uploaded.lock().unwrap();
            if seen.contains(&profile.id) {
                return Ok(());
            }
        }
        let r = profile
            .refs
            .first()
            .ok_or_else(|| VoiceError::Profile(format!("profile {} has no refs", profile.id)))?;
        let wav = std::fs::read(&r.wav).map_err(|e| VoiceError::Io(e.to_string()))?;
        let url = format!("{}/v1/voices", self.base_url);
        let id = profile.id.clone();
        let transcript = r.transcript.clone();
        self.run_blocking(async {
            let form = reqwest::multipart::Form::new()
                .text("voice_id", id)
                .text("transcript", transcript)
                .part(
                    "audio",
                    reqwest::multipart::Part::bytes(wav).file_name("ref.wav"),
                );
            self.client
                .post(&url)
                .multipart(form)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| VoiceError::Engine(format!("voice upload: {e}")))
        })?;
        self.uploaded.lock().unwrap().insert(profile.id.clone());
        Ok(())
    }
}

impl VoiceEngine for SidecarRemote {
    fn id(&self) -> &'static str {
        "sidecar_remote"
    }

    fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        _opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError> {
        self.ensure_uploaded(profile)?;
        let url = format!("{}/v1/audio/speech", self.base_url);
        let body = serde_json::json!({
            "input": text,
            "voice": profile.id,
            "response_format": "wav"
        });
        let bytes = self.run_blocking(async {
            let resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| VoiceError::Engine(format!("speech: {e}")))?;
            resp.bytes()
                .await
                .map_err(|e| VoiceError::Engine(format!("speech body: {e}")))
        })?;
        let bytes = bytes.to_vec();
        if !bytes.starts_with(b"RIFF") {
            return Err(VoiceError::Engine("sidecar output is not a WAV".into()));
        }
        Ok(bytes)
    }

    fn synthesize_stream(
        &self,
        text: &str,
        profile: &VoiceProfile,
        _opts: &SynthOpts,
        cancel: &std::sync::atomic::AtomicBool,
        on_pcm: &mut (dyn FnMut(&[i16]) + Send),
    ) -> Result<(), VoiceError> {
        use std::sync::atomic::Ordering;
        self.ensure_uploaded(profile)?;
        let url = format!("{}/v1/audio/speech/stream", self.base_url);
        let body = serde_json::json!({ "input": text, "voice": profile.id });
        self.run_blocking(async {
            let mut resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| VoiceError::Engine(format!("stream: {e}")))?;

            let mut asm = crate::pcm::PcmStreamAssembler::new();

            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let chunk = resp
                    .chunk()
                    .await
                    .map_err(|e| VoiceError::Engine(format!("stream body: {e}")))?;
                let Some(bytes) = chunk else { break };

                let mut samples: Vec<i16> = Vec::new();
                asm.feed(&bytes, &mut samples)?;
                if !samples.is_empty() {
                    on_pcm(&samples);
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LanguageHint, VoiceRef};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn profile() -> VoiceProfile {
        let tmp = std::env::temp_dir().join("sidecar-ref.wav");
        std::fs::write(&tmp, b"RIFFxxxxWAVE").unwrap();
        VoiceProfile {
            id: "sasha".into(),
            display_name: "S".into(),
            language_hint: LanguageHint::Mixed,
            created: "now".into(),
            notes: String::new(),
            refs: vec![VoiceRef {
                wav: tmp,
                transcript: "hi".into(),
                duration_s: 1.0,
            }],
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn synthesize_posts_and_returns_wav() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/voices"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/speech"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"RIFFdata".to_vec()))
            .mount(&server)
            .await;

        let base = server.uri();
        let bytes = tokio::task::spawn_blocking(move || {
            let eng = SidecarRemote::new(SidecarConfig {
                base_url: base,
                timeout_secs: 10,
            })
            .unwrap();
            eng.synthesize("hello", &profile(), &SynthOpts::default())
        })
        .await
        .unwrap()
        .unwrap();
        assert!(bytes.starts_with(b"RIFF"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn synthesize_from_within_runtime_does_not_panic() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/voices"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/speech"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"RIFFdata".to_vec()))
            .mount(&server)
            .await;
        let base = server.uri();
        // Called directly on the async test task (ambient runtime present) — must NOT panic.
        let eng = SidecarRemote::new(SidecarConfig {
            base_url: base,
            timeout_secs: 10,
        })
        .unwrap();
        let bytes = eng
            .synthesize("hello", &profile(), &SynthOpts::default())
            .unwrap();
        assert!(bytes.starts_with(b"RIFF"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn synthesize_stream_reads_preamble_and_pcm() {
        use crate::pcm::PCM_PREAMBLE_MAGIC;
        // body = valid preamble + 3 samples (1, -1, 256) little-endian
        let mut body = Vec::new();
        body.extend_from_slice(PCM_PREAMBLE_MAGIC);
        body.extend_from_slice(&24000u32.to_le_bytes());
        body.push(1);
        body.push(16);
        for v in [1i16, -1, 256] {
            body.extend_from_slice(&v.to_le_bytes());
        }

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/voices"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/speech/stream"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;

        let base = server.uri();
        let got = tokio::task::spawn_blocking(move || {
            let eng = SidecarRemote::new(SidecarConfig {
                base_url: base,
                timeout_secs: 10,
            })
            .unwrap();
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut out: Vec<i16> = Vec::new();
            eng.synthesize_stream(
                "hi",
                &profile(),
                &SynthOpts::default(),
                &cancel,
                &mut |s: &[i16]| out.extend_from_slice(s),
            )
            .unwrap();
            out
        })
        .await
        .unwrap();
        assert_eq!(got, vec![1i16, -1, 256]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn synthesize_stream_bad_preamble_errs() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/voices"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/speech/stream"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"XXXXbaddata!!".to_vec()))
            .mount(&server)
            .await;
        let base = server.uri();
        let (called, r) = tokio::task::spawn_blocking(move || {
            let eng = SidecarRemote::new(SidecarConfig {
                base_url: base,
                timeout_secs: 10,
            })
            .unwrap();
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let mut called = false;
            let r = eng.synthesize_stream(
                "hi",
                &profile(),
                &SynthOpts::default(),
                &cancel,
                &mut |_: &[i16]| {
                    called = true;
                },
            );
            (called, r)
        })
        .await
        .unwrap();
        assert!(!called, "on_pcm must not fire on a bad preamble");
        assert!(matches!(r, Err(VoiceError::Engine(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn health_false_when_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let base = server.uri();
        let ok = tokio::task::spawn_blocking(move || {
            SidecarRemote::new(SidecarConfig {
                base_url: base,
                timeout_secs: 2,
            })
            .unwrap()
            .health()
        })
        .await
        .unwrap();
        assert!(!ok);
    }
}
