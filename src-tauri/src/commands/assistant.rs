#[tauri::command]
#[specta::specta]
pub async fn assistant_ask(app: tauri::AppHandle, text: String) -> Result<String, String> {
    crate::assistant::ask_assistant(&app, text).await
}
