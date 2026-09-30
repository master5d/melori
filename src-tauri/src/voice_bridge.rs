//! Builds an echo_voice::VoiceRouter from app paths + settings. echo-voice
//! knows no AppHandle; this is the one place that maps app state into it.
use echo_config::AppSettings;
use echo_voice::router::VoiceRouter;
use echo_voice::sidecar::{SidecarConfig, SidecarRemote};
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Manager;

pub fn sidecar_base_url() -> String {
    String::new()
}

/// Find the single `*.onnx` file (not `*.onnx.json`) and its sibling `*.onnx.json`
/// inside `dir`. Returns `None` if the dir is absent or the files are missing.
fn find_piper_model(dir: &std::path::Path) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    if !dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let onnx = entries.flatten().find_map(|e| {
        let path = e.path();
        let name = path.file_name()?.to_str()?;
        // Accept .onnx but not .onnx.json
        if name.ends_with(".onnx") && !name.ends_with(".onnx.json") {
            Some(path)
        } else {
            None
        }
    })?;
    // Sibling json is the onnx path + ".json" (e.g. model.onnx → model.onnx.json)
    let json_path = {
        let mut p = onnx.clone().into_os_string();
        p.push(".json");
        std::path::PathBuf::from(p)
    };
    if json_path.exists() {
        Some((onnx, json_path))
    } else {
        None
    }
}

/// Attempt to construct the local Piper TTS engine from bundled espeak-ng and
/// downloaded piper model directories. Returns `None` if espeak is not bundled,
/// no model dirs are present, or engine initialization fails.
fn build_piper_engine(app: &AppHandle) -> Option<Box<dyn echo_voice::VoiceEngine>> {
    let espeak_bin = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng.exe",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()?;
    let espeak_data = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng-data",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()?;

    let models_dir = crate::portable::app_data_dir(app).ok()?.join("models");

    let ru = find_piper_model(&models_dir.join("vits-piper-ru_RU-irina-medium"));
    let en = find_piper_model(&models_dir.join("vits-piper-en_US-amy-medium"));

    if ru.is_none() && en.is_none() {
        return None;
    }

    match echo_voice_local::LocalTtsEngine::from_lang_paths(ru, en, &espeak_bin, &espeak_data) {
        Ok(e) => Some(Box::new(e)),
        Err(e) => {
            log::info!("local TTS engine unavailable: {e}");
            None
        }
    }
}

