#[cfg(test)]
mod tests {
    use crate::heuristics::Heuristics;

    #[test]
    fn test_russian_commas() {
        let h = Heuristics::new();
        let input = "я хочу спать но нужно работать";
        let output = h.auto_punctuate(input);
        assert_eq!(output, "я хочу спать, но нужно работать.");
    }

    #[test]
    fn test_russian_question() {
        let h = Heuristics::new();
        let input = "почему ты не спишь";
        let output = h.auto_punctuate(input);
        assert_eq!(output, "почему ты не спишь?");
    }

    #[test]
    fn test_mixed_language_commas() {
        let h = Heuristics::new();
        // Testing "but" and "но"
        assert_eq!(
            h.auto_punctuate("I like it but it is expensive"),
            "I like it, but it is expensive."
        );
        assert_eq!(
            h.auto_punctuate("мне нравится но это дорого"),
            "мне нравится, но это дорого."
        );
    }

    #[test]
    fn test_auto_capitalize() {
        let h = Heuristics::new();
        let input = "привет. как дела? хорошо!";
        let output = h.auto_capitalize(input);
        assert_eq!(output, "Привет. Как дела? Хорошо!");
    }

    #[test]
    fn test_complex_russian_conjunctions() {
        let h = Heuristics::new();
        let input = "я приду если будет время";
        let output = h.auto_punctuate(input);
        assert_eq!(output, "я приду, если будет время.");

        let input2 = "он сказал что придет";
        assert_eq!(h.auto_punctuate(input2), "он сказал, что придет.");

        // Let's verify our current list behavior for "чтобы".
        assert_eq!(
            h.auto_punctuate("я пойду чтобы купить хлеба"),
            "я пойду, чтобы купить хлеба."
        );
    }

    #[test]
    fn test_camel_case() {
        let h = Heuristics::new();
        assert_eq!(
            h.to_camel_case("user profile service"),
            "userProfileService"
        );
        assert_eq!(h.to_camel_case("Get Data FROM api"), "getDataFromApi");
        assert_eq!(h.to_camel_case("   multiple   spaces   "), "multipleSpaces");
        assert_eq!(
            h.to_camel_case("with-hyphens_and.dots"),
            "withHyphensAndDots"
        );
        assert_eq!(h.to_camel_case(""), "");
    }
}
