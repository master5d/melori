//! Voice-driven text transforms inspired by Wispr Flow, kept local and
//! deterministic where possible.
//!
//! - Command Mode: a spoken instruction at the start of an utterance
//!   (`detect_prefix_command`) routes the remainder through the LLM, and a
//!   trailing "press enter" (`strip_submit_command`) auto-submits.
//! - Snippets: spoken trigger phrases expand to canned text.
//! - Self-correction: collapses "<A>, no, <B>" restatements to just "<B>".
//! - Spoken lists: turns dictated enumerations into formatted lists.
//! - Dev dictionary: a built-in glossary of developer jargon.

use once_cell::sync::Lazy;
use regex::Regex;

use echo_config::Snippet;

// ── #5 Dev jargon dictionary ────────────────────────────────────────────────

// Moved into echo-config (it is consumed by `AppSettings::effective_custom_words`,
// which lives with the struct there); re-exported so existing
// `voice_commands::DEV_DICTIONARY` paths keep resolving.
pub use echo_config::DEV_DICTIONARY;

// ── #1 Command Mode ─────────────────────────────────────────────────────────

/// A spoken instruction that transforms the dictated payload via the LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceCommand {
    /// Translate the payload into the given language (English name).
    Translate { target: String },
    /// Make the payload more concise.
    Shorten,
    /// Rewrite the payload in a formal, professional tone.
    Formal,
}

impl VoiceCommand {
    /// The LLM instruction this command expands to. The payload text is sent as
    /// the user message; the result replaces the dictated text.
    pub fn instruction(&self) -> String {
        match self {
            VoiceCommand::Translate { target } => format!(
                "Translate the following text into {target}. \
                 Preserve meaning and tone. Output only the translation, with no preamble or quotes."
            ),
            VoiceCommand::Shorten => "Rewrite the following text to be more concise while \
                 preserving its meaning. Output only the rewritten text, with no preamble."
                .to_string(),
            VoiceCommand::Formal => "Rewrite the following text in a formal, professional tone. \
                 Output only the rewritten text, with no preamble."
                .to_string(),
        }
    }
}

/// A detected prefix command plus the remaining dictated payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedCommand {
    pub command: VoiceCommand,
    pub remainder: String,
}

// (prefix-regex, command) pairs. Each regex is anchored at the start, is
// case-insensitive, tolerates a trailing comma/colon, and captures the payload
// in group 1. Ordered so longer/more-specific phrasings win.
static PREFIX_COMMANDS: Lazy<Vec<(Regex, VoiceCommand)>> = Lazy::new(|| {
    let mk = |p: &str| Regex::new(p).unwrap();
    vec![
        (
            mk(
                r"(?i)^\s*(?:translate (?:this )?(?:in)?to english|переведи(?: это)? на английский)[,:]?\s+(.+)$",
            ),
            VoiceCommand::Translate {
                target: "English".to_string(),
            },
        ),
        (
            mk(
                r"(?i)^\s*(?:translate (?:this )?(?:in)?to russian|переведи(?: это)? на русский)[,:]?\s+(.+)$",
            ),
            VoiceCommand::Translate {
                target: "Russian".to_string(),
            },
        ),
        (
            mk(
                r"(?i)^\s*(?:make (?:it |this )?shorter|make (?:it |this )?more concise|сделай(?: это)? короче|сократи)[,:]?\s+(.+)$",
            ),
            VoiceCommand::Shorten,
        ),
        (
            mk(
                r"(?i)^\s*(?:make (?:it |this )?formal|reply formally|ответь формально|сделай(?: это)? формальн(?:ым|о))[,:]?\s+(.+)$",
            ),
            VoiceCommand::Formal,
        ),
    ]
});

/// If `text` begins with a recognized voice command, return the command and the
/// payload that follows it. Otherwise `None`.
pub fn detect_prefix_command(text: &str) -> Option<DetectedCommand> {
    for (re, command) in PREFIX_COMMANDS.iter() {
        if let Some(caps) = re.captures(text) {
            let remainder = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if !remainder.is_empty() {
                return Some(DetectedCommand {
                    command: command.clone(),
                    remainder: remainder.to_string(),
                });
            }
        }
    }
    None
}

/// Split the comma-separated trigger setting into trimmed, non-empty phrases.
pub fn parse_capture_phrases(setting: &str) -> Vec<String> {
    setting
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// If `text` starts (case-insensitively) with one of `phrases` followed by a
/// whitespace boundary, return the trimmed remainder (note body). Empty
/// remainder -> None. First matching phrase wins.
pub fn detect_capture(text: &str, phrases: &[&str]) -> Option<String> {
    let trimmed = text.trim_start();
    let lower = trimmed.to_lowercase();
    for phrase in phrases {
        let p = phrase.trim().to_lowercase();
        if p.is_empty() {
            continue;
        }
        if lower.starts_with(&p) {
            let n = p.chars().count();
            match trimmed.chars().nth(n) {
                Some(c) if c.is_whitespace() => {}
                _ => continue, // mid-word or no boundary
            }
            let body: String = trimmed.chars().skip(n).collect();
            let body = body.trim().to_string();
            if !body.is_empty() {
                return Some(body);
            }
        }
    }
    None
}

static SUBMIT_SUFFIX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)[\s,.]*(?:press enter|hit enter|нажми(?:\s+на)?\s+(?:enter|ввод|энтер))[\s.!]*$",
    )
    .unwrap()
});

