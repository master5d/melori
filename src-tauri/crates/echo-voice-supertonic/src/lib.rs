pub mod config;
pub mod engine;
pub mod text;

use std::path::Path;
use std::sync::Mutex;

use echo_voice::{LanguageHint, SynthOpts, VoiceEngine, VoiceError, VoiceProfile};

use crate::config::{load_voice_style, VoiceStyleData};
use crate::engine::{pcm_f32_to_wav_bytes, SupertonicCore};

// Re-export public API
pub use config::load_config;
pub use config::{AEConfig, Config, StyleComponent, TTLConfig, UnicodeProcessor};
pub use engine::TOTAL_STEP;
// VoiceStyleData and load_voice_style are used internally; re-export as-is via use above

pub fn lang_code(hint: LanguageHint) -> &'static str {
    match hint {
        LanguageHint::En => "en",
        LanguageHint::Ru | LanguageHint::Mixed => "ru",
    }
}

/// Inter-chunk silence duration (seconds), verbatim from the reference
/// implementation's call-site default: `docs/superpowers/reference/supertonic-rust/example_onnx.rs:109`
/// (`text_to_speech.call(..., 0.3)`); the pause-insertion logic itself lives in
/// `helper.rs:707-714` (`Synthesizer::call`).
pub const SILENCE_BETWEEN_CHUNKS_SECS: f32 = 0.3;

/// Concatenates per-chunk PCM, inserting `silence_duration` seconds of silence
/// **between** chunks (never after the last one) — mirrors
/// `docs/superpowers/reference/supertonic-rust/helper.rs:697-715`.
pub(crate) fn join_chunks_with_silence(chunks: Vec<Vec<f32>>, sample_rate: u32) -> Vec<f32> {
    let silence_len = (SILENCE_BETWEEN_CHUNKS_SECS * sample_rate as f32) as usize;
    let mut out: Vec<f32> = Vec::new();
    for (i, chunk) in chunks.into_iter().enumerate() {
        if i > 0 {
            out.extend(std::iter::repeat(0.0f32).take(silence_len));
        }
        out.extend(chunk);
    }
    out
}

/// Local Supertonic TTS engine (whole-WAV). Mutex: ort sessions need &mut for run.
pub struct SupertonicEngine {
    core: Mutex<SupertonicCore>,
    style: VoiceStyleData,
    sample_rate: u32,
}

impl SupertonicEngine {
    pub fn load(model_dir: &Path, style_path: &Path) -> Result<Self, VoiceError> {
        let core = SupertonicCore::load(model_dir)?;
        let sample_rate = core.sample_rate();
        let style = load_voice_style(style_path)?;
        Ok(SupertonicEngine {
            core: Mutex::new(core),
            style,
            sample_rate,
        })
    }
}

impl VoiceEngine for SupertonicEngine {
    fn id(&self) -> &'static str {
        "supertonic_local"
    }

    fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError> {
        if text.trim().is_empty() {
            return pcm_f32_to_wav_bytes(&[], self.sample_rate);
        }
        let lang = lang_code(profile.language_hint);
        let speed = opts.rate.clamp(0.9, 1.5);
        let mut core = self
            .core
            .lock()
            .map_err(|_| VoiceError::Engine("engine poisoned".into()))?;
        let mut chunks: Vec<Vec<f32>> = Vec::new();
        for chunk in crate::text::chunk_text(text, None) {
            if chunk.trim().is_empty() {
                continue;
            }
            chunks.push(core.synthesize_chunk(&chunk, lang, &self.style, speed)?);
        }
        let pcm = join_chunks_with_silence(chunks, self.sample_rate);
        pcm_f32_to_wav_bytes(&pcm, self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use echo_voice::LanguageHint;

    #[test]
    fn lang_mapping_mirrors_piper_pick() {
        assert_eq!(lang_code(LanguageHint::Ru), "ru");
        assert_eq!(lang_code(LanguageHint::En), "en");
        assert_eq!(lang_code(LanguageHint::Mixed), "ru");
    }

    #[test]
    fn join_inserts_silence_only_between_chunks() {
        let sample_rate = 24_000u32;
        let n = 100;
        let chunks = vec![vec![1.0f32; n], vec![2.0f32; n]];
        let joined = join_chunks_with_silence(chunks, sample_rate);
        let expected_silence = (SILENCE_BETWEEN_CHUNKS_SECS * sample_rate as f32) as usize;
        assert_eq!(joined.len(), n * 2 + expected_silence);
        // First chunk immediately, then silence, then second chunk — no trailing silence.
        assert_eq!(&joined[..n], &vec![1.0f32; n][..]);
        assert_eq!(
            &joined[n..n + expected_silence],
            &vec![0.0f32; expected_silence][..]
        );
        assert_eq!(&joined[n + expected_silence..], &vec![2.0f32; n][..]);
    }

    #[test]
    fn join_single_chunk_has_no_silence() {
        let n = 50;
        let chunks = vec![vec![3.0f32; n]];
        let joined = join_chunks_with_silence(chunks, 24_000);
        assert_eq!(joined.len(), n);
        assert_eq!(joined, vec![3.0f32; n]);
    }

    #[test]
    fn load_missing_model_dir_errors() {
        let e = SupertonicEngine::load(
            std::path::Path::new("Z:/absent"),
            std::path::Path::new("Z:/absent/M1.json"),
        );
        assert!(e.is_err());
    }
}
