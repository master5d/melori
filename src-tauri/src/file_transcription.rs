//! Shared offline file-transcription core used by both the CLI
//! (`run_cli_transcription`) and the GUI Tauri command. Extracts audio via
//! ffmpeg, runs the engine for full details (text + segments), and — when
//! requested — runs speaker diarization and attaches the turns.

use crate::managers::model::{EngineType, ModelManager};
use crate::managers::transcription::{TranscriptionDetails, TranscriptionManager};
use crate::settings::{get_settings, write_settings};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Sample rate the engine and diarizer both expect (ffmpeg downmixes to this).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;
#[cfg(not(feature = "diarization"))]
const DIARIZATION_NOT_INCLUDED: &str =
    "diarization not included in this build (rebuild with --features diarization)";

/// `Command` that doesn't flash a console window on Windows. The GUI app runs
/// with `windows_subsystem = "windows"` (no console of its own), so spawning a
/// child process like ffmpeg would otherwise pop a visible console window for
/// the child's lifetime. No-op on other platforms.
fn no_window_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Locate the ffmpeg executable. Tries PATH first, then common package-manager
/// install locations on Windows (WinGet links dir, Chocolatey, Scoop) so the
/// GUI app works even when it was launched before the PATH update took effect.
fn find_ffmpeg() -> Option<String> {
    // Fast path: already on PATH.
    let ok = no_window_command("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        return Some("ffmpeg".into());
    }

    // Windows fallback: check common package-manager locations.
    #[cfg(windows)]
    {
        use std::path::PathBuf;

        let mut candidates: Vec<PathBuf> = Vec::new();

        // WinGet links directory (symlinks created by winget).
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(&local)
                    .join("Microsoft")
                    .join("WinGet")
                    .join("Links")
                    .join("ffmpeg.exe"),
            );
            // Also scan Gyan.FFmpeg package folder (version-agnostic glob).
            let pkg_root = PathBuf::from(&local)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(&pkg_root) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("Gyan.FFmpeg") {
                        let bin = entry.path().join("ffmpeg-*").join("bin").join("ffmpeg.exe");
                        // read_dir the version folder instead of globbing.
                        if let Ok(versions) = std::fs::read_dir(entry.path()) {
                            for v in versions.flatten() {
                                candidates.push(v.path().join("bin").join("ffmpeg.exe"));
                            }
                        }
                        let _ = bin;
                    }
                }
            }
        }

        // Chocolatey.
        candidates.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin\ffmpeg.exe"));

        // Scoop (current user).
        if let Ok(user) = std::env::var("USERPROFILE") {
            candidates.push(
                PathBuf::from(&user)
                    .join("scoop")
                    .join("shims")
                    .join("ffmpeg.exe"),
            );
        }

        for p in &candidates {
            if p.exists() {
                let path_str = p.to_string_lossy().into_owned();
                let probe = no_window_command(&path_str)
                    .arg("-version")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if probe {
                    return Some(path_str);
                }
            }
        }
    }

    None
}

/// Transcribe a file to full details. `diarize`/`speaker_hint` are accepted now
/// but only take effect once `diarization::diarize` is implemented (Phase 2);
/// until then a warning is emitted and speakers stay `None`.
pub fn transcribe_file_detailed(
    app_handle: &AppHandle,
    input: &Path,
    language: Option<&str>,
    model: Option<&str>,
    diarize: bool,
    speaker_hint: Option<usize>,
    want_words: bool,
) -> Result<TranscriptionDetails> {
    let input_str = input.to_str().context("Input path is not valid UTF-8")?;

    // ffmpeg guard — a clear message beats a cryptic spawn failure.
    let Some(ffmpeg_exe) = find_ffmpeg() else {
        anyhow::bail!(
            "ffmpeg not found on PATH. Install it (e.g. `winget install Gyan.FFmpeg`) and retry."
        );
    };

    // Resolve effective model/language and warn if the model can't code-switch RU/EN.
    let base_settings = get_settings(app_handle);
    let effective_model = model
        .map(|s| s.to_string())
        .unwrap_or_else(|| base_settings.selected_model.clone());
    let effective_language = language
        .map(|s| s.to_string())
        .unwrap_or_else(|| base_settings.selected_language.clone());

    let model_mgr = app_handle.state::<Arc<ModelManager>>();
    if let Some(info) = model_mgr.get_model_info(&effective_model) {
        if !matches!(info.engine_type, EngineType::Whisper) {
            eprintln!(
                "[!] Model '{}' is {:?} (single-script). For Russian/English code-switching, \
use a Whisper model — e.g. `--model turbo`.",
                effective_model, info.engine_type
            );
        }
    }

    // Temporarily steer settings (language + model drive the bilingual prompt and
    // engine selection inside transcribe_detailed). Restored afterwards.
    let needs_override = effective_model != base_settings.selected_model
        || effective_language != base_settings.selected_language;
    if needs_override {
        let mut s = base_settings.clone();
        s.selected_model = effective_model.clone();
        s.selected_language = effective_language.clone();
        write_settings(app_handle, s);
    }

    let result = run_engine(
        app_handle,
        input_str,
        &effective_model,
        &ffmpeg_exe,
        diarize,
        speaker_hint,
        want_words,
    );

    if needs_override {
        write_settings(app_handle, base_settings);
    }

    result
}

