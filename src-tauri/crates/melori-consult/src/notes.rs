use serde_json::{json, Value};

pub fn note_request_body(template_id: &str, segments: Option<&[Value]>) -> Value {
    match segments {
        Some(segments) => json!({"template_id": template_id, "segments": segments}),
        None => json!({"template_id": template_id}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_segments_when_provided() {
        let segments = vec![json!({"i": 1, "source": "me", "text": "hello"})];
        assert_eq!(
            note_request_body("dap", Some(&segments)),
            json!({
                "template_id": "dap", "segments": segments
            })
        );
    }

    #[test]
    fn omits_segments_when_transcript_is_retained() {
        assert_eq!(
            note_request_body("soap", None),
            json!({"template_id": "soap"})
        );
    }
}
