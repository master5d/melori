pub use super::state::QuestionEvent;
use super::state::{BridgeState, Outcome};
use super::storage::BridgeStore;
use anyhow::Result;
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server};

pub struct ServerConfig {
    /// 0 = ephemeral (tests); real default comes from settings (4123).
    pub port: u16,
    pub token: String,
}

pub type AskSink = Arc<dyn Fn(QuestionEvent) + Send + Sync>;

#[derive(Deserialize)]
struct AskBody {
    question: String,
    #[serde(default = "default_kind")]
    kind: String, // "text" | "choice" | "confirm"
    #[serde(default)]
    options: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_s: u64,
    #[serde(default)]
    speak: bool,
    #[serde(default = "default_source")]
    source: String,
}
fn default_kind() -> String {
    "text".into()
}
fn default_timeout() -> u64 {
    300
}
fn default_source() -> String {
    "unknown".into()
}

#[derive(Deserialize)]
struct NotifyBody {
    message: String,
    #[serde(default)]
    speak: bool,
    #[serde(default = "default_source")]
    source: String,
}

const MAX_BODY: usize = 64 * 1024;
const MAX_WAITING: usize = 10;
const MAX_TIMEOUT_S: u64 = 30 * 60;

/// Starts the server on 127.0.0.1, returns the bound port.
pub fn start_server(
    cfg: ServerConfig,
    store: Arc<BridgeStore>,
    state: BridgeState,
    sink: AskSink,
) -> Result<u16> {
    let server = Server::http(("127.0.0.1", cfg.port))
        .map_err(|e| anyhow::anyhow!("agent-bridge bind failed: {e}"))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .unwrap_or(cfg.port);
    let token = Arc::new(cfg.token);
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            let store = store.clone();
            let state = state.clone();
            let sink = sink.clone();
            let token = token.clone();
            std::thread::spawn(move || handle(req, &token, store, state, sink));
        }
    });
    Ok(port)
}

