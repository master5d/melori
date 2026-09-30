use super::lang::Lang;

pub fn build_prompt(text: &str, target: Lang) -> String {
    format!(
        "Translate the following segment into {}, without additional explanation.\n\n{}",
        target.display_name(),
        text
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::lang::Lang;
    #[test]
    fn builds_xx_prompt() {
        let p = build_prompt("Привет", Lang::English);
        assert_eq!(
            p,
            "Translate the following segment into English, without additional explanation.\n\nПривет"
        );
    }
}
