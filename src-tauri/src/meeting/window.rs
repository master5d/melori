//! Meeting Copilot (C3) — the floating panel window. Reveal-only: this module NEVER
//! starts capture; the panel's frontend gates consent and calls `start_meeting`.
use melori_consult::window_policy::{content_protected, meeting_window_geometry};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

pub const COPILOT_LABEL: &str = "meeting_copilot";
const W: f64 = 540.0;
const H: f64 = 520.0;

fn window_position(app: &AppHandle) -> Option<(f64, f64)> {
    let monitor = app
        .get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let position = monitor.position();
    let size = monitor.size();
    let (x, y, _, _) = meeting_window_geometry(
        position.x as f64 / scale,
        position.y as f64 / scale,
        size.width as f64 / scale,
    );
    Some((x, y))
}

/// Apply capture protection; a failure is logged, never swallowed — otherwise the panel's
/// "hidden from sharing" badge would claim a protection the OS did not grant.
fn apply_protection(win: &tauri::WebviewWindow, protected: bool) {
    if let Err(e) = win.set_content_protected(protected) {
        log::warn!("meeting copilot: content protection ({protected}) not applied: {e}");
    }
}

/// Apply the current capture policy to an existing notes window.
pub fn set_content_protected(app: &AppHandle, enabled: bool) {
    if let Some(win) = app.get_webview_window(COPILOT_LABEL) {
        apply_protection(&win, content_protected(enabled));
    }
}

/// Create (once) and reveal the meeting copilot panel: always-on-top, undecorated,
/// skips the taskbar, focusable + resizable. If it already exists, just show + focus it.
/// REVEAL ONLY — it does not start capture.
pub fn show_meeting_copilot(app: &AppHandle) {
    let settings = crate::settings::get_settings(app);
    let protected = content_protected(settings.hide_notes_from_screen_share);
    if let Some(win) = app.get_webview_window(COPILOT_LABEL) {
        apply_protection(&win, protected);
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    // Build off the main thread. Every caller (the sync Tauri command, the tray menu, the
    // hotkey action) runs on the main thread, and on Windows `build()` there waits for the
    // event loop it is blocking: the window stays on about:blank, never paints, and the
    // whole app — tray "Quit" included — freezes.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut builder = WebviewWindowBuilder::new(
            &app,
            COPILOT_LABEL,
            tauri::WebviewUrl::App("src/meeting/index.html".into()),
        )
        .title("melori — meeting")
        .inner_size(W, H)
        .min_inner_size(W, 52.0)
        .resizable(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(protected)
        .visible(true);
        if let Some((x, y)) = window_position(&app) {
            builder = builder.position(x, y);
        }
        match builder.build() {
            Ok(win) => apply_protection(&win, protected),
            Err(e) => log::error!("meeting copilot window: {e}"),
        }
    });
}

/// Hide the panel (kept alive for re-reveal). No-op if it doesn't exist.
pub fn hide_meeting_copilot(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(COPILOT_LABEL) {
        let _ = win.hide();
    }
}
