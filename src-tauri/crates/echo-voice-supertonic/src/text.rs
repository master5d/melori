use ndarray::Array3;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use echo_voice::VoiceError;

// ============================================================================
// Available Languages
// ============================================================================

pub const AVAILABLE_LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu",
    "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi", "na",
];

pub fn is_valid_lang(lang: &str) -> bool {
    AVAILABLE_LANGS.contains(&lang)
}

// ============================================================================
// Text Preprocessing
// ============================================================================

pub fn preprocess_text(text: &str, lang: &str) -> Result<String, VoiceError> {
    // NFKD normalization
    let mut text: String = text.nfkd().collect();

    // Remove emojis (wide Unicode range)
    let emoji_pattern = Regex::new(r"[\x{1F600}-\x{1F64F}\x{1F300}-\x{1F5FF}\x{1F680}-\x{1F6FF}\x{1F700}-\x{1F77F}\x{1F780}-\x{1F7FF}\x{1F800}-\x{1F8FF}\x{1F900}-\x{1F9FF}\x{1FA00}-\x{1FA6F}\x{1FA70}-\x{1FAFF}\x{2600}-\x{26FF}\x{2700}-\x{27BF}\x{1F1E6}-\x{1F1FF}]+").unwrap();
    text = emoji_pattern.replace_all(&text, "").to_string();

    // Replace various dashes and symbols
    let replacements = [
        ("–", "-"),         // en dash
        ("‑", "-"),         // non-breaking hyphen
        ("—", "-"),         // em dash
        ("_", " "),         // underscore
        ("\u{201C}", "\""), // left double quote
        ("\u{201D}", "\""), // right double quote
        ("\u{2018}", "'"),  // left single quote
        ("\u{2019}", "'"),  // right single quote
        ("´", "'"),         // acute accent
        ("`", "'"),         // grave accent
        ("[", " "),         // left bracket
        ("]", " "),         // right bracket
        ("|", " "),         // vertical bar
        ("/", " "),         // slash
        ("#", " "),         // hash
        ("→", " "),         // right arrow
        ("←", " "),         // left arrow
    ];

    for (from, to) in &replacements {
        text = text.replace(from, to);
    }

    // Remove special symbols
    let special_symbols = ["♥", "☆", "♡", "©", "\\"];
    for symbol in &special_symbols {
        text = text.replace(symbol, "");
    }

    // Replace known expressions
    let expr_replacements = [
        ("@", " at "),
        ("e.g.,", "for example, "),
        ("i.e.,", "that is, "),
    ];

    for (from, to) in &expr_replacements {
        text = text.replace(from, to);
    }

    // Fix spacing around punctuation
    text = Regex::new(r" ,")
        .unwrap()
        .replace_all(&text, ",")
        .to_string();
    text = Regex::new(r" \.")
        .unwrap()
        .replace_all(&text, ".")
        .to_string();
    text = Regex::new(r" !")
        .unwrap()
        .replace_all(&text, "!")
        .to_string();
    text = Regex::new(r" \?")
        .unwrap()
        .replace_all(&text, "?")
        .to_string();
    text = Regex::new(r" ;")
        .unwrap()
        .replace_all(&text, ";")
        .to_string();
    text = Regex::new(r" :")
        .unwrap()
        .replace_all(&text, ":")
        .to_string();
    text = Regex::new(r" '")
        .unwrap()
        .replace_all(&text, "'")
        .to_string();

    // Remove duplicate quotes
    while text.contains("\"\"") {
        text = text.replace("\"\"", "\"");
    }
    while text.contains("''") {
        text = text.replace("''", "'");
    }
    while text.contains("``") {
        text = text.replace("``", "`");
    }

    // Remove extra spaces
    text = Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();
    text = text.trim().to_string();

    // If text doesn't end with punctuation, quotes, or closing brackets, add a period
    if !text.is_empty() {
        let ends_with_punct =
            Regex::new(r#"[.!?;:,'"\u{201C}\u{201D}\u{2018}\u{2019})\]}…。」』】〉》›»]$"#)
                .unwrap();
        if !ends_with_punct.is_match(&text) {
            text.push('.');
        }
    }

    // Validate language
    if !is_valid_lang(lang) {
        return Err(VoiceError::Engine(format!(
            "Invalid language: {}. Available: {:?}",
            lang, AVAILABLE_LANGS
        )));
    }

    // Wrap text with language tags
    text = format!("<{}>{}</{}>", lang, text, lang);

    Ok(text)
}

// ============================================================================
// Unicode Value Conversion
// ============================================================================

pub fn text_to_unicode_values(text: &str) -> Vec<usize> {
    text.chars().map(|c| c as usize).collect()
}

// ============================================================================
// Text Masking
// ============================================================================

pub fn length_to_mask(lengths: &[usize], max_len: Option<usize>) -> Array3<f32> {
    let bsz = lengths.len();
    let max_len = max_len.unwrap_or_else(|| *lengths.iter().max().unwrap_or(&0));

    let mut mask = Array3::<f32>::zeros((bsz, 1, max_len));
    for (i, &len) in lengths.iter().enumerate() {
        for j in 0..len.min(max_len) {
            mask[[i, 0, j]] = 1.0;
        }
    }
    mask
}

pub fn get_text_mask(text_ids_lengths: &[usize]) -> Array3<f32> {
    let max_len = *text_ids_lengths.iter().max().unwrap_or(&0);
    length_to_mask(text_ids_lengths, Some(max_len))
}

// ============================================================================
// Text Chunking
// ============================================================================

const MAX_CHUNK_LENGTH: usize = 300;

const ABBREVIATIONS: &[&str] = &[
    "Dr.", "Mr.", "Mrs.", "Ms.", "Prof.", "Sr.", "Jr.", "St.", "Ave.", "Rd.", "Blvd.", "Dept.",
    "Inc.", "Ltd.", "Co.", "Corp.", "etc.", "vs.", "i.e.", "e.g.", "Ph.D.",
];

pub fn chunk_text(text: &str, max_len: Option<usize>) -> Vec<String> {
    let max_len = max_len.unwrap_or(MAX_CHUNK_LENGTH);
    let text = text.trim();

    if text.is_empty() {
        return vec![String::new()];
    }

    // Split by paragraphs
    let para_re = Regex::new(r"\n\s*\n").unwrap();
    let paragraphs: Vec<&str> = para_re.split(text).collect();
    let mut chunks = Vec::new();

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if para.len() <= max_len {
            chunks.push(para.to_string());
            continue;
        }

        // Split by sentences
        let sentences = split_sentences(para);
        let mut current = String::new();
        let mut current_len = 0;

        for sentence in sentences {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }

            let sentence_len = sentence.len();
            if sentence_len > max_len {
                // If sentence is longer than max_len, split by comma or space
                if !current.is_empty() {
                    chunks.push(current.trim().to_string());
                    current.clear();
                    current_len = 0;
                }

                // Try splitting by comma
                let parts: Vec<&str> = sentence.split(',').collect();
                for part in parts {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }

                    let part_len = part.len();
                    if part_len > max_len {
                        // Split by space as last resort
                        let words: Vec<&str> = part.split_whitespace().collect();
                        let mut word_chunk = String::new();
                        let mut word_chunk_len = 0;

                        for word in words {
                            let word_len = word.len();
                            if word_chunk_len + word_len + 1 > max_len && !word_chunk.is_empty() {
                                chunks.push(word_chunk.trim().to_string());
                                word_chunk.clear();
                                word_chunk_len = 0;
                            }

                            if !word_chunk.is_empty() {
                                word_chunk.push(' ');
                                word_chunk_len += 1;
                            }
                            word_chunk.push_str(word);
                            word_chunk_len += word_len;
                        }

                        if !word_chunk.is_empty() {
                            chunks.push(word_chunk.trim().to_string());
                        }
                    } else {
                        if current_len + part_len + 1 > max_len && !current.is_empty() {
                            chunks.push(current.trim().to_string());
                            current.clear();
                            current_len = 0;
                        }

                        if !current.is_empty() {
                            current.push_str(", ");
                            current_len += 2;
                        }
                        current.push_str(part);
                        current_len += part_len;
                    }
                }
                continue;
            }

            if current_len + sentence_len + 1 > max_len && !current.is_empty() {
                chunks.push(current.trim().to_string());
                current.clear();
                current_len = 0;
            }

            if !current.is_empty() {
                current.push(' ');
                current_len += 1;
            }
            current.push_str(sentence);
            current_len += sentence_len;
        }

        if !current.is_empty() {
            chunks.push(current.trim().to_string());
        }
    }

    if chunks.is_empty() {
        vec![String::new()]
    } else {
        chunks
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    // Rust's regex doesn't support lookbehind, so we use a simpler approach
    // Split on sentence boundaries and then check if they're abbreviations
    let re = Regex::new(r"([.!?])\s+").unwrap();

    // Find all matches
    let matches: Vec<_> = re.find_iter(text).collect();
    if matches.is_empty() {
        return vec![text.to_string()];
    }

    let mut sentences = Vec::new();
    let mut last_end = 0;

    for m in matches {
        // Get the text before the punctuation
        let before_punc = &text[last_end..m.start()];

        // Check if this ends with an abbreviation
        let mut is_abbrev = false;
        for abbrev in ABBREVIATIONS {
            let combined = format!("{}{}", before_punc.trim(), &text[m.start()..m.start() + 1]);
            if combined.ends_with(abbrev) {
                is_abbrev = true;
                break;
            }
        }

        if !is_abbrev {
            // This is a real sentence boundary
            sentences.push(text[last_end..m.end()].to_string());
            last_end = m.end();
        }
    }

    // Add the remaining text
    if last_end < text.len() {
        sentences.push(text[last_end..].to_string());
    }

    if sentences.is_empty() {
        vec![text.to_string()]
    } else {
        sentences
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_lang_tags_and_appends_period() {
        let out = preprocess_text("Привет мир", "ru").unwrap();
        assert_eq!(out, "<ru>Привет мир.</ru>");
    }

    #[test]
    fn keeps_existing_terminal_punctuation() {
        let out = preprocess_text("Hello there!", "en").unwrap();
        assert_eq!(out, "<en>Hello there!</en>");
    }

    #[test]
    fn strips_emoji_and_collapses_spaces() {
        let out = preprocess_text("ok 👍  now", "en").unwrap();
        assert_eq!(out, "<en>ok now.</en>");
    }

    #[test]
    fn replaces_em_dash_and_at() {
        let out = preprocess_text("a—b @ c", "en").unwrap();
        assert_eq!(out, "<en>a-b at c.</en>");
    }

    #[test]
    fn invalid_lang_is_engine_error() {
        assert!(matches!(
            preprocess_text("x", "xx").unwrap_err(),
            echo_voice::VoiceError::Engine(_)
        ));
    }

    #[test]
    fn ru_and_uk_are_valid_langs() {
        assert!(is_valid_lang("ru"));
        assert!(is_valid_lang("uk"));
        assert!(!is_valid_lang("xx"));
    }

    #[test]
    fn mask_is_one_up_to_length() {
        let m = length_to_mask(&[2, 3], Some(3));
        assert_eq!(m[[0, 0, 1]], 1.0);
        assert_eq!(m[[0, 0, 2]], 0.0);
        assert_eq!(m[[1, 0, 2]], 1.0);
    }

    #[test]
    fn short_text_is_single_chunk() {
        assert_eq!(
            chunk_text("Short one.", None),
            vec!["Short one.".to_string()]
        );
    }

    #[test]
    fn long_paragraph_splits_by_sentences_under_limit() {
        let long = "One sentence here. ".repeat(30); // ~570 chars
        let chunks = chunk_text(&long, Some(300));
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|c| c.len() <= 300));
    }

    #[test]
    fn abbreviation_does_not_split_sentence() {
        let chunks = chunk_text(
            &format!("{} {}", "Dr. Smith arrived.", "x".repeat(290)),
            Some(300),
        );
        // "Dr." не считается концом предложения — первая часть остаётся целой
        assert!(chunks[0].starts_with("Dr. Smith arrived."));
    }
}
