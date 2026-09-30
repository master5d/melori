use anyhow::{Context, Result};
use std::path::Path;

pub const TOKEN_FILE: &str = "agent_bridge_token";

/// Loads the bearer token from `<app_data>/agent_bridge_token`, creating a
/// random one (32 bytes hex) on first run. Agents read the same file.
pub fn load_or_create_token(app_data_dir: &Path) -> Result<String> {
    let path = app_data_dir.join(TOKEN_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).context("getrandom failed")?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::create_dir_all(app_data_dir)?;
    write_owner_only(&path, &token)?;
    Ok(token)
}

/// Owner-only (0600) on Unix; on Windows the per-user %APPDATA% ACL applies.
#[cfg(unix)]
fn write_owner_only(path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(contents.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_owner_only(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_then_reuses_token() {
        let dir = tempfile::tempdir().unwrap();
        let t1 = load_or_create_token(dir.path()).unwrap();
        let t2 = load_or_create_token(dir.path()).unwrap();
        assert_eq!(t1, t2);
        assert_eq!(t1.len(), 64); // 32 bytes hex
        assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
