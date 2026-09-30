use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};

/// Filename for a captured note, e.g. `2026-05-29-141503-echo-note.md`.
pub fn capture_filename(now: DateTime<Local>) -> String {
    format!("{}-echo-note.md", now.format("%Y-%m-%d-%H%M%S"))
}

/// Build the markdown note: YAML frontmatter + blank line + body.
pub fn build_note(body: &str, now: DateTime<Local>, lang: Option<&str>) -> String {
    let mut fm = String::from("---\nsource: echo\n");
    fm.push_str(&format!("created: {}\n", now.to_rfc3339()));
    if let Some(l) = lang {
        if !l.is_empty() && l != "auto" {
            fm.push_str(&format!("lang: {}\n", l));
        }
    }
    fm.push_str("---\n\n");
    fm.push_str(body.trim());
    fm.push('\n');
    fm
}

/// Write `contents` into `dir` as `filename`, creating `dir` if needed.
/// On a same-second collision, append `-2`, `-3`, ... before `.md`.
pub fn write_capture(dir: &Path, filename: &str, contents: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating capture dir {:?}", dir))?;
    let mut path = dir.join(filename);
    if path.exists() {
        let stem = filename.strip_suffix(".md").unwrap_or(filename);
        for n in 2.. {
            let candidate = dir.join(format!("{stem}-{n}.md"));
            if !candidate.exists() {
                path = candidate;
                break;
            }
        }
    }
    std::fs::write(&path, contents).with_context(|| format!("writing note {:?}", path))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 5, 29, 14, 15, 3).unwrap()
    }

    #[test]
    fn filename_has_expected_shape() {
        assert_eq!(capture_filename(fixed()), "2026-05-29-141503-echo-note.md");
    }

    #[test]
    fn note_has_frontmatter_and_body() {
        let note = build_note("  buy milk  ", fixed(), Some("ru"));
        assert!(note.contains("source: echo"));
        assert!(note.contains("created: 2026-05-29T14:15:03"));
        assert!(note.contains("lang: ru"));
        assert!(note.trim_end().ends_with("buy milk"));
    }

    #[test]
    fn note_omits_lang_when_absent_or_auto() {
        assert!(!build_note("x", fixed(), None).contains("lang:"));
        assert!(!build_note("x", fixed(), Some("auto")).contains("lang:"));
    }

    #[test]
    fn write_round_trips_and_avoids_collision() {
        let dir = std::env::temp_dir().join("echo_capture_test_unique_xyz");
        let _ = std::fs::remove_dir_all(&dir);
        let p1 = write_capture(&dir, "note.md", "one").unwrap();
        let p2 = write_capture(&dir, "note.md", "two").unwrap();
        assert_ne!(p1, p2);
        assert_eq!(std::fs::read_to_string(&p1).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&p2).unwrap(), "two");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
