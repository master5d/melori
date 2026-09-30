//! Замеры «до/после» ежедневной практики (интейк #136, B-23).
//!
//! Паттерн школы: эталонная запись («Точка А») в первый день и периодические
//! повторы той же фразы — слышимое доказательство прогресса. Файлы живут в
//! `<app-data>/practice/` (собственный каталог в asset-scope — см. `lib.rs`;
//! в `recordings/` их класть нельзя: History считала бы их диктовками).
//!
//! Формат — как отдал MediaRecorder (webm/opus): webview его и пишет, и играет;
//! перекодирование добавило бы зависимость ради нуля пользы.
use serde::Serialize;
use std::path::PathBuf;

fn practice_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = crate::portable::app_data_dir(app)
        .map_err(|e| format!("app data dir: {e}"))?
        .join("practice");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create practice dir: {e}"))?;
    Ok(dir)
}

/// Метка файла: только [a-z0-9-], до 32 символов — имя попадает в путь,
/// и валидация здесь строже, чем «почистим потом».
fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty() || label.len() > 32 {
        return Err("label: 1..=32 chars".into());
    }
    if !label
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("label: only [a-z0-9-]".into());
    }
    Ok(())
}

#[derive(Serialize, specta::Type)]
pub struct PracticeTake {
    pub path: String,
    pub label: String,
    /// unix-секунды из имени файла — момент записи, не mtime (копирование файла
    /// не должно менять его место на шкале «до/после»).
    pub created_unix: i64,
}

#[tauri::command]
#[specta::specta]
pub fn practice_save_take(
    app: tauri::AppHandle,
    label: String,
    data: Vec<u8>,
) -> Result<PracticeTake, String> {
    validate_label(&label)?;
    if data.is_empty() {
        return Err("empty audio".into());
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    let dir = practice_dir(&app)?;
    let path = dir.join(format!("practice-{label}-{ts}.webm"));
    std::fs::write(&path, data).map_err(|e| format!("write take: {e}"))?;
    Ok(PracticeTake {
        path: path.to_string_lossy().into_owned(),
        label,
        created_unix: ts,
    })
}

#[tauri::command]
#[specta::specta]
pub fn practice_takes(app: tauri::AppHandle) -> Result<Vec<PracticeTake>, String> {
    let dir = practice_dir(&app)?;
    let mut out: Vec<PracticeTake> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("read practice dir: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // practice-<label>-<unix>.webm; чужие файлы молча пропускаются
        let Some(stem) = name
            .strip_prefix("practice-")
            .and_then(|s| s.strip_suffix(".webm"))
        else {
            continue;
        };
        let Some((label, ts)) = stem.rsplit_once('-') else {
            continue;
        };
        let Ok(created_unix) = ts.parse::<i64>() else {
            continue;
        };
        out.push(PracticeTake {
            path: entry.path().to_string_lossy().into_owned(),
            label: label.to_string(),
            created_unix,
        });
    }
    out.sort_by_key(|t| t.created_unix);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::validate_label;

    #[test]
    fn labels_are_path_safe() {
        assert!(validate_label("baseline").is_ok());
        assert!(validate_label("diag-60s").is_ok());
        assert!(validate_label("").is_err());
        assert!(validate_label("има-кириллица").is_err());
        assert!(validate_label("dots.and/slashes").is_err());
        assert!(validate_label(&"x".repeat(33)).is_err());
    }
}
