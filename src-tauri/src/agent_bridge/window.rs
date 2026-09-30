use tauri::{AppHandle, Manager, WebviewWindowBuilder};

pub const PANEL_LABEL: &str = "agent_panel";
const W: f64 = 400.0;
const H: f64 = 240.0;

/// Creates (once) and shows the agent question panel: always-on-top,
/// undecorated, skips taskbar, but CAN take focus (text input).
pub fn show_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(PANEL_LABEL) {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let builder = WebviewWindowBuilder::new(
        app,
        PANEL_LABEL,
        tauri::WebviewUrl::App("src/agent-panel/index.html".into()),
    )
    .title("melori — agent question")
    .inner_size(W, H)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(true);
    if let Err(e) = builder.build() {
        log::error!("agent panel window: {e}");
    }
}

pub fn hide_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(PANEL_LABEL) {
        let _ = win.hide();
    }
}
