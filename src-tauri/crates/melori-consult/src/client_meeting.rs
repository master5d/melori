use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Binding {
    pub client_id: String,
    pub session_id: String,
}

#[derive(Default)]
pub struct ClientBinding(Mutex<Option<Binding>>);

impl ClientBinding {
    pub fn get(&self) -> Option<Binding> {
        self.0.lock().unwrap().clone()
    }

    pub fn set(&self, binding: Binding) {
        *self.0.lock().unwrap() = Some(binding);
    }

    pub fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SaveResult {
    pub retained: bool,
}

pub fn close_body(markdown: &str) -> Value {
    json!({ "note": markdown })
}

pub fn map_status(code: u16, body: &str) -> Result<(), String> {
    if (200..300).contains(&code) {
        return Ok(());
    }
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("detail").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_owned());
    match code {
        403 => Err(format!("consent: {detail}")),
        404 => Err(format!("not found: {detail}")),
        _ => Err(format!("engine error {code}")),
    }
}

pub fn finish_save(
    binding: &ClientBinding,
    result: Result<SaveResult, String>,
) -> Result<SaveResult, String> {
    if result.is_ok() {
        binding.clear();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_maps_to_consent_error() {
        let e = map_status(403, r#"{"detail":"permission 'transcript' not granted for client anna"}"#).unwrap_err();
        assert!(e.starts_with("consent:"));
    }

    #[test]
    fn ok_maps_to_ok() {
        assert!(map_status(200, "{}").is_ok());
    }

    #[test]
    fn server_error_keeps_binding() {
        let b = ClientBinding::default();
        b.set(Binding { client_id: "anna".into(), session_id: "2026-09-27-01".into() });
        let r = finish_save(&b, Err("engine error 500".into()));
        assert!(r.is_err());
        assert!(b.get().is_some(), "binding must survive a failed save so it can be retried");
    }

    #[test]
    fn successful_save_clears_binding() {
        let b = ClientBinding::default();
        b.set(Binding { client_id: "anna".into(), session_id: "2026-09-27-01".into() });
        let r = finish_save(&b, Ok(SaveResult { retained: true }));
        assert!(r.is_ok());
        assert!(b.get().is_none());
    }

    #[test]
    fn close_body_carries_note() {
        assert_eq!(close_body("# md")["note"], "# md");
    }
}
