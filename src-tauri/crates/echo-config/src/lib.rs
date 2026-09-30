//! Pure, platform-agnostic shared config types for Echo.
//! No Tauri, no I/O — safe to compile and test on any target (incl. Android).
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::fmt;

mod lang;
mod phoneme_report;
mod progress;

pub use lang::Lang;
pub use phoneme_report::{PhonemeCell, PhonemeReport, PhonemeStatus, WordPhonemes};
pub use progress::{ProgressPhase, TranscriptionProgress};

pub const APPLE_INTELLIGENCE_PROVIDER_ID: &str = "apple_intelligence";
pub const APPLE_INTELLIGENCE_DEFAULT_MODEL_ID: &str = "Apple Intelligence";

/// A spoken-trigger text expansion: saying `trigger` inserts `text`.
#[derive(Serialize, Deserialize, Debug, Clone, Type, PartialEq, Eq)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
}

/// Правило «окно → политика форматирования». `match_title` — регистронезависимая
/// ПОДСТРОКА заголовка активного окна (не regex: заголовки меняются, а пользователь
/// пишет это руками).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Type)]
pub struct AppProfile {
    pub match_title: String,
    #[serde(default)]
    pub strip_trailing_punct: bool,
    #[serde(default)]
    pub camel_case_trigger: bool,
    #[serde(default)]
    pub post_process_prompt_id: Option<String>,
}

/// Что делать с текстом для текущего окна.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppPolicy {
    pub strip_trailing_punct: bool,
    pub camel_case_trigger: bool,
    pub post_process_prompt_id: Option<String>,
}

/// Встроенный список code-редакторов — поведение по умолчанию, когда
/// пользователь не завёл ни одного профиля на это окно.
pub const BUILTIN_CODE_EDITORS: &[&str] = &[
    "visual studio code",
    "cursor",
    "zed",
    "sublime",
    "intellij",
    "webstorm",
    "pycharm",
    "clion",
    "neovim",
    "visual studio",
];

/// Первый совпавший профиль побеждает; иначе — встроенный список; иначе — пусто.
pub fn resolve_app_policy(window_title: &str, profiles: &[AppProfile]) -> AppPolicy {
    let title = window_title.to_lowercase();
    for p in profiles {
        let needle = p.match_title.trim().to_lowercase();
        if needle.is_empty() {
            continue; // «правило на всё» — не допускаем
        }
        if title.contains(&needle) {
            return AppPolicy {
                strip_trailing_punct: p.strip_trailing_punct,
                camel_case_trigger: p.camel_case_trigger,
                post_process_prompt_id: p.post_process_prompt_id.clone(),
            };
        }
    }
    if BUILTIN_CODE_EDITORS.iter().any(|e| title.contains(e)) {
        return AppPolicy {
            strip_trailing_punct: true,
            camel_case_trigger: true,
            post_process_prompt_id: None,
        };
    }
    AppPolicy::default()
}

/// Какой промпт пост-обработки применится фактически.
///
/// Профиль приложения — УТОЧНЕНИЕ поверх выбранного промпта, а не замена всего
/// шага: если в профиле стоит id, которого больше нет (промпт переименовали или
/// удалили), пост-обработка обязана продолжить работать по глобальному выбору.
/// Иначе одна опечатка в профиле тихо выключала бы LLM-шаг, и это выглядело бы
/// как «модель перестала отвечать», а не как ошибка конфигурации.
pub fn effective_prompt_id(
    profile_prompt_id: Option<&str>,
    selected_prompt_id: Option<&str>,
    prompts: &[LLMPrompt],
) -> Option<String> {
    if let Some(id) = profile_prompt_id {
        if prompts.iter().any(|p| p.id == id) {
            return Some(id.to_string());
        }
    }
    selected_prompt_id.map(|s| s.to_string())
}

