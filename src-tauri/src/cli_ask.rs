use anyhow::{Context, Result};

pub fn run_ask(
    question: &str,
    options: Option<&str>,
    timeout_s: u64,
    speak: bool,
    port: u16,
) -> Result<i32> {
    let token_path = crate::portable::data_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dirs_path().expect("app data dir"))
        .join(crate::agent_bridge::token::TOKEN_FILE);
    let token = std::fs::read_to_string(&token_path)
        .with_context(|| {
            format!(
                "token not found at {} — is Echo running?",
                token_path.display()
            )
        })?
        .trim()
        .to_string();
    let opts: Vec<String> = options
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_default();
    let body = serde_json::json!({
        "question": question,
        "kind": if opts.is_empty() { "text" } else { "choice" },
        "options": opts,
        "timeout_s": timeout_s,
        "speak": speak,
        "source": "cli",
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let resp: serde_json::Value = rt.block_on(async {
        let r = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/v1/ask"))
            .bearer_auth(&token)
            .json(&body)
            // saturating: a huge --ask-timeout must not overflow the Duration.
            .timeout(std::time::Duration::from_secs(timeout_s.saturating_add(30)))
            .send()
            .await?;
        let status = r.status();
        let body: serde_json::Value = r.json().await?;
        // Surface server-side rejections (401/429/400) instead of exiting silently.
        if !status.is_success() {
            let msg = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("request rejected");
            anyhow::bail!("server returned {}: {}", status.as_u16(), msg);
        }
        Ok::<_, anyhow::Error>(body)
    })?;
    match resp["status"].as_str() {
        Some("answered") => {
            println!("{}", resp["answer"].as_str().unwrap_or_default());
            Ok(0)
        }
        Some("timeout") => Ok(2),
        _ => Ok(3),
    }
}

/// Application data directory equivalent without an AppHandle.
fn dirs_path() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("lab.melori.app"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support/lab.melori.app"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".local/share/lab.melori.app"))
    }
}
