//! echo-voice: Tauri-free cloned-voice synthesis. The host app injects paths
//! and settings; this crate holds no AppHandle. Engines implement `VoiceEngine`.
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

// VoiceTier is defined in echo-config (single source of truth) and re-exported
// here so router.rs and callers can use `echo_voice::VoiceTier` unchanged.
pub use echo_config::VoiceTier;

pub mod chunker;
pub mod fish;
pub mod narrate;
pub mod pcm;
pub mod profile;
pub mod router;
pub mod sidecar;
pub mod stream;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
pub struct VoiceRef {
    pub wav: PathBuf,
    pub transcript: String,
    pub duration_s: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LanguageHint {
    Ru,
    En,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
pub struct VoiceProfile {
    pub id: String,
    pub display_name: String,
    pub language_hint: LanguageHint,
    pub created: String,
    #[serde(default)]
    pub notes: String,
    pub refs: Vec<VoiceRef>,
}

#[derive(Debug, Clone, Copy)]
pub struct SynthOpts {
    pub rate: f32,
}

impl Default for SynthOpts {
    fn default() -> Self {
        Self { rate: 1.0 }
    }
}

#[derive(Debug, Clone)]
pub enum VoiceError {
    NoEngine,
    Engine(String),
    Profile(String),
    Io(String),
}

impl std::fmt::Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoiceError::NoEngine => write!(f, "no voice engine available"),
            VoiceError::Engine(m) => write!(f, "voice engine error: {m}"),
            VoiceError::Profile(m) => write!(f, "voice profile error: {m}"),
            VoiceError::Io(m) => write!(f, "voice io error: {m}"),
        }
    }
}

impl std::error::Error for VoiceError {}

/// A synthesis backend. `synthesize` returns WAV bytes (RIFF container).
pub trait VoiceEngine: Send + Sync {
    fn id(&self) -> &'static str;
    fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError>;

    /// Stream s16 PCM samples via `on_pcm` as they arrive. Default: unsupported.
    /// Only the sidecar implements this (Lever 2).
    fn synthesize_stream(
        &self,
        _text: &str,
        _profile: &VoiceProfile,
        _opts: &SynthOpts,
        _cancel: &std::sync::atomic::AtomicBool,
        _on_pcm: &mut (dyn FnMut(&[i16]) + Send),
    ) -> Result<(), VoiceError> {
        Err(VoiceError::Engine("streaming not supported".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_opts_default_rate_is_one() {
        assert_eq!(SynthOpts::default().rate, 1.0);
    }

    #[test]
    fn language_hint_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&LanguageHint::Mixed).unwrap(),
            "\"mixed\""
        );
        // VoiceTier serialization is tested in echo-config's voice_enums_serialize_snake_case
    }

    #[test]
    fn voice_error_displays() {
        let e = VoiceError::Profile("bad".into());
        assert!(format!("{e}").contains("bad"));
    }

    #[test]
    fn default_synthesize_stream_is_unsupported() {
        struct Dummy;
        impl VoiceEngine for Dummy {
            fn id(&self) -> &'static str {
                "dummy"
            }
            fn synthesize(
                &self,
                _t: &str,
                _p: &VoiceProfile,
                _o: &SynthOpts,
            ) -> Result<Vec<u8>, VoiceError> {
                Ok(vec![])
            }
        }
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut sink = |_: &[i16]| {};
        let r = Dummy.synthesize_stream(
            "x",
            &VoiceProfile {
                id: "p".into(),
                display_name: "P".into(),
                language_hint: LanguageHint::En,
                created: "n".into(),
                notes: String::new(),
                refs: vec![],
            },
            &SynthOpts::default(),
            &cancel,
            &mut sink,
        );
        assert!(matches!(r, Err(VoiceError::Engine(_))));
    }
}