/// Built-in developer terminology merged into the custom-words glossary when
/// `dev_dictionary_enabled`. Steers transcription toward the correct casing and
/// spelling of common tools, languages, and acronyms.
pub const DEV_DICTIONARY: &[&str] = &[
    "GitHub",
    "GitLab",
    "Cloudflare",
    "Vercel",
    "Supabase",
    "Netlify",
    "Kubernetes",
    "Docker",
    "Postgres",
    "PostgreSQL",
    "SQLite",
    "Redis",
    "Nginx",
    "TypeScript",
    "JavaScript",
    "Python",
    "Rust",
    "Tauri",
    "React",
    "Next.js",
    "Node.js",
    "Deno",
    "Bun",
    "npm",
    "pnpm",
    "Webpack",
    "Vite",
    "ESLint",
    "Prettier",
    "GraphQL",
    "OAuth",
    "Tailwind",
    "Figma",
    "Notion",
    "Slack",
    "Jira",
    "Linear",
    "Anthropic",
    "OpenAI",
    "Claude",
    "Ollama",
    "Whisper",
    "CUDA",
    "Vulkan",
    "WebSocket",
    "localhost",
    "middleware",
    "async",
    "await",
];

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMPrompt {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_base_url_edit: bool,
    #[serde(default)]
    pub models_endpoint: Option<String>,
    #[serde(default)]
    pub supports_structured_output: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec15, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
    ExternalScript,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    DontModify,
    CopyToClipboard,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoSubmitKey {
    Enter,
    CtrlEnter,
    CmdEnter,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

/// Live-subtitle text size. The actual CSS pixel mapping is applied in the
/// overlay frontend (`SUBTITLE_FONT_PX`), keyed off this enum's serialized name.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleFontSize {
    Small,
    Medium,
    Large,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardImplementation {
    Tauri,
    // Deserialize-only back-compat: stores written before the
    // handy_keys -> echo_keys rename must not fail the whole
    // AppSettings parse (get_settings resets to defaults on Err).
    #[serde(alias = "handy_keys")]
    EchoKeys,
}

impl Default for KeyboardImplementation {
    fn default() -> Self {
        #[cfg(target_os = "linux")]
        return KeyboardImplementation::Tauri;
        #[cfg(not(target_os = "linux"))]
        return KeyboardImplementation::EchoKeys;
    }
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Min5
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Direct (native Unicode typing via enigo `text()` -> SendInput
        // KEYEVENTF_UNICODE on Windows) is the default everywhere. For
        // Russian-primary bilingual dictation this avoids clobbering the
        // user's clipboard and the save/paste/restore round-trip race.
        // Users who paste very long text or hit app-specific quirks can
        // switch back to CtrlV in settings.
        PasteMethod::Direct
    }
}

impl Default for ClipboardHandling {
    fn default() -> Self {
        ClipboardHandling::DontModify
    }
}

impl Default for AutoSubmitKey {
    fn default() -> Self {
        AutoSubmitKey::Enter
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec15 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec15 => Some(15),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(&self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(&self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(&self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TypingTool {
    Auto,
    Wtype,
    Kwtype,
    Dotool,
    Ydotool,
    Xdotool,
}

impl Default for TypingTool {
    fn default() -> Self {
        TypingTool::Auto
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum WhisperAcceleratorSetting {
    Auto,
    Cpu,
    Gpu,
}

impl Default for WhisperAcceleratorSetting {
    fn default() -> Self {
        WhisperAcceleratorSetting::Auto
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum OrtAcceleratorSetting {
    Auto,
    Cpu,
    Cuda,
    #[serde(rename = "directml")]
    DirectMl,
    Rocm,
}

impl Default for OrtAcceleratorSetting {
    fn default() -> Self {
        OrtAcceleratorSetting::Auto
    }
}

// `pub` (was `pub(crate)` when this lived in the app): the app crate reaches the
// map through the `AppSettings.post_process_api_keys` field via Deref/DerefMut.
// The newtype field itself stays private.
#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(transparent)]
pub struct SecretMap(HashMap<String, String>);

impl fmt::Debug for SecretMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted: HashMap<&String, &str> = self
            .0
            .iter()
            .map(|(k, v)| (k, if v.is_empty() { "" } else { "[REDACTED]" }))
            .collect();
        redacted.fmt(f)
    }
}

impl std::ops::Deref for SecretMap {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SecretMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A single secret value: serializes transparently, Debug redacts.
#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, Type)]
#[serde(transparent)]
pub struct SecretString(pub String);

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"***\"")
    }
}

impl std::ops::Deref for SecretString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum VoiceTier {
    Auto,
    LocalOnly,
    SidecarOnly,
}

impl Default for VoiceTier {
    fn default() -> Self {
        VoiceTier::Auto
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TtsVoiceMode {
    System,
    Cloned,
}

impl Default for TtsVoiceMode {
    fn default() -> Self {
        TtsVoiceMode::System
    }
}

/* still handy for composing the initial JSON in the store ------------- */
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_input_gain")]
    pub input_gain: f32,
    #[serde(default = "default_monitor_output_device")]
    pub monitor_output_device: Option<String>,
    #[serde(default = "default_monitor_volume")]
    pub monitor_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_ui_theme")]
    pub ui_theme: String,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    /// Base URL for privately mirrored model artifacts. Empty means unavailable.
    #[serde(default)]
    pub model_mirror_url: String,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default = "default_agent_bridge_enabled")]
    pub agent_bridge_enabled: bool,
    #[serde(default = "default_agent_bridge_port")]
    pub agent_bridge_port: u16,
    #[serde(default = "default_tts_enabled")]
    pub tts_enabled: bool,
    #[serde(default)]
    pub tts_voice_id: Option<String>,
    #[serde(default = "default_tts_rate")]
    pub tts_rate: f32,
    #[serde(default)]
    pub tts_voice_mode: TtsVoiceMode,
    #[serde(default)]
    pub tts_voice_profile: Option<String>,
    #[serde(default)]
    pub voice_tier: VoiceTier,
    /// Local TTS engine for the readback floor: "supertonic" (default) or "piper".
    #[serde(default = "default_local_tts_engine")]
    pub local_tts_engine: String,
    /// Supertonic voice style id (M1..M5, F1..F5).
    #[serde(default = "default_supertonic_voice")]
    pub supertonic_voice: String,
    /// Fish Audio cloud TTS tier. Empty key or model id = tier off.
    #[serde(default)]
    pub fish_api_key: SecretString,
    #[serde(default)]
    pub fish_model_id: String,
    /// HTTP `model` header for Fish TTS ("s2.1-pro-free" during the promo, "s2.1-pro" after).
    #[serde(default = "default_fish_tts_model")]
    pub fish_tts_model: String,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default)]
    pub auto_submit_key: AutoSubmitKey,
    #[serde(default = "default_post_process_enabled")]
    pub post_process_enabled: bool,
    #[serde(default = "default_post_process_provider_id")]
    pub post_process_provider_id: String,
    #[serde(default = "default_post_process_providers")]
    pub post_process_providers: Vec<PostProcessProvider>,
    #[serde(default = "default_post_process_api_keys")]
    pub post_process_api_keys: SecretMap,
    #[serde(default = "default_post_process_models")]
    pub post_process_models: HashMap<String, String>,
    #[serde(default = "default_post_process_prompts")]
    pub post_process_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub post_process_selected_prompt_id: Option<String>,
    #[serde(default = "default_translate_enabled")]
    pub translate_enabled: bool,
    #[serde(default = "default_translate_target")]
    pub translate_target: Lang,
    #[serde(default = "default_translate_model")]
    pub translate_model: String,
    #[serde(default = "default_translate_base_url")]
    pub translate_base_url: String,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default)]
    pub experimental_enabled: bool,
    #[serde(default)]
    pub lazy_stream_close: bool,
    #[serde(default)]
    pub keyboard_implementation: KeyboardImplementation,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_typing_tool")]
    pub typing_tool: TypingTool,
    pub external_script_path: Option<String>,
    #[serde(default)]
    pub capture_folder: String,
    #[serde(default)]
    pub capture_trigger_phrases: String,
    #[serde(default)]
    pub custom_filler_words: Option<Vec<String>>,
    #[serde(default)]
    pub whisper_accelerator: WhisperAcceleratorSetting,
    #[serde(default)]
    pub ort_accelerator: OrtAcceleratorSetting,
    #[serde(default = "default_whisper_gpu_device")]
    pub whisper_gpu_device: i32,
    #[serde(default)]
    pub extra_recording_buffer_ms: u64,
    #[serde(default = "default_auto_punctuate")]
    pub auto_punctuate: bool,
    #[serde(default = "default_auto_capitalize")]
    pub auto_capitalize: bool,
    #[serde(default = "default_subtitle_overlay")]
    pub subtitle_overlay: bool,
    #[serde(default = "default_subtitle_font_size")]
    pub subtitle_font_size: SubtitleFontSize,
    #[serde(default = "default_subtitle_max_chars")]
    pub subtitle_max_chars: u32,
    #[serde(default = "default_subtitle_refresh_ms")]
    pub subtitle_refresh_ms: u32,
    #[serde(default)]
    pub command_mode_enabled: bool,
    #[serde(default)]
    pub coach_toast_enabled: bool,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    /// Профили «приложение → форматирование». Пусто = встроенное поведение.
    #[serde(default)]
    pub app_profiles: Vec<AppProfile>,
    #[serde(default)]
    pub self_correction_enabled: bool,
    #[serde(default = "default_spoken_lists_enabled")]
    pub spoken_lists_enabled: bool,
    #[serde(default)]
    pub dev_dictionary_enabled: bool,
    #[serde(default = "default_assistant_enabled")]
    pub assistant_enabled: bool,
    #[serde(default)]
    pub assistant_system_prompt: String,
    #[serde(default = "default_tutor_enabled")]
    pub tutor_enabled: bool,
    #[serde(default)]
    pub meeting_consent_acked: bool,
    #[serde(default = "default_auto_stop_silence_min")]
    pub auto_stop_silence_min: u32,
    #[serde(default = "default_wellbeing_url")]
    pub wellbeing_url: String,
    #[serde(default)]
    pub hide_notes_from_screen_share: bool,
    /// Which suite shell skin to render: "rail" | "home" | "deck" (Warm
    /// Studio Audio Suite redesign, spec §4). Plain `String` rather than an
    /// enum so an unrecognized/future value never fails settings deserialize
    /// — the frontend's `ShellHost` falls back to `RailShell` for anything
    /// it doesn't recognize.
    #[serde(default = "default_shell_skin")]
    pub shell_skin: String,
    #[serde(default)]
    pub engine_python: String,
    #[serde(default)]
    pub llm_base_url: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default)]
    pub corpus_dir: String,
    #[serde(default)]
    pub llm_embed_model: String,
}

