//! Pure transcript formatting: turns engine segments (+ optional speaker turns)
//! into plain text, inline timestamps, SRT, WebVTT, or JSON. No I/O, no engines.

use serde::Serialize;

/// One transcript segment with start/end in seconds (from the ASR engine).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimedSegment {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// Zero-based speaker index. Rendered to humans as `Speaker {id+1}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SpeakerId(pub u32);

/// A contiguous interval attributed to one speaker (from diarization).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerTurn {
    pub start: f32,
    pub end: f32,
    pub speaker: SpeakerId,
}

/// Output format selected via CLI `--format` or GUI dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Plain,
    Inline,
    Srt,
    Vtt,
    Json,
    Karaoke,
    /// Speaker-grouped transcript: consecutive same-speaker segments collapsed
    /// into one block headed `[Speaker N] (M:SS - M:SS)` followed by the text.
    /// Intended for diarized output (`--diarize`).
    Speaker,
}

impl OutputFormat {
    /// Parse the CLI `--format` value. Case-insensitive. Returns None if unknown.
    pub fn from_cli(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "plain" | "txt" | "text" => Some(Self::Plain),
            "inline" => Some(Self::Inline),
            "srt" => Some(Self::Srt),
            "vtt" | "webvtt" => Some(Self::Vtt),
            "json" => Some(Self::Json),
            "karaoke" => Some(Self::Karaoke),
            "speaker" | "speakers" | "diarized" => Some(Self::Speaker),
            _ => None,
        }
    }

    /// Infer a format from an output file extension (used when `--format` is
    /// omitted but `-o foo.srt` is given). Unknown extensions → None.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "srt" => Some(Self::Srt),
            "vtt" => Some(Self::Vtt),
            "json" => Some(Self::Json),
            "txt" | "text" => Some(Self::Plain),
            "md" | "markdown" => Some(Self::Plain),
            _ => None,
        }
    }

    /// Formats that consume per-word timings.
    pub fn is_word_level(self) -> bool {
        matches!(self, OutputFormat::Karaoke)
    }
}

/// Human label for a speaker, 1-based: SpeakerId(0) -> "Speaker 1".
pub fn speaker_label(id: SpeakerId) -> String {
    format!("Speaker {}", id.0 + 1)
}

