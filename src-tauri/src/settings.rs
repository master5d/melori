use log::{debug, warn};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

// The pure config model (AppSettings + its enums, defaults, and helper methods)
// lives in the Tauri-free `echo-config` crate; re-exported here so the app's
// `crate::settings::X` call sites keep resolving unchanged.
pub use echo_config::{
    default_agent_bridge_enabled, default_agent_bridge_port, default_app_language,
    default_assistant_enabled, default_audio_feedback_volume, default_auto_capitalize,
    default_auto_punctuate, default_auto_stop_silence_min, default_auto_submit,
    default_autostart_enabled, default_history_limit, default_input_gain, default_local_tts_engine,
    default_log_level, default_model_for_provider, default_monitor_output_device,
    default_monitor_volume, default_overlay_position, default_paste_delay_ms,
    default_post_process_api_keys, default_post_process_enabled, default_post_process_models,
    default_post_process_prompts, default_post_process_provider_id, default_post_process_providers,
    default_recording_retention_period, default_shell_skin, default_show_tray_icon,
    default_sound_theme, default_spoken_lists_enabled, default_start_hidden,
    default_subtitle_font_size, default_subtitle_max_chars, default_subtitle_overlay,
    default_subtitle_refresh_ms, default_supertonic_voice, default_translate_base_url,
    default_translate_enabled, default_translate_model, default_translate_target,
    default_tts_enabled, default_tts_rate, default_tutor_enabled, default_typing_tool,
    default_update_checks_enabled, default_wellbeing_url, default_whisper_gpu_device,
    default_word_correction_threshold, AppSettings, AutoSubmitKey, ClipboardHandling,
    KeyboardImplementation, LLMPrompt, LogLevel, ModelUnloadTimeout, OrtAcceleratorSetting,
    OverlayPosition, PasteMethod, PostProcessProvider, RecordingRetentionPeriod, ShortcutBinding,
    Snippet, SoundTheme, SubtitleFontSize, TypingTool, WhisperAcceleratorSetting,
    APPLE_INTELLIGENCE_PROVIDER_ID,
};
// Its only in-app consumer (shortcut::fetch_post_process_models) imports it under
// this same cfg; an unconditional re-export would be an unused import elsewhere.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub use echo_config::APPLE_INTELLIGENCE_DEFAULT_MODEL_ID;

/// Map our LogLevel to the tauri-plugin-log level (was a From impl; LogLevel now
/// lives in echo-config, so an orphan-rule-safe free fn replaces it).
pub fn log_level_to_plugin(level: LogLevel) -> tauri_plugin_log::LogLevel {
    match level {
        LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
        LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
        LogLevel::Info => tauri_plugin_log::LogLevel::Info,
        LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
        LogLevel::Error => tauri_plugin_log::LogLevel::Error,
    }
}