/// Attempt to construct the local Kitten TTS engine (B-24, EN-only) from the
/// downloaded `kitten-tts-mini-en` bundle. EN-only by design: the engine itself
/// refuses Ru/Mixed, so it MUST NOT be the sole local engine for RU flows —
/// `build_local_engine` keeps Piper/Supertonic as the fallback.
fn build_kitten_engine(app: &AppHandle) -> Option<Box<dyn echo_voice::VoiceEngine>> {
    let espeak_bin = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng.exe",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()?;
    let espeak_data = app
        .path()
        .resolve(
            "resources/tts/espeak-ng/espeak-ng-data",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()?;
    let model_dir = crate::portable::app_data_dir(app)
        .ok()?
        .join("models")
        .join("kitten-tts-mini-en");
    if !model_dir.join("model.onnx").exists() {
        return None;
    }
    match echo_voice_local::kitten::KittenEngine::load(
        &model_dir,
        &espeak_bin,
        &espeak_data,
        echo_voice_local::kitten::DEFAULT_KITTEN_VOICE,
    ) {
        Ok(e) => Some(Box::new(e)),
        Err(e) => {
            log::info!("kitten local TTS unavailable: {e}");
            None
        }
    }
}

/// Attempt to construct the local Supertonic TTS engine from the downloaded
/// `supertonic-3` model directory. Returns `None` if the model files are
/// missing or engine initialization fails.
fn build_supertonic_engine(
    app: &AppHandle,
    settings: &AppSettings,
) -> Option<Box<dyn echo_voice::VoiceEngine>> {
    let base = crate::portable::app_data_dir(app)
        .ok()?
        .join("models")
        .join("supertonic-3");
    let onnx_dir = base.join("onnx");
    let voice = if echo_config::SUPERTONIC_VOICES.contains(&settings.supertonic_voice.as_str()) {
        settings.supertonic_voice.clone()
    } else {
        log::warn!(
            "invalid supertonic_voice '{}', using {}",
            settings.supertonic_voice,
            echo_config::default_supertonic_voice()
        );
        echo_config::default_supertonic_voice()
    };
    let style = base.join("voice_styles").join(format!("{voice}.json"));
    match echo_voice_supertonic::SupertonicEngine::load(&onnx_dir, &style) {
        Ok(e) => Some(Box::new(e)),
        Err(e) => {
            log::info!("supertonic local TTS unavailable: {e}");
            None
        }
    }
}

/// Два локальных движка как один: primary → per-call fallback. Нужен ровно
/// потому, что слот local у роутера ОДИН, а Kitten — EN-only и отказывает
/// Ru/Mixed НА ВЫЗОВЕ: без per-call фолбэка выбор kitten ронял бы русскую
/// локалку в SAPI. `or_else` при построении такое не чинит — он срабатывает,
/// только если primary не ЗАГРУЗИЛСЯ.
struct ChainedLocal {
    primary: Box<dyn echo_voice::VoiceEngine>,
    fallback: Box<dyn echo_voice::VoiceEngine>,
}

impl echo_voice::VoiceEngine for ChainedLocal {
    fn id(&self) -> &'static str {
        self.primary.id()
    }
    fn synthesize(
        &self,
        text: &str,
        profile: &echo_voice::VoiceProfile,
        opts: &echo_voice::SynthOpts,
    ) -> Result<Vec<u8>, echo_voice::VoiceError> {
        match self.primary.synthesize(text, profile, opts) {
            Ok(b) => Ok(b),
            Err(e) => {
                log::debug!(
                    "local primary {} failed ({e}), trying fallback",
                    self.primary.id()
                );
                self.fallback.synthesize(text, profile, opts)
            }
        }
    }
}

/// Build the local-tier engine per `settings.local_tts_engine` (supertonic or
/// piper), falling back to the other engine if the chosen one fails to
/// construct. Returns `None` if neither is available (caller keeps SAPI).
fn build_local_engine(
    app: &AppHandle,
    settings: &AppSettings,
) -> Option<Box<dyn echo_voice::VoiceEngine>> {
    let engine = settings.local_tts_engine.as_str();
    if engine != "supertonic" && engine != "piper" && engine != "kitten" {
        log::warn!("invalid local_tts_engine '{engine}', using supertonic");
    }
    // Chosen engine first; fallback engine per-call (ChainedLocal); None -> SAPI.
    let (primary, fallback) = match engine {
        "piper" => (
            build_piper_engine(app),
            build_supertonic_engine(app, settings),
        ),
        // Kitten EN-only: многоязычный движок ОБЯЗАН стоять за ним per-call.
        "kitten" => (
            build_kitten_engine(app),
            build_supertonic_engine(app, settings).or_else(|| build_piper_engine(app)),
        ),
        _ => (
            build_supertonic_engine(app, settings),
            build_piper_engine(app),
        ),
    };
    match (primary, fallback) {
        (Some(p), Some(f)) => Some(Box::new(ChainedLocal {
            primary: p,
            fallback: f,
        })),
        (Some(p), None) => Some(p),
        (None, f) => f,
    }
}

