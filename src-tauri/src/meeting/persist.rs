//! Meeting Copilot (C2) — compose a meeting session as a markdown file.
//! Pure helpers; the async command in commands/meeting.rs adds the LLM tag call + write.

use chrono::{DateTime, Local};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::meeting::analyze::{AnalysisMode, MeetingAnalysis};
use crate::meeting::session::LiveTranscript;

/// Factual + tag metadata for a saved session.
pub struct SessionMeta {
    pub meeting_type: &'static str, // "business" | "session"
    pub created: String,            // rfc3339
    pub duration_ms: u64,
    pub sources: &'static str, // "me+others" | "mic only"
    pub segments: usize,
    pub tags: Vec<String>, // filled by the command after the LLM call
}

/// Derive factual metadata from the transcript snapshot. `tags` starts empty.
pub fn compute_meta(t: &LiveTranscript, mode: AnalysisMode, now_rfc3339: String) -> SessionMeta {
    let meeting_type = match mode {
        AnalysisMode::Business => "business",
        AnalysisMode::Session => "session",
    };
    let duration_ms = t.segments.iter().map(|s| s.end_ms).max().unwrap_or(0);
    let sources = if t.loopback_active {
        "me+others"
    } else {
        "mic only"
    };
    SessionMeta {
        meeting_type,
        created: now_rfc3339,
        duration_ms,
        sources,
        segments: t.segments.len(),
        tags: Vec::new(),
    }
}

/// Whole minutes, e.g. "38m". Floors; 0 → "0m".
pub fn format_duration(ms: u64) -> String {
    format!("{}m", ms / 60_000)
}

/// e.g. "2026-06-22-154011-session-meeting.md" (mirrors capture_filename shape).
pub fn session_filename(now: DateTime<Local>, meeting_type: &str) -> String {
    format!(
        "{}-{}-meeting.md",
        now.format("%Y-%m-%d-%H%M%S"),
        meeting_type
    )
}

const TAGS_PROMPT: &str = "Extract 3 to 5 short topical tags (single lowercase words or short \
hyphenated phrases) summarizing this meeting transcript. Reply ONLY with JSON matching the schema.";

pub fn tags_prompt() -> &'static str {
    TAGS_PROMPT
}

pub fn tags_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["tags"],
        "properties": {
            "tags": { "type": "array", "items": { "type": "string" } }
        }
    })
}

