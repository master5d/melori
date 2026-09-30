//! App-side glue that satisfies echo-core's injected traits with Tauri.
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use echo_config::AppSettings;
use echo_core::{EventSink, SettingsAccess};

/// Emits echo-core's raw string events through Tauri.
pub struct AppHandleEventSink(pub AppHandle);
impl EventSink for AppHandleEventSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event, payload);
    }
}

/// Live settings access backed by the tauri-plugin-store functions.
pub struct AppSettingsAccess(pub AppHandle);
impl SettingsAccess for AppSettingsAccess {
    fn get(&self) -> AppSettings {
        crate::settings::get_settings(&self.0)
    }
    fn set(&self, settings: AppSettings) {
        crate::settings::write_settings(&self.0, settings);
    }
}

/// Builds the bundled-resource resolver echo-core's ModelManager needs.
pub fn resource_resolver(app: &AppHandle) -> Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync> {
    let app = app.clone();
    Arc::new(move |rel: &str| {
        app.path()
            .resolve(rel, tauri::path::BaseDirectory::Resource)
            .ok()
    })
}
