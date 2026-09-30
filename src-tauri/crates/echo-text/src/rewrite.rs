//! Write Mode: голосовая инструкция + выделенный текст -> переписанный текст.
//! Только чистые функции: сеть и буфер обмена живут в app-крейте.

/// Инструкция для LLM. Выделенный текст подаётся ОТДЕЛЬНО (как `transcription`
/// в `run_llm_prompt`), поэтому здесь его нет — иначе он склеился бы с
/// инструкцией и модель путала бы, что переписывать.
pub fn build_rewrite_prompt(instruction: &str) -> String {
    let instr = instruction.trim();
    let what = if instr.is_empty() {
        "Improve the text: fix grammar and clarity, keep the meaning."
    } else {
        instr
    };
    format!(
        "You are rewriting a snippet the user selected in another application.\n\
         Instruction: {what}\n\
         Rules: return ONLY the rewritten text — no preamble, no explanation, \
         no code fences, no quotes around the whole answer. \
         Keep the original language of the text unless the instruction says otherwise. \
         Preserve leading/trailing whitespace semantics of a plain snippet."
    )
}

/// Ответ модели идёт прямо в чужое поле ввода, поэтому срезаем типовой шум:
/// ограждения кода, вводную фразу, кавычки вокруг ВСЕГО ответа.
pub fn clean_rewrite_output(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // ```lang\n ... \n```
    if s.starts_with("```") {
        if let Some(first_nl) = s.find('\n') {
            let body = &s[first_nl + 1..];
            let body = body.strip_suffix("```").unwrap_or(body);
            s = body.trim().to_string();
        }
    }

    // Вводная фраза одной строкой: «...:» и дальше текст.
    const PREAMBLES: &[&str] = &[
        "here is the rewritten text:",
        "here's the rewritten text:",
        "rewritten text:",
        "вот переписанный текст:",
        "переписанный текст:",
    ];
    if let Some((head, tail)) = s.split_once('\n') {
        let head_low = head.trim().to_lowercase();
        if PREAMBLES.iter().any(|p| head_low == *p) {
            s = tail.trim().to_string();
        }
    }

    // Кавычки вокруг всего ответа — но только если внутри их больше нет,
    // иначе снимем настоящие кавычки из текста пользователя.
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') && s.matches('"').count() == 2 {
        s = s[1..s.len() - 1].to_string();
    }

    s
}

/// Пустое выделение = переписывать нечего (вызывающий уходит в обычную диктовку).
pub fn is_usable_selection(s: &str) -> bool {
    !s.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_instruction_and_forbids_chatter() {
        let p = build_rewrite_prompt("сделай формальнее");
        assert!(p.contains("сделай формальнее"));
        let low = p.to_lowercase();
        // Модель обязана вернуть ТОЛЬКО текст: результат идёт прямо в чужое поле ввода.
        assert!(low.contains("only") || low.contains("только"));
    }

    #[test]
    fn blank_instruction_still_produces_a_prompt() {
        // Пустая инструкция = «просто перепиши»; молча падать нельзя.
        let p = build_rewrite_prompt("   ");
        assert!(!p.trim().is_empty());
    }

    #[test]
    fn clean_strips_code_fences_and_preamble() {
        assert_eq!(clean_rewrite_output("```\nПривет\n```"), "Привет");
        assert_eq!(clean_rewrite_output("```text\nПривет\n```"), "Привет");
        assert_eq!(
            clean_rewrite_output("Here is the rewritten text:\nПривет"),
            "Привет"
        );
        assert_eq!(
            clean_rewrite_output("Вот переписанный текст:\nПривет"),
            "Привет"
        );
    }

    #[test]
    fn clean_preserves_inner_content_and_trims_edges() {
        // Внутренние переводы строк — часть текста, их не трогаем.
        assert_eq!(clean_rewrite_output("  первая\nвторая  "), "первая\nвторая");
        // Одиночная кавычка-обёртка вокруг всего ответа — снимаем.
        assert_eq!(clean_rewrite_output("\"Привет\""), "Привет");
        // Но кавычки ВНУТРИ текста остаются.
        assert_eq!(
            clean_rewrite_output("он сказал \"да\" вчера"),
            "он сказал \"да\" вчера"
        );
    }

    #[test]
    fn empty_or_whitespace_selection_is_unusable() {
        assert!(!is_usable_selection(""));
        assert!(!is_usable_selection("   \n\t "));
        assert!(is_usable_selection("текст"));
    }
}
