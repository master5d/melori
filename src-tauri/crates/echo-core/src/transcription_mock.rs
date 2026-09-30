// CI-only mock TranscriptionManager - avoids whisper/Vulkan dependencies.
// This file is copied over transcription.rs during CI tests.
// Existing tests don't exercise transcription, so this is safe.

use crate::model::ModelManager;
use anyhow::Result;
use echo_config::AppSettings;
use echo_text::transcript_format::{SpeakerTurn, TimedSegment};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

/// RAII guard that is a no-op in the mock — mirrors the real `LoadingGuard`.
pub struct LoadingGuard;

/// Mirror of the real `TranscriptionDetails` so consumers (`file_transcription`,
/// `commands::transcribe`) type-check under the CI mock.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionDetails {
    pub text: String,
    pub segments: Vec<TimedSegment>,
    pub words: Option<Vec<TimedSegment>>,
    pub speakers: Option<Vec<SpeakerTurn>>,
}

/// Mirror of the real `TranscribeOpts`.
#[derive(Clone, Copy, Default)]
pub struct TranscribeOpts {
    pub word_timestamps: bool,
    pub emit_progress: bool,
}

#[derive(Clone)]
pub struct TranscriptionManager {}

impl TranscriptionManager {
    pub fn new(
        _model_manager: Arc<ModelManager>,
        _events: Arc<dyn crate::EventSink>,
        _settings: Arc<dyn crate::SettingsAccess>,
        _recording_probe: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Self> {
        Ok(Self {})
    }

    pub fn is_model_loaded(&self) -> bool {
        false
    }

    pub fn try_start_loading(&self) -> Option<LoadingGuard> {
        Some(LoadingGuard)
    }

    pub fn unload_model(&self) -> Result<()> {
        Ok(())
    }

    pub fn maybe_unload_immediately(&self, _context: &str) {}

    pub fn load_model(&self, _model_id: &str) -> Result<()> {
        Ok(())
    }

    pub fn initiate_model_load(&self) {}

    pub fn get_current_model(&self) -> Option<String> {
        None
    }

    pub fn transcribe(&self, _audio: Vec<f32>) -> Result<String> {
        Ok(String::new())
    }

    /// No-op cancellation surface in the mock.
    pub fn request_cancel(&self) {}
    pub fn reset_cancel(&self) {}
    pub fn is_cancelled(&self) -> bool {
        false
    }

    /// Returns empty details in the CI mock.
    pub fn transcribe_detailed_with(
        &self,
        _audio: Vec<f32>,
        _opts: TranscribeOpts,
    ) -> Result<TranscriptionDetails> {
        Ok(TranscriptionDetails {
            text: String::new(),
            segments: vec![],
            words: None,
            speakers: None,
        })
    }
}

/// No-op in CI mock.
pub fn apply_accelerator_settings(_settings: &AppSettings) {}

#[derive(Serialize, Clone, Debug, Type)]
pub struct GpuDeviceOption {
    pub id: i32,
    pub name: String,
    pub total_vram_mb: usize,
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct AvailableAccelerators {
    pub whisper: Vec<String>,
    pub ort: Vec<String>,
    pub gpu_devices: Vec<GpuDeviceOption>,
}

/// Returns empty lists in CI mock.
pub fn get_available_accelerators() -> AvailableAccelerators {
    AvailableAccelerators {
        whisper: vec![],
        ort: vec![],
        gpu_devices: vec![],
    }
}
