use echo_voice::VoiceError;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub sample_rate: u32,
    pub phoneme_id_map: HashMap<String, Vec<i64>>,
    pub num_speakers: i64,
}

#[derive(Deserialize)]
struct RawAudio {
    sample_rate: u32,
}

#[derive(Deserialize)]
struct RawConfig {
    audio: RawAudio,
    #[serde(default = "one")]
    num_speakers: i64,
    phoneme_id_map: HashMap<String, Vec<i64>>,
}

fn one() -> i64 {
    1
}

impl ModelConfig {
    pub fn from_json_str(s: &str) -> Result<ModelConfig, VoiceError> {
        let r: RawConfig = serde_json::from_str(s)
            .map_err(|e| VoiceError::Engine(format!("model config parse: {e}")))?;
        Ok(ModelConfig {
            sample_rate: r.audio.sample_rate,
            phoneme_id_map: r.phoneme_id_map,
            num_speakers: r.num_speakers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_piper_config() {
        let s = include_str!("../tests/fixtures/piper_ru.onnx.json");
        let c = ModelConfig::from_json_str(s).unwrap();
        assert_eq!(c.sample_rate, 22050);
        assert_eq!(c.phoneme_id_map.get("_"), Some(&vec![0]));
        assert_eq!(c.num_speakers, 1);
    }
}
