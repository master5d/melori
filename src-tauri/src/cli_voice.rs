use anyhow::{anyhow, Result};
use echo_voice::{LanguageHint, VoiceRef};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

fn voices_dir(app: &AppHandle) -> PathBuf {
    crate::portable::app_data_dir(app)
        .map(|d| d.join("voices"))
        .unwrap_or_else(|_| PathBuf::from("voices"))
}

fn parse_lang(s: Option<&str>) -> LanguageHint {
    match s.map(|x| x.to_lowercase()).as_deref() {
        Some("ru") => LanguageHint::Ru,
        Some("en") => LanguageHint::En,
        _ => LanguageHint::Mixed,
    }
}

pub fn run_voice_add(
    app: &AppHandle,
    name: &str,
    refs: &[PathBuf],
    ref_text: Option<&str>,
    lang: Option<&str>,
) -> Result<()> {
    if refs.is_empty() {
        return Err(anyhow!("--voice-add requires at least one --ref <wav>"));
    }
    let mut voice_refs = Vec::new();
    for (i, wav) in refs.iter().enumerate() {
        // Transcript: use --ref-text for a single ref, else auto-transcribe with Echo STT.
        let transcript = if refs.len() == 1 && ref_text.is_some() {
            ref_text.unwrap().to_string()
        } else {
            transcribe_ref(app, wav)?
        };
        let duration_s = wav_duration_secs(wav).unwrap_or(0.0);
        voice_refs.push(VoiceRef {
            wav: wav.clone(),
            transcript,
            duration_s,
        });
        eprintln!("  ref {} transcribed", i + 1);
    }
    let vdir = voices_dir(app);
    let profile = echo_voice::profile::create_profile(&vdir, name, parse_lang(lang), voice_refs)
        .map_err(|e| anyhow!(e.to_string()))?;
    println!(
        "created voice profile: {} ({} refs)",
        profile.id,
        profile.refs.len()
    );
    Ok(())
}

pub fn run_voice_list(app: &AppHandle) -> Result<()> {
    let vdir = voices_dir(app);
    let profiles = echo_voice::profile::list_profiles(&vdir).map_err(|e| anyhow!(e.to_string()))?;
    if profiles.is_empty() {
        println!("(no voice profiles)");
    }
    for p in profiles {
        println!(
            "{}\t{}\t{:?}\t{} refs",
            p.id,
            p.display_name,
            p.language_hint,
            p.refs.len()
        );
    }
    Ok(())
}

pub fn run_voice_remove(app: &AppHandle, id: &str) -> Result<()> {
    let vdir = voices_dir(app);
    echo_voice::profile::remove_profile(&vdir, id).map_err(|e| anyhow!(e.to_string()))?;
    println!("removed voice profile: {id}");
    Ok(())
}

pub fn run_narrate(
    app: &AppHandle,
    input: &Path,
    voice_id: &str,
    out: Option<&Path>,
    tier: Option<&str>,
) -> Result<()> {
    let md = std::fs::read_to_string(input)?;
    let vdir = voices_dir(app);
    let profile =
        echo_voice::profile::load_profile(&vdir, voice_id).map_err(|e| anyhow!(e.to_string()))?;
    echo_voice::profile::validate_profile(&profile).map_err(|e| anyhow!(e.to_string()))?;

    let mut settings = crate::settings::get_settings(app);
    if let Some(t) = tier {
        settings.voice_tier = match t.to_lowercase().as_str() {
            "local" => echo_config::VoiceTier::LocalOnly,
            "sidecar" => echo_config::VoiceTier::SidecarOnly,
            _ => echo_config::VoiceTier::Auto,
        };
    }
    let router = crate::voice_bridge::build_router(app, &settings).ok_or_else(|| {
        anyhow!("no voice engine available (install the local engine or reach the sidecar)")
    })?;

    let cache = std::env::temp_dir().join(format!("echo-narrate-{}", profile.id));
    let opts = echo_voice::narrate::NarrateOpts {
        max_chars: 240,
        cache_dir: cache,
    };
    let wav = echo_voice::narrate::narrate(&router, &md, &profile, &opts, &mut |d, t| {
        eprintln!("chunk {d}/{t}");
    })
    .map_err(|e| anyhow!(e.to_string()))?;

    let out_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| input.with_extension("wav"));
    std::fs::write(&out_path, wav)?;
    println!("narrated -> {}", out_path.display());
    Ok(())
}

/// Auto-transcribe a reference clip with Echo's own STT (reuses the CLI path).
fn transcribe_ref(app: &AppHandle, wav: &Path) -> Result<String> {
    let out = std::env::temp_dir().join("echo-ref-transcript.txt");
    crate::cli_transcription::run_cli_transcription(
        app,
        wav,
        Some(&out),
        Some("auto"),
        None,
        Some("plain"),
        false,
        None,
        None,
    )
    .map_err(|e| anyhow!("auto-transcribe failed: {e}"))?;
    Ok(std::fs::read_to_string(&out)?.trim().to_string())
}

/// WAV duration from the header (bytes 40..44 data size / byte rate at 28..32).
fn wav_duration_secs(wav: &Path) -> Option<f32> {
    let bytes = std::fs::read(wav).ok()?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" {
        return None;
    }
    let byte_rate = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
    let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
    if byte_rate == 0 {
        return None;
    }
    Some(data_len as f32 / byte_rate as f32)
}
