use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;
use strsim::levenshtein;

static PUNCTUATION: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{L}\s]").unwrap());
static WHITESPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct WordScore {
    pub reference: String,
    pub spoken: Option<String>,
    pub matched: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ScoreReport {
    pub overall: u8,           // 0..=100
    pub words: Vec<WordScore>, // per reference word, aligned
    pub reference_word_count: usize,
    pub matched_word_count: usize,
    pub note: String, // short human feedback, e.g. "Great!" / "Watch: <words>"
}

fn normalize(text: &str) -> Vec<String> {
    let text = text.to_lowercase();
    let text = PUNCTUATION.replace_all(&text, "");
    let text = WHITESPACE.replace_all(&text, " ");
    text.trim()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub fn score_pronunciation(reference: &str, spoken: &str) -> ScoreReport {
    let ref_words = normalize(reference);
    let spoken_words = normalize(spoken);

    if ref_words.is_empty() {
        return ScoreReport {
            overall: 0,
            words: Vec::new(),
            reference_word_count: 0,
            matched_word_count: 0,
            note: "Empty reference phrase.".to_string(),
        };
    }

    let mut word_scores = Vec::with_capacity(ref_words.len());
    let mut matched_count = 0;
    let mut spoken_idx = 0;

    for ref_word in &ref_words {
        let mut best_match: Option<(usize, bool)> = None;

        // Greedy search for the best match in the remaining spoken words
        // We look ahead a bit to allow for some misrecognitions or skipped words
        let lookahead = 3;
        let end = (spoken_idx + lookahead).min(spoken_words.len());

        for i in spoken_idx..end {
            let spoken_word = &spoken_words[i];
            let is_match = if ref_word == spoken_word {
                true
            } else {
                let dist = levenshtein(ref_word, spoken_word);
                dist <= 1.max(ref_word.len() / 4)
            };

            if is_match {
                best_match = Some((i, true));
                break;
            }
        }

        if let Some((idx, is_match)) = best_match {
            word_scores.push(WordScore {
                reference: ref_word.clone(),
                spoken: Some(spoken_words[idx].clone()),
                matched: is_match,
            });
            matched_count += 1;
            spoken_idx = idx + 1;
        } else {
            word_scores.push(WordScore {
                reference: ref_word.clone(),
                spoken: None,
                matched: false,
            });
        }
    }

    let overall = (100 * matched_count / ref_words.len()) as u8;

    let unmatched_words: Vec<String> = word_scores
        .iter()
        .filter(|w| !w.matched)
        .map(|w| w.reference.clone())
        .take(3)
        .collect();

    let note = if overall >= 90 {
        "Great pronunciation!".to_string()
    } else if overall >= 70 {
        format!("Good — review: {}", unmatched_words.join(", "))
    } else {
        format!("Keep practicing: {}", unmatched_words.join(", "))
    };

    ScoreReport {
        overall,
        words: word_scores,
        reference_word_count: ref_words.len(),
        matched_word_count: matched_count,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let report = score_pronunciation("Hello world", "hello world");
        assert_eq!(report.overall, 100);
        assert_eq!(report.matched_word_count, 2);
    }

    #[test]
    fn test_one_wrong() {
        let report = score_pronunciation("one two three four", "one two skip four");
        assert_eq!(report.overall, 75);
        assert_eq!(report.matched_word_count, 3);
    }

    #[test]
    fn test_empty_reference() {
        let report = score_pronunciation("", "something");
        assert_eq!(report.overall, 0);
    }

    #[test]
    fn test_russian() {
        let report = score_pronunciation("привет как дела", "привет как дела");
        assert_eq!(report.overall, 100);
    }

    #[test]
    fn test_levenshtein_tolerance() {
        // "pronunciation" len 13, 13/4 = 3.
        // "pronunshation" dist 2
        let report = score_pronunciation("pronunciation", "pronunshation");
        assert_eq!(report.overall, 100);
        assert!(report.words[0].matched);
    }
}
