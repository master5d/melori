use std::sync::Arc;
use std::time::Duration;

use melori_consult::asks::ask_request_body;
use melori_consult::client_meeting::{finish_save, map_status, Binding, ClientBinding, SaveResult};
use melori_consult::notes::note_request_body;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};

use crate::consult::engine::EngineManager;
use crate::meeting::session::MeetingSession;

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct ClientBindingPayload {
    pub client_id: String,
    pub session_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct ClientSaveResult {
    pub retained: bool,
}

fn path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn client() -> Result<reqwest::Client, String> {
    client_with_timeout(Duration::from_secs(30))
}

/// Note generation waits for the language model: the engine's own LLM timeout plus a margin.
fn generation_client() -> Result<reqwest::Client, String> {
    let timeout_secs = std::env::var("MELORI_LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600);
    client_with_timeout(Duration::from_secs(timeout_secs.saturating_add(30)))
}

fn client_with_timeout(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .build()
        .map_err(|e| format!("engine client unavailable: {e}"))
}

/// Meeting segments in the engine's transcript shape (`source` is `me` / `others`).
pub fn segments_json(
    segments: &[crate::meeting::session::Segment],
    marks: &[u64],
) -> Vec<serde_json::Value> {
    let mut rows: Vec<(u64, serde_json::Value)> = segments
        .iter()
        .map(|s| {
            (
                s.start_ms,
                serde_json::json!({
                    "source": match s.source {
                        crate::meeting::Source::Me => "me",
                        crate::meeting::Source::Others => "others",
                    },
                    "start_ms": s.start_ms,
                    "end_ms": s.end_ms,
                    "text": s.text,
                }),
            )
        })
        .collect();
    rows.extend(marks.iter().copied().map(|at| {
        (
            at,
            serde_json::json!({
                "source": "mark", "start_ms": at, "end_ms": at, "text": ""
            }),
        )
    }));
    rows.sort_by_key(|(at, _)| *at);
    rows.into_iter()
        .enumerate()
        .map(|(i, (_, mut row))| {
            row["i"] = serde_json::json!(i + 1);
            row
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct AskProvenance {
    pub model: String,
    pub endpoint_local: bool,
    pub input: String,
    pub chars_sent: u64,
    pub until_ms: u64,
    pub truncated: bool,
    pub language: String,
    pub attempts: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct AskView {
    pub at: String,
    pub elapsed_ms: u64,
    pub kind: String,
    pub question: String,
    pub answer: String,
    pub provenance: AskProvenance,
    pub stored: bool,
}

pub async fn ask(
    engine: &EngineManager,
    binding: &ClientBinding,
    question: &str,
    kind: &str,
    segments: &[serde_json::Value],
    elapsed_ms: u64,
) -> Result<AskView, String> {
    let active = binding
        .get()
        .ok_or_else(|| "no client meeting binding".to_string())?;
    let (url, token) = engine_endpoint(
        engine,
        &format!(
            "/api/clients/{}/sessions/{}/asks",
            path_segment(&active.client_id),
            path_segment(&active.session_id)
        ),
    )?;
    let response = generation_client()?
        .post(url)
        .header("X-Melori-Token", token)
        .json(&ask_request_body(question, kind, segments, elapsed_ms))
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    map_status(status, &body)?;
    #[derive(Deserialize)]
    struct Response {
        ask: AskView,
        stored: bool,
    }
    let parsed: Response =
        serde_json::from_str(&body).map_err(|e| format!("engine response invalid: {e}"))?;
    Ok(AskView {
        stored: parsed.stored,
        ..parsed.ask
    })
}

fn engine_endpoint(engine: &EngineManager, path: &str) -> Result<(String, String), String> {
    let base = engine
        .url()
        .ok_or_else(|| "consult engine unavailable".to_string())?;
    Ok((
        format!("{}{path}", base.trim_end_matches('/')),
        engine.token(),
    ))
}

#[derive(Deserialize)]
struct StartResponse {
    id: String,
}

#[tauri::command]
#[specta::specta]
pub async fn start_client_meeting(
    app: AppHandle,
    engine: State<'_, Arc<EngineManager>>,
    binding: State<'_, ClientBinding>,
    sess: State<'_, MeetingSession>,
    client_id: String,
) -> Result<String, String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("client id is required".into());
    }
    let (url, token) = engine_endpoint(
        &engine,
        &format!("/api/clients/{}/sessions", path_segment(&client_id)),
    )?;
    let response = client()?
        .post(url)
        .header("X-Melori-Token", token)
        .json(&serde_json::json!({ "meeting_type": "session" }))
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    map_status(status, &body)?;
    let session_id = serde_json::from_str::<StartResponse>(&body)
        .map_err(|e| format!("engine response invalid: {e}"))?
        .id;

    binding.set(Binding {
        client_id,
        session_id: session_id.clone(),
    });
    if let Err(e) = crate::commands::meeting::start_meeting(app, sess).await {
        // Capture did not start: do not leave a binding that would route a later save/council
        // to a session that never ran.
        binding.clear();
        return Err(e);
    }
    Ok(session_id)
}

#[tauri::command]
#[specta::specta]
pub async fn save_client_meeting(
    engine: State<'_, Arc<EngineManager>>,
    binding: State<'_, ClientBinding>,
    sess: State<'_, MeetingSession>,
) -> Result<ClientSaveResult, String> {
    let active = binding
        .get()
        .ok_or_else(|| "no client meeting binding".to_string())?;
    let snapshot = sess.snapshot();
    if snapshot.segments.is_empty() {
        return Err("nothing to save yet".into());
    }
    let (url, token) = engine_endpoint(
        &engine,
        &format!(
            "/api/clients/{}/sessions/{}/close",
            path_segment(&active.client_id),
            path_segment(&active.session_id)
        ),
    )?;

    let response = client()?
        .post(url)
        .header("X-Melori-Token", token)
        .json(&serde_json::json!({ "note": "" }))
        .send()
        .await;
    let outcome = match response {
        Err(e) => Err(format!("engine unavailable: {e}")),
        Ok(response) => {
            let status = response.status().as_u16();
            match response.text().await {
                Err(e) => Err(format!("engine response unreadable: {e}")),
                Ok(body) => map_status(status, &body).and_then(|()| {
                    serde_json::from_str::<SaveResult>(&body)
                        .map_err(|e| format!("engine response invalid: {e}"))
                }),
            }
        }
    };
    finish_save(&binding, outcome).map(|result| ClientSaveResult {
        retained: result.retained,
    })
}

pub async fn push_transcript(
    engine: &EngineManager,
    binding: &ClientBinding,
    segments: &[serde_json::Value],
    duration_ms: u64,
) -> Result<(), String> {
    let active = binding
        .get()
        .ok_or_else(|| "no client meeting binding".to_string())?;
    let (url, token) = engine_endpoint(
        engine,
        &format!(
            "/api/clients/{}/sessions/{}/transcript",
            path_segment(&active.client_id),
            path_segment(&active.session_id)
        ),
    )?;
    let response = client()?
        .post(url)
        .header("X-Melori-Token", token)
        .json(&serde_json::json!({"segments": segments, "duration_ms": duration_ms}))
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    map_status(status, &body)
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct ClientNote {
    pub n: Option<u64>,
    pub template_id: String,
    pub sections: Vec<(String, String)>,
    pub fields: std::collections::HashMap<String, String>,
    pub stored: bool,
}

pub async fn generate_note(
    engine: &EngineManager,
    binding: &ClientBinding,
    template_id: &str,
    segments_if_not_retained: Option<&[serde_json::Value]>,
) -> Result<ClientNote, String> {
    let active = binding
        .get()
        .ok_or_else(|| "no client meeting binding".to_string())?;
    let permissions = bound_client_permissions(engine, binding)
        .await?
        .ok_or_else(|| "no client meeting binding".to_string())?;
    let segments = if permissions.iter().any(|p| p == "retain") {
        None
    } else {
        segments_if_not_retained
    };
    let (url, token) = engine_endpoint(
        engine,
        &format!(
            "/api/clients/{}/sessions/{}/notes:generate",
            path_segment(&active.client_id),
            path_segment(&active.session_id)
        ),
    )?;
    let response = generation_client()?
        .post(url)
        .header("X-Melori-Token", token)
        .json(&note_request_body(template_id, segments))
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    map_status(status, &body)?;
    #[derive(Deserialize)]
    struct Response {
        note: NotePayload,
        stored: bool,
    }
    #[derive(Deserialize)]
    struct NotePayload {
        n: Option<u64>,
        template_id: String,
        sections: Vec<(String, String)>,
        fields: std::collections::HashMap<String, String>,
    }
    let parsed: Response =
        serde_json::from_str(&body).map_err(|e| format!("engine response invalid: {e}"))?;
    Ok(ClientNote {
        n: parsed.note.n,
        template_id: parsed.note.template_id,
        sections: parsed.note.sections,
        fields: parsed.note.fields,
        stored: parsed.stored,
    })
}

#[tauri::command]
#[specta::specta]
pub fn client_binding(binding: State<'_, ClientBinding>) -> Option<ClientBindingPayload> {
    binding.get().map(|value| ClientBindingPayload {
        client_id: value.client_id,
        session_id: value.session_id,
    })
}

#[derive(Deserialize)]
struct ClientPermissions {
    consent: ConsentPermissions,
}

#[derive(Deserialize)]
struct ConsentPermissions {
    permissions: Vec<String>,
}

/// Consent permissions of the client bound to the running meeting; `None` when the
/// meeting has no client. An unreachable engine is an error — consent cannot be checked.
pub async fn bound_client_permissions(
    engine: &EngineManager,
    binding: &ClientBinding,
) -> Result<Option<Vec<String>>, String> {
    let Some(active) = binding.get() else {
        return Ok(None);
    };
    let (url, token) = engine_endpoint(
        engine,
        &format!("/api/clients/{}", path_segment(&active.client_id)),
    )?;
    let response = client()?
        .get(url)
        .header("X-Melori-Token", token)
        .send()
        .await
        .map_err(|e| format!("engine unavailable: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| format!("engine response unreadable: {e}"))?;
    map_status(status, &body)?;
    #[derive(Deserialize)]
    struct Detail {
        client: ClientPermissions,
    }
    let detail: Detail =
        serde_json::from_str(&body).map_err(|e| format!("engine response invalid: {e}"))?;
    Ok(Some(detail.client.consent.permissions))
}