fn ensure_post_process_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    for provider in default_post_process_providers() {
        // Use match to do a single lookup - either sync existing or add new
        match settings
            .post_process_providers
            .iter_mut()
            .find(|p| p.id == provider.id)
        {
            Some(existing) => {
                // Sync supports_structured_output field for existing providers (migration)
                if existing.supports_structured_output != provider.supports_structured_output {
                    debug!(
                        "Updating supports_structured_output for provider '{}' from {} to {}",
                        provider.id,
                        existing.supports_structured_output,
                        provider.supports_structured_output
                    );
                    existing.supports_structured_output = provider.supports_structured_output;
                    changed = true;
                }
            }
            None => {
                // Provider doesn't exist, add it
                settings.post_process_providers.push(provider.clone());
                changed = true;
            }
        }

        if !settings.post_process_api_keys.contains_key(&provider.id) {
            settings
                .post_process_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        let default_model = default_model_for_provider(&provider.id);
        match settings.post_process_models.get_mut(&provider.id) {
            Some(existing) => {
                if existing.is_empty() && !default_model.is_empty() {
                    *existing = default_model.clone();
                    changed = true;
                }
            }
            None => {
                settings
                    .post_process_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }
    }

    // Ensure default prompts exist
    for default_prompt in default_post_process_prompts() {
        if !settings
            .post_process_prompts
            .iter()
            .any(|p| p.id == default_prompt.id)
        {
            debug!("Adding missing default prompt: {}", default_prompt.id);
            settings.post_process_prompts.push(default_prompt);
            changed = true;
        }
    }

    // Default to semantic-smoothing if no prompt selected
    if settings.post_process_selected_prompt_id.is_none() {
        settings.post_process_selected_prompt_id = Some("semantic-smoothing".to_string());
        changed = true;
    }

    changed
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    let default_shortcut = "";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    let default_post_process_shortcut = "ctrl+shift+space";
    #[cfg(target_os = "macos")]
    let default_post_process_shortcut = "option+shift+space";
    #[cfg(target_os = "linux")]
    let default_post_process_shortcut = "ctrl+shift+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_post_process_shortcut = "alt+shift+space";

    bindings.insert(
        "transcribe_with_post_process".to_string(),
        ShortcutBinding {
            id: "transcribe_with_post_process".to_string(),
            name: "Transcribe with Post-Processing".to_string(),
            description: "Converts your speech into text and applies AI post-processing."
                .to_string(),
            default_binding: default_post_process_shortcut.to_string(),
            current_binding: default_post_process_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );
    bindings.insert(
        "toggle_meeting_copilot".to_string(),
        ShortcutBinding {
            id: "toggle_meeting_copilot".to_string(),
            name: "Toggle Meeting Copilot".to_string(),
            description: "Shows the meeting copilot panel.".to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    for (id, name, description) in [
        (
            "meeting_toggle",
            "Meeting: Toggle",
            "Start or stop the meeting.",
        ),
        (
            "meeting_window",
            "Meeting: Window",
            "Show or hide the meeting window.",
        ),
        (
            "meeting_ask_focus",
            "Meeting: Ask field",
            "Put the cursor in the meeting's Ask field.",
        ),
        (
            "meeting_ask_recap5",
            "Meeting: Recap",
            "Ask for the last five minutes.",
        ),
        (
            "meeting_ask_missed",
            "Meeting: Missed",
            "Ask what may have been missed.",
        ),
        (
            "meeting_ask_agreed",
            "Meeting: Agreed",
            "Ask what was agreed.",
        ),
        (
            "meeting_analyze",
            "Meeting: Analyze",
            "Analyze the meeting.",
        ),
        (
            "meeting_copy_note",
            "Meeting: Copy note",
            "Copy the meeting note.",
        ),
        (
            "meeting_council",
            "Meeting: Council",
            "Open the meeting council.",
        ),
        (
            "meeting_mark",
            "Meeting: Mark",
            "Mark a moment in the meeting.",
        ),
    ] {
        bindings.insert(
            id.to_string(),
            ShortcutBinding {
                id: id.to_string(),
                name: name.to_string(),
                description: description.to_string(),
                default_binding: String::new(),
                current_binding: String::new(),
            },
        );
    }

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        input_gain: default_input_gain(),
        monitor_output_device: default_monitor_output_device(),
        monitor_volume: default_monitor_volume(),
        sound_theme: default_sound_theme(),
        ui_theme: "system".to_string(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        model_mirror_url: String::new(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: "auto".to_string(),
        overlay_position: default_overlay_position(),
        agent_bridge_enabled: default_agent_bridge_enabled(),
        agent_bridge_port: default_agent_bridge_port(),
        tts_enabled: default_tts_enabled(),
        local_tts_engine: default_local_tts_engine(),
        tts_voice_id: None,
        tts_rate: default_tts_rate(),
        tts_voice_mode: echo_config::TtsVoiceMode::System,
        tts_voice_profile: None,
        voice_tier: echo_config::VoiceTier::Auto,
        supertonic_voice: default_supertonic_voice(),
        fish_api_key: echo_config::SecretString::default(),
        fish_model_id: String::new(),
        fish_tts_model: echo_config::default_fish_tts_model(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        model_unload_timeout: ModelUnloadTimeout::default(),
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        auto_submit: default_auto_submit(),
        auto_submit_key: AutoSubmitKey::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: None,
        translate_enabled: default_translate_enabled(),
        translate_target: default_translate_target(),
        translate_model: default_translate_model(),
        translate_base_url: default_translate_base_url(),
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        experimental_enabled: false,
        lazy_stream_close: false,
        keyboard_implementation: KeyboardImplementation::default(),
        show_tray_icon: default_show_tray_icon(),
        paste_delay_ms: default_paste_delay_ms(),
        typing_tool: default_typing_tool(),
        external_script_path: None,
        capture_folder: String::new(),
        capture_trigger_phrases: "capture note, сохрани заметку".to_string(),
        custom_filler_words: None,
        whisper_accelerator: WhisperAcceleratorSetting::default(),
        ort_accelerator: OrtAcceleratorSetting::default(),
        whisper_gpu_device: default_whisper_gpu_device(),
        extra_recording_buffer_ms: 0,
        auto_punctuate: default_auto_punctuate(),
        auto_capitalize: default_auto_capitalize(),
        subtitle_overlay: default_subtitle_overlay(),
        subtitle_font_size: default_subtitle_font_size(),
        subtitle_max_chars: default_subtitle_max_chars(),
        subtitle_refresh_ms: default_subtitle_refresh_ms(),
        command_mode_enabled: false,
        coach_toast_enabled: false,
        snippets: Vec::new(),
        app_profiles: Vec::new(),
        self_correction_enabled: false,
        spoken_lists_enabled: default_spoken_lists_enabled(),
        dev_dictionary_enabled: false,
        assistant_enabled: default_assistant_enabled(),
        assistant_system_prompt: String::new(),
        tutor_enabled: default_tutor_enabled(),
        meeting_consent_acked: false,
        auto_stop_silence_min: default_auto_stop_silence_min(),
        wellbeing_url: default_wellbeing_url(),
        hide_notes_from_screen_share: false,
        shell_skin: default_shell_skin(),
        engine_python: String::new(),
        llm_base_url: String::new(),
        llm_model: String::new(),
        corpus_dir: String::new(),
        llm_embed_model: String::new(),
    }
}

/// Back up an unreadable settings blob before it is overwritten with defaults,
/// so a serde parse failure (e.g. a future incompatible enum rename) can't
/// silently wipe the user's real settings — the historical "settings reset"
/// failure mode. Best-effort; logs loudly and never panics.
fn backup_unreadable_settings(app: &AppHandle, raw: &serde_json::Value, err: &str) {
    warn!(
        "Settings failed to parse ({err}); backing up the unreadable store before \
         resetting to defaults to avoid silent data loss"
    );
    let dir = match crate::portable::app_data_dir(app) {
        Ok(d) => d,
        Err(e) => {
            warn!("could not resolve app data dir for settings backup: {e}");
            return;
        }
    };
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(echo_config::corrupt_settings_backup_filename(secs));
    match serde_json::to_string_pretty(raw) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(()) => warn!("backed up unreadable settings to {}", path.display()),
            Err(e) => warn!("failed to write settings backup to {}: {e}", path.display()),
        },
        Err(e) => warn!("failed to serialize unreadable settings for backup: {e}"),
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object (clone so the raw blob survives for
        // a backup if deserialization fails).
        match serde_json::from_value::<AppSettings>(settings_value.clone()) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if !settings.bindings.contains_key(&key) {
                        debug!("Adding missing binding: {}", key);
                        settings.bindings.insert(key, value);
                        updated = true;
                    }
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                // Back up the unreadable store before overwriting, so a parse
                // failure never silently wipes the user's real settings.
                backup_unreadable_settings(app, &settings_value, &e.to_string());
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        match serde_json::from_value::<AppSettings>(settings_value.clone()) {
            Ok(s) => s,
            Err(e) => {
                // Back up the unreadable store before overwriting with defaults.
                backup_unreadable_settings(app, &settings_value, &e.to_string());
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(crate::portable::store_path(SETTINGS_STORE_PATH))
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_disable_auto_submit() {
        let settings = get_default_settings();
        assert!(!settings.auto_submit);
        assert_eq!(settings.auto_submit_key, AutoSubmitKey::Enter);
    }

    #[test]
    fn debug_output_redacts_api_keys() {
        let mut settings = get_default_settings();
        settings
            .post_process_api_keys
            .insert("openai".to_string(), "sk-proj-secret-key-12345".to_string());
        settings.post_process_api_keys.insert(
            "anthropic".to_string(),
            "sk-ant-secret-key-67890".to_string(),
        );
        settings
            .post_process_api_keys
            .insert("empty_provider".to_string(), "".to_string());

        let debug_output = format!("{:?}", settings);

        assert!(!debug_output.contains("sk-proj-secret-key-12345"));
        assert!(!debug_output.contains("sk-ant-secret-key-67890"));
        assert!(debug_output.contains("[REDACTED]"));
    }
}

#[cfg(test)]
mod translate_settings_tests {
    use super::*;

    #[test]
    fn missing_translate_keys_use_defaults() {
        let mut value =
            serde_json::to_value(get_default_settings()).expect("default settings serialize");
        let object = value.as_object_mut().expect("settings serialize as object");
        object.remove("translate_enabled");
        object.remove("translate_target");
        object.remove("translate_model");
        object.remove("translate_base_url");

        let settings: AppSettings =
            serde_json::from_value(value).expect("translate defaults fill missing fields");
        assert!(!settings.translate_enabled);
        assert_eq!(settings.translate_target, crate::translate::Lang::English);
        assert_eq!(settings.translate_model, "hy-mt1.5");
        assert_eq!(settings.translate_base_url, "http://127.0.0.1:11434/v1");
    }
}

#[cfg(test)]
mod c3_settings_tests {
    use super::get_default_settings;

    #[test]
    fn defaults_include_unacked_consent_and_unbound_copilot_binding() {
        let s = get_default_settings();
        assert_eq!(s.ui_theme, "system");
        assert!(
            !s.meeting_consent_acked,
            "consent must default to NOT acked"
        );
        let b = s
            .bindings
            .get("toggle_meeting_copilot")
            .expect("toggle_meeting_copilot binding must exist");
        assert_eq!(b.current_binding, "", "copilot hotkey must ship unbound");
        assert_eq!(
            b.default_binding, "",
            "copilot hotkey default must be unbound"
        );
    }

    #[test]
    fn melori_dictation_hotkey_off_by_default() {
        let s = get_default_settings();
        let b = s
            .bindings
            .get("transcribe")
            .expect("transcribe binding must exist");
        assert!(b.current_binding.is_empty());
        assert!(b.default_binding.is_empty());
    }
}
