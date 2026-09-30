//! CLI offline transcription: thin wrapper over the shared file-transcription
//! core that renders the result in the requested format to stdout or a file.

use crate::file_transcription::transcribe_file_detailed;
use crate::transcript_format::OutputFormat;
use anyhow::{Context, Result};
use std::path::Path;
use tauri::AppHandle;

#[allow(clippy::too_many_arguments)]
pub fn run_cli_transcription(
    app_handle: &AppHandle,
    input: &Path,
    output: Option<&Path>,
    language: Option<&str>,
    model: Option<&str>,
    format: Option<&str>,
    diarize: bool,
    speaker_hint: Option<usize>,
    translate: Option<&str>,
) -> Result<()> {
    println!("[*] Starting CLI transcription...");
    println!("[*] Input: {}", input.display());

    if speaker_hint.is_some() {
        if !diarize {
            eprintln!("[!] --speakers was given without --diarize; ignoring the speaker count.");
        } else {
            eprintln!(
                "[!] --speakers is not yet supported by the diarization engine (speaker count \
is auto-detected); the value is ignored."
            );
        }
    }

    // Resolve output format: explicit --format wins, else infer from -o extension, else plain.
    let fmt = match format {
        Some(f) => OutputFormat::from_cli(f).with_context(|| {
            format!("Unknown --format '{f}'. Use plain|inline|srt|vtt|json|karaoke|speaker.")
        })?,
        None => output
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .and_then(OutputFormat::from_extension)
            .unwrap_or(OutputFormat::Plain),
    };

    // Resolve an optional translation target up front so a bad code fails fast,
    // before the (slow) transcription runs.
    let translate_target = match translate {
        Some(code) => Some(crate::translate::Lang::from_code(code).with_context(|| {
            format!("Unknown --translate '{code}'. Use a language code like en|ru|uk.")
        })?),
        None => None,
    };

    let want_words = diarize || fmt.is_word_level();
    let details = transcribe_file_detailed(
        app_handle,
        input,
        language,
        model,
        diarize,
        speaker_hint,
        want_words,
    )?;
    let rendered = if let Some(target) = translate_target {
        // Translation v1: translate the plain transcript prose and emit that.
        // (Per-segment translation that preserves timestamps/speaker markers is
        // future work; translating a timecoded render would garble the markers.)
        let settings = crate::settings::get_settings(app_handle);
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
        maybe_translate_transcript(&details.text, Some(target), &translator)
    } else {
        match fmt {
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
        }
    };

    if let Some(out_path) = output {
        std::fs::write(out_path, &rendered).context("Failed to write output file")?;
        println!("[+] Transcription saved to: {}", out_path.display());
    } else {
        println!("--- TRANSCRIPTION START ---");
        println!("{}", rendered);
        println!("--- TRANSCRIPTION END ---");
    }

    Ok(())
}

/// Translate the transcript prose into `target`, gracefully. On any error the
/// ORIGINAL transcript is returned with a logged "translation skipped" note —
/// translation is additive and must never lose the transcript.
fn maybe_translate_transcript(
    text: &str,
    target: Option<crate::translate::Lang>,
    translator: &dyn crate::translate::Translator,
) -> String {
    let Some(target) = target else {
        return text.to_string();
    };
    if text.trim().is_empty() {
        return text.to_string();
    }
    match translator.translate(text, target) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[!] translation skipped: {e:#}");
            text.to_string()
        }
    }
}

#[cfg(test)]
mod translate_transcript_tests {
    use super::*;
    use crate::translate::{Lang, Translator};

    struct OkTranslator;
    impl Translator for OkTranslator {
        fn translate(&self, _text: &str, _target: Lang) -> anyhow::Result<String> {
            Ok("TRANSLATED".to_string())
        }
    }
    struct ErrTranslator;
    impl Translator for ErrTranslator {
        fn translate(&self, _text: &str, _target: Lang) -> anyhow::Result<String> {
            Err(anyhow::anyhow!("server down"))
        }
    }

    #[test]
    fn translates_when_target_set() {
        let out = maybe_translate_transcript("привет мир", Some(Lang::English), &OkTranslator);
        assert_eq!(out, "TRANSLATED");
    }

    #[test]
    fn passthrough_when_no_target() {
        let out = maybe_translate_transcript("привет мир", None, &OkTranslator);
        assert_eq!(out, "привет мир");
    }

    #[test]
    fn graceful_returns_original_on_error() {
        let out = maybe_translate_transcript("привет мир", Some(Lang::English), &ErrTranslator);
        assert_eq!(out, "привет мир");
    }
}
