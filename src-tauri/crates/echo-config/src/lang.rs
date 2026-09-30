#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Lang {
    Chinese,
    English,
    French,
    Portuguese,
    Spanish,
    Japanese,
    Turkish,
    Russian,
    Arabic,
    Korean,
    Thai,
    Italian,
    German,
    Vietnamese,
    Malay,
    Indonesian,
    Filipino,
    Hindi,
    TraditionalChinese,
    Polish,
    Czech,
    Dutch,
    Khmer,
    Burmese,
    Persian,
    Gujarati,
    Urdu,
    Telugu,
    Marathi,
    Hebrew,
    Bengali,
    Tamil,
    Ukrainian,
    Tibetan,
    Kazakh,
    Mongolian,
    Uyghur,
    Cantonese,
}

impl Lang {
    pub fn from_code(code: &str) -> Option<Lang> {
        match code.to_lowercase().as_str() {
            "zh" => Some(Lang::Chinese),
            "en" => Some(Lang::English),
            "fr" => Some(Lang::French),
            "pt" => Some(Lang::Portuguese),
            "es" => Some(Lang::Spanish),
            "ja" => Some(Lang::Japanese),
            "tr" => Some(Lang::Turkish),
            "ru" => Some(Lang::Russian),
            "ar" => Some(Lang::Arabic),
            "ko" => Some(Lang::Korean),
            "th" => Some(Lang::Thai),
            "it" => Some(Lang::Italian),
            "de" => Some(Lang::German),
            "vi" => Some(Lang::Vietnamese),
            "ms" => Some(Lang::Malay),
            "id" => Some(Lang::Indonesian),
            "tl" => Some(Lang::Filipino),
            "hi" => Some(Lang::Hindi),
            "zh-hant" => Some(Lang::TraditionalChinese),
            "pl" => Some(Lang::Polish),
            "cs" => Some(Lang::Czech),
            "nl" => Some(Lang::Dutch),
            "km" => Some(Lang::Khmer),
            "my" => Some(Lang::Burmese),
            "fa" => Some(Lang::Persian),
            "gu" => Some(Lang::Gujarati),
            "ur" => Some(Lang::Urdu),
            "te" => Some(Lang::Telugu),
            "mr" => Some(Lang::Marathi),
            "he" => Some(Lang::Hebrew),
            "bn" => Some(Lang::Bengali),
            "ta" => Some(Lang::Tamil),
            "uk" => Some(Lang::Ukrainian),
            "bo" => Some(Lang::Tibetan),
            "kk" => Some(Lang::Kazakh),
            "mn" => Some(Lang::Mongolian),
            "ug" => Some(Lang::Uyghur),
            "yue" => Some(Lang::Cantonese),
            _ => None,
        }
    }

    pub fn from_display_name(name: &str) -> Option<Lang> {
        let name_lower = name.to_lowercase();
        Lang::all()
            .iter()
            .find(|l| l.display_name().to_lowercase() == name_lower)
            .copied()
    }

