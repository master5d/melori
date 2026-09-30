pub mod config;
pub mod kitten;
pub mod onnx;
pub mod pcm;
pub mod phonemize;

use std::path::Path;
use std::time::Duration;

use echo_voice::{LanguageHint, SynthOpts, VoiceEngine, VoiceError, VoiceProfile};

use crate::config::ModelConfig;
use crate::onnx::OnnxTts;
use crate::pcm::pcm_f32_to_wav;
use crate::phonemize::Phonemizer;

/// Per-language Piper model bundle.
struct Lang {
    onnx: OnnxTts,
    cfg: ModelConfig,
    espeak_voice: String,
}

/// Local Piper TTS engine. Load via [`LocalTtsEngine::load`].
pub struct LocalTtsEngine {
    ru: Option<Lang>,
    en: Option<Lang>,
    phon: Phonemizer,
}

impl LocalTtsEngine {
    /// Load models from `model_dir`. Expects sub-directories `ru/` and/or `en/`,
    /// each containing `model.onnx` and `model.onnx.json`.
    ///
    /// Returns `Err(VoiceError::NoEngine)` if neither language is available.
    pub fn load(
        model_dir: &Path,
        phonemizer_bin: &Path,
        phonemizer_data_dir: &Path,
    ) -> Result<LocalTtsEngine, VoiceError> {
        let ru = try_build_lang(model_dir, "ru", "ru")?;
        let en = try_build_lang(model_dir, "en", "en-us")?;

        if ru.is_none() && en.is_none() {
            return Err(VoiceError::NoEngine);
        }

        let phon = Phonemizer {
            bin: phonemizer_bin.into(),
            data_dir: phonemizer_data_dir.into(),
            timeout: Duration::from_secs(5),
        };

        Ok(LocalTtsEngine { ru, en, phon })
    }

    /// Build the engine from explicit model file paths (no `<dir>/lang/model.onnx` convention).
    ///
    /// Each language is provided as `Some((onnx_path, onnx_json_path))`. Pass `None` to skip
    /// a language. Returns `Err(VoiceError::NoEngine)` if both are `None`.
    pub fn from_lang_paths(
        ru: Option<(std::path::PathBuf, std::path::PathBuf)>,
        en: Option<(std::path::PathBuf, std::path::PathBuf)>,
        phonemizer_bin: &Path,
        phonemizer_data_dir: &Path,
    ) -> Result<LocalTtsEngine, VoiceError> {
        let ru = ru
            .map(|(onnx, json)| build_lang_from_paths(&onnx, &json, "ru"))
            .transpose()?;
        let en = en
            .map(|(onnx, json)| build_lang_from_paths(&onnx, &json, "en-us"))
            .transpose()?;

        if ru.is_none() && en.is_none() {
            return Err(VoiceError::NoEngine);
        }

        let phon = Phonemizer {
            bin: phonemizer_bin.into(),
            data_dir: phonemizer_data_dir.into(),
            timeout: Duration::from_secs(5),
        };

        Ok(LocalTtsEngine { ru, en, phon })
    }

    /// Select the language branch for the given `hint`.
    ///
    /// `En` → en branch; `Ru | Mixed` → ru branch (espeak `ru` handles Cyrillic;
    /// Mixed defaults to ru). Returns `Err` when the required branch is absent.
    fn pick(&self, hint: LanguageHint) -> Result<&Lang, VoiceError> {
        let opt = match hint {
            LanguageHint::En => &self.en,
            LanguageHint::Ru | LanguageHint::Mixed => &self.ru,
        };
        opt.as_ref()
            .ok_or_else(|| VoiceError::Engine("language unavailable".into()))
    }
}

/// Build a [`Lang`] from explicit onnx and json file paths.
fn build_lang_from_paths(
    onnx_path: &Path,
    json_path: &Path,
    espeak_voice: &str,
) -> Result<Lang, VoiceError> {
    let json = std::fs::read_to_string(json_path)
        .map_err(|e| VoiceError::Io(format!("read model config: {e}")))?;
    let cfg = ModelConfig::from_json_str(&json)?;
    let onnx = OnnxTts::load(onnx_path, cfg.sample_rate)?;
    Ok(Lang {
        onnx,
        cfg,
        espeak_voice: espeak_voice.into(),
    })
}

