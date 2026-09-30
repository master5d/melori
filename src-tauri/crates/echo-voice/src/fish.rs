//! Fish Audio cloud TTS engine (S2.1 Pro). Mirrors SidecarRemote's blocking
//! construction: reqwest + current-thread runtime + run_blocking.
use crate::{SynthOpts, VoiceEngine, VoiceError, VoiceProfile};

pub struct FishConfig {
    pub base_url: String,
    pub api_key: String,
    pub model_id: String,
    pub tts_model: String,
    pub timeout_secs: u64,
}

pub struct FishRemote {
    cfg: FishConfig,
    client: reqwest::Client,
    rt: Option<tokio::runtime::Runtime>,
}

impl Drop for FishRemote {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::spawn(move || drop(rt));
            }
        }
    }
}

/// Append `chunk` to the s16le decode state: whole samples go to `out`,
/// a trailing odd byte stays in `carry` until the next chunk.
pub(crate) fn drain_s16le(carry: &mut Vec<u8>, chunk: &[u8], out: &mut Vec<i16>) {
    carry.extend_from_slice(chunk);
    let pairs = carry.len() / 2;
    for i in 0..pairs {
        out.push(i16::from_le_bytes([carry[2 * i], carry[2 * i + 1]]));
    }
    carry.drain(..pairs * 2);
}

impl FishRemote {
    pub fn new(cfg: FishConfig) -> Result<Self, VoiceError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
            .build()
            .map_err(|e| VoiceError::Engine(format!("http client: {e}")))?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| VoiceError::Engine(format!("runtime: {e}")))?;
        Ok(Self {
            cfg: FishConfig {
                base_url: cfg.base_url.trim_end_matches('/').to_string(),
                ..cfg
            },
            client,
            rt: Some(rt),
        })
    }

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

    fn request(
        &self,
        text: &str,
        format: &str,
        latency: &str,
        sample_rate: Option<u32>,
    ) -> reqwest::RequestBuilder {
        let mut body = serde_json::json!({
            "text": text,
            "reference_id": self.cfg.model_id,
            "format": format,
            "latency": latency,
        });
        if let Some(sr) = sample_rate {
            body["sample_rate"] = serde_json::json!(sr);
        }
        self.client
            .post(format!("{}/v1/tts", self.cfg.base_url))
            .bearer_auth(&self.cfg.api_key)
            .header("model", &self.cfg.tts_model)
            .json(&body)
    }
}

/// Status + up to 200 bytes of body; never includes request headers (=> never the key).
fn http_fail(status: reqwest::StatusCode, body: &str) -> VoiceError {
    let mut b = body.to_string();
    b.truncate(200);
    VoiceError::Engine(format!("fish tts: HTTP {status}: {b}"))
}

