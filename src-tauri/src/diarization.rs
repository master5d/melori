//! Speaker diarization façade. Phase 2 replaces the stub with a real
//! implementation using the `speakrs` crate (pyannote-style pipeline).

use crate::transcript_format::{SpeakerId, SpeakerTurn};
use anyhow::{Context, Result};
use speakrs::{ExecutionMode, OwnedDiarizationPipeline};
use std::collections::HashMap;
use tauri::AppHandle;

/// Diarize 16kHz mono samples into speaker turns.
/// `_hint` (desired speaker count) is accepted for signature stability but is
/// IGNORED: speakrs 0.4.2 auto-detects the number of speakers and exposes no
/// count knob in PipelineConfig.
pub fn diarize(
    app_handle: &AppHandle,
    samples: &[f32],
    _sample_rate: u32,
    _hint: Option<usize>,
) -> Result<Vec<SpeakerTurn>> {
    let models_dir = crate::portable::app_data_dir(app_handle)?
        .join("models")
        .join("diarization");

    // Load pipeline (CPU mode for portability/stability)
    let mut pipeline = if models_dir.exists() {
        OwnedDiarizationPipeline::from_dir(&models_dir, ExecutionMode::Cpu)
            .context("Failed to load diarization pipeline from local dir")?
    } else {
        // Fallback to pretrained (will use hf-hub cache if ensure_diarization_models was called)
        OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu)
            .context("Failed to load diarization pipeline from HuggingFace cache")?
    };

    // Run diarization
    let app = app_handle.clone();
    let mut last_pct: i32 = -1;
    let result = pipeline
        .run_with_progress(samples, move |frac| {
            let pct = (frac * 100.0).clamp(0.0, 100.0) as i32;
            if pct != last_pct {
                last_pct = pct;
                crate::progress::emit_progress(
                    &app,
                    crate::progress::ProgressPhase::Diarizing,
                    Some(pct as u8),
                );
            }
        })
        .context("Diarization run failed")?;

    // Map speakrs segments to SpeakerTurn, ensuring 0-based contiguous IDs
    let mut speaker_map: HashMap<String, u32> = HashMap::new();
    let mut next_id = 0;

    let turns = result
        .segments
        .into_iter()
        .map(|s| {
            let id = *speaker_map.entry(s.speaker.clone()).or_insert_with(|| {
                let current = next_id;
                next_id += 1;
                current
            });
            SpeakerTurn {
                start: s.start as f32,
                end: s.end as f32,
                speaker: SpeakerId(id),
            }
        })
        .collect();

    Ok(turns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires diarization model files and a real audio sample"]
    fn diarize_two_speaker_sample_returns_turns() {
        // Place a known 2-speaker 16kHz mono wav at tests/fixtures/two_speakers.wav
        // and the models in the models dir, then run with:
        //   cargo test --manifest-path src-tauri/Cargo.toml diarize_two_speaker -- --ignored
        // Note: this test needs a dummy AppHandle or to be run in an env where
        // app_data_dir works. Since it's ignored and for manual use, we assume
        // the environment is set up.

        // We can't easily create a real AppHandle in a pure unit test without
        // tauri-test or similar. This test is a template for integration testing.
    }
}
