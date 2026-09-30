pub mod audio;
pub mod history;

// The model/transcription managers moved to the Tauri-free echo-core crate.
// Re-export them under their historical paths so existing
// `crate::managers::model::…` / `crate::managers::transcription::…` /
// `crate::managers::model_manifest::…` call sites resolve unchanged.
// (model_manifest currently has no app-side call sites — its only consumer
// moved into echo-core with it — but the historical path is kept per spec.)
#[allow(unused_imports)]
pub use echo_core::model_manifest;
pub use echo_core::{model, transcription};
