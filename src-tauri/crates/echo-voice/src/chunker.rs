/// Strip markdown to plain speech text. Deliberately simple line/char scanning
/// (no full markdown parser dep): fenced code removed wholesale, inline code
/// unwrapped, links reduced to their text, bare URLs dropped, heading/emphasis
/// markers removed.
pub fn clean_markdown(md: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in md.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut l = strip_heading_marker(line);
        l = unwrap_links(&l);
        l = drop_bare_urls(&l);
        l = strip_inline_markers(&l);
        out.push_str(l.trim_end());
        // A blank line becomes a sentence boundary; a normal line a space.
        out.push(if l.trim().is_empty() { '\n' } else { ' ' });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_heading_marker(line: &str) -> String {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix('#') {
        let rest = rest.trim_start_matches('#').trim_start();
        // headings end a spoken sentence
        return format!("{rest}.");
    }
    line.to_string()
}

fn unwrap_links(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if ch == '[' {
            if let Some(close) = s[i..].find("](") {
                let text = &s[i + 1..i + close];
                if let Some(paren) = s[i + close + 2..].find(')') {
                    out.push_str(text);
                    let skip_to = i + close + 2 + paren + 1;
                    // advance char_indices past the consumed region
                    while chars.peek().map(|&(j, _)| j < skip_to).unwrap_or(false) {
                        chars.next();
                    }
                    continue;
                }
            }
        }
        out.push(ch);
    }
    out
}

fn drop_bare_urls(s: &str) -> String {
    s.split_whitespace()
        .filter(|w| !(w.starts_with("http://") || w.starts_with("https://")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_inline_markers(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '`' | '*' | '_' | '#' | '>'))
        .collect()
}

/// Split into speech chunks of at most `max_chars`, packing whole sentences.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let sentences = split_sentences(text);
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for s in sentences {
        for piece in split_oversize(&s, max_chars) {
            if cur.is_empty() {
                cur = piece;
            } else if cur.len() + 1 + piece.len() <= max_chars {
                cur.push(' ');
                cur.push_str(&piece);
            } else {
                chunks.push(std::mem::take(&mut cur));
                cur = piece;
            }
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?' | '…') {
            let t = cur.trim().to_string();
            if !t.is_empty() {
                out.push(t);
            }
            cur.clear();
        }
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

fn split_oversize(sentence: &str, max_chars: usize) -> Vec<String> {
    if sentence.len() <= max_chars {
        return vec![sentence.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in sentence.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.len() + 1 + word.len() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            out.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_code_fence_and_keeps_prose() {
        let md = "Intro line.\n\n```rust\nfn x() {}\n```\n\nOutro line.";
        let out = clean_markdown(md);
        assert!(out.contains("Intro line."));
        assert!(out.contains("Outro line."));
        assert!(!out.contains("fn x"));
    }

    #[test]
    fn unwraps_links_and_inline_code() {
        let out = clean_markdown("See [the docs](https://x.io) and `cargo test`.");
        assert!(out.contains("the docs"));
        assert!(out.contains("cargo test"));
        assert!(!out.contains("https://x.io"));
        assert!(!out.contains("]("));
    }

    #[test]
    fn heading_marker_removed_text_kept() {
        let out = clean_markdown("# Title\nBody.");
        assert!(out.contains("Title"));
        assert!(!out.trim_start().starts_with('#'));
    }

    #[test]
    fn chunks_pack_sentences_under_limit() {
        let text = "One two three. Four five six. Seven eight nine.";
        let chunks = chunk_text(text, 20);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            assert!(c.len() <= 20 || !c.contains(' '), "chunk too long: {c:?}");
        }
        assert_eq!(chunks.join(" ").replace("  ", " "), text);
    }

    #[test]
    fn russian_sentences_split() {
        let text = "Привет мир. Как дела? Хорошо!";
        let chunks = chunk_text(text, 12);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn oversize_single_sentence_splits_on_whitespace() {
        let text = "alpha beta gamma delta epsilon zeta";
        let chunks = chunk_text(text, 12);
        assert!(chunks.iter().all(|c| c.len() <= 12));
    }

    #[test]
    fn clean_markdown_preserves_russian_prose() {
        let out = clean_markdown("Привет мир. Как [дела](https://x.io)?");
        assert!(out.contains("Привет мир"));
        assert!(out.contains("дела"));
        assert!(!out.contains("https://x.io"));
    }
}
