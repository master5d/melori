use crate::agent_bridge::state::{BridgeState, Outcome, QuestionEvent};
use crate::agent_bridge::storage::{BridgeStore, QuestionRow};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn agent_bridge_answer(
    app: tauri::AppHandle,
    state: State<BridgeState>,
    id: i64,
    answer: String,
) -> Result<(), String> {
    // Hide regardless of the resolve outcome: the question may have just
    // timed out server-side, but the panel must not linger.
    crate::agent_bridge::window::hide_panel(&app);
    if state.resolve(id, Outcome::Answered(answer)) {
        Ok(())
    } else {
        Err("question already resolved".into())
    }
}

#[tauri::command]
#[specta::specta]
pub fn agent_bridge_dismiss(
    app: tauri::AppHandle,
    state: State<BridgeState>,
    id: i64,
) -> Result<(), String> {
    crate::agent_bridge::window::hide_panel(&app);
    if state.resolve(id, Outcome::Dismissed) {
        Ok(())
    } else {
        Err("question already resolved".into())
    }
}

/// The question currently on screen, if any — pulled by the panel on mount
/// (the `agent-question` event can race past a cold webview).
#[tauri::command]
#[specta::specta]
pub fn agent_bridge_current(state: State<BridgeState>) -> Option<QuestionEvent> {
    state.current()
}

#[tauri::command]
#[specta::specta]
pub fn agent_bridge_answers(
    store: State<Arc<BridgeStore>>,
    since_ms: i64,
) -> Result<Vec<QuestionRow>, String> {
    store.list_since(since_ms).map_err(|e| e.to_string())
}