/// Parse `{ "tags": [...] }`; never errors (best-effort) — malformed → empty.
/// Trims, drops empties, caps at 5.
pub fn parse_tags(json: &str) -> Vec<String> {
    #[derive(Deserialize)]
    struct T {
        tags: Vec<String>,
    }
    match serde_json::from_str::<T>(json) {
        Ok(t) => t
            .tags
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.contains(['[', ']', ',', ':', '\n', '\r']))
            .take(5)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Compose the full markdown session file: YAML frontmatter + transcript + optional analysis.
pub fn format_session_markdown(
    meta: &SessionMeta,
    transcript: &str,
    analysis: Option<&MeetingAnalysis>,
) -> String {
    let mut s = String::from("---\n");
    s.push_str(&format!("type: {}\n", meta.meeting_type));
    s.push_str(&format!("created: {}\n", meta.created));
    s.push_str(&format!(
        "duration: {}\n",
        format_duration(meta.duration_ms)
    ));
    s.push_str(&format!("sources: {}\n", meta.sources));
    s.push_str(&format!("segments: {}\n", meta.segments));
    if !meta.tags.is_empty() {
        s.push_str(&format!("tags: [{}]\n", meta.tags.join(", ")));
    }
    s.push_str("---\n\n# Transcript\n\n");
    s.push_str(transcript.trim());
    s.push('\n');

    if let Some(a) = analysis {
        match a {
            MeetingAnalysis::Brief {
                summary,
                key_points,
                action_items,
                open_questions,
            } => {
                s.push_str("\n# Brief\n\n");
                s.push_str(&format!("## Summary\n\n{}\n\n", summary.trim()));
                s.push_str("## Key points\n\n");
                for k in key_points {
                    s.push_str(&format!("- {}\n", k));
                }
                s.push_str("\n## Action items\n\n");
                for a in action_items {
                    s.push_str(&format!("- {}\n", a));
                }
                s.push_str("\n## Open questions\n\n");
                for q in open_questions {
                    s.push_str(&format!("- {}\n", q));
                }
            }
            MeetingAnalysis::Soap {
                subjective,
                objective,
                assessment,
                plan,
            } => {
                s.push_str("\n# SOAP\n\n");
                s.push_str(&format!("## Subjective\n\n{}\n\n", subjective.trim()));
                s.push_str(&format!("## Objective\n\n{}\n\n", objective.trim()));
                s.push_str(&format!("## Assessment\n\n{}\n\n", assessment.trim()));
                s.push_str(&format!("## Plan\n\n{}\n", plan.trim()));
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::session::Segment;
    use crate::meeting::Source;
    use chrono::TimeZone;

    fn seg(end_ms: u64) -> Segment {
        Segment {
            id: 1,
            source: Source::Me,
            speaker: None,
            start_ms: 0,
            end_ms,
            text: "hi".into(),
        }
    }

    fn transcript(loopback: bool, ends: &[u64]) -> LiveTranscript {
        LiveTranscript {
            started_at: "2026-06-22T15:00:00Z".into(),
            active: false,
            loopback_active: loopback,
            segments: ends.iter().map(|e| seg(*e)).collect(),
        }
    }

    #[test]
    fn compute_meta_derives_fields() {
        let t = transcript(true, &[60_000, 120_000]);
        let m = compute_meta(
            &t,
            AnalysisMode::Session,
            "2026-06-22T15:40:11+00:00".into(),
        );
        assert_eq!(m.meeting_type, "session");
        assert_eq!(m.duration_ms, 120_000);
        assert_eq!(m.sources, "me+others");
        assert_eq!(m.segments, 2);
        assert!(m.tags.is_empty());

        let t2 = transcript(false, &[]);
        let m2 = compute_meta(&t2, AnalysisMode::Business, "x".into());
        assert_eq!(m2.meeting_type, "business");
        assert_eq!(m2.sources, "mic only");
        assert_eq!(m2.duration_ms, 0);
    }

    #[test]
    fn format_duration_floors_minutes() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(120_000), "2m");
        assert_eq!(format_duration(38 * 60_000 + 59_000), "38m");
    }

    #[test]
    fn session_filename_shape() {
        let now = Local.with_ymd_and_hms(2026, 6, 22, 15, 40, 11).unwrap();
        assert_eq!(
            session_filename(now, "session"),
            "2026-06-22-154011-session-meeting.md"
        );
    }

    #[test]
    fn parse_tags_handles_ok_and_malformed() {
        assert_eq!(
            parse_tags(r#"{"tags":["sleep"," boundaries ",""]}"#),
            vec!["sleep", "boundaries"]
        );
        assert_eq!(parse_tags("not json"), Vec::<String>::new());
        assert_eq!(parse_tags(r#"{"nope":1}"#), Vec::<String>::new());
        // tags with frontmatter-breaking characters are dropped
        assert_eq!(
            parse_tags(r#"{"tags":["ok","a]b","c,d","line\nbreak","x"]}"#),
            vec!["ok", "x"]
        );
    }

    #[test]
    fn markdown_frontmatter_and_transcript_always_present() {
        let mut m = compute_meta(
            &transcript(true, &[60_000]),
            AnalysisMode::Business,
            "C".into(),
        );
        let md = format_session_markdown(&m, "me: hello", None);
        assert!(md.contains("type: business"));
        assert!(md.contains("created: C"));
        assert!(md.contains("duration: 1m"));
        assert!(md.contains("sources: me+others"));
        assert!(md.contains("segments: 1"));
        assert!(!md.contains("tags:")); // empty tags → no line
        assert!(md.contains("# Transcript"));
        assert!(md.contains("me: hello"));
        assert!(!md.contains("# Brief"));
        assert!(!md.contains("# SOAP"));

        m.tags = vec!["a".into(), "b".into()];
        let md2 = format_session_markdown(&m, "x", None);
        assert!(md2.contains("tags: [a, b]"));
    }

    #[test]
    fn markdown_includes_brief_or_soap() {
        let m = compute_meta(
            &transcript(true, &[60_000]),
            AnalysisMode::Business,
            "C".into(),
        );
        let brief = MeetingAnalysis::Brief {
            summary: "s".into(),
            key_points: vec!["k".into()],
            action_items: vec!["a".into()],
            open_questions: vec!["q".into()],
        };
        let md = format_session_markdown(&m, "t", Some(&brief));
        assert!(md.contains("# Brief"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("- k"));
        assert!(!md.contains("# SOAP"));

        let soap = MeetingAnalysis::Soap {
            subjective: "su".into(),
            objective: "o".into(),
            assessment: "as".into(),
            plan: "p".into(),
        };
        let md2 = format_session_markdown(&m, "t", Some(&soap));
        assert!(md2.contains("# SOAP"));
        assert!(md2.contains("## Subjective"));
        assert!(md2.contains("## Plan"));
        assert!(!md2.contains("# Brief"));
    }
}
