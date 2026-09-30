//! Write Mode: захват выделения из активного приложения через буфер обмена.
//! Буфер пользователя обязан вернуться в исходное состояние на ЛЮБОМ выходе.

use crate::platform::clipboard::{read_clipboard, write_clipboard};
use crate::platform::input::EnigoState;
use tauri::{AppHandle, Manager};

/// Копирует текущее выделение активного окна и возвращает его текст.
///
/// Исходное содержимое буфера сохраняется и восстанавливается на любом пути
/// выхода, включая ошибочный — но ТОЛЬКО текстовое. Если в буфере лежала
/// картинка или список файлов, `read_clipboard` вернёт `Err`, восстанавливать
/// будет нечего, и там останется скопированное выделение. Это ровно то же
/// поведение, что у обычной вставки Echo (`paste_via_clipboard` тоже читает
/// только текст), поэтому write-mode ничего не ухудшает; полноценное сохранение
/// нетекстового буфера потребует image-API плагина и делается отдельно.
pub async fn capture_selection(app: &AppHandle) -> Result<String, String> {
    let original = read_clipboard(app).ok(); // может быть картинка/пусто — это нормально

    let copied = (|| -> Result<String, String> {
        {
            let enigo_state = app
                .try_state::<EnigoState>()
                .ok_or("Enigo state not initialized")?;
            let mut enigo = enigo_state
                .0
                .lock()
                .map_err(|e| format!("Failed to lock Enigo: {}", e))?;
            crate::platform::input::send_copy_ctrl_c(&mut enigo)?;
        }
        // Приложению нужно время положить выделение в буфер; величина взята
        // из того же порядка, что задержки вставки в clipboard.rs.
        std::thread::sleep(std::time::Duration::from_millis(120));
        read_clipboard(app)
    })();

    // Возврат буфера — ДО обработки результата, чтобы ошибка не оставила
    // пользователя с подменённым буфером.
    if let Some(orig) = original {
        let _ = write_clipboard(app, &orig);
    }

    copied
}

/// Переписать выделенный в активном приложении текст по голосовой инструкции.
/// Возвращает вставленный текст (для истории/тестов).
#[tauri::command]
#[specta::specta]
pub async fn rewrite_selection(app: AppHandle, instruction: String) -> Result<String, String> {
    let selection = capture_selection(&app).await?;
    if !echo_text::rewrite::is_usable_selection(&selection) {
        return Err("nothing selected".into());
    }
    let settings = crate::settings::get_settings(&app);
    let prompt = echo_text::rewrite::build_rewrite_prompt(&instruction);
    let raw = crate::actions::run_llm_prompt(&settings, &prompt, &selection)
        .await
        .ok_or_else(|| "LLM step unavailable (no provider/model configured)".to_string())?;
    let text = echo_text::rewrite::clean_rewrite_output(&raw);
    if text.trim().is_empty() {
        return Err("model returned empty text".into());
    }
    // Вставка заменяет выделение (оно всё ещё выделено после Ctrl+C).
    crate::platform::clipboard::paste(text.clone(), app, false)?;
    Ok(text)
}
