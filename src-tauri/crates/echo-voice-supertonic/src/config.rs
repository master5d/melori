use serde::{Deserialize, Serialize};
use std::path::Path;

use echo_voice::VoiceError;

// ============================================================================
// Configuration Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ae: AEConfig,
    pub ttl: TTLConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AEConfig {
    pub sample_rate: i32,
    pub base_chunk_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTLConfig {
    pub chunk_compress_factor: i32,
    pub latent_dim: i32,
}

// ============================================================================
// Voice Style Data Structure
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStyleData {
    pub style_ttl: StyleComponent,
    pub style_dp: StyleComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleComponent {
    pub data: Vec<Vec<Vec<f32>>>,
    pub dims: Vec<usize>,
    #[serde(rename = "type")]
    pub dtype: String,
}

// ============================================================================
// Configuration Loaders
// ============================================================================

/// Load configuration from JSON file at `onnx_dir/tts.json`
pub fn load_config(onnx_dir: &Path) -> Result<Config, VoiceError> {
    let cfg_path = onnx_dir.join("tts.json");
    let file = std::fs::read_to_string(&cfg_path)
        .map_err(|e| VoiceError::Io(format!("failed to read tts.json: {e}")))?;
    let config = serde_json::from_str(&file)
        .map_err(|e| VoiceError::Engine(format!("failed to parse tts.json: {e}")))?;
    Ok(config)
}

/// Load voice style data from a single JSON file
pub fn load_voice_style(path: &Path) -> Result<VoiceStyleData, VoiceError> {
    let file = std::fs::read_to_string(path)
        .map_err(|e| VoiceError::Io(format!("failed to read voice style: {e}")))?;
    let style = serde_json::from_str(&file)
        .map_err(|e| VoiceError::Engine(format!("failed to parse voice style: {e}")))?;
    Ok(style)
}

// ============================================================================
// Unicode Text Processor
// ============================================================================

pub struct UnicodeProcessor {
    indexer: Vec<i64>,
}

impl UnicodeProcessor {
    /// Load unicode indexer from JSON file (array of i64 values)
    pub fn new(path: &Path) -> Result<Self, VoiceError> {
        let file = std::fs::read_to_string(path)
            .map_err(|e| VoiceError::Io(format!("failed to read unicode indexer: {e}")))?;
        let indexer = serde_json::from_str(&file)
            .map_err(|e| VoiceError::Engine(format!("failed to parse unicode indexer: {e}")))?;
        Ok(UnicodeProcessor { indexer })
    }

    /// Map a unicode codepoint to its index; returns -1 if out of range
    pub fn map(&self, unicode_val: usize) -> i64 {
        if unicode_val < self.indexer.len() {
            self.indexer[unicode_val]
        } else {
            -1
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tts_json_fixture() {
        let json = r#"{"ae":{"sample_rate":44100,"base_chunk_size":512},
                       "ttl":{"chunk_compress_factor":6,"latent_dim":24}}"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.ae.sample_rate, 44100);
        assert_eq!(cfg.ttl.chunk_compress_factor, 6);
    }

    #[test]
    fn parses_voice_style_fixture() {
        let json = r#"{"style_ttl":{"data":[[[0.1,0.2]]],"dims":[1,1,2],"type":"float32"},
                       "style_dp":{"data":[[[0.3]]],"dims":[1,1,1],"type":"float32"}}"#;
        let s: VoiceStyleData = serde_json::from_str(json).unwrap();
        assert_eq!(s.style_ttl.dims, vec![1, 1, 2]);
        assert_eq!(s.style_dp.data[0][0][0], 0.3);
    }

    #[test]
    fn unicode_processor_maps_and_defaults_to_minus_one() {
        // indexer: unicode codepoint -> id; vne range -> -1
        let dir = std::env::temp_dir().join("svt-test-indexer");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("unicode_indexer.json");
        std::fs::write(&p, "[5,6,7]").unwrap();
        let u = UnicodeProcessor::new(&p).unwrap();
        assert_eq!(u.map(1), 6);
        assert_eq!(u.map(999), -1);
    }

    #[test]
    fn load_config_missing_file_is_io_error() {
        let err = load_config(Path::new("Z:/definitely/absent")).unwrap_err();
        assert!(matches!(err, VoiceError::Io(_)));
    }
}
