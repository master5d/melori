//! ort session wrapper: Piper VITS model (phoneme ids → f32 PCM).
//!
//! Tensor contract (FIXED — do not modify names/shapes):
//! - input `input`:         int64  [1, N]
//! - input `input_lengths`: int64  [1]   = [N]
//! - input `scales`:        f32    [3]   = piper_scales(rate)
//! - output `output`:       f32    [1,1,1,T] → flattened Vec<f32> PCM

use echo_voice::VoiceError;
use std::path::Path;
use std::sync::Mutex;

/// Piper-VITS scale vector: [noise_scale, length_scale, noise_w].
///
/// `rate > 0` → `length_scale = 1.0 / rate`; otherwise 1.0 (normal speed).
pub(crate) fn piper_scales(rate: f32) -> [f32; 3] {
    let length_scale = if rate > 0.0 { 1.0 / rate } else { 1.0 };
    [0.667, length_scale, 0.8]
}

/// Wrapper around an `ort` session for Piper VITS inference.
pub struct OnnxTts {
    /// Interior mutability: `ort::Session::run` takes `&mut self`.
    /// `None` only in unit tests (dummy instance, never inferred against).
    session: Option<Mutex<ort::session::Session>>,
    pub sample_rate: u32,
}

impl OnnxTts {
    /// Load a Piper VITS ONNX model from `model_path`.
    pub fn load(model_path: &Path, sample_rate: u32) -> Result<OnnxTts, VoiceError> {
        let session = ort::session::Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder: {e}")))?
            .commit_from_file(model_path)
            .map_err(|e| VoiceError::Engine(format!("ort load: {e}")))?;
        Ok(OnnxTts {
            session: Some(Mutex::new(session)),
            sample_rate,
        })
    }

    /// Create a non-functional stub for unit tests. Never call `infer` on this.
    #[cfg(test)]
    pub(crate) fn dummy(sample_rate: u32) -> OnnxTts {
        OnnxTts {
            session: None,
            sample_rate,
        }
    }

    /// Run Piper VITS inference.
    ///
    /// `ids`  – phoneme id sequence (shape [N])
    /// `rate` – speech rate multiplier (1.0 = normal)
    ///
    /// Returns flat f32 PCM samples (shape [T] from model output [1,1,1,T]).
    pub fn infer(&self, ids: &[i64], rate: f32) -> Result<Vec<f32>, VoiceError> {
        use ort::value::Tensor;

        let n = ids.len();

        // --- build input tensors ---
        let input = Tensor::<i64>::from_array(([1_usize, n], ids.to_vec()))
            .map_err(|e| VoiceError::Engine(format!("ort input tensor: {e}")))?;

        let input_lengths = Tensor::<i64>::from_array(([1_usize], vec![n as i64]))
            .map_err(|e| VoiceError::Engine(format!("ort input_lengths tensor: {e}")))?;

        let scales_arr = piper_scales(rate);
        let scales = Tensor::<f32>::from_array(([3_usize], scales_arr.to_vec()))
            .map_err(|e| VoiceError::Engine(format!("ort scales tensor: {e}")))?;

        // --- run inference ---
        let named_inputs = ort::inputs![
            "input"         => input,
            "input_lengths" => input_lengths,
            "scales"        => scales
        ];

        let mut session = self
            .session
            .as_ref()
            .ok_or_else(|| VoiceError::Engine("ort session not loaded (dummy)".into()))?
            .lock()
            .map_err(|e| VoiceError::Engine(format!("ort session lock: {e}")))?;

        let outputs = session
            .run(named_inputs)
            .map_err(|e| VoiceError::Engine(format!("ort run: {e}")))?;

        // --- extract output [1,1,1,T] → flat Vec<f32> PCM ---
        let (_, data) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| VoiceError::Engine(format!("ort extract output: {e}")))?;

        Ok(data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_three_values_defaults() {
        let s = piper_scales(1.0);
        assert_eq!(s.len(), 3);
        assert!((s[0] - 0.667).abs() < 1e-6);
        assert!((s[1] - 1.0).abs() < 1e-6);
        assert!((s[2] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn faster_rate_shortens_length_scale() {
        assert!(piper_scales(2.0)[1] < piper_scales(1.0)[1]);
    }
}