/// Build a router. Returns None if no engine could be constructed (caller keeps SAPI).
pub fn build_router(app: &AppHandle, settings: &AppSettings) -> Option<VoiceRouter> {
    // Local TTS engine: supertonic or piper per settings, with cross-fallback.
    let local: Option<Box<dyn echo_voice::VoiceEngine>> = build_local_engine(app, settings);

    // The sidecar is opt-in. An empty base URL means the public build has no
    // remote voice endpoint configured.
    let sidecar_cfg = SidecarConfig {
        base_url: sidecar_base_url(),
        timeout_secs: 30,
    };
    let (sidecar, health): (Option<Box<dyn echo_voice::VoiceEngine>>, _) =
        match if sidecar_cfg.base_url.is_empty() {
            None
        } else {
            SidecarRemote::new(sidecar_cfg).ok()
        } {
            Some(s) => {
                let probe = Arc::new(s);
                let probe2 = probe.clone();
                let health: Box<dyn Fn() -> bool + Send + Sync> = Box::new(move || probe2.health());
                (Some(Box::new(ArcEngine(probe))), health)
            }
            None => (
                None,
                Box::new(|| false) as Box<dyn Fn() -> bool + Send + Sync>,
            ),
        };

    // Fish cloud tier: active iff key AND model id are set. Errors at call time
    // fall through to sidecar/local via the router.
    let cloud: Option<Box<dyn echo_voice::VoiceEngine>> =
        if !settings.fish_api_key.is_empty() && !settings.fish_model_id.is_empty() {
            match echo_voice::fish::FishRemote::new(echo_voice::fish::FishConfig {
                base_url: "https://api.fish.audio".to_string(),
                api_key: settings.fish_api_key.0.clone(),
                model_id: settings.fish_model_id.clone(),
                tts_model: settings.fish_tts_model.clone(),
                timeout_secs: 30,
            }) {
                Ok(e) => Some(Box::new(e)),
                Err(e) => {
                    log::warn!("fish cloud tier unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };

    if local.is_none() && sidecar.is_none() && cloud.is_none() {
        return None;
    }
    Some(VoiceRouter::new(
        settings.voice_tier,
        cloud,
        local,
        sidecar,
        health,
    ))
}

/// Adapter so an Arc<SidecarRemote> can be a boxed VoiceEngine while the same
/// Arc backs the health closure.
struct ArcEngine(Arc<SidecarRemote>);

impl echo_voice::VoiceEngine for ArcEngine {
    fn id(&self) -> &'static str {
        self.0.id()
    }

    fn synthesize(
        &self,
        text: &str,
        profile: &echo_voice::VoiceProfile,
        opts: &echo_voice::SynthOpts,
    ) -> Result<Vec<u8>, echo_voice::VoiceError> {
        self.0.synthesize(text, profile, opts)
    }
}

/// (Re)build the cloned-voice engine from current settings. Called at startup
/// and whenever a fish_* setting changes.
pub fn install_cloned_engine(app_handle: &AppHandle) {
    let voice_settings = crate::settings::get_settings(app_handle);
    let mut engine = None;
    if matches!(
        voice_settings.tts_voice_mode,
        echo_config::TtsVoiceMode::Cloned
    ) {
        if let Some(profile_id) = voice_settings.tts_voice_profile.clone() {
            if let Some(router) = build_router(app_handle, &voice_settings) {
                let vdir = crate::portable::app_data_dir(app_handle)
                    .map(|d| d.join("voices"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("voices"));
                engine = Some(crate::tts::cloned::ClonedVoiceEngine::new(
                    router, vdir, profile_id,
                ));
            }
        }
    }
    if let Some(tts) = app_handle.try_state::<std::sync::Arc<crate::tts::TtsManager>>() {
        tts.set_cloned_engine(engine);
    }
}

/// (Re)build the local readback floor from current settings. Called at startup
/// and whenever local_tts_engine / supertonic_voice change.
pub fn install_local_readback(app_handle: &AppHandle) {
    let voice_settings = crate::settings::get_settings(app_handle);
    let mut local_settings = voice_settings.clone();
    local_settings.voice_tier = echo_config::VoiceTier::LocalOnly;
    let engine = crate::voice_bridge::build_router(app_handle, &local_settings).map(|router| {
        let default_language = match voice_settings.selected_language.as_str() {
            "ru" => echo_voice::LanguageHint::Ru,
            "en" => echo_voice::LanguageHint::En,
            _ => echo_voice::LanguageHint::Mixed,
        };
        crate::tts::local_readback::LocalReadbackEngine::new(router, default_language)
    });
    if let Some(tts) = app_handle.try_state::<std::sync::Arc<crate::tts::TtsManager>>() {
        tts.set_local_engine(engine);
    }
}