/// Try to build a [`Lang`] from `model_dir/<lang>/model.onnx` + `model.onnx.json`.
/// Returns `Ok(None)` when either file is absent (lang is simply skipped).
fn try_build_lang(
    model_dir: &Path,
    lang: &str,
    espeak_voice: &str,
) -> Result<Option<Lang>, VoiceError> {
    let onnx_path = model_dir.join(lang).join("model.onnx");
    let json_path = model_dir.join(lang).join("model.onnx.json");

    if !onnx_path.exists() || !json_path.exists() {
        return Ok(None);
    }

    Ok(Some(build_lang_from_paths(
        &onnx_path,
        &json_path,
        espeak_voice,
    )?))
}

impl VoiceEngine for LocalTtsEngine {
    fn id(&self) -> &'static str {
        "piper_local"
    }

    fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError> {
        let lang = self.pick(profile.language_hint)?;

        if text.trim().is_empty() {
            return pcm_f32_to_wav(&[], lang.cfg.sample_rate);
        }

        let ids = self
            .phon
            .text_to_ids(text, &lang.espeak_voice, &lang.cfg.phoneme_id_map)?;
        let pcm = lang.onnx.infer(&ids, opts.rate)?;
        pcm_f32_to_wav(&pcm, lang.cfg.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Build a dummy (non-functional) `Lang` for routing tests only.
    fn dummy_lang(sample_rate: u32, espeak: &str) -> Lang {
        Lang {
            onnx: OnnxTts::dummy(sample_rate),
            cfg: ModelConfig {
                sample_rate,
                phoneme_id_map: HashMap::new(),
                num_speakers: 1,
            },
            espeak_voice: espeak.into(),
        }
    }

    /// Build a `LocalTtsEngine` with the specified branches populated.
    fn engine_with(ru_present: bool, en_present: bool) -> LocalTtsEngine {
        LocalTtsEngine {
            ru: if ru_present {
                Some(dummy_lang(22050, "ru"))
            } else {
                None
            },
            en: if en_present {
                Some(dummy_lang(22050, "en-us"))
            } else {
                None
            },
            phon: Phonemizer {
                bin: std::path::PathBuf::from("nonexistent"),
                data_dir: std::path::PathBuf::from("nonexistent"),
                timeout: Duration::from_secs(1),
            },
        }
    }

    #[test]
    fn from_lang_paths_both_none_returns_no_engine() {
        let err = LocalTtsEngine::from_lang_paths(None, None, Path::new("x"), Path::new("y"))
            .err()
            .expect("should be Err when both langs are None");
        assert!(
            matches!(err, VoiceError::NoEngine),
            "both None should yield VoiceError::NoEngine, got: {err:?}"
        );
    }

    #[test]
    fn id_is_piper_local() {
        assert_eq!(engine_with(true, false).id(), "piper_local");
    }

    #[test]
    fn routes_ru_to_ru_branch() {
        let engine = engine_with(true, false);
        assert!(
            engine.pick(LanguageHint::Ru).is_ok(),
            "Ru hint should hit ru branch"
        );
    }

    #[test]
    fn routes_mixed_to_ru_branch() {
        let engine = engine_with(true, false);
        assert!(
            engine.pick(LanguageHint::Mixed).is_ok(),
            "Mixed hint should hit ru branch"
        );
    }

    #[test]
    fn routes_en_to_en_branch() {
        let engine = engine_with(false, true);
        assert!(
            engine.pick(LanguageHint::En).is_ok(),
            "En hint should hit en branch"
        );
    }

    #[test]
    fn missing_ru_branch_returns_engine_err() {
        let engine = engine_with(false, true); // ru absent
        let err = engine.pick(LanguageHint::Ru).err().expect("should be Err");
        assert!(
            matches!(err, VoiceError::Engine(_)),
            "absent ru should yield VoiceError::Engine"
        );
    }

    #[test]
    fn missing_en_branch_returns_engine_err() {
        let engine = engine_with(true, false); // en absent
        let err = engine.pick(LanguageHint::En).err().expect("should be Err");
        assert!(
            matches!(err, VoiceError::Engine(_)),
            "absent en should yield VoiceError::Engine"
        );
    }

    #[test]
    fn missing_mixed_branch_returns_engine_err() {
        let engine = engine_with(false, true); // ru absent, Mixed routes to ru
        let err = engine
            .pick(LanguageHint::Mixed)
            .err()
            .expect("should be Err");
        assert!(
            matches!(err, VoiceError::Engine(_)),
            "Mixed with absent ru should yield VoiceError::Engine"
        );
    }
}