/// Strip a trailing "press enter" voice command. Returns the cleaned text and
/// whether the submit command was present.
pub fn strip_submit_command(text: &str) -> (String, bool) {
    if SUBMIT_SUFFIX.is_match(text) {
        let cleaned = SUBMIT_SUFFIX.replace(text, "").trim_end().to_string();
        (cleaned, true)
    } else {
        (text.to_string(), false)
    }
}

// ── #2 Snippets ──────────────────────────────────────────────────────────────

/// Expand spoken snippet triggers into their canned text. Matching is
/// case-insensitive and bounded by non-word characters so a trigger only fires
/// as a whole phrase, not as a substring of another word.
pub fn expand_snippets(text: &str, snippets: &[Snippet]) -> String {
    let mut out = text.to_string();
    for snippet in snippets {
        let trigger = snippet.trigger.trim();
        if trigger.is_empty() {
            continue;
        }
        // Build a case-insensitive, whole-phrase pattern for this trigger.
        let pattern = format!(r"(?i)(?:^|\b){}(?:\b|$)", regex::escape(trigger));
        if let Ok(re) = Regex::new(&pattern) {
            out = re.replace_all(&out, snippet.text.as_str()).to_string();
        }
    }
    out
}

// ── #3 Self-correction ───────────────────────────────────────────────────────

static CORRECTION_MARKER: Lazy<Regex> = Lazy::new(|| {
    // A comma-introduced correction cue. Requiring a leading comma (produced by
    // auto-punctuation, which runs first) keeps legitimate uses of "no"/"нет"
    // from being treated as corrections.
    Regex::new(
        r"(?i),\s*(?:no|nope|i mean|i meant|rather|actually|scratch that|нет|не|точнее|вернее|то есть)\s*,?\s*",
    )
    .unwrap()
});

/// Collapse a spoken self-correction ("meet at 2, no, at 3" → "meet at 3").
///
/// The text after the correction cue is kept verbatim. From the text before the
/// cue we drop the *restated tail*: starting at the last occurrence of the word
/// the correction re-opens with (the pivot), so "meet at 2" with a correction
/// re-opening on "at" drops "at 2" and keeps "meet". If no pivot is shared, only
/// the single word immediately before the cue is dropped.
pub fn apply_self_correction(text: &str) -> String {
    let mut current = text.to_string();
    // Resolve corrections left-to-right, re-scanning after each fix so chained
    // corrections ("2, no 3, no 4") collapse fully.
    loop {
        let Some(m) = CORRECTION_MARKER.find(&current) else {
            break;
        };
        let before = current[..m.start()].to_string();
        let after = current[m.end()..].to_string();

        let pivot = after.split_whitespace().next().unwrap_or("");
        let kept_before = drop_restated_tail(&before, pivot);

        let joined = match (kept_before.trim_end(), after.trim_start()) {
            ("", a) => a.to_string(),
            (b, "") => b.to_string(),
            (b, a) => format!("{b} {a}"),
        };
        if joined == current {
            break; // no progress; avoid looping forever
        }
        current = joined;
    }
    current
}

/// Drop the restated tail of `before` starting at the last whole-word match of
/// `pivot` (case-insensitive). Falls back to dropping the final word.
fn drop_restated_tail(before: &str, pivot: &str) -> String {
    let trimmed = before.trim_end();
    if !pivot.is_empty() {
        let pivot_lower = pivot.to_lowercase();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if let Some(idx) = words.iter().rposition(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
                == pivot_lower
        }) {
            return words[..idx].join(" ");
        }
    }
    // No shared pivot: drop the last word.
    match trimmed.rsplit_once(char::is_whitespace) {
        Some((head, _)) => head.to_string(),
        None => String::new(),
    }
}

// ── #4 Spoken lists ──────────────────────────────────────────────────────────

static SPOKEN_LIST_ITEM: Lazy<Regex> = Lazy::new(|| {
    // Matches an ordinal cue that introduces a list item, e.g. "number one",
    // "first", "1.", "первое", "пункт один". Captured so we can split on them.
    Regex::new(
        r"(?i)(?:^|[\s,;.])(?:number\s+(?:one|two|three|four|five|six|seven|eight|nine|ten)|(?:first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth)(?:ly)?|(?:перв|втор|трет|четверт|пят|шест|седьм|восьм|девят|десят)(?:ое|ый|ья|е)|пункт\s+\w+)[\s,:.-]+",
    )
    .unwrap()
});

