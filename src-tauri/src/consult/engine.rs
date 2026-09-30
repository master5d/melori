use melori_consult::Supervisor;
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

#[derive(Clone)]
pub struct EngineManager {
    supervisor: Supervisor,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct EngineStatus {
    pub state: String,
    pub url: Option<String>,
    pub disk_encryption: Option<String>,
}

impl EngineManager {
    pub fn new(
        python: String,
        dir: PathBuf,
        base_url: String,
        model: String,
        corpus_dir: String,
        embed_model: String,
    ) -> Self {
        Self {
            supervisor: Supervisor::new(python, dir, base_url, model, corpus_dir, embed_model),
        }
    }
    pub fn start(&self) {
        self.supervisor.start();
    }
    pub fn shutdown(&self) {
        self.supervisor.shutdown();
    }
    pub fn set_llm_config(&self, base: String, model: String, corpus: String, embed: String) {
        self.supervisor.set_llm_config(base, model, corpus, embed);
    }
    pub fn restart(&self) {
        self.supervisor.restart();
    }
    pub fn status(&self) -> EngineStatus {
        let status = self.supervisor.status();
        EngineStatus {
            state: status.state,
            url: status.url,
            disk_encryption: status.disk_encryption,
        }
    }
    pub fn url(&self) -> Option<String> {
        self.supervisor.url()
    }
    pub fn token(&self) -> String {
        self.supervisor.token()
    }
}

#[tauri::command]
#[specta::specta]
pub fn engine_status(state: State<'_, Arc<EngineManager>>) -> EngineStatus {
    state.status()
}

#[tauri::command]
#[specta::specta]
pub async fn engine_request(
    state: State<'_, Arc<EngineManager>>,
    method: String,
    path: String,
    body: Option<String>,
) -> Result<String, String> {
    if !melori_consult::is_allowed_engine_path(&method, &path) {
        return Err("engine path is not allowed".into());
    }
    let base = state
        .url()
        .ok_or_else(|| "consult engine unavailable".to_string())?;
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("engine client unavailable: {e}"))?;
    let method = reqwest::Method::from_bytes(method.trim().as_bytes())
        .map_err(|e| format!("invalid method: {e}"))?;
    let mut request = client
        .request(method, url)
        .header("X-Melori-Token", state.token());
    if path
        .split('?')
        .next()
        .unwrap_or(&path)
        .ends_with("/consent/signed")
    {
        let raw = body.ok_or_else(|| "signed consent body is required".to_string())?;
        let input: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("invalid signed consent body: {e}"))?;
        let filename = input
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("consent.bin");
        let filename = filename.replace(['\\', '/', '"', '\r', '\n'], "_");
        let encoded = input
            .get("content_base64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "content_base64 is required".to_string())?;
        let bytes = decode_base64(encoded).ok_or_else(|| "invalid content_base64".to_string())?;
        let boundary = "melori-consent-boundary";
        let mut multipart = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        ).into_bytes();
        multipart.extend(bytes);
        multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        request = request
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(multipart);
    } else if let Some(raw) = body {
        request = request.header("Content-Type", "application/json").body(raw);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    if !status.is_success() {
        return Err(text);
    }
    Ok(text)
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes().filter(|b| !b.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | digit;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod proxy_tests {
    #[test]
    fn engine_request_rejects_path_outside_allowlist() {
        assert!(!melori_consult::is_allowed_engine_path(
            "GET",
            "/admin/token"
        ));
        assert!(melori_consult::is_allowed_engine_path("GET", "/health"));
    }
}
