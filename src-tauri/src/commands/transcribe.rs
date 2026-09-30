use crate::file_transcription::transcribe_file_detailed;
use crate::transcript_format::OutputFormat;
use crate::translate::Translator;
use std::path::PathBuf;
use tauri::AppHandle;

/// Transcribe a file from the GUI and return the rendered string.
/// `format` is one of: plain|inline|srt|vtt|json.
/// `translate` is an optional target language code (e.g. "en"/"ru"); when set, the
/// transcript prose is translated (offline, Hy-MT) and returned as plain text.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_file_to_string(
    app_handle: AppHandle,
    path: String,
    language: Option<String>,
    model: Option<String>,
    diarize: bool,
    speaker_hint: Option<u32>,
    format: String,
    translate: Option<String>,
) -> Result<String, String> {
    let fmt =
        OutputFormat::from_cli(&format).ok_or_else(|| format!("Unknown format '{format}'"))?;
    let translate_target = match translate {
        Some(code) => Some(
            crate::translate::Lang::from_code(&code)
                .ok_or_else(|| format!("Unknown translate language '{code}'"))?,
        ),
        None => None,
    };
    let input = PathBuf::from(path);
    let want_words = diarize || fmt.is_word_level();
    // Run the blocking pipeline (and optional translation) off the async runtime thread.
    let body = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let details = transcribe_file_detailed(
            &app_handle,
            &input,
            language.as_deref(),
            model.as_deref(),
            diarize,
            speaker_hint.map(|n| n as usize),
            want_words,
        )
        .map_err(|e| e.to_string())?;

        // Translation v1: translate the plain transcript prose and return that.
        // Graceful — on any translator error the original transcript is returned.
        if let Some(target) = translate_target {
            let settings = crate::settings::get_settings(&app_handle);
            let translator = crate::translate::ServerTranslator {
                provider: crate::settings::PostProcessProvider {
                    id: "translate-local".to_string(),
                    label: "Translate".to_string(),
                    base_url: settings.translate_base_url.clone(),
                    allow_base_url_edit: false,
                    models_endpoint: None,
                    supports_structured_output: false,
                },
                model: settings.translate_model.clone(),
                api_key: String::new(),
            };
            return Ok(match translator.translate(&details.text, target) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("File translation skipped; returning original: {e:#}");
                    details.text.clone()
                }
            });
        }

        Ok(match fmt {
            OutputFormat::Json if details.words.is_some() => {
                crate::transcript_format::render_word_json(
                    details.words.as_deref().unwrap(),
                    details.speakers.as_deref(),
                )
            }
            OutputFormat::Karaoke => crate::transcript_format::render_karaoke(
                details.words.as_deref().unwrap_or(&[]),
                details.speakers.as_deref(),
            ),
            _ => crate::transcript_format::render(
                &details.text,
                &details.segments,
                details.words.as_deref(),
                details.speakers.as_deref(),
                fmt,
            ),
        })
    })
    .await
    .map_err(|e| format!("task join error: {e}"))??;

    Ok(body)
}

/// Request cancellation of the in-flight file transcription (Transcribe-file tab).
#[tauri::command]
#[specta::specta]
pub async fn cancel_file_transcription(app: AppHandle) -> Result<(), String> {
    use crate::managers::transcription::TranscriptionManager;
    use std::sync::Arc;
    use tauri::Manager;
    app.state::<Arc<TranscriptionManager>>().request_cancel();
    Ok(())
}
