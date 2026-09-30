//! ONNX inference engine: 4-session Supertonic TTS chain.
//!
//! Ported from `docs/superpowers/reference/supertonic-rust/helper.rs`
//! (`load_text_to_speech` + `TextToSpeech::_infer` + `sample_noisy_latent`).
//! Fidelity: input/output tensor names, shapes and inference order match the
//! reference exactly. Deviations (all behavior-preserving): `anyhow::Result` →
//! `Result<_, VoiceError>`; the reference's `bsz` batching is fixed to `bsz = 1`
//! (shapes unchanged — this crate only ever infers one chunk at a time);
//! `ort` rc.7 idioms → rc.12 (`Session::builder()...commit_from_file`,
//! `session.run(ort::inputs![...])`, `try_extract_tensor`); no rayon/CLI glue.

use ndarray::{Array, Array2, Array3};
use ort::session::Session;
use ort::value::Tensor;
use rand::thread_rng;
use rand_distr::{Distribution, Normal};
use std::path::Path;

use echo_voice::VoiceError;

use crate::config::{self, Config, UnicodeProcessor, VoiceStyleData};
use crate::text::{get_text_mask, preprocess_text, text_to_unicode_values};

/// Fixed diffusion step count for the vector estimator denoising loop.
pub const TOTAL_STEP: usize = 8;

/// Speed multiplier range accepted by [`SupertonicCore::synthesize_chunk`].
const SPEED_MIN: f32 = 0.9;
const SPEED_MAX: f32 = 1.5;

/// Loaded Supertonic ONNX engine: 4 sessions (duration predictor, text
/// encoder, vector estimator, vocoder) + config + unicode indexer.
pub struct SupertonicCore {
    cfg: Config,
    text_processor: UnicodeProcessor,
    dp_session: Session,
    text_enc_session: Session,
    vector_est_session: Session,
    vocoder_session: Session,
}

impl SupertonicCore {
    /// Load the 4 ONNX sessions + `tts.json` + `unicode_indexer.json` from
    /// `onnx_dir`. Mirrors reference `load_text_to_speech` (CPU-only; no
    /// `use_gpu` branch — this port never enables GPU).
    pub fn load(onnx_dir: &Path) -> Result<Self, VoiceError> {
        let cfg = config::load_config(onnx_dir)?;

        let dp_path = onnx_dir.join("duration_predictor.onnx");
        let text_enc_path = onnx_dir.join("text_encoder.onnx");
        let vector_est_path = onnx_dir.join("vector_estimator.onnx");
        let vocoder_path = onnx_dir.join("vocoder.onnx");

        let dp_session = Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder (duration_predictor): {e}")))?
            .commit_from_file(&dp_path)
            .map_err(|e| VoiceError::Engine(format!("ort load duration_predictor.onnx: {e}")))?;

        let text_enc_session = Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder (text_encoder): {e}")))?
            .commit_from_file(&text_enc_path)
            .map_err(|e| VoiceError::Engine(format!("ort load text_encoder.onnx: {e}")))?;

        let vector_est_session = Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder (vector_estimator): {e}")))?
            .commit_from_file(&vector_est_path)
            .map_err(|e| VoiceError::Engine(format!("ort load vector_estimator.onnx: {e}")))?;

        let vocoder_session = Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder (vocoder): {e}")))?
            .commit_from_file(&vocoder_path)
            .map_err(|e| VoiceError::Engine(format!("ort load vocoder.onnx: {e}")))?;

        let unicode_indexer_path = onnx_dir.join("unicode_indexer.json");
        let text_processor = UnicodeProcessor::new(&unicode_indexer_path)?;

        Ok(SupertonicCore {
            cfg,
            text_processor,
            dp_session,
            text_enc_session,
            vector_est_session,
            vocoder_session,
        })
    }

    /// Sample rate declared by `tts.json` (`ae.sample_rate`). Consumed by
    /// Task 4's `VoiceEngine` impl.
    pub fn sample_rate(&self) -> u32 {
        self.cfg.ae.sample_rate as u32
    }

