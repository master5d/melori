//! Meeting Copilot (C4) — descriptive analysis of the live transcript.
//! Business → Brief, Session → SOAP. These helpers are pure; the async command
//! in commands/meeting.rs composes them with the existing LLM call.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;

use crate::meeting::session::Segment;
use crate::meeting::Source;

#[derive(Serialize, Deserialize, Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisMode {
    Business,
    Session,
}

/// The structured analysis returned to the frontend. Internally tagged with
/// `kind` ("brief" | "soap") so the TS side narrows on `a.kind`.
#[derive(Serialize, Deserialize, Type, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MeetingAnalysis {
    Brief {
        summary: String,
        key_points: Vec<String>,
        action_items: Vec<String>,
        open_questions: Vec<String>,
    },
    Soap {
        subjective: String,
        objective: String,
        assessment: String,
        plan: String,
    },
}

/// One line per segment, "{source}: {text}". Empty input → empty string.
pub fn format_transcript(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| {
            let who = match s.source {
                Source::Me => "me",
                Source::Others => "others",
            };
            format!("{who}: {}", s.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const BRIEF_PROMPT: &str =
    "You analyze a meeting transcript labeled by speaker side (me / others). \
Produce a concise business meeting brief STRICTLY from what was said — do not invent facts. \
Reply ONLY with JSON matching the provided schema. Write in the same language as the transcript.";

const SOAP_PROMPT: &str = "You are documenting a practitioner session from a transcript labeled by \
speaker side (me = practitioner, others = client). Produce a SOAP note (Subjective, Objective, \
Assessment, Plan) as DESCRIPTIVE documentation drawn ONLY from what was actually said — do not \
invent findings, diagnoses, or measurements that are not present. Reply ONLY with JSON matching the \
provided schema. Write in the same language as the transcript.";

pub fn prompt_for(mode: AnalysisMode) -> &'static str {
    match mode {
        AnalysisMode::Business => BRIEF_PROMPT,
        AnalysisMode::Session => SOAP_PROMPT,
    }
}

pub fn schema_for(mode: AnalysisMode) -> Value {
    match mode {
        AnalysisMode::Business => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["summary", "key_points", "action_items", "open_questions"],
            "properties": {
                "summary": { "type": "string" },
                "key_points": { "type": "array", "items": { "type": "string" } },
                "action_items": { "type": "array", "items": { "type": "string" } },
                "open_questions": { "type": "array", "items": { "type": "string" } }
            }
        }),
        AnalysisMode::Session => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["subjective", "objective", "assessment", "plan"],
            "properties": {
                "subjective": { "type": "string" },
                "objective": { "type": "string" },
                "assessment": { "type": "string" },
                "plan": { "type": "string" }
            }
        }),
    }
}

/// Parse the model's JSON (bare fields, no `kind` tag) into the mode's variant.
pub fn parse_analysis(mode: AnalysisMode, json: &str) -> Result<MeetingAnalysis, String> {
    match mode {
        AnalysisMode::Business => {
            #[derive(Deserialize)]
            struct B {
                summary: String,
                key_points: Vec<String>,
                action_items: Vec<String>,
                open_questions: Vec<String>,
            }
            let b: B =
                serde_json::from_str(json).map_err(|e| format!("could not parse brief: {e}"))?;
            Ok(MeetingAnalysis::Brief {
                summary: b.summary,
                key_points: b.key_points,
                action_items: b.action_items,
                open_questions: b.open_questions,
            })
        }
        AnalysisMode::Session => {
            #[derive(Deserialize)]
            struct S {
                subjective: String,
                objective: String,
                assessment: String,
                plan: String,
            }
            let s: S =
                serde_json::from_str(json).map_err(|e| format!("could not parse SOAP: {e}"))?;
            Ok(MeetingAnalysis::Soap {
                subjective: s.subjective,
                objective: s.objective,
                assessment: s.assessment,
                plan: s.plan,
            })
        }
    }
}

