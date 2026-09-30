#[tauri::command]
#[specta::specta]
pub fn tutor_score(reference: String, spoken: String) -> crate::tutor::ScoreReport {
    crate::tutor::score_pronunciation(&reference, &spoken)
}

#[tauri::command]
#[specta::specta]
pub fn phoneme_compare(
    app: tauri::AppHandle,
    reference: String,
    spoken: String,
    lang: String,
) -> Result<echo_config::PhonemeReport, String> {
    use tauri::Manager;
    // espeak is bundled Windows-only; if the resource can't be resolved the
    // feature is unavailable — the frontend falls back to word-level scoring.
    let bin = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng.exe",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|_| "unavailable".to_string())?;
    if !bin.exists() {
        return Err("unavailable".to_string());
    }
    let data_dir = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng-data",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|_| "unavailable".to_string())?;

    let voice = match lang.as_str() {
        "ru" => "ru",
        _ => "en-us",
    };

    let phon = echo_voice_local::phonemize::Phonemizer {
        bin,
        data_dir,
        timeout: std::time::Duration::from_secs(5),
    };

    let ref_words: Vec<String> = reference
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let spoken_words: Vec<String> = spoken.split_whitespace().map(|s| s.to_string()).collect();
    let ref_ipa = phon
        .text_to_words_ipa(&reference, voice)
        .map_err(|e| format!("phonemize reference: {e}"))?;
    let spoken_ipa = phon
        .text_to_words_ipa(&spoken, voice)
        .map_err(|e| format!("phonemize spoken: {e}"))?;

    Ok(echo_text::phoneme::build_phoneme_report(
        &ref_words,
        &ref_ipa,
        &spoken_words,
        &spoken_ipa,
    ))
}
