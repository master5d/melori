use super::lang::Lang;
use super::prompt::build_prompt;
use crate::settings::PostProcessProvider;

/// Runtime-agnostic translation seam. Callers depend only on this trait, so the
/// backend (currently a local Ollama / llama-server over `llm_client`) can change
/// without touching the dictation / file / CLI call sites.
///
/// The method is intentionally **synchronous**: the file-transcription pipeline is
/// sync and calls it directly, while the async dictation pipeline wraps it in
/// `tokio::task::spawn_blocking` so it never stalls the async executor.
pub trait Translator: Send + Sync {
    /// Translate `text` into `target`. Source language is auto-detected by the model.
    fn translate(&self, text: &str, target: Lang) -> anyhow::Result<String>;
}

/// `Translator` backed by an OpenAI-compatible chat server (Ollama serving Hy-MT1.5
/// on `127.0.0.1:11434/v1`). Reuses the existing `llm_client`. Sampling params
/// (temperature/top_p/top_k/repeat_penalty) are baked into the Ollama Modelfile,
/// so they are applied server-side and need not be threaded through here.
pub struct ServerTranslator {
    pub provider: PostProcessProvider,
    pub model: String,
    pub api_key: String,
}

impl Translator for ServerTranslator {
    fn translate(&self, text: &str, target: Lang) -> anyhow::Result<String> {
        let prompt = build_prompt(text, target);
        let provider = self.provider.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();

        // Drive the async client on a dedicated thread owning a fresh current-thread
        // runtime. `Runtime::block_on` panics if called from within an existing tokio
        // runtime worker, so we isolate it on its own thread to be safe regardless of
        // whether the caller is sync (file path) or already async (dictation path,
        // which additionally wraps this in spawn_blocking).
        let handle = std::thread::spawn(move || -> Result<Option<String>, String> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            rt.block_on(crate::llm_client::send_chat_completion(
                &provider, api_key, &model, prompt, None, None,
            ))
        });

        let out = handle
            .join()
            .map_err(|_| anyhow::anyhow!("translation thread panicked"))?
            .map_err(|e| anyhow::anyhow!(e))?;

        out.ok_or_else(|| anyhow::anyhow!("empty translation"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Minimal one-shot HTTP server: accepts a single connection, drains the
    /// request, and returns a canned OpenAI chat-completion JSON. Runs on its own
    /// thread (the translator makes a real blocking HTTP call to it).
    fn spawn_mock_server(body_content: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Read the request headers (enough to not RST the client).
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let json = format!(
                    r#"{{"choices":[{{"message":{{"role":"assistant","content":"{}"}}}}]}}"#,
                    body_content
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    json.len(),
                    json
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://{}", addr)
    }

    fn provider(base_url: String) -> PostProcessProvider {
        PostProcessProvider {
            id: "test".into(),
            label: "test".into(),
            base_url,
            allow_base_url_edit: false,
            models_endpoint: None,
            supports_structured_output: false,
        }
    }

    #[test]
    fn posts_prompt_and_returns_content() {
        let base = spawn_mock_server("Hello");
        let t = ServerTranslator {
            provider: provider(base),
            model: "hy-mt1.5".into(),
            api_key: String::new(),
        };
        let out = t.translate("Привет", Lang::English).unwrap();
        assert_eq!(out, "Hello");
    }

    #[test]
    fn empty_content_is_error() {
        // Server returns a body with no choices content -> translate errors out
        // (graceful-degradation is handled by callers, not here).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let json = r#"{"choices":[]}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    json.len(),
                    json
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        let t = ServerTranslator {
            provider: provider(format!("http://{}", addr)),
            model: "hy-mt1.5".into(),
            api_key: String::new(),
        };
        assert!(t.translate("Привет", Lang::English).is_err());
    }
}
