use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

static WORD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\wа-яА-ЯёЁ]+").unwrap());

pub struct Heuristics {
    question_words: HashSet<String>,
    comma_before_words: HashSet<String>,
}

impl Heuristics {
    // Best-effort, single-token punctuation heuristics. These are matched
    // against individual whitespace-split tokens, so multi-word conjunctions
    // (e.g. "так как", "прежде чем") cannot match here and are intentionally
    // omitted — they would be silent dead entries otherwise. This is a coarse
    // assist gated behind the auto_punctuate/auto_capitalize settings, not a
    // grammar engine.
    pub fn new() -> Self {
        let question_words: HashSet<String> = [
            "who",
            "what",
            "where",
            "when",
            "why",
            "how",
            "is",
            "are",
            "was",
            "were",
            "do",
            "does",
            "did",
            "can",
            "could",
            "will",
            "would",
            "should",
            "shall",
            "have",
            "has",
            "had",
            "may",
            "might",
            "кто",
            "что",
            "где",
            "когда",
            "почему",
            "как",
            "какой",
            "какая",
            "какие",
            "сколько",
            "зачем",
            "откуда",
            "чей",
            "чья",
            "чьё",
            "чьи",
            "куда",
            "доколе",
            "неужели",
            "разве",
            "ли",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let comma_before_words: HashSet<String> = [
            "but",
            "however",
            "although",
            "though",
            "yet",
            "so",
            "which",
            "because",
            "while",
            "whereas",
            "но",
            "а",
            "однако",
            "хотя",
            "потому",
            "поэтому",
            "который",
            "которая",
            "которые",
            "которое",
            "что",
            "чтобы",
            "если",
            "ибо",
            "словно",
            "будто",
            "нежели",
            "пока",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            question_words,
            comma_before_words,
        }
    }

    pub fn to_camel_case(&self, text: &str) -> String {
        let mut result = String::new();
        let mut first_word = true;
        let mut current_word = String::new();

        for c in text.chars() {
            if c.is_alphanumeric() {
                current_word.push(c);
            } else if !current_word.is_empty() {
                self.append_word(&mut result, &current_word, first_word);
                first_word = false;
                current_word.clear();
            }
        }

        if !current_word.is_empty() {
            self.append_word(&mut result, &current_word, first_word);
        }

        result
    }

    fn append_word(&self, result: &mut String, word: &str, is_first: bool) {
        if is_first {
            result.push_str(&word.to_lowercase());
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                result.push_str(&first.to_uppercase().to_string());
                result.push_str(&chars.as_str().to_lowercase());
            }
        }
    }

    pub fn auto_punctuate(&self, text: &str) -> String {
        if text.trim().is_empty() {
            return text.to_string();
        }

        let trimmed_text = text.trim();
        let last_char = trimmed_text.chars().last().unwrap();
        if last_char == '.' || last_char == '!' || last_char == '?' || last_char == '…' {
            return text.to_string();
        }

        let mut words: Vec<String> = trimmed_text
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        for i in 1..words.len() {
            if let Some(mat) = WORD_REGEX.find(&words[i]) {
                let clean_word = mat.as_str().to_lowercase();
                if self.comma_before_words.contains(&clean_word) {
                    let prev_index = i - 1;
                    let prev_word = &words[prev_index];
                    if !prev_word
                        .chars()
                        .last()
                        .map_or(false, |c| c.is_ascii_punctuation() || c == ',')
                    {
                        words[prev_index] = format!("{},", prev_word);
                    }
                }
            }
        }

        let mut result = words.join(" ");

        if let Some(mat) = WORD_REGEX.find(&words[0]) {
            let first_word_clean = mat.as_str().to_lowercase();
            if self.question_words.contains(&first_word_clean) {
                result.push('?');
            } else {
                result.push('.');
            }
        } else {
            result.push('.');
        }

        result
    }

    pub fn auto_capitalize(&self, text: &str) -> String {
        if text.trim().is_empty() {
            return text.to_string();
        }

        let mut result = String::with_capacity(text.len());
        let mut capitalize_next = true;

        for ch in text.chars() {
            if capitalize_next && ch.is_alphabetic() {
                result.extend(ch.to_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch);
                if ch == '.' || ch == '!' || ch == '?' {
                    capitalize_next = true;
                }
            }
        }

        result
    }
}