pub fn default_tutor_enabled() -> bool {
    false
}

pub fn default_auto_stop_silence_min() -> u32 { 5 }

pub fn default_wellbeing_url() -> String {
    "http://127.0.0.1:8000".to_string()
}

pub fn default_spoken_lists_enabled() -> bool {
    true
}

pub fn default_assistant_enabled() -> bool {
    false
}

pub fn default_auto_punctuate() -> bool {
    true
}

pub fn default_auto_capitalize() -> bool {
    true
}

pub fn default_subtitle_overlay() -> bool {
    // Off by default: the streaming live-subtitle task re-transcribes the whole
    // buffer every ~300ms with a blocking call. That's fine for sub-second
    // engines (Parakeet/Moonshine) but on a slow engine (Whisper large on CPU)
    // it floods the inference engine and makes the pipeline appear to hang for
    // a very long time. Opt in only when running a fast model.
    false
}

pub fn default_subtitle_font_size() -> SubtitleFontSize {
    SubtitleFontSize::Medium
}

pub fn default_subtitle_max_chars() -> u32 {
    140
}

pub fn default_subtitle_refresh_ms() -> u32 {
    300
}

fn default_model() -> String {
    "".to_string()
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

pub fn default_translate_enabled() -> bool {
    false
}

pub fn default_translate_target() -> Lang {
    Lang::English
}

pub fn default_translate_model() -> String {
    "hy-mt1.5".to_string()
}

pub fn default_translate_base_url() -> String {
    "http://127.0.0.1:11434/v1".to_string()
}

pub fn default_start_hidden() -> bool {
    false
}

pub fn default_autostart_enabled() -> bool {
    false
}

/// melori ships with the updater plugin disabled and no endpoints — nothing to check.
pub fn default_update_checks_enabled() -> bool {
    false
}

fn default_selected_language() -> String {
    "auto".to_string()
}

pub fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

pub fn default_log_level() -> LogLevel {
    LogLevel::Debug
}

pub fn default_agent_bridge_enabled() -> bool {
    true
}

pub fn default_agent_bridge_port() -> u16 {
    4123
}

pub fn default_tts_enabled() -> bool {
    true
}

pub fn default_tts_rate() -> f32 {
    1.0
}

pub fn default_word_correction_threshold() -> f64 {
    0.18
}

pub fn default_paste_delay_ms() -> u64 {
    60
}

pub fn default_auto_submit() -> bool {
    false
}

pub fn default_history_limit() -> usize {
    5
}

pub fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

pub fn default_audio_feedback_volume() -> f32 {
    1.0
}

pub fn default_input_gain() -> f32 {
    0.0
}

pub fn default_monitor_output_device() -> Option<String> {
    None
}

pub fn default_monitor_volume() -> f32 {
    1.0
}

pub fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

pub fn default_ui_theme() -> String {
    "system".to_string()
}

pub fn default_post_process_enabled() -> bool {
    false
}

pub fn default_shell_skin() -> String {
    "home".to_string()
}

pub fn default_app_language() -> String {
    // Was `tauri_plugin_os::locale()` in the app. That function is a thin
    // wrapper over `sys_locale::get_locale()`; calling sys-locale directly
    // keeps this crate Tauri-free with identical behavior.
    sys_locale::get_locale()
        .map(|l| l.replace('_', "-"))
        .unwrap_or_else(|| "en".to_string())
}

pub fn default_show_tray_icon() -> bool {
    true
}

pub fn default_post_process_provider_id() -> String {
    "custom".to_string()
}

pub fn default_post_process_providers() -> Vec<PostProcessProvider> {
    let mut providers = vec![
        PostProcessProvider {
            id: "openai".to_string(),
            label: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: true,
        },
        PostProcessProvider {
            id: "zai".to_string(),
            label: "Z.AI".to_string(),
            base_url: "https://api.z.ai/api/paas/v4".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: true,
        },
        PostProcessProvider {
            id: "openrouter".to_string(),
            label: "OpenRouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: true,
        },
        PostProcessProvider {
            id: "anthropic".to_string(),
            label: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: false,
        },
        PostProcessProvider {
            id: "groq".to_string(),
            label: "Groq".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: false,
        },
        PostProcessProvider {
            id: "cerebras".to_string(),
            label: "Cerebras".to_string(),
            base_url: "https://api.cerebras.ai/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_structured_output: true,
        },
    ];

    // Note: We always include Apple Intelligence on macOS ARM64 without checking availability
    // at startup. The availability check is deferred to when the user actually tries to use it
    // (in actions.rs). This prevents crashes on macOS 26.x beta where accessing
    // SystemLanguageModel.default during early app initialization causes SIGABRT.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        providers.push(PostProcessProvider {
            id: APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
            label: "Apple Intelligence".to_string(),
            base_url: "apple-intelligence://local".to_string(),
            allow_base_url_edit: false,
            models_endpoint: None,
            supports_structured_output: true,
        });
    }

    // AWS Bedrock via Mantle (OpenAI-compatible endpoint)
    providers.push(PostProcessProvider {
        id: "bedrock_mantle".to_string(),
        label: "AWS Bedrock (Mantle)".to_string(),
        base_url: "https://bedrock-mantle.us-east-1.api.aws/v1".to_string(),
        allow_base_url_edit: false,
        models_endpoint: Some("/models".to_string()),
        supports_structured_output: true,
    });

    // Custom provider: a local LLM via a LiteLLM-style gateway (or any
    // OpenAI-compatible endpoint). id stays "custom" — load-bearing for stored configs.
    providers.push(PostProcessProvider {
        id: "custom".to_string(),
        label: "Local LLM (LiteLLM)".to_string(),
        base_url: "http://localhost:4000/v1".to_string(),
        allow_base_url_edit: true,
        models_endpoint: Some("/models".to_string()),
        supports_structured_output: true,
    });

    providers
}