    pub fn all() -> &'static [Lang] {
        &[
            Lang::Chinese,
            Lang::English,
            Lang::French,
            Lang::Portuguese,
            Lang::Spanish,
            Lang::Japanese,
            Lang::Turkish,
            Lang::Russian,
            Lang::Arabic,
            Lang::Korean,
            Lang::Thai,
            Lang::Italian,
            Lang::German,
            Lang::Vietnamese,
            Lang::Malay,
            Lang::Indonesian,
            Lang::Filipino,
            Lang::Hindi,
            Lang::TraditionalChinese,
            Lang::Polish,
            Lang::Czech,
            Lang::Dutch,
            Lang::Khmer,
            Lang::Burmese,
            Lang::Persian,
            Lang::Gujarati,
            Lang::Urdu,
            Lang::Telugu,
            Lang::Marathi,
            Lang::Hebrew,
            Lang::Bengali,
            Lang::Tamil,
            Lang::Ukrainian,
            Lang::Tibetan,
            Lang::Kazakh,
            Lang::Mongolian,
            Lang::Uyghur,
            Lang::Cantonese,
        ]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::Chinese => "zh",
            Lang::English => "en",
            Lang::French => "fr",
            Lang::Portuguese => "pt",
            Lang::Spanish => "es",
            Lang::Japanese => "ja",
            Lang::Turkish => "tr",
            Lang::Russian => "ru",
            Lang::Arabic => "ar",
            Lang::Korean => "ko",
            Lang::Thai => "th",
            Lang::Italian => "it",
            Lang::German => "de",
            Lang::Vietnamese => "vi",
            Lang::Malay => "ms",
            Lang::Indonesian => "id",
            Lang::Filipino => "tl",
            Lang::Hindi => "hi",
            Lang::TraditionalChinese => "zh-Hant",
            Lang::Polish => "pl",
            Lang::Czech => "cs",
            Lang::Dutch => "nl",
            Lang::Khmer => "km",
            Lang::Burmese => "my",
            Lang::Persian => "fa",
            Lang::Gujarati => "gu",
            Lang::Urdu => "ur",
            Lang::Telugu => "te",
            Lang::Marathi => "mr",
            Lang::Hebrew => "he",
            Lang::Bengali => "bn",
            Lang::Tamil => "ta",
            Lang::Ukrainian => "uk",
            Lang::Tibetan => "bo",
            Lang::Kazakh => "kk",
            Lang::Mongolian => "mn",
            Lang::Uyghur => "ug",
            Lang::Cantonese => "yue",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Lang::Chinese => "Chinese",
            Lang::English => "English",
            Lang::French => "French",
            Lang::Portuguese => "Portuguese",
            Lang::Spanish => "Spanish",
            Lang::Japanese => "Japanese",
            Lang::Turkish => "Turkish",
            Lang::Russian => "Russian",
            Lang::Arabic => "Arabic",
            Lang::Korean => "Korean",
            Lang::Thai => "Thai",
            Lang::Italian => "Italian",
            Lang::German => "German",
            Lang::Vietnamese => "Vietnamese",
            Lang::Malay => "Malay",
            Lang::Indonesian => "Indonesian",
            Lang::Filipino => "Filipino",
            Lang::Hindi => "Hindi",
            Lang::TraditionalChinese => "Traditional Chinese",
            Lang::Polish => "Polish",
            Lang::Czech => "Czech",
            Lang::Dutch => "Dutch",
            Lang::Khmer => "Khmer",
            Lang::Burmese => "Burmese",
            Lang::Persian => "Persian",
            Lang::Gujarati => "Gujarati",
            Lang::Urdu => "Urdu",
            Lang::Telugu => "Telugu",
            Lang::Marathi => "Marathi",
            Lang::Hebrew => "Hebrew",
            Lang::Bengali => "Bengali",
            Lang::Tamil => "Tamil",
            Lang::Ukrainian => "Ukrainian",
            Lang::Tibetan => "Tibetan",
            Lang::Kazakh => "Kazakh",
            Lang::Mongolian => "Mongolian",
            Lang::Uyghur => "Uyghur",
            Lang::Cantonese => "Cantonese",
        }
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_names() {
        assert_eq!(Lang::from_code("ru"), Some(Lang::Russian));
        assert_eq!(Lang::from_code("EN"), Some(Lang::English)); // case-insensitive
        assert_eq!(Lang::Russian.code(), "ru");
        assert_eq!(Lang::Russian.display_name(), "Russian");
        assert_eq!(Lang::from_code("xx"), None);
    }

    #[test]
    fn display_names_and_all() {
        assert_eq!(Lang::from_display_name("Russian"), Some(Lang::Russian));
        assert_eq!(Lang::from_display_name("chinese"), Some(Lang::Chinese));
        assert_eq!(
            Lang::from_display_name("Traditional Chinese"),
            Some(Lang::TraditionalChinese)
        );
        assert_eq!(Lang::from_display_name("Unknown"), None);

        assert_eq!(format!("{}", Lang::Russian), "Russian");

        let all = Lang::all();
        assert!(all.contains(&Lang::English));
        assert!(all.contains(&Lang::Russian));
        assert_eq!(all.len(), 38); // Current count of variants
    }
}
