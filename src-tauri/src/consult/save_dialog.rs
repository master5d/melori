//! Save text through the native "Save as" dialog.
//!
//! WebView2 ignores `<a download>` on blob URLs, so exporting a session or the consent
//! text from the webview did nothing. The webview passes a suggested file name and the
//! text; the file is written only to the path the practitioner picks in the OS dialog.
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Keep only a file name: no directories, no characters Windows rejects.
pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if "<>:\"|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "melori.txt".into()
    } else {
        trimmed
    }
}

/// Ask where to save `contents`; `Ok(None)` when the dialog is cancelled.
#[tauri::command]
#[specta::specta]
pub async fn save_text_file_dialog(
    app: AppHandle,
    default_name: String,
    contents: String,
) -> Result<Option<String>, String> {
    let name = safe_file_name(&default_name);
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_string());
    let mut dialog = app.dialog().file().set_file_name(&name);
    if let Some(ext) = extension.as_deref() {
        dialog = dialog.add_filter(ext.to_uppercase(), &[ext]);
    }
    let Some(picked) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| format!("save path is not a local file: {e}"))?;
    std::fs::write(&path, contents)
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::safe_file_name;

    #[test]
    fn keeps_a_plain_name() {
        assert_eq!(
            safe_file_name("anna-2026-09-28-01.md"),
            "anna-2026-09-28-01.md"
        );
    }

    #[test]
    fn strips_directories_and_forbidden_characters() {
        assert_eq!(safe_file_name("..\\..\\Windows\\evil.md"), "evil.md");
        assert_eq!(safe_file_name("a/b/c:d?.txt"), "c_d_.txt");
    }

    #[test]
    fn falls_back_when_nothing_is_left() {
        assert_eq!(safe_file_name(".."), "melori.txt");
        assert_eq!(safe_file_name(""), "melori.txt");
    }
}
