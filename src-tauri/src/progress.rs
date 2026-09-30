//! The `transcription-progress` Tauri event payload for the Transcribe-file UI.
use tauri::{AppHandle, Emitter};

pub use echo_config::{ProgressPhase, TranscriptionProgress};

/// Emit a `transcription-progress` event. Best-effort (ignores emit errors).
pub fn emit_progress(app: &AppHandle, phase: ProgressPhase, percent: Option<u8>) {
    let _ = app.emit(
        "transcription-progress",
        TranscriptionProgress { phase, percent },
    );
}
