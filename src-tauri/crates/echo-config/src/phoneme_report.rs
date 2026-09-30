//! Shared specta types for the Course phoneme-compare report (bindings surface).
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
#[serde(rename_all = "lowercase")]
pub enum PhonemeStatus {
    Ok,
    Sub,
    Ins,
    Del,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct PhonemeCell {
    pub ipa: String,
    pub status: PhonemeStatus,
    pub said: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct WordPhonemes {
    pub text: String,
    pub cells: Vec<PhonemeCell>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct PhonemeReport {
    pub overall: u8,
    pub words: Vec<WordPhonemes>,
    pub note: String,
}