fn json_response(code: u16, body: &serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let data = serde_json::to_vec(body).unwrap_or_default();
    Response::from_data(data)
        .with_status_code(code)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

fn authed(req: &tiny_http::Request, token: &str) -> bool {
    req.headers().iter().any(|h| {
        h.field
            .as_str()
            .as_str()
            .eq_ignore_ascii_case("authorization")
            && h.value.as_str() == format!("Bearer {token}")
    })
}

fn read_body<T: serde::de::DeserializeOwned>(req: &mut tiny_http::Request) -> Result<T, String> {
    let mut buf = Vec::new();
    use std::io::Read;
    req.as_reader()
        .take(MAX_BODY as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.len() > MAX_BODY {
        return Err("body too large".into());
    }
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

fn handle(
    mut req: tiny_http::Request,
    token: &str,
    store: Arc<BridgeStore>,
    state: BridgeState,
    sink: AskSink,
) {
    if !authed(&req, token) {
        let _ = req.respond(json_response(
            401,
            &serde_json::json!({"error": "unauthorized"}),
        ));
        return;
    }
    let url = req.url().to_string();
    let method = req.method().clone();
    let resp = match (method, url.as_str()) {
        (Method::Post, "/v1/ask") => match read_body::<AskBody>(&mut req) {
            Ok(b) => handle_ask(b, &store, &state, &sink),
            Err(e) => json_response(400, &serde_json::json!({"error": e})),
        },
        (Method::Post, "/v1/notify") => match read_body::<NotifyBody>(&mut req) {
            Ok(b) => {
                let _ = store.insert_question(&b.source, "notify", &b.message, None);
                sink(QuestionEvent {
                    id: 0,
                    kind: "notify".into(),
                    question: b.message,
                    options: vec![],
                    timeout_s: 0,
                    speak: b.speak,
                    source: b.source,
                });
                json_response(202, &serde_json::json!({"status": "accepted"}))
            }
            Err(e) => json_response(400, &serde_json::json!({"error": e})),
        },
        (Method::Get, u) if u.starts_with("/v1/answers") => {
            let since: i64 = u
                .split("since=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match store.list_since(since) {
                Ok(rows) => json_response(200, &serde_json::to_value(rows).unwrap_or_default()),
                Err(e) => json_response(500, &serde_json::json!({"error": e.to_string()})),
            }
        }
        _ => json_response(404, &serde_json::json!({"error": "not found"})),
    };
    let _ = req.respond(resp);
}

fn handle_ask(
    b: AskBody,
    store: &Arc<BridgeStore>,
    state: &BridgeState,
    sink: &AskSink,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if b.question.trim().is_empty() {
        return json_response(400, &serde_json::json!({"error": "empty question"}));
    }
    if state.waiting.load(Ordering::SeqCst) >= MAX_WAITING {
        return json_response(429, &serde_json::json!({"error": "ask queue full"}));
    }
    state.waiting.fetch_add(1, Ordering::SeqCst);
    // One question on screen at a time; recover from poisoning so a panicked
    // handler can't wedge every future ask.
    let _serial = state.ask_serial.lock().unwrap_or_else(|e| e.into_inner());
    state.waiting.fetch_sub(1, Ordering::SeqCst);

    let options_json = if b.options.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&b.options).unwrap_or_default())
    };
    let id = match store.insert_question(&b.source, &b.kind, &b.question, options_json.as_deref()) {
        Ok(id) => id,
        Err(e) => return json_response(500, &serde_json::json!({"error": e.to_string()})),
    };
    let timeout_s = b.timeout_s.clamp(5, MAX_TIMEOUT_S);
    let pending = state.begin_question(id);
    let ev = QuestionEvent {
        id,
        kind: b.kind.clone(),
        question: b.question.clone(),
        options: b.options.clone(),
        timeout_s,
        speak: b.speak,
        source: b.source.clone(),
    };
    // Recorded BEFORE the sink so a freshly created panel window can pull it
    // on mount even if the emitted event raced past an unloaded webview.
    state.set_current(ev.clone());
    sink(ev);
    let outcome = pending.wait(Duration::from_secs(timeout_s));
    state.clear_current_if(id);
    let (status, answer) = match &outcome {
        Outcome::Answered(a) => {
            let _ = store.mark_answered(id, a);
            ("answered", Some(a.clone()))
        }
        Outcome::Dismissed => {
            let _ = store.mark_dismissed(id);
            ("dismissed", None)
        }
        Outcome::Timeout => {
            let _ = store.mark_timeout(id);
            ("timeout", None)
        }
    };
    json_response(
        200,
        &serde_json::json!({"id": id, "status": status, "answer": answer}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn boot(answer_with: Option<&'static str>) -> (u16, Arc<BridgeStore>) {
        let store = Arc::new(BridgeStore::open_in_memory().unwrap());
        let state = BridgeState::new();
        let st = state.clone();
        let sink: AskSink = Arc::new(move |ev: QuestionEvent| {
            if let Some(ans) = answer_with {
                let st = st.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(30));
                    st.resolve(ev.id, Outcome::Answered(ans.to_string()));
                });
            }
        });
        let port = start_server(
            ServerConfig {
                port: 0,
                token: "secret".into(),
            },
            store.clone(),
            state,
            sink,
        )
        .unwrap();
        (port, store)
    }

    #[test]
    fn rejects_missing_token() {
        let (port, _) = boot(None);
        let err = ureq::post(&format!("http://127.0.0.1:{port}/v1/ask"))
            .send_json(ureq::json!({"question": "hi"}))
            .unwrap_err();
        match err {
            ureq::Error::Status(code, _) => assert_eq!(code, 401),
            other => panic!("expected status error, got {other:?}"),
        }
    }

    #[test]
    fn ask_roundtrip_answered() {
        let (port, store) = boot(Some("да"));
        let resp: serde_json::Value = ureq::post(&format!("http://127.0.0.1:{port}/v1/ask"))
            .set("Authorization", "Bearer secret")
            .send_json(ureq::json!({"question": "Готово?", "kind": "confirm", "timeout_s": 5}))
            .unwrap()
            .into_json()
            .unwrap();
        assert_eq!(resp["status"], "answered");
        assert_eq!(resp["answer"], "да");
        assert_eq!(store.list_since(0).unwrap()[0].status, "answered");
    }

    #[test]
    fn ask_times_out() {
        let (port, store) = boot(None);
        let resp: serde_json::Value = ureq::post(&format!("http://127.0.0.1:{port}/v1/ask"))
            .set("Authorization", "Bearer secret")
            .send_json(ureq::json!({"question": "Эй?", "timeout_s": 1}))
            .unwrap()
            .into_json()
            .unwrap();
        assert_eq!(resp["status"], "timeout");
        assert_eq!(store.list_since(0).unwrap()[0].status, "timeout");
    }

    #[test]
    fn notify_and_answers_endpoints() {
        let (port, _) = boot(None);
        let r = ureq::post(&format!("http://127.0.0.1:{port}/v1/notify"))
            .set("Authorization", "Bearer secret")
            .send_json(ureq::json!({"message": "done"}))
            .unwrap();
        assert_eq!(r.status(), 202);
        let rows: serde_json::Value =
            ureq::get(&format!("http://127.0.0.1:{port}/v1/answers?since=0"))
                .set("Authorization", "Bearer secret")
                .call()
                .unwrap()
                .into_json()
                .unwrap();
        assert!(rows.as_array().unwrap().len() >= 1); // notify journaled as kind=notify
    }
}
