//! Shared shortcut event handling logic
//!
//! This module contains the common logic for handling shortcut events,
//! used by both the Tauri and handy-keys implementations.

use log::warn;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use crate::settings::get_settings;
use crate::transcription_coordinator::is_transcribe_binding;
use crate::TranscriptionCoordinator;

/// Handle a shortcut event from either implementation.
///
/// This function contains the shared logic for:
/// - Looking up the action in ACTION_MAP
/// - Handling the cancel binding (only fires when recording)
/// - Handling push-to-talk mode (start on press, stop on release)
/// - Handling toggle mode (toggle state on press only)
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `binding_id` - The ID of the binding (e.g., "transcribe", "cancel")
/// * `hotkey_string` - The string representation of the hotkey
/// * `is_pressed` - Whether this is a key press (true) or release (false)
pub fn handle_shortcut_event(
    app: &AppHandle,
    binding_id: &str,
    hotkey_string: &str,
    is_pressed: bool,
) {
    if is_pressed && binding_id.starts_with("meeting_") {
        match binding_id {
            "meeting_toggle" => {
                let sess = app.state::<crate::meeting::session::MeetingSession>();
                if sess.is_active() {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let sess = app.state::<crate::meeting::session::MeetingSession>();
                        let binding = app.state::<melori_consult::client_meeting::ClientBinding>();
                        let _ = crate::commands::meeting::stop_meeting_inner(&app, &sess, &binding)
                            .await;
                    });
                } else {
                    // like the microphone button: the meeting window asks for the client and the
                    // participants' consent — a hotkey must not start capture past that step
                    crate::meeting::window::show_meeting_copilot(app);
                }
            }
            "meeting_window" => {
                if let Some(win) = app.get_webview_window(crate::meeting::window::COPILOT_LABEL) {
                    if win.is_visible().unwrap_or(false) {
                        crate::meeting::window::hide_meeting_copilot(app);
                    } else {
                        crate::meeting::window::show_meeting_copilot(app);
                    }
                } else {
                    crate::meeting::window::show_meeting_copilot(app);
                }
            }
            "meeting_mark" => {
                let sess = app.state::<crate::meeting::session::MeetingSession>();
                if sess.is_active() {
                    let elapsed = sess.elapsed_ms();
                    sess.add_mark(elapsed);
                }
            }
            _ => {
                let _ = (crate::commands::meeting::MeetingShortcutEvent {
                    action: binding_id.to_string(),
                })
                .emit(app);
            }
        }
        return;
    }

    let settings = get_settings(app);

    // Transcribe bindings are handled by the coordinator.
    if is_transcribe_binding(binding_id) {
        if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
            coordinator.send_input(binding_id, hotkey_string, is_pressed, settings.push_to_talk);
        } else {
            warn!("TranscriptionCoordinator is not initialized");
        }
        return;
    }

    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!(
            "No action defined in ACTION_MAP for shortcut ID '{}'. Shortcut: '{}', Pressed: {}",
            binding_id, hotkey_string, is_pressed
        );
        return;
    };

    // Cancel binding: only fires when recording and key is pressed
    if binding_id == "cancel" {
        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
        if audio_manager.is_recording() && is_pressed {
            action.start(app, binding_id, hotkey_string);
        }
        return;
    }

    // Remaining bindings (e.g. "test") use simple start/stop on press/release.
    if is_pressed {
        action.start(app, binding_id, hotkey_string);
    } else {
        action.stop(app, binding_id, hotkey_string);
    }
}