/// True when the provider base URL's HOST is the local machine (loopback).
/// Parses the host exactly (not a substring scan) so `https://localhost.evil.com`
/// is correctly classified as NON-local.
pub fn is_local_url(base_url: &str) -> bool {
    let after_scheme = base_url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(base_url);
    // authority = up to the first path / query / fragment delimiter
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    // strip optional userinfo@
    let hostport = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    let host = if let Some(rest) = hostport.strip_prefix('[') {
        // IPv6 literal: [::1]:port -> ::1
        rest.split(']').next().unwrap_or("")
    } else {
        // host:port -> host
        hostport.split(':').next().unwrap_or("")
    };
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "0.0.0.0" | "::1"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::session::Segment;
    use crate::meeting::Source;

    fn seg(source: Source, text: &str) -> Segment {
        Segment {
            id: 1,
            source,
            speaker: None,
            start_ms: 0,
            end_ms: 1,
            text: text.into(),
        }
    }

    #[test]
    fn format_transcript_labels_and_joins() {
        let segs = vec![seg(Source::Me, "hello"), seg(Source::Others, "hi there")];
        assert_eq!(format_transcript(&segs), "me: hello\nothers: hi there");
        assert_eq!(format_transcript(&[]), "");
    }

    #[test]
    fn prompts_and_schemas_differ_by_mode() {
        assert_ne!(
            prompt_for(AnalysisMode::Business),
            prompt_for(AnalysisMode::Session)
        );
        let b = schema_for(AnalysisMode::Business);
        let s = schema_for(AnalysisMode::Session);
        assert!(b["properties"]["action_items"].is_object());
        assert!(b["properties"]["subjective"].is_null());
        assert!(s["properties"]["assessment"].is_object());
        assert!(s["properties"]["summary"].is_null());
    }

    #[test]
    fn parse_brief_ok() {
        let json =
            r#"{"summary":"s","key_points":["a"],"action_items":["b"],"open_questions":["c"]}"#;
        let a = parse_analysis(AnalysisMode::Business, json).unwrap();
        match a {
            MeetingAnalysis::Brief {
                summary,
                key_points,
                action_items,
                open_questions,
            } => {
                assert_eq!(summary, "s");
                assert_eq!(key_points, vec!["a"]);
                assert_eq!(action_items, vec!["b"]);
                assert_eq!(open_questions, vec!["c"]);
            }
            _ => panic!("expected Brief"),
        }
    }

    #[test]
    fn parse_soap_ok() {
        let json = r#"{"subjective":"s","objective":"o","assessment":"a","plan":"p"}"#;
        let a = parse_analysis(AnalysisMode::Session, json).unwrap();
        match a {
            MeetingAnalysis::Soap {
                subjective,
                objective,
                assessment,
                plan,
            } => {
                assert_eq!(
                    (subjective, objective, assessment, plan),
                    ("s".into(), "o".into(), "a".into(), "p".into())
                );
            }
            _ => panic!("expected Soap"),
        }
    }

    #[test]
    fn parse_rejects_malformed_json() {
        assert!(parse_analysis(AnalysisMode::Business, "not json").is_err());
        assert!(parse_analysis(AnalysisMode::Session, r#"{"subjective":"s"}"#).is_err());
    }

    #[test]
    fn is_local_url_detects_loopback() {
        assert!(is_local_url("http://127.0.0.1:4001/v1"));
        assert!(is_local_url("http://localhost:4001"));
        assert!(is_local_url("http://[::1]:8080"));
        assert!(!is_local_url("https://api.openai.com/v1"));
        assert!(!is_local_url("https://openrouter.ai/api/v1"));
        assert!(!is_local_url("https://localhost.evil.com/v1"));
        assert!(!is_local_url("http://127.0.0.1.attacker.com/v1"));
    }

    #[test]
    fn meeting_analysis_serializes_with_kind_tag() {
        let a = MeetingAnalysis::Brief {
            summary: "s".into(),
            key_points: vec![],
            action_items: vec![],
            open_questions: vec![],
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["kind"], "brief");
    }
}