fn run_engine(
    app_handle: &AppHandle,
    input_str: &str,
    model_id: &str,
    ffmpeg_exe: &str,
    diarize: bool,
    speaker_hint: Option<usize>,
    want_words: bool,
) -> Result<TranscriptionDetails> {
    #[cfg(not(feature = "diarization"))]
    if diarize {
        anyhow::bail!(DIARIZATION_NOT_INCLUDED);
    }
    #[cfg(not(feature = "diarization"))]
    let _ = speaker_hint;

    use crate::audio_toolkit::audio::read_wav_samples;
    use crate::managers::transcription::TranscribeOpts;
    use crate::progress::{emit_progress, ProgressPhase};

    emit_progress(app_handle, ProgressPhase::Decoding, None);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let temp_wav = std::env::temp_dir().join(format!("echo_cli_{}.wav", ts));

    println!("[*] Extracting audio via ffmpeg (16kHz mono)...");
    let status = no_window_command(ffmpeg_exe)
        .args([
            "-i",
            input_str,
            "-ar",
            &TARGET_SAMPLE_RATE.to_string(),
            "-ac",
            "1",
            "-f",
            "wav",
            "-vn",
            temp_wav.to_str().context("Temp path is not valid UTF-8")?,
            "-y",
        ])
        .status()
        .context("Failed to execute ffmpeg")?;
    if !status.success() {
        anyhow::bail!("ffmpeg failed to extract audio");
    }

    println!("[*] Loading audio samples...");
    let samples = read_wav_samples(&temp_wav).context("Failed to read WAV samples")?;
    let _ = std::fs::remove_file(&temp_wav);

    let manager = app_handle.state::<Arc<TranscriptionManager>>();

    manager.reset_cancel();
    emit_progress(app_handle, ProgressPhase::LoadingModel, None);

    if !manager.is_model_loaded() {
        println!("[*] Loading model: {}...", model_id);
        manager
            .load_model(model_id)
            .context("Failed to load transcription model")?;
    }

    println!("[*] Transcribing (this may take a while for large files)...");
    emit_progress(app_handle, ProgressPhase::Transcribing, None);
    #[cfg_attr(not(feature = "diarization"), allow(unused_mut))]
    let mut details = manager
        .transcribe_detailed_with(
            samples.clone(),
            TranscribeOpts {
                word_timestamps: want_words,
                emit_progress: true,
                ..Default::default()
            },
        )
        .context("Transcription failed")?;

    if manager.is_cancelled() {
        anyhow::bail!("cancelled");
    }

    if diarize {
        // Phase 2 attaches real turns here.
        emit_progress(app_handle, ProgressPhase::Diarizing, None);
        println!("[*] Ensuring diarization models...");
        let model_mgr = app_handle.state::<Arc<ModelManager>>();
        model_mgr
            .ensure_diarization_models()
            .context("Diarization model setup failed")?;

        #[cfg(feature = "diarization")]
        {
            match crate::diarization::diarize(
                app_handle,
                &samples,
                TARGET_SAMPLE_RATE,
                speaker_hint,
            ) {
                Ok(turns) if !turns.is_empty() => details.speakers = Some(turns),
                Ok(_) => {
                    eprintln!(
                        "[!] Diarization produced no speaker turns; output has timestamps only."
                    )
                }
                Err(e) => eprintln!("[!] Diarization failed ({e}); output has timestamps only."),
            }
        }

        if manager.is_cancelled() {
            anyhow::bail!("cancelled");
        }
    }

    emit_progress(app_handle, ProgressPhase::Formatting, None);
    emit_progress(app_handle, ProgressPhase::Done, None);

    Ok(details)
}
