//! Meeting Copilot (C5b) — SSE client types + frame parsing for the wellbeing psych-council
//! stream. Pure parsing here; the streaming command in commands/meeting.rs does the HTTP + emit.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CouncilOpinion {
    pub specialist_id: String,
    pub name: String,
    pub paradigm: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CouncilSynthesis {
    pub text: String,
    pub convergences: Vec<String>,
    pub divergences: Vec<String>,
}

/// One council stream frame. Tagged so the frontend narrows on `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CouncilFrame {
    /// Which lenses run and how many side by side.
    Plan {
        total: u32,
        concurrency: u32,
        specialists: Vec<String>,
    },
    #[serde(rename = "lens_start")]
    LensStart {
        specialist_id: String,
        name: String,
        index: u32,
    },
    Opinion {
        opinion: CouncilOpinion,
    },
    #[serde(rename = "lens_done")]
    LensDone {
        specialist_id: String,
        elapsed_s: f64,
    },
    #[serde(rename = "synthesis_start")]
    SynthesisStart,
    Synthesis {
        synthesis: CouncilSynthesis,
    },
    Done,
    Error {
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct CouncilEvent {
    pub frame: CouncilFrame,
}

/// Split an accumulated SSE buffer into complete frames (separated by a blank line) plus the
/// partial trailing remainder to carry into the next read.
pub fn split_frames(buf: &str) -> (Vec<String>, String) {
    let mut frames: Vec<String> = Vec::new();
    let mut rest = buf;
    while let Some(idx) = rest.find("\n\n") {
        let (frame, after) = rest.split_at(idx);
        if !frame.trim().is_empty() {
            frames.push(frame.to_string());
        }
        rest = &after[2..];
    }
    (frames, rest.to_string())
}

/// Parse one SSE frame ("event: X\ndata: {json}") into a CouncilFrame. Best-effort: a
/// malformed/unknown frame returns None so the stream keeps going.
pub fn parse_sse_frame(frame: &str) -> Option<CouncilFrame> {
    let mut event: Option<&str> = None;
    let mut data: Option<&str> = None;
    for line in frame.lines() {
        let line = line.trim_start();
        if let Some(v) = line.strip_prefix("event:") {
            event = Some(v.trim());
        } else if let Some(v) = line.strip_prefix("data:") {
            data = Some(v.trim());
        }
    }
    match event? {
        "opinion" => Some(CouncilFrame::Opinion {
            opinion: serde_json::from_str(data?).ok()?,
        }),
        "synthesis" => Some(CouncilFrame::Synthesis {
            synthesis: serde_json::from_str(data?).ok()?,
        }),
        "done" => Some(CouncilFrame::Done),
        "plan" => {
            #[derive(Deserialize)]
            struct P {
                total: u32,
                concurrency: u32,
                specialists: Vec<String>,
            }
            let p: P = serde_json::from_str(data?).ok()?;
            Some(CouncilFrame::Plan {
                total: p.total,
                concurrency: p.concurrency,
                specialists: p.specialists,
            })
        }
        "lens_start" => {
            #[derive(Deserialize)]
            struct S {
                specialist_id: String,
                name: String,
                index: u32,
            }
            let s: S = serde_json::from_str(data?).ok()?;
            Some(CouncilFrame::LensStart {
                specialist_id: s.specialist_id,
                name: s.name,
                index: s.index,
            })
        }
        "lens_done" => {
            #[derive(Deserialize)]
            struct D {
                specialist_id: String,
                elapsed_s: f64,
            }
            let d: D = serde_json::from_str(data?).ok()?;
            Some(CouncilFrame::LensDone {
                specialist_id: d.specialist_id,
                elapsed_s: d.elapsed_s,
            })
        }
        "synthesis_start" => Some(CouncilFrame::SynthesisStart),
        "error" => {
            #[derive(Deserialize)]
            struct E {
                detail: String,
            }
            let e: E = serde_json::from_str(data.unwrap_or("{}")).ok()?;
            Some(CouncilFrame::Error { detail: e.detail })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stage_frames() {
        assert_eq!(
            parse_sse_frame("event: plan\ndata: {\"total\":2,\"concurrency\":2,\"specialists\":[\"cbt\",\"emdr\"]}"),
            Some(CouncilFrame::Plan { total: 2, concurrency: 2, specialists: vec!["cbt".into(), "emdr".into()] })
        );
        assert_eq!(
            parse_sse_frame(
                "event: lens_start\ndata: {\"specialist_id\":\"cbt\",\"name\":\"CBT\",\"index\":1}"
            ),
            Some(CouncilFrame::LensStart {
                specialist_id: "cbt".into(),
                name: "CBT".into(),
                index: 1
            })
        );
        assert_eq!(
            parse_sse_frame(
                "event: lens_done\ndata: {\"specialist_id\":\"cbt\",\"elapsed_s\":12.5}"
            ),
            Some(CouncilFrame::LensDone {
                specialist_id: "cbt".into(),
                elapsed_s: 12.5
            })
        );
        assert_eq!(
            parse_sse_frame("event: synthesis_start\ndata: {\"opinions\":2}"),
            Some(CouncilFrame::SynthesisStart)
        );
    }

    #[test]
    fn parse_opinion_frame_ignores_extra_fields() {
        let f = "event: opinion\ndata: {\"specialist_id\":\"cbt\",\"name\":\"CBT\",\
\"paradigm\":\"Cognitive\",\"text\":\"t\",\"citations\":[],\"route\":\"psych_opinion\"}";
        match parse_sse_frame(f) {
            Some(CouncilFrame::Opinion { opinion }) => {
                assert_eq!(opinion.specialist_id, "cbt");
                assert_eq!(opinion.text, "t");
            }
            other => panic!("expected Opinion, got {other:?}"),
        }
    }

    #[test]
    fn parse_synthesis_frame() {
        let f = "event: synthesis\ndata: {\"text\":\"s\",\"convergences\":[\"a\"],\"divergences\":[\"b\"]}";
        assert_eq!(
            parse_sse_frame(f),
            Some(CouncilFrame::Synthesis {
                synthesis: CouncilSynthesis {
                    text: "s".into(),
                    convergences: vec!["a".into()],
                    divergences: vec!["b".into()],
                }
            })
        );
    }

    #[test]
    fn parse_done_and_error_frames() {
        assert_eq!(
            parse_sse_frame("event: done\ndata: {}"),
            Some(CouncilFrame::Done)
        );
        assert_eq!(
            parse_sse_frame("event: error\ndata: {\"detail\":\"down\"}"),
            Some(CouncilFrame::Error {
                detail: "down".into()
            })
        );
    }

    #[test]
    fn parse_rejects_malformed() {
        assert_eq!(parse_sse_frame("event: opinion\ndata: not json"), None);
        assert_eq!(parse_sse_frame("data: {}"), None);
        assert_eq!(parse_sse_frame("event: bogus\ndata: {}"), None);
    }

    #[test]
    fn split_frames_extracts_complete_and_keeps_partial() {
        let buf = "event: opinion\ndata: {}\n\nevent: done\ndata: {}\n\nevent: synth";
        let (frames, rest) = split_frames(buf);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], "event: opinion\ndata: {}");
        assert_eq!(rest, "event: synth");
    }
}
