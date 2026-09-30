use crate::{LanguageHint, VoiceError, VoiceProfile, VoiceRef};
use std::path::Path;

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "voice".to_string()
    } else {
        out
    }
}

fn io_err<E: std::fmt::Display>(e: E) -> VoiceError {
    VoiceError::Io(e.to_string())
}

pub fn create_profile(
    voices_dir: &Path,
    display_name: &str,
    language_hint: LanguageHint,
    refs: Vec<VoiceRef>,
) -> Result<VoiceProfile, VoiceError> {
    let id = slugify(display_name);
    let dir = voices_dir.join(&id);
    std::fs::create_dir_all(&dir).map_err(io_err)?;

    let mut copied = Vec::with_capacity(refs.len());
    for (i, r) in refs.into_iter().enumerate() {
        let dest = dir.join(format!("ref-{:02}.wav", i + 1));
        std::fs::copy(&r.wav, &dest).map_err(io_err)?;
        copied.push(VoiceRef {
            wav: dest,
            transcript: r.transcript,
            duration_s: r.duration_s,
        });
    }

    let profile = VoiceProfile {
        id: id.clone(),
        display_name: display_name.to_string(),
        language_hint,
        created: now_iso8601(),
        notes: String::new(),
        refs: copied,
    };
    write_profile(&dir, &profile)?;
    Ok(profile)
}

fn write_profile(dir: &Path, p: &VoiceProfile) -> Result<(), VoiceError> {
    let json = serde_json::to_string_pretty(p).map_err(io_err)?;
    std::fs::write(dir.join("profile.json"), json).map_err(io_err)
}

pub fn load_profile(voices_dir: &Path, id: &str) -> Result<VoiceProfile, VoiceError> {
    let path = voices_dir.join(id).join("profile.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| VoiceError::Profile(format!("cannot read profile {id}: {e}")))?;
    serde_json::from_str(&raw)
        .map_err(|e| VoiceError::Profile(format!("corrupt profile {id}: {e}")))
}

pub fn list_profiles(voices_dir: &Path) -> Result<Vec<VoiceProfile>, VoiceError> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(voices_dir) {
        Ok(e) => e,
        Err(_) => return Ok(out), // no voices dir yet = no profiles
    };
    for entry in entries.flatten() {
        if entry.path().join("profile.json").exists() {
            if let Some(id) = entry.file_name().to_str() {
                if let Ok(p) = load_profile(voices_dir, id) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn remove_profile(voices_dir: &Path, id: &str) -> Result<(), VoiceError> {
    let dir = voices_dir.join(id);
    if !dir.exists() {
        return Err(VoiceError::Profile(format!("no such profile: {id}")));
    }
    std::fs::remove_dir_all(&dir).map_err(io_err)
}

pub fn validate_profile(p: &VoiceProfile) -> Result<(), VoiceError> {
    if p.refs.is_empty() {
        return Err(VoiceError::Profile(format!(
            "profile {} has no reference audio",
            p.id
        )));
    }
    for r in &p.refs {
        if !r.wav.exists() {
            return Err(VoiceError::Profile(format!(
                "profile {} reference missing on disk: {}",
                p.id,
                r.wav.display()
            )));
        }
        if r.transcript.trim().is_empty() {
            return Err(VoiceError::Profile(format!(
                "profile {} has a reference with a blank transcript",
                p.id
            )));
        }
    }
    Ok(())
}

fn now_iso8601() -> String {
    // Seconds since epoch is enough provenance; avoids a chrono dep in echo-voice.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_ref(dir: &Path, name: &str) -> VoiceRef {
        let wav = dir.join(name);
        std::fs::write(&wav, b"RIFFxxxxWAVE").unwrap();
        VoiceRef {
            wav,
            transcript: "hello world".into(),
            duration_s: 2.0,
        }
    }

    #[test]
    fn create_then_load_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let voices = tmp.path().join("voices");
        let src = TempDir::new().unwrap();
        let r = make_ref(src.path(), "clip.wav");
        let p = create_profile(&voices, "Sasha M", LanguageHint::Mixed, vec![r]).unwrap();
        assert_eq!(p.id, "sasha-m");
        assert_eq!(p.refs.len(), 1);
        // ref wav was copied into the profile dir
        assert!(p.refs[0].wav.starts_with(voices.join("sasha-m")));
        assert!(p.refs[0].wav.exists());
        let loaded = load_profile(&voices, "sasha-m").unwrap();
        assert_eq!(loaded, p);
    }

    #[test]
    fn list_and_remove() {
        let tmp = TempDir::new().unwrap();
        let voices = tmp.path().join("voices");
        let src = TempDir::new().unwrap();
        create_profile(
            &voices,
            "A",
            LanguageHint::En,
            vec![make_ref(src.path(), "a.wav")],
        )
        .unwrap();
        create_profile(
            &voices,
            "B",
            LanguageHint::Ru,
            vec![make_ref(src.path(), "b.wav")],
        )
        .unwrap();
        assert_eq!(list_profiles(&voices).unwrap().len(), 2);
        remove_profile(&voices, "a").unwrap();
        assert_eq!(list_profiles(&voices).unwrap().len(), 1);
    }

    #[test]
    fn validate_rejects_empty_refs() {
        let p = VoiceProfile {
            id: "x".into(),
            display_name: "X".into(),
            language_hint: LanguageHint::En,
            created: "now".into(),
            notes: String::new(),
            refs: vec![],
        };
        assert!(matches!(validate_profile(&p), Err(VoiceError::Profile(_))));
    }

    #[test]
    fn validate_rejects_missing_wav() {
        let p = VoiceProfile {
            id: "x".into(),
            display_name: "X".into(),
            language_hint: LanguageHint::En,
            created: "now".into(),
            notes: String::new(),
            refs: vec![VoiceRef {
                wav: PathBuf::from("/no/such.wav"),
                transcript: "hi".into(),
                duration_s: 1.0,
            }],
        };
        assert!(matches!(validate_profile(&p), Err(VoiceError::Profile(_))));
    }
}