pub fn default_post_process_api_keys() -> SecretMap {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(provider.id, String::new());
    }
    SecretMap(map)
}

pub fn default_model_for_provider(provider_id: &str) -> String {
    if provider_id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string();
    }
    if provider_id == "custom" {
        return "qwen2.5-coder-32b-instruct".to_string(); // SOTA local model fallback
    }
    String::new()
}

pub fn default_post_process_models() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(
            provider.id.clone(),
            default_model_for_provider(&provider.id),
        );
    }
    map
}

pub fn default_post_process_prompts() -> Vec<LLMPrompt> {
    vec![
        LLMPrompt {
            id: "semantic-smoothing".to_string(),
            name: "Semantic Smoothing".to_string(),
            prompt: "You are a professional transcription editor. Fix grammar, punctuation, and remove disfluencies (um, uh, fillers, repeated words, э-э, ну, типа) from the following text. PRESERVE THE ORIGINAL LANGUAGE (Russian or English) strictly. Output ONLY the corrected text without any chat or explanation: ${output}".to_string(),
        },
        LLMPrompt {
            id: "default_improve_transcriptions".to_string(),
            name: "Improve Transcriptions".to_string(),
            prompt: "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors\n2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)\n3. Replace spoken punctuation with symbols (period → ., comma → ,, question mark → ?)\n4. Remove filler words (um, uh, like as filler)\n5. Keep the language in the original version (if it was french, keep it in french for example)\n\nPreserve exact meaning and word order. Do not paraphrase or reorder content.\n\nReturn only the cleaned transcript.\n\nTranscript:\n${output}".to_string(),
        }
    ]
}

