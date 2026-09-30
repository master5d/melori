//! Language of the session, detected from the source transcript. The council and the
//! analysis answer in it. Same rule as the engine's `council/language.py`.

/// "ru" when at least 30% of the letters are Cyrillic, else "en". A Russian session is
/// full of English loanwords and fillers, so a majority rule would misfire.
pub fn transcript_language(text: &str) -> &'static str {
    let mut letters = 0usize;
    let mut cyrillic = 0usize;
    for ch in text.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        if ('\u{0400}'..='\u{04FF}').contains(&ch) {
            cyrillic += 1;
        }
    }
    if letters > 0 && cyrillic * 10 >= letters * 3 {
        "ru"
    } else {
        "en"
    }
}

/// English name of the language, for the instruction appended to a prompt.
pub fn language_name(code: &str) -> &'static str {
    match code {
        "ru" => "Russian",
        _ => "English",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_session_is_russian_even_with_english_fillers() {
        assert_eq!(
            transcript_language("me: ok, sure. others: Потом долго молчал о работе и сне."),
            "ru"
        );
    }

    #[test]
    fn english_session_is_english() {
        assert_eq!(
            transcript_language("me: how was the week? others: busy, slept badly."),
            "en"
        );
    }

    #[test]
    fn empty_is_english() {
        assert_eq!(transcript_language(""), "en");
        assert_eq!(language_name("xx"), "English");
        assert_eq!(language_name("ru"), "Russian");
    }
}