    /// Synthesize one text chunk (already split by `text::chunk_text`
    /// upstream) into f32 PCM at [`SupertonicCore::sample_rate`].
    ///
    /// `speed` is clamped to `[0.9, 1.5]` (reference `--speed` range).
    pub fn synthesize_chunk(
        &mut self,
        text: &str,
        lang: &str,
        style: &VoiceStyleData,
        speed: f32,
    ) -> Result<Vec<f32>, VoiceError> {
        let speed = speed.clamp(SPEED_MIN, SPEED_MAX);

        // --- text -> ids + mask (bsz = 1) ---
        let processed = preprocess_text(text, lang)?;
        let unicode_vals = text_to_unicode_values(&processed);
        let ids: Vec<i64> = unicode_vals
            .iter()
            .map(|&v| self.text_processor.map(v))
            .collect();
        let len = ids.len();

        let text_ids_array = Array2::from_shape_vec((1, len), ids)
            .map_err(|e| VoiceError::Engine(format!("text_ids shape: {e}")))?;
        let text_mask = get_text_mask(&[len]);

        // --- style tensors ---
        let style_dp = style_component_to_array3(&style.style_dp)?;
        let style_ttl = style_component_to_array3(&style.style_ttl)?;

        let text_ids_tensor = Tensor::from_array(text_ids_array.clone())
            .map_err(|e| VoiceError::Engine(format!("ort tensor text_ids: {e}")))?;
        let text_mask_tensor = Tensor::from_array(text_mask.clone())
            .map_err(|e| VoiceError::Engine(format!("ort tensor text_mask: {e}")))?;
        let style_dp_tensor = Tensor::from_array(style_dp)
            .map_err(|e| VoiceError::Engine(format!("ort tensor style_dp: {e}")))?;

        // --- predict duration ---
        let dp_outputs = self
            .dp_session
            .run(ort::inputs! {
                "text_ids" => text_ids_tensor,
                "style_dp" => style_dp_tensor,
                "text_mask" => text_mask_tensor
            })
            .map_err(|e| VoiceError::Engine(format!("ort run duration_predictor: {e}")))?;

        let (_, duration_data) = dp_outputs["duration"]
            .try_extract_tensor::<f32>()
            .map_err(|e| VoiceError::Engine(format!("ort extract duration: {e}")))?;
        let mut duration: Vec<f32> = duration_data.to_vec();
        for dur in duration.iter_mut() {
            *dur /= speed;
        }

        // --- encode text ---
        let text_ids_tensor2 = Tensor::from_array(text_ids_array)
            .map_err(|e| VoiceError::Engine(format!("ort tensor text_ids (2): {e}")))?;
        let text_mask_tensor2 = Tensor::from_array(text_mask.clone())
            .map_err(|e| VoiceError::Engine(format!("ort tensor text_mask (2): {e}")))?;
        let style_ttl_tensor = Tensor::from_array(style_ttl.clone())
            .map_err(|e| VoiceError::Engine(format!("ort tensor style_ttl: {e}")))?;

        let text_enc_outputs = self
            .text_enc_session
            .run(ort::inputs! {
                "text_ids" => text_ids_tensor2,
                "style_ttl" => style_ttl_tensor,
                "text_mask" => text_mask_tensor2
            })
            .map_err(|e| VoiceError::Engine(format!("ort run text_encoder: {e}")))?;

        let (text_emb_shape, text_emb_data) = text_enc_outputs["text_emb"]
            .try_extract_tensor::<f32>()
            .map_err(|e| VoiceError::Engine(format!("ort extract text_emb: {e}")))?;
        let text_emb = Array3::from_shape_vec(
            (
                text_emb_shape[0] as usize,
                text_emb_shape[1] as usize,
                text_emb_shape[2] as usize,
            ),
            text_emb_data.to_vec(),
        )
        .map_err(|e| VoiceError::Engine(format!("text_emb shape: {e}")))?;

        // --- sample noisy latent ---
        let (mut xt, latent_mask) = sample_noisy_latent(
            &duration,
            self.cfg.ae.sample_rate,
            self.cfg.ae.base_chunk_size,
            self.cfg.ttl.chunk_compress_factor,
            self.cfg.ttl.latent_dim,
        );

        // --- denoising loop (TOTAL_STEP fixed steps, not a parameter) ---
        let total_step_array = Array::from_elem(1, TOTAL_STEP as f32);

        for step in 0..TOTAL_STEP {
            let current_step_array = Array::from_elem(1, step as f32);

            let xt_tensor = Tensor::from_array(xt.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor noisy_latent: {e}")))?;
            let text_emb_tensor = Tensor::from_array(text_emb.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor text_emb: {e}")))?;
            let style_ttl_tensor2 = Tensor::from_array(style_ttl.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor style_ttl (2): {e}")))?;
            let latent_mask_tensor = Tensor::from_array(latent_mask.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor latent_mask: {e}")))?;
            let text_mask_tensor3 = Tensor::from_array(text_mask.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor text_mask (3): {e}")))?;
            let current_step_tensor = Tensor::from_array(current_step_array)
                .map_err(|e| VoiceError::Engine(format!("ort tensor current_step: {e}")))?;
            let total_step_tensor = Tensor::from_array(total_step_array.clone())
                .map_err(|e| VoiceError::Engine(format!("ort tensor total_step: {e}")))?;

            let vector_est_outputs = self
                .vector_est_session
                .run(ort::inputs! {
                    "noisy_latent" => xt_tensor,
                    "text_emb" => text_emb_tensor,
                    "style_ttl" => style_ttl_tensor2,
                    "latent_mask" => latent_mask_tensor,
                    "text_mask" => text_mask_tensor3,
                    "current_step" => current_step_tensor,
                    "total_step" => total_step_tensor
                })
                .map_err(|e| VoiceError::Engine(format!("ort run vector_estimator: {e}")))?;

            let (denoised_shape, denoised_data) = vector_est_outputs["denoised_latent"]
                .try_extract_tensor::<f32>()
                .map_err(|e| VoiceError::Engine(format!("ort extract denoised_latent: {e}")))?;
            xt = Array3::from_shape_vec(
                (
                    denoised_shape[0] as usize,
                    denoised_shape[1] as usize,
                    denoised_shape[2] as usize,
                ),
                denoised_data.to_vec(),
            )
            .map_err(|e| VoiceError::Engine(format!("denoised_latent shape: {e}")))?;
        }

        // --- generate waveform ---
        let final_latent_tensor = Tensor::from_array(xt)
            .map_err(|e| VoiceError::Engine(format!("ort tensor latent (final): {e}")))?;
        let vocoder_outputs = self
            .vocoder_session
            .run(ort::inputs! {
                "latent" => final_latent_tensor
            })
            .map_err(|e| VoiceError::Engine(format!("ort run vocoder: {e}")))?;

        let (_, wav_data) = vocoder_outputs["wav_tts"]
            .try_extract_tensor::<f32>()
            .map_err(|e| VoiceError::Engine(format!("ort extract wav_tts: {e}")))?;
        let wav: Vec<f32> = wav_data.to_vec();

        // Trim to the actual predicted duration (reference `TextToSpeech::call`).
        let dur = duration.first().copied().unwrap_or(0.0);
        let wav_len = (self.cfg.ae.sample_rate as f32 * dur) as usize;
        Ok(wav[..wav_len.min(wav.len())].to_vec())
    }
}

/// Flatten a [`crate::config::StyleComponent`] (`bsz=1` in this crate) into
/// an `Array3<f32>` per its declared `dims`.
fn style_component_to_array3(comp: &config::StyleComponent) -> Result<Array3<f32>, VoiceError> {
    if comp.dims.len() != 3 {
        return Err(VoiceError::Engine(format!(
            "style component dims must be 3D, got {:?}",
            comp.dims
        )));
    }
    let (d0, d1, d2) = (comp.dims[0], comp.dims[1], comp.dims[2]);
    let mut flat = Vec::with_capacity(d0 * d1 * d2);
    for batch in &comp.data {
        for row in batch {
            for &val in row {
                flat.push(val);
            }
        }
    }
    Array3::from_shape_vec((d0, d1, d2), flat)
        .map_err(|e| VoiceError::Engine(format!("style component shape: {e}")))
}

/// Sample noisy latent from a standard normal distribution and apply the
/// latent-length mask. Ported verbatim from the reference (bsz generalized,
/// but this crate only ever calls it with `duration.len() == 1`).
fn sample_noisy_latent(
    duration: &[f32],
    sample_rate: i32,
    base_chunk_size: i32,
    chunk_compress: i32,
    latent_dim: i32,
) -> (Array3<f32>, Array3<f32>) {
    let bsz = duration.len();
    let max_dur = duration.iter().fold(0.0f32, |a, &b| a.max(b));

    let wav_len_max = (max_dur * sample_rate as f32) as usize;
    let wav_lengths: Vec<usize> = duration
        .iter()
        .map(|&d| (d * sample_rate as f32) as usize)
        .collect();

    let chunk_size = (base_chunk_size * chunk_compress) as usize;
    let latent_len = (wav_len_max + chunk_size - 1) / chunk_size;
    let latent_dim_val = (latent_dim * chunk_compress) as usize;

    let mut noisy_latent = Array3::<f32>::zeros((bsz, latent_dim_val, latent_len));

    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut rng = thread_rng();

    for b in 0..bsz {
        for d in 0..latent_dim_val {
            for t in 0..latent_len {
                noisy_latent[[b, d, t]] = normal.sample(&mut rng);
            }
        }
    }

    let latent_lengths: Vec<usize> = wav_lengths
        .iter()
        .map(|&len| (len + chunk_size - 1) / chunk_size)
        .collect();

    let latent_mask = get_text_mask_from_lengths(&latent_lengths, latent_len);

    // Apply mask
    for b in 0..bsz {
        for d in 0..latent_dim_val {
            for t in 0..latent_len {
                noisy_latent[[b, d, t]] *= latent_mask[[b, 0, t]];
            }
        }
    }

    (noisy_latent, latent_mask)
}

/// `length_to_mask` equivalent with an explicit `max_len` (reference uses
/// this form for the latent mask, distinct from `text::get_text_mask`'s
/// self-derived max).
fn get_text_mask_from_lengths(lengths: &[usize], max_len: usize) -> Array3<f32> {
    crate::text::length_to_mask(lengths, Some(max_len))
}

// ============================================================================
// WAV encoding
// ============================================================================

/// Encode f32 PCM samples (clamped to `[-1.0, 1.0]`) into a mono 16-bit
/// RIFF/WAVE byte buffer. Mirrors reference `write_wav_file`, but writes to
/// an in-memory buffer (`crates/echo-voice-local/src/pcm.rs` approach)
/// instead of a file.
pub fn pcm_f32_to_wav_bytes(pcm: &[f32], sample_rate: u32) -> Result<Vec<u8>, VoiceError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut writer = hound::WavWriter::new(&mut buf, spec)
            .map_err(|e| VoiceError::Engine(format!("wav writer: {e}")))?;
        for &sample in pcm {
            let clamped = sample.clamp(-1.0, 1.0);
            let val = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(val)
                .map_err(|e| VoiceError::Engine(format!("wav sample: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| VoiceError::Engine(format!("wav finalize: {e}")))?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_bytes_have_riff_header_44100_mono_16bit() {
        let wav = pcm_f32_to_wav_bytes(&[0.0f32; 441], 44100).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        // fmt: каналы (offset 22) = 1, sample rate (offset 24) = 44100, bps (offset 34) = 16
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1);
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            44100
        );
        assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16);
    }

    #[test]
    fn wav_bytes_clamp_out_of_range_samples() {
        let wav = pcm_f32_to_wav_bytes(&[2.0f32, -2.0f32], 44100).unwrap();
        let data_start = wav.len() - 4;
        let s0 = i16::from_le_bytes([wav[data_start], wav[data_start + 1]]);
        let s1 = i16::from_le_bytes([wav[data_start + 2], wav[data_start + 3]]);
        assert_eq!(s0, 32767);
        assert_eq!(s1, -32767);
    }

    #[test]
    fn load_from_missing_dir_is_error_not_panic() {
        assert!(SupertonicCore::load(std::path::Path::new("Z:/absent-model-dir")).is_err());
    }

    /// Живой инференс. Запуск вручную при скачанной модели:
    /// $env:SUPERTONIC_DIR="<путь к папке с 4 .onnx + tts.json + unicode_indexer.json>"
    /// $env:SUPERTONIC_STYLE="<путь к M1.json>"
    /// cargo test -p echo-voice-supertonic -- --ignored
    #[test]
    #[ignore]
    fn live_synthesis_produces_nonempty_audio() {
        let dir = std::path::PathBuf::from(std::env::var("SUPERTONIC_DIR").unwrap());
        let style = crate::config::load_voice_style(&std::path::PathBuf::from(
            std::env::var("SUPERTONIC_STYLE").unwrap(),
        ))
        .unwrap();
        let mut core = SupertonicCore::load(&dir).unwrap();
        let pcm = core
            .synthesize_chunk("Hello from the test.", "en", &style, 1.05)
            .unwrap();
        assert!(
            pcm.len() > 44100,
            "expected >1s of audio, got {} samples",
            pcm.len()
        );
        let peak = pcm.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(peak > 0.01, "audio is silent, peak={peak}");
    }
}