pub fn default_whisper_gpu_device() -> i32 {
    -1 // auto
}

pub fn default_typing_tool() -> TypingTool {
    TypingTool::Auto
}

pub const SUPERTONIC_VOICES: &[&str] =
    &["M1", "M2", "M3", "M4", "M5", "F1", "F2", "F3", "F4", "F5"];

pub fn default_local_tts_engine() -> String {
    "supertonic".to_string()
}

pub fn default_supertonic_voice() -> String {
    "M1".to_string()
}

pub fn default_fish_tts_model() -> String {
    "s2.1-pro-free".to_string()
}

impl AppSettings {
    /// The glossary used to steer/correct transcription: the user's custom words
    /// plus the built-in developer dictionary when `dev_dictionary_enabled`.
    pub fn effective_custom_words(&self) -> Vec<String> {
        if self.dev_dictionary_enabled {
            let mut words = self.custom_words.clone();
            words.extend(DEV_DICTIONARY.iter().map(|w| w.to_string()));
            words
        } else {
            self.custom_words.clone()
        }
    }

    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
    }

    pub fn post_process_provider(&self, provider_id: &str) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn post_process_provider_mut(
        &mut self,
        provider_id: &str,
    ) -> Option<&mut PostProcessProvider> {
        self.post_process_providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
    }
}