impl VoiceEngine for FishRemote {
    fn id(&self) -> &'static str {
        "fish_remote"
    }

    fn synthesize(
        &self,
        text: &str,
        _profile: &VoiceProfile,
        _opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError> {
        self.run_blocking(async {
            let resp = self
                .request(text, "wav", "normal", None)
                .send()
                .await
                .map_err(|e| VoiceError::Engine(format!("fish tts: {e}")))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(http_fail(status, &body));
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| VoiceError::Engine(format!("fish tts body: {e}")))?;
            Ok(bytes.to_vec())
        })
    }

    fn synthesize_stream(
        &self,
        text: &str,
        _profile: &VoiceProfile,
        _opts: &SynthOpts,
        cancel: &std::sync::atomic::AtomicBool,
        on_pcm: &mut (dyn FnMut(&[i16]) + Send),
    ) -> Result<(), VoiceError> {
        self.run_blocking(async {
            let mut resp = self
                .request(text, "pcm", "low", Some(24_000))
                .send()
                .await
                .map_err(|e| VoiceError::Engine(format!("fish tts: {e}")))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(http_fail(status, &body));
            }
            let mut carry: Vec<u8> = Vec::new();
            let mut samples: Vec<i16> = Vec::new();
            while let Some(chunk) = resp
                .chunk()
                .await
                .map_err(|e| VoiceError::Engine(format!("fish tts stream: {e}")))?
            {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(()); // dropping resp closes the connection
                }
                samples.clear();
                drain_s16le(&mut carry, &chunk, &mut samples);
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
    use crate::{LanguageHint, SynthOpts, VoiceEngine, VoiceProfile};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn prof() -> VoiceProfile {
        VoiceProfile {
            id: "p".into(),
            display_name: "P".into(),
            language_hint: LanguageHint::Ru,
            created: "n".into(),
            notes: String::new(),
            refs: vec![],
        }
    }

    /// Одноразовый mock-HTTP сервер: читает запрос до конца заголовков + body,
    /// отвечает `status_line` + body, возвращает (адрес, JoinHandle с сырым запросом).
    fn one_shot_server(
        status_line: &'static str,
        body: Vec<u8>,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let mut req = Vec::new();
            // читаем, пока не получим headers + Content-Length байт тела
            loop {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some(hdr_end) = text.find("\r\n\r\n") {
                    let cl = text.lines().find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    });
                    let have = req.len() - (hdr_end + 4);
                    if cl.map_or(true, |c| have >= c) {
                        break;
                    }
                }
            }
            let resp = format!(
                "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            s.write_all(resp.as_bytes()).unwrap();
            s.write_all(&body).unwrap();
            String::from_utf8_lossy(&req).into_owned()
        });
        (addr, h)
    }

    fn cfg(base: String) -> FishConfig {
        FishConfig {
            base_url: base,
            api_key: "SECRET-KEY-42".into(),
            model_id: "modelXYZ".into(),
            tts_model: "s2.1-pro-free".into(),
            timeout_secs: 5,
        }
    }

    #[test]
    fn synthesize_returns_wav_and_sends_contract_headers() {
        let (addr, h) = one_shot_server("HTTP/1.1 200 OK", b"RIFFdata".to_vec());
        let eng = FishRemote::new(cfg(addr)).unwrap();
        let out = eng
            .synthesize("привет", &prof(), &SynthOpts::default())
            .unwrap();
        assert_eq!(out, b"RIFFdata");
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /v1/tts"));
        assert!(
            req.contains("authorization: Bearer SECRET-KEY-42")
                || req.contains("Authorization: Bearer SECRET-KEY-42")
        );
        assert!(req.to_ascii_lowercase().contains("model: s2.1-pro-free"));
        assert!(req.contains("\"reference_id\":\"modelXYZ\""));
    }

    #[test]
    fn non_2xx_maps_to_engine_error_without_key() {
        let (addr, _h) = one_shot_server(
            "HTTP/1.1 402 Payment Required",
            br#"{"message":"Insufficient API credit","status":402}"#.to_vec(),
        );
        let eng = FishRemote::new(cfg(addr)).unwrap();
        let err = eng
            .synthesize("x", &prof(), &SynthOpts::default())
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("402"), "want status in error, got: {msg}");
        assert!(
            !msg.contains("SECRET-KEY-42"),
            "key leaked into error: {msg}"
        );
    }

    #[test]
    fn stream_delivers_pcm_and_survives_odd_chunk_boundary() {
        // 5 сэмплов s16le = 10 байт; сервер шлёт их одним телом — важно, что
        // drain_s16le не теряет байты на нечётных границах (юнит ниже), а
        // стрим целиком доносит все сэмплы.
        let pcm: Vec<u8> = vec![1, 0, 2, 0, 3, 0, 4, 0, 5, 0];
        let (addr, _h) = one_shot_server("HTTP/1.1 200 OK", pcm);
        let eng = FishRemote::new(cfg(addr)).unwrap();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut got: Vec<i16> = Vec::new();
        eng.synthesize_stream("x", &prof(), &SynthOpts::default(), &cancel, &mut |s| {
            got.extend_from_slice(s)
        })
        .unwrap();
        assert_eq!(got, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn drain_s16le_carries_odd_byte() {
        let mut carry = Vec::new();
        let mut out = Vec::new();
        drain_s16le(&mut carry, &[1, 0, 2], &mut out);
        assert_eq!(out, vec![1i16]);
        assert_eq!(carry, vec![2]);
        drain_s16le(&mut carry, &[0], &mut out);
        assert_eq!(out, vec![1i16, 2]);
        assert!(carry.is_empty());
    }
}