/// Turn a dictated enumeration into a numbered Markdown list. Conservative: only
/// fires when at least two ordinal cues are present, otherwise returns the text
/// unchanged.
pub fn format_spoken_list(text: &str) -> String {
    let cue_count = SPOKEN_LIST_ITEM.find_iter(text).count();
    if cue_count < 2 {
        return text.to_string();
    }

    // Split on each cue; the text before the first cue (if any) is a lead-in.
    let mut items: Vec<String> = Vec::new();
    let mut last_end = 0usize;
    let mut lead_in = String::new();
    let mut first = true;
    for m in SPOKEN_LIST_ITEM.find_iter(text) {
        let segment = text[last_end..m.start()].trim();
        if first {
            lead_in = segment.to_string();
            first = false;
        } else if !segment.is_empty() {
            items.push(segment.to_string());
        }
        last_end = m.end();
    }
    let tail = text[last_end..].trim();
    if !tail.is_empty() {
        items.push(tail.to_string());
    }
    if items.len() < 2 {
        return text.to_string();
    }

    let mut out = String::new();
    if !lead_in.is_empty() {
        out.push_str(&lead_in);
        out.push('\n');
    }
    for (i, item) in items.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, item));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(trigger: &str, text: &str) -> Snippet {
        Snippet {
            trigger: trigger.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn detects_translate_prefix() {
        let d = detect_prefix_command("translate to english привет мир").unwrap();
        assert_eq!(
            d.command,
            VoiceCommand::Translate {
                target: "English".to_string()
            }
        );
        assert_eq!(d.remainder, "привет мир");
    }

    #[test]
    fn detects_russian_shorten_prefix() {
        let d = detect_prefix_command("сделай короче: это очень длинный текст").unwrap();
        assert_eq!(d.command, VoiceCommand::Shorten);
        assert_eq!(d.remainder, "это очень длинный текст");
    }

    #[test]
    fn ignores_non_command_text() {
        assert!(detect_prefix_command("just some normal dictation").is_none());
        // Command word with no payload is not a command.
        assert!(detect_prefix_command("make it shorter").is_none());
    }

    #[test]
    fn strips_trailing_submit() {
        // The comma before "press enter" is an auto-punctuation artifact of the
        // spoken pause, so it is stripped along with the command.
        let (t, submit) = strip_submit_command("send the report, press enter");
        assert!(submit);
        assert_eq!(t, "send the report");
        let (t2, submit2) = strip_submit_command("нажми ввод");
        assert!(submit2);
        assert_eq!(t2, "");
        let (t3, submit3) = strip_submit_command("no command here");
        assert!(!submit3);
        assert_eq!(t3, "no command here");
    }

    #[test]
    fn expands_snippets_whole_phrase() {
        let snippets = vec![snippet("calendar link", "https://cal.com/me")];
        assert_eq!(
            expand_snippets("here is my calendar link for booking", &snippets),
            "here is my https://cal.com/me for booking"
        );
        // Does not fire mid-word.
        let s2 = vec![snippet("cal", "CALENDAR")];
        assert_eq!(expand_snippets("a calculation", &s2), "a calculation");
    }

    #[test]
    fn self_correction_keeps_pivot_lead_in() {
        assert_eq!(
            apply_self_correction("meet at 2, no, at 3 today"),
            "meet at 3 today"
        );
    }

    #[test]
    fn self_correction_russian() {
        assert_eq!(
            apply_self_correction("встречаемся в 2, нет, в 3 часа"),
            "встречаемся в 3 часа"
        );
    }

    #[test]
    fn self_correction_noop_without_marker() {
        assert_eq!(
            apply_self_correction("no corrections here"),
            "no corrections here"
        );
    }

    #[test]
    fn formats_spoken_numbered_list() {
        let out = format_spoken_list("groceries first apples second bananas third oranges");
        assert_eq!(out, "groceries\n1. apples\n2. bananas\n3. oranges");
    }

    #[test]
    fn spoken_list_requires_two_cues() {
        let single = "first of all this is fine";
        assert_eq!(format_spoken_list(single), single);
    }

    #[test]
    fn detect_capture_single_phrase() {
        assert_eq!(
            detect_capture("capture note buy milk", &["capture note"]).as_deref(),
            Some("buy milk")
        );
    }
    #[test]
    fn detect_capture_multi_phrase_and_case_insensitive() {
        assert_eq!(
            detect_capture("Save To Vault Hello", &["save note", "save to vault"]).as_deref(),
            Some("Hello")
        );
    }
    #[test]
    fn detect_capture_negatives() {
        assert!(detect_capture("just talking", &["capture note"]).is_none());
        assert!(detect_capture("capture note", &["capture note"]).is_none()); // empty body
        assert!(detect_capture("capture notebook stuff", &["capture note"]).is_none()); // prefix mid-word
        assert!(detect_capture("anything", &[]).is_none()); // no phrases
    }
    #[test]
    fn parse_phrases_trims_and_drops_empty() {
        assert_eq!(
            parse_capture_phrases("a, b ,, c "),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
