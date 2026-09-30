//! Bridges echo_voice into the app's TtsEngine trait so the readback flow is
//! unchanged. Loads the selected profile per call (fresh, cheap: one JSON read).
use crate::tts::{TtsEngine, VoiceInfo};
use echo_voice::router::VoiceRouter;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

pub struct ClonedVoiceEngine {
    router: VoiceRouter,
    voices_dir: PathBuf,
    profile_id: String,
}

impl ClonedVoiceEngine {
    pub fn new(router: VoiceRouter, voices_dir: PathBuf, profile_id: String) -> Self {
        Self {
            router,
            voices_dir,
            profile_id,
        }
    }

    /// Whether the low-latency streaming path should run: either the cloud tier
    /// is present (its health = fall-through on error), or the tier permits the
    /// sidecar AND the sidecar is healthy right now.
    pub fn stream_eligible(&self) -> bool {
        self.router.has_cloud() || (self.router.allows_sidecar() && self.router.health())
    }

    /// Stream a readback: chunk `text`, synthesize each chunk in order through the
    /// router, and emit each result via `on_chunk`. Loads+validates the profile once.
    pub fn speak_stream(
        &self,
        text: &str,
        voice_id: Option<&str>,
        cancel: &AtomicBool,
        on_chunk: &mut dyn FnMut(echo_voice::stream::ChunkOutput),
    ) -> Result<(), String> {
        let id = voice_id.unwrap_or(&self.profile_id);
        let profile =
            echo_voice::profile::load_profile(&self.voices_dir, id).map_err(|e| e.to_string())?;
        echo_voice::profile::validate_profile(&profile).map_err(|e| e.to_string())?;
        let opts = echo_voice::SynthOpts { rate: 1.0 };
        let used = echo_voice::stream::synthesize_stream(
            text,
            &profile,
            &self.router,
            &opts,
            echo_voice::stream::READBACK_MAX_CHARS,
            cancel,
            on_chunk,
        )
        .map_err(|e| e.to_string())?;
        log::info!("cloned readback streamed via {used}");
        Ok(())
    }

    /// Stream PCM for a readback: loads+validates the profile, routes to the sidecar
    /// streaming tier, forwarding samples to `on_pcm`.
    pub fn synthesize_stream_pcm(
        &self,
        text: &str,
        voice_id: Option<&str>,
        cancel: &std::sync::atomic::AtomicBool,
        on_pcm: &mut (dyn FnMut(&[i16]) + Send),
    ) -> Result<(), String> {
        let id = voice_id.unwrap_or(&self.profile_id);
        let profile =
            echo_voice::profile::load_profile(&self.voices_dir, id).map_err(|e| e.to_string())?;
        echo_voice::profile::validate_profile(&profile).map_err(|e| e.to_string())?;
        let opts = echo_voice::SynthOpts { rate: 1.0 };
        self.router
            .synthesize_stream(text, &profile, &opts, cancel, on_pcm)
            .map_err(|e| e.to_string())
    }
}

impl TtsEngine for ClonedVoiceEngine {
    fn list_voices(&self) -> Result<Vec<VoiceInfo>, String> {
        let profiles =
            echo_voice::profile::list_profiles(&self.voices_dir).map_err(|e| e.to_string())?;
        Ok(profiles
            .into_iter()
            .map(|p| VoiceInfo {
                id: p.id,
                display_name: p.display_name,
                language: match p.language_hint {
                    echo_voice::LanguageHint::Ru => "ru".into(),
                    echo_voice::LanguageHint::En => "en".into(),
                    echo_voice::LanguageHint::Mixed => "mixed".into(),
                },
            })
            .collect())
    }

    fn synthesize(&self, text: &str, voice_id: Option<&str>, rate: f32) -> Result<Vec<u8>, String> {
        let id = voice_id.unwrap_or(&self.profile_id);
        let profile =
            echo_voice::profile::load_profile(&self.voices_dir, id).map_err(|e| e.to_string())?;
        echo_voice::profile::validate_profile(&profile).map_err(|e| e.to_string())?;
        let opts = echo_voice::SynthOpts { rate };
        self.router
            .synthesize(text, &profile, &opts)
            .map(|(wav, _used)| wav)
            .map_err(|e| e.to_string())
    }
}
