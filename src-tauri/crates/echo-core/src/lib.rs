//! echo-core: the Tauri-free ML inference spine (model catalog/download +
//! transcription engine). Depends on no Tauri types; the host app injects
//! event emission, settings access, and resolved paths via the traits below.
use echo_config::AppSettings;

/// Sink for the raw string-named events the engine emits (model-* download/
/// verification/extraction progress, transcription-progress). The app
/// implements this over `tauri::AppHandle::emit`.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

/// Live settings access. `get` returns a FRESH read each call (matches the
/// app's `get_settings`), `set` persists (matches `write_settings`).
pub trait SettingsAccess: Send + Sync {
    fn get(&self) -> AppSettings;
    fn set(&self, settings: AppSettings);
}

pub mod model;
pub mod model_manifest;
pub mod transcription;
pub mod transcription_mock;
