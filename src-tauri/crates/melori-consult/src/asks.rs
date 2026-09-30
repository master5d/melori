use serde_json::{json, Value};
pub fn ask_request_body(question: &str, kind: &str, segments: &[Value], elapsed_ms: u64) -> Value {
    json!({"question": question, "kind": kind, "segments": segments, "elapsed_ms": elapsed_ms})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_carries_segments_and_time() {
        let segments = vec![json!({"source":"me","text":"hi"})];
        let body = ask_request_body("q", "free", &segments, 123);
        assert_eq!(body["question"], "q");
        assert_eq!(body["kind"], "free");
        assert_eq!(body["segments"], json!(segments));
        assert_eq!(body["elapsed_ms"], 123);
    }
}