/// Filename for a backup of an unreadable settings store. When `AppSettings`
/// fails to deserialize (e.g. a future incompatible enum rename), the app backs
/// the raw JSON up under this name next to the live store *before* overwriting
/// it with defaults — so a parse failure never silently wipes the user's real
/// settings. Pure (no I/O), so it stays testable in this Tauri/I-O-free crate;
/// the caller (`settings.rs`) performs the actual write.
pub fn corrupt_settings_backup_filename(unix_secs: u64) -> String {
    format!("settings_store.corrupt-{unix_secs}.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_backup_filename_is_timestamped_sibling() {
        let name = corrupt_settings_backup_filename(1_720_000_000);
        assert_eq!(name, "settings_store.corrupt-1720000000.json");
        assert!(name.starts_with("settings_store.corrupt-"));
        assert!(name.ends_with(".json"));
    }

    #[test]
    fn default_input_gain_is_zero_db() {
        assert_eq!(default_input_gain(), 0.0);
    }

    #[test]
    fn default_monitor_volume_is_unity() {
        assert_eq!(default_monitor_volume(), 1.0);
    }

    #[test]
    fn default_monitor_output_device_is_none() {
        assert_eq!(default_monitor_output_device(), None);
    }

    #[test]
    fn voice_enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&VoiceTier::SidecarOnly).unwrap(),
            "\"sidecar_only\""
        );
        assert_eq!(
            serde_json::to_string(&TtsVoiceMode::Cloned).unwrap(),
            "\"cloned\""
        );
    }

    #[test]
    fn keyboard_implementation_accepts_legacy_handy_keys() {
        // Stores written before the handy_keys -> echo_keys rename (28ae421)
        // carry "handy_keys"; without an alias the whole AppSettings parse
        // fails and get_settings silently resets everything to defaults.
        let legacy: KeyboardImplementation = serde_json::from_str("\"handy_keys\"").unwrap();
        assert_eq!(legacy, KeyboardImplementation::EchoKeys);
        // Forward vocabulary unchanged: serializes as echo_keys, still parses.
        assert_eq!(
            serde_json::to_string(&KeyboardImplementation::EchoKeys).unwrap(),
            "\"echo_keys\""
        );
        let current: KeyboardImplementation = serde_json::from_str("\"echo_keys\"").unwrap();
        assert_eq!(current, KeyboardImplementation::EchoKeys);
    }

    #[test]
    fn snippet_serde_roundtrip() {
        let s = Snippet {
            trigger: "brb".into(),
            text: "be right back".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Snippet = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn empty_profiles_preserve_builtin_code_editor_behavior() {
        // Инвариант регресса: пока профилей нет, всё работает как раньше.
        let p = resolve_app_policy("main.rs — Visual Studio Code", &[]);
        assert!(p.strip_trailing_punct);
        assert!(p.camel_case_trigger);
        assert!(p.post_process_prompt_id.is_none());

        let p = resolve_app_policy("Telegram", &[]);
        assert!(!p.strip_trailing_punct);
        assert!(!p.camel_case_trigger);
    }

    #[test]
    fn profile_matches_case_insensitive_substring() {
        let profiles = vec![AppProfile {
            match_title: "telegram".into(),
            strip_trailing_punct: false,
            camel_case_trigger: false,
            post_process_prompt_id: Some("casual".into()),
        }];
        let p = resolve_app_policy("Saved Messages — TELEGRAM Desktop", &profiles);
        assert_eq!(p.post_process_prompt_id.as_deref(), Some("casual"));
    }

    #[test]
    fn first_matching_profile_wins_and_overrides_builtin() {
        // Профиль на code-редактор ДОЛЖЕН побеждать встроенный список,
        // иначе пользователь не сможет отключить срез пунктуации в своём редакторе.
        let profiles = vec![
            AppProfile {
                match_title: "cursor".into(),
                strip_trailing_punct: false,
                camel_case_trigger: false,
                post_process_prompt_id: Some("code".into()),
            },
            AppProfile {
                match_title: "cursor".into(),
                strip_trailing_punct: true,
                camel_case_trigger: true,
                post_process_prompt_id: Some("late".into()),
            },
        ];
        let p = resolve_app_policy("lib.rs — Cursor", &profiles);
        assert!(
            !p.strip_trailing_punct,
            "первый совпавший профиль должен победить"
        );
        assert_eq!(p.post_process_prompt_id.as_deref(), Some("code"));
    }

    #[test]
    fn effective_prompt_id_falls_back_when_profile_id_is_unknown() {
        let prompts = vec![LLMPrompt {
            id: "global".into(),
            name: "G".into(),
            prompt: "g".into(),
        }];
        // Профиль указывает на исчезнувший промпт -> работаем по глобальному,
        // а не выключаем LLM-шаг молча.
        assert_eq!(
            effective_prompt_id(Some("gone"), Some("global"), &prompts).as_deref(),
            Some("global")
        );
        // Известный id профиля побеждает глобальный.
        let prompts2 = vec![
            LLMPrompt {
                id: "global".into(),
                name: "G".into(),
                prompt: "g".into(),
            },
            LLMPrompt {
                id: "code".into(),
                name: "C".into(),
                prompt: "c".into(),
            },
        ];
        assert_eq!(
            effective_prompt_id(Some("code"), Some("global"), &prompts2).as_deref(),
            Some("code")
        );
        // Нет ни того, ни другого -> шага нет.
        assert_eq!(effective_prompt_id(Some("gone"), None, &prompts), None);
        assert_eq!(effective_prompt_id(None, None, &prompts), None);
    }

    #[test]
    fn blank_match_title_never_matches() {
        // Пустая строка — подстрока чего угодно; такой профиль стал бы
        // молчаливым «правилом на всё». Игнорируем его.
        let profiles = vec![AppProfile {
            match_title: "   ".into(),
            strip_trailing_punct: true,
            camel_case_trigger: true,
            post_process_prompt_id: Some("x".into()),
        }];
        let p = resolve_app_policy("Telegram", &profiles);
        assert!(!p.strip_trailing_punct);
        assert!(p.post_process_prompt_id.is_none());
    }

    #[test]
    fn app_profiles_default_on_old_settings_json() {
        let json = serde_json::json!({
            "bindings": {}, "push_to_talk": false, "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(json).unwrap();
        assert!(s.app_profiles.is_empty());
    }

    #[test]
    fn update_checks_default_off() {
        let json = serde_json::json!({
            "bindings": {}, "push_to_talk": false, "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(json).unwrap();
        assert!(!s.update_checks_enabled);
    }

    #[test]
    fn ui_theme_defaults_to_system() {
        let json = serde_json::json!({
            "bindings": {}, "push_to_talk": false, "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(json).unwrap();
        assert_eq!(s.ui_theme, "system");
    }

    #[test]
    fn secret_map_debug_redacts_values() {
        let map = SecretMap(HashMap::from([("key".into(), "secret".into())]));
        let out = format!("{:?}", map);
        assert!(!out.contains("secret"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn secret_string_debug_redacts() {
        let s = SecretString("sk-fish-XYZ".into());
        assert!(!format!("{s:?}").contains("XYZ"));
        assert_eq!(&*s, "sk-fish-XYZ");
    }

    #[test]
    fn fish_fields_default_on_old_settings_json() {
        // Старый settings_store.json без fish-полей обязан десериализоваться.
        let json = serde_json::json!({
            "bindings": {}, "push_to_talk": false, "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(json).unwrap();
        assert_eq!(&*s.fish_api_key, "");
        assert_eq!(s.fish_model_id, "");
        assert_eq!(s.fish_tts_model, "s2.1-pro-free");
    }

    #[test]
    fn app_settings_serde_roundtrip() {
        // AppSettings derives neither Default nor PartialEq (kept verbatim so
        // bindings don't change), so build a minimal value — only the fields
        // without #[serde(default)] are required — and compare round-trips via
        // serde_json::Value.
        let min = serde_json::json!({
            "bindings": {},
            "push_to_talk": true,
            "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(min).unwrap();
        let v1 = serde_json::to_value(&s).unwrap();
        let back: AppSettings = serde_json::from_value(v1.clone()).unwrap();
        assert_eq!(v1, serde_json::to_value(&back).unwrap());
    }

    #[test]
    fn local_tts_defaults() {
        let min = serde_json::json!({
            "bindings": {},
            "push_to_talk": true,
            "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(min).unwrap();
        assert_eq!(s.local_tts_engine, "supertonic");
        assert_eq!(s.supertonic_voice, "M1");
    }

    #[test]
    fn old_settings_json_without_new_fields_still_parses() {
        // Simulate old settings without the new fields.
        let v: serde_json::Value = serde_json::json!({
            "bindings": {},
            "push_to_talk": true,
            "audio_feedback": false,
            "external_script_path": null
        });
        // Explicitly don't include local_tts_engine and supertonic_voice
        let s: AppSettings = serde_json::from_value(v).unwrap();
        assert_eq!(s.local_tts_engine, "supertonic");
        assert_eq!(s.supertonic_voice, "M1");
    }

    #[test]
    fn old_settings_json_defaults_notes_capture_protection_to_false() {
        let v = serde_json::json!({
            "bindings": {},
            "push_to_talk": true,
            "audio_feedback": false,
            "external_script_path": null
        });
        let s: AppSettings = serde_json::from_value(v).unwrap();
        assert!(!s.hide_notes_from_screen_share);
    }

    #[test]
    fn supertonic_voices_list_is_ten() {
        assert_eq!(SUPERTONIC_VOICES.len(), 10);
        assert!(SUPERTONIC_VOICES.contains(&"M1"));
        assert!(SUPERTONIC_VOICES.contains(&"F5"));
    }
}