/// `HH:MM:SS,mmm` (SRT uses a comma before milliseconds).
pub fn fmt_srt(secs: f32) -> String {
    let (h, m, s, ms) = hmsms(secs);
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// `HH:MM:SS.mmm` (WebVTT uses a dot before milliseconds).
pub fn fmt_vtt(secs: f32) -> String {
    let (h, m, s, ms) = hmsms(secs);
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}

/// `MM:SS` for inline prefixes. Minutes may exceed 59 (e.g. `75:30`) so a single
/// glance reads as "minutes:seconds" without hour parsing.
pub fn fmt_inline(secs: f32) -> String {
    let total = secs.max(0.0).round() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// `M:SS` clock for speaker-block headers — minutes have no leading zero and may
/// exceed 59 (e.g. `0:01`, `2:14`, `75:30`). Matches the `(0:01 - 0:16)` style.
pub fn fmt_clock(secs: f32) -> String {
    let total = secs.max(0.0).round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

fn hmsms(secs: f32) -> (u64, u64, u64, u64) {
    let clamped = secs.max(0.0);
    let total_ms = (clamped * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    (total_s / 3600, (total_s % 3600) / 60, total_s % 60, ms)
}

/// Assign a speaker to each segment by maximum time overlap with the turns.
/// Segments with no overlapping turn get `None`. Output is index-aligned to
/// `segments`. Pure and deterministic.
pub fn assign_speakers(segments: &[TimedSegment], turns: &[SpeakerTurn]) -> Vec<Option<SpeakerId>> {
    segments
        .iter()
        .map(|seg| {
            let mut best: Option<(f32, SpeakerId)> = None;
            for turn in turns {
                let overlap = (seg.end.min(turn.end) - seg.start.max(turn.start)).max(0.0);
                if overlap > 0.0 && best.map(|(o, _)| overlap > o).unwrap_or(true) {
                    best = Some((overlap, turn.speaker));
                }
            }
            best.map(|(_, sp)| sp)
        })
        .collect()
}

/// Split each segment into sub-segments at speaker-turn boundaries. Words are
/// distributed across the segment's duration uniformly (assuming even speaking
/// rate), each word assigned to the turn covering its midpoint; consecutive
/// words with the same speaker are regrouped into one sub-segment. Segments that
/// overlap no turn are emitted unchanged with `None`. Pure and deterministic.
pub fn split_segments_by_speakers(
    segments: &[TimedSegment],
    turns: &[SpeakerTurn],
) -> Vec<(TimedSegment, Option<SpeakerId>)> {
    let mut out: Vec<(TimedSegment, Option<SpeakerId>)> = Vec::new();

    for seg in segments {
        let words: Vec<&str> = seg.text.split_whitespace().collect();
        let dur = (seg.end - seg.start).max(0.0);

        // Fast path: no words or zero duration -> single chunk by max overlap.
        if words.is_empty() || dur == 0.0 {
            let sp = speaker_at(seg.start, seg.end, turns);
            out.push((seg.clone(), sp));
            continue;
        }

        // Assign each word to a speaker by its midpoint time.
        let n = words.len();
        let mut word_speakers: Vec<Option<SpeakerId>> = Vec::with_capacity(n);
        for (i, _w) in words.iter().enumerate() {
            let w_start = seg.start + dur * (i as f32) / (n as f32);
            let w_end = seg.start + dur * ((i + 1) as f32) / (n as f32);
            word_speakers.push(speaker_at(w_start, w_end, turns));
        }

        // Group consecutive words with the same speaker into sub-segments.
        let mut i = 0;
        while i < n {
            let sp = word_speakers[i];
            let mut j = i + 1;
            while j < n && word_speakers[j] == sp {
                j += 1;
            }
            let sub_start = seg.start + dur * (i as f32) / (n as f32);
            let sub_end = seg.start + dur * (j as f32) / (n as f32);
            out.push((
                TimedSegment {
                    start: sub_start,
                    end: sub_end,
                    text: words[i..j].join(" "),
                },
                sp,
            ));
            i = j;
        }
    }

    out
}

/// Assign each word to the speaker covering its midpoint (via `speaker_at`), then
/// regroup consecutive same-speaker words into sub-segments. Pure/deterministic.
pub fn assign_words_to_speakers(
    words: &[TimedSegment],
    turns: &[SpeakerTurn],
) -> Vec<(TimedSegment, Option<SpeakerId>)> {
    let mut out: Vec<(TimedSegment, Option<SpeakerId>)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let sp = speaker_at(words[i].start, words[i].end, turns);
        let mut j = i + 1;
        while j < words.len() && speaker_at(words[j].start, words[j].end, turns) == sp {
            j += 1;
        }
        let text = words[i..j]
            .iter()
            .map(|w| w.text.trim())
            .collect::<Vec<_>>()
            .join(" ");
        out.push((
            TimedSegment {
                start: words[i].start,
                end: words[j - 1].end,
                text,
            },
            sp,
        ));
        i = j;
    }
    out
}

/// Speaker whose turn has the greatest overlap with [start, end]; None if none overlap.
fn speaker_at(start: f32, end: f32, turns: &[SpeakerTurn]) -> Option<SpeakerId> {
    let mut best: Option<(f32, SpeakerId)> = None;
    for turn in turns {
        let overlap = (end.min(turn.end) - start.max(turn.start)).max(0.0);
        if overlap > 0.0 && best.map(|(o, _)| overlap > o).unwrap_or(true) {
            best = Some((overlap, turn.speaker));
        }
    }
    best.map(|(_, sp)| sp)
}

#[derive(serde::Serialize)]
struct JsonWord {
    word: String,
    start: f32,
    end: f32,
    speaker: Option<u32>,            // 0-based (Deepgram convention)
    speaker_confidence: Option<f32>, // overlap-based proxy, see note
}

/// Fraction of [start,end] covered by the assigned speaker's turn(s). 1.0 = fully
/// inside one turn, <1.0 = spans a boundary. NOT an ML confidence (overlap proxy).
pub fn speaker_overlap_confidence(
    start: f32,
    end: f32,
    sp: SpeakerId,
    turns: &[SpeakerTurn],
) -> f32 {
    let dur = (end - start).max(f32::EPSILON);
    let covered: f32 = turns
        .iter()
        .filter(|t| t.speaker == sp)
        .map(|t| (end.min(t.end) - start.max(t.start)).max(0.0))
        .sum();
    (covered / dur).clamp(0.0, 1.0)
}

/// Render Deepgram-shaped word-level JSON. `speaker` is 0-based.
pub fn render_word_json(words: &[TimedSegment], turns: Option<&[SpeakerTurn]>) -> String {
    let json_words: Vec<JsonWord> = words
        .iter()
        .map(|w| {
            let sp = turns.and_then(|t| speaker_at(w.start, w.end, t));
            JsonWord {
                word: w.text.trim().to_string(),
                start: w.start,
                end: w.end,
                speaker: sp.map(|s| s.0),
                speaker_confidence: sp
                    .map(|s| speaker_overlap_confidence(w.start, w.end, s, turns.unwrap_or(&[]))),
            }
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "words": json_words }))
        .unwrap_or_else(|_| "{\"words\":[]}".to_string())
}

/// Per-word WebVTT with inline timing tags (karaoke highlight). One cue per line;
/// within a cue each word is prefixed with its `<mm:ss.mmm>` start tag. Lines are
/// grouped by consecutive same-speaker run when diarization turns are given, else by
/// sentence-final punctuation (`. ! ? …`) so cues read as natural sentences instead of
/// one word per cue.
pub fn render_karaoke(words: &[TimedSegment], turns: Option<&[SpeakerTurn]>) -> String {
    let mut out = String::from("WEBVTT\n\n");
    let mut i = 0;
    while i < words.len() {
        // Speaker of the line (for the optional <v> voice tag), from the first word.
        let sp = turns.and_then(|t| speaker_at(words[i].start, words[i].end, t));
        // Extend the line until the grouping boundary.
        let mut j = i + 1;
        while j < words.len() {
            let boundary = match turns {
                // Diarized: break when the speaker changes.
                Some(t) => speaker_at(words[j].start, words[j].end, t) != sp,
                // Undiarized: break after a word ending a sentence.
                None => words[j - 1].text.trim_end().ends_with(['.', '!', '?', '…']),
            };
            if boundary {
                break;
            }
            j += 1;
        }
        let speaker_tag = sp
            .map(|s| format!("<v {}>", speaker_label(s)))
            .unwrap_or_default();
        let body: String = words[i..j]
            .iter()
            .map(|w| format!("<{}>{}", fmt_vtt_short(w.start), w.text.trim()))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "{} --> {}\n{}{}\n\n",
            fmt_vtt(words[i].start),
            fmt_vtt(words[j - 1].end),
            speaker_tag,
            body
        ));
        i = j;
    }
    out.trim_end().to_string()
}

