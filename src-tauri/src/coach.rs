use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct WordCount {
    pub word: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub enum PaceBand {
    Slow,
    Good,
    Fast,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct CoachMetrics {
    pub word_count: u32,
    pub duration_ms: u64,
    pub wpm: u32,
    pub pace_band: PaceBand,
    pub fillers: Vec<WordCount>,
    pub filler_total: u32,
    pub filler_rate: f32,
    pub weak_words: Vec<WordCount>,
}

// Categorized bilingual default lists (v0; user-editable lists are a fast-follow).
const EN_FILLERS: &[&str] = &[
    "um",
    "uh",
    "er",
    "ah",
    "like",
    "basically",
    "actually",
    "literally",
    "so",
    "well",
    "kinda",
    "sorta",
];
const RU_FILLERS: &[&str] = &["э", "ну", "типа", "короче", "вот", "значит", "собственно"];
const EN_FILLER_PHRASES: &[[&str; 2]] = &[["you", "know"], ["i", "mean"]];
const RU_FILLER_PHRASES: &[[&str; 2]] = &[
    ["как", "бы"],
    ["это", "самое"],
    ["в", "общем"],
    ["так", "сказать"],
];
const EN_HEDGES: &[&str] = &["just", "maybe", "perhaps", "probably", "hopefully"];
const RU_HEDGES: &[&str] = &["наверное", "просто", "вроде", "возможно", "кажется"];
const EN_HEDGE_PHRASES: &[[&str; 2]] = &[["sort", "of"], ["kind", "of"], ["i", "think"]];

pub fn analyze(text: &str, duration_ms: u64) -> CoachMetrics {
    let tokens: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let word_count = tokens.len() as u32;

    let mut filler_counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut weak_counts: BTreeMap<String, u32> = BTreeMap::new();

    for t in &tokens {
        let s = t.as_str();
        if EN_FILLERS.contains(&s) || RU_FILLERS.contains(&s) {
            *filler_counts.entry(t.clone()).or_insert(0) += 1;
        } else if EN_HEDGES.contains(&s) || RU_HEDGES.contains(&s) {
            *weak_counts.entry(t.clone()).or_insert(0) += 1;
        }
    }

    for w in tokens.windows(2) {
        let pair = [w[0].as_str(), w[1].as_str()];
        let key = format!("{} {}", w[0], w[1]);
        if EN_FILLER_PHRASES.contains(&pair) || RU_FILLER_PHRASES.contains(&pair) {
            *filler_counts.entry(key).or_insert(0) += 1;
        } else if EN_HEDGE_PHRASES.contains(&pair) {
            *weak_counts.entry(key).or_insert(0) += 1;
        }
    }

    let filler_total: u32 = filler_counts.values().sum();
    let wpm = if duration_ms == 0 {
        0
    } else {
        ((word_count as u64 * 60_000) / duration_ms) as u32
    };
    let pace_band = if wpm == 0 {
        PaceBand::Good
    } else if wpm < 110 {
        PaceBand::Slow
    } else if wpm <= 170 {
        PaceBand::Good
    } else {
        PaceBand::Fast
    };
    let filler_rate = if word_count == 0 {
        0.0
    } else {
        filler_total as f32 / word_count as f32 * 100.0
    };

    let to_sorted = |m: BTreeMap<String, u32>| -> Vec<WordCount> {
        let mut v: Vec<WordCount> = m
            .into_iter()
            .map(|(word, count)| WordCount { word, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then(a.word.cmp(&b.word)));
        v
    };

    CoachMetrics {
        word_count,
        duration_ms,
        wpm,
        pace_band,
        fillers: to_sorted(filler_counts),
        filler_total,
        weak_words: to_sorted(weak_counts),
        filler_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_of<'a>(v: &'a [WordCount], word: &str) -> u32 {
        v.iter()
            .find(|w| w.word == word)
            .map(|w| w.count)
            .unwrap_or(0)
    }

    #[test]
    fn counts_en_fillers() {
        let m = analyze("um so I uh think it is basically fine", 0);
        assert_eq!(count_of(&m.fillers, "um"), 1);
        assert_eq!(count_of(&m.fillers, "uh"), 1);
        assert_eq!(count_of(&m.fillers, "so"), 1);
        assert_eq!(count_of(&m.fillers, "basically"), 1);
        assert_eq!(m.filler_total, 4);
    }

    #[test]
    fn counts_ru_fillers_and_hyphenated_eh() {
        // «э-э» splits on the hyphen into two "э" filler tokens.
        let m = analyze("ну э-э это самое как бы вот", 0);
        assert_eq!(count_of(&m.fillers, "ну"), 1);
        assert_eq!(count_of(&m.fillers, "э"), 2);
        assert_eq!(count_of(&m.fillers, "это самое"), 1);
        assert_eq!(count_of(&m.fillers, "как бы"), 1);
        assert_eq!(count_of(&m.fillers, "вот"), 1);
    }

    #[test]
    fn counts_multiword_en_filler_phrase() {
        let m = analyze("you know it works i mean really", 0);
        assert_eq!(count_of(&m.fillers, "you know"), 1);
        assert_eq!(count_of(&m.fillers, "i mean"), 1);
    }

    #[test]
    fn separates_weak_words_from_fillers() {
        let m = analyze("i think it is just maybe fine", 0);
        assert_eq!(count_of(&m.weak_words, "i think"), 1);
        assert_eq!(count_of(&m.weak_words, "just"), 1);
        assert_eq!(count_of(&m.weak_words, "maybe"), 1);
        assert_eq!(m.filler_total, 0);
    }

    #[test]
    fn wpm_math_and_bands() {
        // 150 words in 60s -> 150 wpm -> Good
        let text = (0..150).map(|_| "word").collect::<Vec<_>>().join(" ");
        let m = analyze(&text, 60_000);
        assert_eq!(m.word_count, 150);
        assert_eq!(m.wpm, 150);
        assert_eq!(m.pace_band, PaceBand::Good);
    }

    #[test]
    fn pace_band_boundaries() {
        assert_eq!(analyze(&"w ".repeat(109), 60_000).pace_band, PaceBand::Slow); // 109
        assert_eq!(analyze(&"w ".repeat(110), 60_000).pace_band, PaceBand::Good); // 110
        assert_eq!(analyze(&"w ".repeat(170), 60_000).pace_band, PaceBand::Good); // 170
        assert_eq!(analyze(&"w ".repeat(171), 60_000).pace_band, PaceBand::Fast);
        // 171
    }

    #[test]
    fn zero_duration_yields_zero_wpm_neutral_band() {
        let m = analyze("hello world", 0);
        assert_eq!(m.wpm, 0);
        assert_eq!(m.pace_band, PaceBand::Good);
    }

    #[test]
    fn empty_text_is_all_zero() {
        let m = analyze("", 0);
        assert_eq!(m.word_count, 0);
        assert_eq!(m.filler_total, 0);
        assert_eq!(m.filler_rate, 0.0);
        assert!(m.fillers.is_empty());
    }
}