/// `MM:SS.mmm` (karaoke word tag form, no hours).
fn fmt_vtt_short(t: f32) -> String {
    let total_ms = (t * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let s = (total_ms / 1000) % 60;
    let m = total_ms / 60000;
    format!("{:02}:{:02}.{:03}", m, s, ms)
}

/// JSON shape for `OutputFormat::Json`.
#[derive(Serialize)]
struct JsonSegment {
    start: f32,
    end: f32,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<String>,
}

#[derive(Serialize)]
struct JsonDoc {
    segments: Vec<JsonSegment>,
}

/// Render segments (+ optional speaker turns) into the chosen format.
///
/// `plain_text` is the already-joined, post-processed transcript used for
/// `Plain` (preserves current CLI behavior exactly). Other formats build from
/// `segments`.
pub fn render(
    plain_text: &str,
    segments: &[TimedSegment],
    words: Option<&[TimedSegment]>,
    speakers: Option<&[SpeakerTurn]>,
    format: OutputFormat,
) -> String {
    if format == OutputFormat::Plain {
        return plain_text.to_string();
    }

    let resolved: Vec<(TimedSegment, Option<SpeakerId>)> = match (words, speakers) {
        // Accurate word path: group words by speaker.
        (Some(w), Some(turns)) => assign_words_to_speakers(w, turns),
        // Words but no diarization: collapse to the sentence segments (avoid per-word cues).
        (Some(_), None) => segments.iter().map(|s| (s.clone(), None)).collect(),
        // No words: existing behavior.
        (None, Some(turns)) => split_segments_by_speakers(segments, turns),
        (None, None) => segments.iter().map(|s| (s.clone(), None)).collect(),
    };

    let prefix = |sp: Option<SpeakerId>| -> String {
        sp.map(|s| format!("{}: ", speaker_label(s)))
            .unwrap_or_default()
    };

    match format {
        OutputFormat::Plain => unreachable!(),

        OutputFormat::Inline => resolved
            .iter()
            .map(|(s, sp)| format!("[{}] {}{}", fmt_inline(s.start), prefix(*sp), s.text.trim()))
            .collect::<Vec<_>>()
            .join("\n"),

        OutputFormat::Srt => resolved
            .iter()
            .enumerate()
            .map(|(i, (s, sp))| {
                format!(
                    "{}\n{} --> {}\n{}{}\n",
                    i + 1,
                    fmt_srt(s.start),
                    fmt_srt(s.end),
                    prefix(*sp),
                    s.text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),

        OutputFormat::Vtt => {
            let mut out = String::from("WEBVTT\n\n");
            for (s, sp) in &resolved {
                let body = match sp {
                    Some(id) => format!("<v {}>{}", speaker_label(*id), s.text.trim()),
                    None => s.text.trim().to_string(),
                };
                out.push_str(&format!(
                    "{} --> {}\n{}\n\n",
                    fmt_vtt(s.start),
                    fmt_vtt(s.end),
                    body
                ));
            }
            out.trim_end().to_string()
        }

        OutputFormat::Json => {
            let docs = JsonDoc {
                segments: resolved
                    .iter()
                    .map(|(s, sp)| JsonSegment {
                        start: s.start,
                        end: s.end,
                        text: s.text.trim().to_string(),
                        speaker: sp.map(speaker_label),
                    })
                    .collect(),
            };
            serde_json::to_string_pretty(&docs).unwrap_or_else(|_| "{\"segments\":[]}".to_string())
        }

        OutputFormat::Karaoke => render_karaoke(words.unwrap_or(&[]), speakers),

        OutputFormat::Speaker => render_speaker_blocks(words, segments, speakers),
    }
}

/// Majority speaker among `words` by per-word max-overlap vote; `None` if none of
/// the words overlap any turn.
fn majority_speaker(words: &[TimedSegment], turns: &[SpeakerTurn]) -> Option<SpeakerId> {
    use std::collections::HashMap;
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in words {
        if let Some(sp) = speaker_at(w.start, w.end, turns) {
            *counts.entry(sp.0).or_insert(0) += 1;
        }
    }
    // Max count; ties broken by lowest speaker id for determinism.
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(id, _)| SpeakerId(id))
}

/// Speaker-grouped transcript: `[Speaker N] (M:SS - M:SS)\n<turn text>` blocks,
/// blank line between turns. Attribution is **sentence-level** — words are
/// grouped into sentences (ending on `. ! ? …`) and each sentence takes the
/// majority speaker of its words, so turns align to sentences instead of
/// fragmenting on per-word diarization jitter. Falls back to segment-level when
/// no word timings are available. Gaps with no diarization coverage inherit the
/// neighbouring speaker (forward- then backward-fill) to avoid stray `Speaker ?`.
pub fn render_speaker_blocks(
    words: Option<&[TimedSegment]>,
    segments: &[TimedSegment],
    turns: Option<&[SpeakerTurn]>,
) -> String {
    struct Unit {
        start: f32,
        end: f32,
        text: String,
        sp: Option<SpeakerId>,
    }

    let mut units: Vec<Unit> = match (words, turns) {
        (Some(w), Some(t)) if !w.is_empty() => {
            let mut out: Vec<Unit> = Vec::new();
            let mut i = 0;
            while i < w.len() {
                let mut j = i;
                loop {
                    let ends = w[j].text.trim_end().ends_with(['.', '!', '?', '…']);
                    j += 1;
                    if ends || j >= w.len() {
                        break;
                    }
                }
                out.push(Unit {
                    start: w[i].start,
                    end: w[j - 1].end,
                    text: w[i..j]
                        .iter()
                        .map(|x| x.text.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                    sp: majority_speaker(&w[i..j], t),
                });
                i = j;
            }
            out
        }
        // No word timings: treat each segment as a unit tagged by max overlap.
        _ => segments
            .iter()
            .map(|s| Unit {
                start: s.start,
                end: s.end,
                text: s.text.trim().to_string(),
                sp: turns.and_then(|t| speaker_at(s.start, s.end, t)),
            })
            .collect(),
    };

    // Fill `None` units from neighbours (forward then backward).
    let mut last: Option<SpeakerId> = None;
    for u in units.iter_mut() {
        match u.sp {
            Some(sp) => last = Some(sp),
            None => u.sp = last,
        }
    }
    let mut next: Option<SpeakerId> = None;
    for u in units.iter_mut().rev() {
        match u.sp {
            Some(sp) => next = Some(sp),
            None => u.sp = next,
        }
    }

    // Merge consecutive same-speaker units into blocks.
    let mut out = String::new();
    let mut i = 0;
    while i < units.len() {
        let sp = units[i].sp;
        let mut j = i + 1;
        while j < units.len() && units[j].sp == sp {
            j += 1;
        }
        let label = sp
            .map(speaker_label)
            .unwrap_or_else(|| "Speaker ?".to_string());
        let text = units[i..j]
            .iter()
            .map(|u| u.text.as_str())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "[{}] ({} - {})\n{}\n\n",
            label,
            fmt_clock(units[i].start),
            fmt_clock(units[j - 1].end),
            text
        ));
        i = j;
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_extension_md_is_plain() {
        assert_eq!(
            OutputFormat::from_extension("md"),
            Some(OutputFormat::Plain)
        );
        assert_eq!(
            OutputFormat::from_extension("markdown"),
            Some(OutputFormat::Plain)
        );
        assert_eq!(
            OutputFormat::from_extension("txt"),
            Some(OutputFormat::Plain)
        );
    }

    fn seg(start: f32, end: f32, text: &str) -> TimedSegment {
        TimedSegment {
            start,
            end,
            text: text.to_string(),
        }
    }

    #[test]
    fn split_separates_two_speakers_within_one_segment() {
        // One 0..10s segment, 4 words; turns: spk0 0..5, spk1 5..10.
        let segs = vec![seg(0.0, 10.0, "aaa bbb ccc ddd")];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 5.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 5.0,
                end: 10.0,
                speaker: SpeakerId(1),
            },
        ];
        let out = split_segments_by_speakers(&segs, &turns);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, Some(SpeakerId(0)));
        assert_eq!(out[0].0.text, "aaa bbb");
        assert_eq!(out[1].1, Some(SpeakerId(1)));
        assert_eq!(out[1].0.text, "ccc ddd");
    }

    #[test]
    fn split_single_speaker_segment_stays_whole() {
        let segs = vec![seg(0.0, 4.0, "one two three")];
        let turns = vec![SpeakerTurn {
            start: 0.0,
            end: 4.0,
            speaker: SpeakerId(0),
        }];
        let out = split_segments_by_speakers(&segs, &turns);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, Some(SpeakerId(0)));
        assert_eq!(out[0].0.text, "one two three");
    }

    #[test]
    fn split_no_turns_yields_none() {
        let segs = vec![seg(0.0, 4.0, "hello world")];
        let out = split_segments_by_speakers(&segs, &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, None);
        assert_eq!(out[0].0.text, "hello world");
    }

    #[test]
    fn timecodes_format_correctly() {
        assert_eq!(fmt_srt(0.0), "00:00:00,000");
        assert_eq!(fmt_srt(12.5), "00:00:12,500");
        assert_eq!(fmt_srt(3661.0), "01:01:01,000");
        assert_eq!(fmt_vtt(12.5), "00:00:12.500");
        assert_eq!(fmt_inline(75.4), "01:15");
        assert_eq!(fmt_inline(0.0), "00:00");
    }

    #[test]
    fn plain_returns_joined_text_verbatim() {
        let segs = vec![seg(0.0, 1.0, "hello"), seg(1.0, 2.0, "world")];
        assert_eq!(
            render("hello world", &segs, None, None, OutputFormat::Plain),
            "hello world"
        );
    }

    #[test]
    fn inline_prefixes_timestamps() {
        let segs = vec![seg(0.0, 2.0, "привет"), seg(12.0, 14.0, "world")];
        let out = render("привет world", &segs, None, None, OutputFormat::Inline);
        assert_eq!(out, "[00:00] привет\n[00:12] world");
    }

    #[test]
    fn srt_numbers_and_arrows() {
        let segs = vec![seg(0.0, 2.5, "hi")];
        let out = render("hi", &segs, None, None, OutputFormat::Srt);
        assert_eq!(out, "1\n00:00:00,000 --> 00:00:02,500\nhi\n");
    }

    #[test]
    fn vtt_has_header_and_dot_separator() {
        let segs = vec![seg(1.0, 2.0, "hi")];
        let out = render("hi", &segs, None, None, OutputFormat::Vtt);
        assert!(out.starts_with("WEBVTT\n\n"));
        assert!(out.contains("00:00:01.000 --> 00:00:02.000"));
    }

    #[test]
    fn json_includes_segments() {
        let segs = vec![seg(0.0, 1.0, "hi")];
        let out = render("hi", &segs, None, None, OutputFormat::Json);
        assert!(out.contains("\"segments\""));
        assert!(out.contains("\"text\": \"hi\""));
        assert!(!out.contains("\"speaker\"")); // omitted when no speakers
    }

    #[test]
    fn assign_speakers_by_max_overlap() {
        let segs = vec![seg(0.0, 2.0, "a"), seg(5.0, 7.0, "b"), seg(20.0, 21.0, "c")];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 3.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 4.0,
                end: 8.0,
                speaker: SpeakerId(1),
            },
        ];
        let got = assign_speakers(&segs, &turns);
        assert_eq!(got, vec![Some(SpeakerId(0)), Some(SpeakerId(1)), None]);
    }

    #[test]
    fn straddling_segment_takes_larger_overlap() {
        // segment 2.0..4.0 overlaps spk0 by 1.0 (2..3) and spk1 by 1.0... tie ->
        // make spk1 larger: spk1 covers 2.5..4.0 => overlap 1.5 > 1.0.
        let segs = vec![seg(2.0, 4.0, "x")];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 3.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 2.5,
                end: 6.0,
                speaker: SpeakerId(1),
            },
        ];
        assert_eq!(assign_speakers(&segs, &turns), vec![Some(SpeakerId(1))]);
    }

    #[test]
    fn speaker_labels_render_in_each_format() {
        let segs = vec![seg(0.0, 2.0, "hi"), seg(2.0, 4.0, "yo")];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 2.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 2.0,
                end: 4.0,
                speaker: SpeakerId(1),
            },
        ];
        let inline = render("hi yo", &segs, None, Some(&turns), OutputFormat::Inline);
        assert_eq!(inline, "[00:00] Speaker 1: hi\n[00:02] Speaker 2: yo");
        let vtt = render("hi yo", &segs, None, Some(&turns), OutputFormat::Vtt);
        assert!(vtt.contains("<v Speaker 1>hi"));
        let srt = render("hi yo", &segs, None, Some(&turns), OutputFormat::Srt);
        assert!(srt.contains("Speaker 2: yo"));
    }

    #[test]
    fn words_split_at_real_speaker_boundary() {
        let words = vec![
            seg(0.0, 1.0, "aaa"),
            seg(1.0, 2.0, "bbb"),
            seg(6.0, 7.0, "ccc"),
        ];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 5.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 5.0,
                end: 8.0,
                speaker: SpeakerId(1),
            },
        ];
        let out = assign_words_to_speakers(&words, &turns);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, Some(SpeakerId(0)));
        assert_eq!(out[0].0.text, "aaa bbb");
        assert_eq!(out[1].1, Some(SpeakerId(1)));
        assert_eq!(out[1].0.text, "ccc");
    }

    #[test]
    fn words_no_turns_one_group_none() {
        let words = vec![seg(0.0, 1.0, "a"), seg(1.0, 2.0, "b")];
        let out = assign_words_to_speakers(&words, &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, None);
        assert_eq!(out[0].0.text, "a b");
    }

    #[test]
    fn word_json_is_deepgram_shaped() {
        let words = vec![seg(15.2, 15.5, "hello"), seg(15.6, 16.1, "there")];
        let turns = vec![SpeakerTurn {
            start: 0.0,
            end: 20.0,
            speaker: SpeakerId(0),
        }];
        let json = render_word_json(&words, Some(&turns));
        // 0-based speaker, deepgram field names, words present
        assert!(json.contains("\"word\": \"hello\""));
        assert!(json.contains("\"speaker\": 0"));
        assert!(json.contains("\"speaker_confidence\""));
        // fully inside the single turn -> confidence 1.0
        assert!(json.contains("\"speaker_confidence\": 1.0"));
    }

    #[test]
    fn overlap_confidence_is_fractional_on_boundary() {
        // word 4..6 with turn 0..5 -> half inside
        let c = speaker_overlap_confidence(
            4.0,
            6.0,
            SpeakerId(0),
            &[SpeakerTurn {
                start: 0.0,
                end: 5.0,
                speaker: SpeakerId(0),
            }],
        );
        assert!((c - 0.5).abs() < 1e-6);
    }

    #[test]
    fn karaoke_vtt_has_word_tags() {
        let words = vec![seg(15.259, 15.6, "hello"), seg(15.6, 16.1, "there")];
        let turns = vec![SpeakerTurn {
            start: 0.0,
            end: 20.0,
            speaker: SpeakerId(0),
        }];
        let out = render_karaoke(&words, Some(&turns));
        assert!(out.starts_with("WEBVTT"));
        assert!(out.contains("<v Speaker 1>")); // 1-based human label
        assert!(out.contains("<00:15.259>hello")); // per-word timing tag
        assert!(out.contains("<00:15.600>there"));
    }

    #[test]
    fn karaoke_no_diarize_groups_lines_by_sentence_punctuation() {
        let words = vec![
            seg(0.0, 0.5, "Hello"),
            seg(0.5, 1.0, "world."),
            seg(1.0, 1.5, "Bye"),
        ];
        let out = render_karaoke(&words, None);
        // Two cues: "Hello world." then "Bye" — not three one-word cues.
        assert_eq!(out.matches("-->").count(), 2);
        assert!(out.contains("<00:00.000>Hello <00:00.500>world."));
        assert!(out.contains("<00:01.000>Bye"));
        assert!(!out.contains("<v ")); // no speaker tag without diarization
    }

    #[test]
    fn is_word_level_flags() {
        assert!(OutputFormat::Karaoke.is_word_level());
        assert!(!OutputFormat::Srt.is_word_level());
    }

    #[test]
    fn fmt_clock_has_no_leading_zero_minute() {
        assert_eq!(fmt_clock(1.0), "0:01");
        assert_eq!(fmt_clock(16.0), "0:16");
        assert_eq!(fmt_clock(134.0), "2:14");
        assert_eq!(fmt_clock(4530.0), "75:30");
    }

    #[test]
    fn speaker_format_groups_turns_with_clock_headers() {
        let segs = vec![seg(1.0, 16.0, "Hi there."), seg(17.0, 56.0, "Vastu is.")];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 16.5,
                speaker: SpeakerId(1),
            },
            SpeakerTurn {
                start: 16.5,
                end: 60.0,
                speaker: SpeakerId(0),
            },
        ];
        let out = render("", &segs, None, Some(&turns), OutputFormat::Speaker);
        assert!(out.contains("[Speaker 2] (0:01 - 0:16)\nHi there."));
        assert!(out.contains("[Speaker 1] (0:17 - 0:56)\nVastu is."));
        // one blank line between turn blocks, no trailing blank
        assert!(out.contains("Hi there.\n\n[Speaker 1]"));
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn speaker_format_parses_from_cli() {
        assert_eq!(
            OutputFormat::from_cli("speaker"),
            Some(OutputFormat::Speaker)
        );
        assert_eq!(
            OutputFormat::from_cli("diarized"),
            Some(OutputFormat::Speaker)
        );
    }

    #[test]
    fn speaker_format_is_sentence_level_not_word_jittery() {
        // Word-level path: a single stray word overlaps the other speaker's turn,
        // but sentence-majority keeps each sentence whole — no mid-phrase split.
        let words = vec![
            seg(0.0, 1.0, "Hi"),
            seg(1.0, 2.0, "there"),
            seg(2.0, 3.0, "friend."),
            seg(3.0, 4.0, "How"),
            seg(4.0, 5.0, "are"),
            seg(5.0, 6.0, "you?"),
        ];
        let turns = vec![
            SpeakerTurn {
                start: 0.0,
                end: 4.0,
                speaker: SpeakerId(0),
            },
            SpeakerTurn {
                start: 4.0,
                end: 5.0,
                speaker: SpeakerId(1),
            },
            SpeakerTurn {
                start: 5.0,
                end: 6.0,
                speaker: SpeakerId(0),
            },
        ];
        let out = render("", &[], Some(&words), Some(&turns), OutputFormat::Speaker);
        assert!(out.contains("[Speaker 1] (0:00 - 0:06)"));
        assert!(out.contains("Hi there friend. How are you?"));
        // The 4..5 blip does not fragment the turn into extra blocks.
        assert_eq!(out.matches("[Speaker").count(), 1);
    }
}
