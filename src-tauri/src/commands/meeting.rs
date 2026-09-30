//! Meeting Copilot (C1.4): start/stop/status Tauri commands + segment/state events,
//! plus the real worker-thread capture→VAD→ASR→session glue.
//!
//! Real-time discipline: the cpal `on_frame` callback (audio thread) ONLY sends a
//! `Vec<f32>` over an mpsc channel. A dedicated worker thread PER SOURCE consumes that
//! channel and runs the `Utterancer` + ASR loop, appending to the session and emitting
//! `SegmentEvent`. VAD/ASR never run on the audio thread.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::meeting::analyze::{
    format_transcript, is_local_url, parse_analysis, prompt_for, schema_for, AnalysisMode,
    MeetingAnalysis,
};
use crate::meeting::capture::{start_capture, CaptureHandle};
use crate::meeting::session::{LiveTranscript, MeetingSession, Segment};
use crate::meeting::transcribe::{MeetingAsr, Utterancer, VadGate};
use crate::meeting::Source;
use crate::settings::{get_settings, write_settings};

// ── Engine / frame sizing ──────────────────────────────────────────────────────────
// SileroVad requires EXACTLY 480 samples per frame (16000 Hz * 30 ms / 1000). The capture
// layer resamples to `ENGINE_HZ` and re-chunks to `FRAME_SAMPLES`, so these MUST line up.
const ENGINE_HZ: usize = 16_000;
const FRAME_SAMPLES: usize = 480; // 30 ms @ 16 kHz — Silero's fixed frame size
const FRAME_MS: u64 = 30;
/// ~540 ms of trailing silence closes an utterance (18 * 30 ms).
const HANGOVER_FRAMES: usize = 18;

// ── Pure guards (TDD'd) ─────────────────────────────────────────────────────────────

/// Reject a double-start; otherwise begin the session. All branching lives here.
pub fn guard_start(
    sess: &MeetingSession,
    started_at: String,
    loopback_active: bool,
) -> Result<(), String> {
    if sess.is_active() {
        return Err("a meeting is already active".into());
    }
    sess.start(started_at, loopback_active);
    Ok(())
}

/// Finish the session (safe even when inactive — returns the final, deactivated transcript).
pub fn guard_stop(sess: &MeetingSession) -> LiveTranscript {
    sess.finish()
}

/// Snapshot the current state into a `MeetingStatus`.
pub fn status_of(sess: &MeetingSession) -> MeetingStatus {
    let snap = sess.snapshot();
    MeetingStatus {
        active: snap.active,
        loopback_active: snap.loopback_active,
        segment_count: snap.segments.len() as u32,
    }
}

// ── Event payloads + status (mirror managers::history event style) ──────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SegmentEvent {
    pub segment: Segment,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MeetingStateEvent {
    pub active: bool,
    pub loopback_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MeetingVoiceEvent {
    pub source: Source,
    pub speaking: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MeetingSilenceEvent {
    pub silent_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MeetingShortcutEvent {
    pub action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingStatus {
    pub active: bool,
    pub loopback_active: bool,
    pub segment_count: u32,
}

// ── Real ASR adapter: delegates to the loaded streaming engine via TranscriptionManager ──
//
// `TranscriptionManager` is `Clone` + `Send` + `Sync` (all fields are `Arc<…>`), and it
// already serializes engine access behind its own mutex — so a single shared adapter is
// safe to share across both worker threads. `.transcribe(audio)` runs whichever engine the
// user loaded (MoonshineStreaming included) and returns the final text.
struct ManagerAsr {
    manager: Arc<crate::managers::transcription::TranscriptionManager>,
}

impl MeetingAsr for ManagerAsr {
    fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<String> {
        // The meeting never went through dictation, so nothing has loaded the model (and an
        // idle unload can drop it mid-meeting). Returning "" here dropped every utterance
        // without a trace; instead start the load — a no-op while loaded or loading — and let
        // `transcribe` wait for it.
        if !self.manager.is_model_loaded() {
            self.manager.initiate_model_load();
        }
        self.manager.transcribe(pcm.to_vec())
    }
}

// ── Real VAD adapter: wraps SmoothedVad(SileroVad) and exposes the boolean decision ──
struct SmoothedVadGate {
    inner: crate::audio_toolkit::vad::SmoothedVad,
}

impl VadGate for SmoothedVadGate {
    fn is_voice(&mut self, frame: &[f32]) -> bool {
        // A wrong-size frame or model error is treated as silence (never panics the worker).
        use crate::audio_toolkit::VoiceActivityDetector;
        self.inner.is_voice(frame).unwrap_or(false)
    }
}

/// Build one VAD gate (one per source). Mirrors the dictation path's SmoothedVad construction.
fn build_vad(app: &AppHandle) -> Result<Box<dyn VadGate + Send>, String> {
    let vad_path = app
        .path()
        .resolve(
            "resources/models/silero_vad_v4.onnx",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| format!("failed to resolve VAD model path: {e}"))?;
    let silero = crate::audio_toolkit::SileroVad::new(
        vad_path.to_str().ok_or("VAD path is not valid UTF-8")?,
        0.3,
    )
    .map_err(|e| format!("failed to create SileroVad: {e}"))?;
    // (prefill, hangover, onset) mirror managers::audio::create_audio_recorder.
    let smoothed = crate::audio_toolkit::vad::SmoothedVad::new(Box::new(silero), 15, 15, 2);
    Ok(Box::new(SmoothedVadGate { inner: smoothed }))
}

// ── MeetingRuntime: managed state holding the live capture + worker threads ──────────

struct RunningCapture {
    capture: CaptureHandle,
    workers: Vec<JoinHandle<()>>,
    timer_stop: Arc<AtomicBool>,
    timer: JoinHandle<()>,
    silence: Arc<Mutex<melori_consult::silence::SilenceTimer>>,
    started: Instant,
}

#[derive(Default)]
pub struct MeetingRuntime {
    running: Mutex<Option<RunningCapture>>,
}

/// Worker implementation kept separate to make the cpal callback's contract obvious.
fn spawn_worker(
    app: AppHandle,
    source: Source,
    rx: mpsc::Receiver<Vec<f32>>,
    vad: Box<dyn VadGate + Send>,
    asr: Arc<ManagerAsr>,
    silence: Arc<Mutex<melori_consult::silence::SilenceTimer>>,
    started: Instant,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let sess = app.state::<MeetingSession>();
        let mut u = Utterancer::new(vad, FRAME_SAMPLES, FRAME_MS, HANGOVER_FRAMES);
        let mut edge = melori_consult::voice::VoiceEdge::default();
        for frame in rx {
            let closed = u.push(&frame);
            if let Some(speaking) = edge.update(u.speaking()) {
                let _ = (MeetingVoiceEvent { source, speaking }).emit(&app);
            }
            if u.speaking() {
                silence
                    .lock()
                    .unwrap()
                    .on_voice(started.elapsed().as_millis() as u64);
            }
            if let Some((s, e, pcm)) = closed {
                emit_utterance(&app, &sess, source, asr.as_ref(), s, e, pcm);
            }
        }
        if let Some((s, e, pcm)) = u.flush() {
            emit_utterance(&app, &sess, source, asr.as_ref(), s, e, pcm);
        }
    })
}

fn emit_utterance(
    app: &AppHandle,
    sess: &MeetingSession,
    source: Source,
    asr: &dyn MeetingAsr,
    s: u64,
    e: u64,
    pcm: Vec<f32>,
) {
    match asr.transcribe(&pcm) {
        Ok(text) if !text.trim().is_empty() => {
            let id = sess.append(source, s, e, text.clone());
            if id != 0 {
                // build the event from what was appended — no whole-session snapshot per utterance
                let segment = Segment {
                    id,
                    source,
                    speaker: None,
                    start_ms: s,
                    end_ms: e,
                    text,
                };
                if let Err(err) = (SegmentEvent { segment }).emit(app) {
                    log::error!("meeting: failed to emit SegmentEvent: {err}");
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::error!("meeting asr failed on a chunk (dropped): {e}"),
    }
}

/// Start the real capture pipeline. Returns whether system-audio (loopback) capture is active.
fn start_meeting_capture(app: &AppHandle) -> Result<bool, String> {
    let runtime = app.state::<MeetingRuntime>();
    let mut guard = runtime.running.lock().unwrap();
    if guard.is_some() {
        return Err("capture already running".into());
    }

    // One shared ASR adapter (TranscriptionManager serializes engine access internally).
    let manager = app
        .try_state::<Arc<crate::managers::transcription::TranscriptionManager>>()
        .ok_or("transcription manager not initialized")?
        .inner()
        .clone();
    // warm the recognition model while the first utterance is still being spoken
    manager.initiate_model_load();
    let asr = Arc::new(ManagerAsr { manager });

    // One VAD + one channel + one worker per source.
    let started = Instant::now();
    let silence = Arc::new(Mutex::new(melori_consult::silence::SilenceTimer::new(
        crate::settings::get_settings(app).auto_stop_silence_min as u64 * 60_000,
    )));
    silence.lock().unwrap().on_voice(0);
    let (me_tx, me_rx) = mpsc::channel::<Vec<f32>>();
    let (others_tx, others_rx) = mpsc::channel::<Vec<f32>>();

    let me_vad = build_vad(app)?;
    let others_vad = build_vad(app)?;

    let mut workers = Vec::new();
    workers.push(spawn_worker(
        app.clone(),
        Source::Me,
        me_rx,
        me_vad,
        asr.clone(),
        silence.clone(),
        started,
    ));
    workers.push(spawn_worker(
        app.clone(),
        Source::Others,
        others_rx,
        others_vad,
        asr.clone(),
        silence.clone(),
        started,
    ));

    // cpal callback (audio thread): ONLY forward frames over the channel. Cheap + lock-free.
    let on_frame = move |source: Source, frame: &[f32]| {
        let tx = match source {
            Source::Me => &me_tx,
            Source::Others => &others_tx,
        };
        let _ = tx.send(frame.to_vec());
    };

    let mic_name = crate::settings::get_settings(app).selected_microphone;
    let capture = match start_capture(ENGINE_HZ, FRAME_SAMPLES, mic_name.as_deref(), on_frame) {
        Ok(c) => c,
        Err(e) => {
            // Dropping the channels' Senders (held by `on_frame`, dropped on error here)
            // closes the receivers so the just-spawned workers flush and exit.
            for w in workers {
                let _ = w.join();
            }
            return Err(format!("failed to start capture: {e}"));
        }
    };

    let loopback_active = capture.loopback_active;
    let timer_stop = Arc::new(AtomicBool::new(false));
    let timer_flag = timer_stop.clone();
    let timer_app = app.clone();
    let timer_silence = silence.clone();
    let timer = std::thread::spawn(move || {
        while !timer_flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            if timer_flag.load(Ordering::Relaxed) {
                break;
            }
            if let Some(silent_ms) = timer_silence
                .lock()
                .unwrap()
                .tick(started.elapsed().as_millis() as u64)
            {
                let _ = (MeetingSilenceEvent { silent_ms }).emit(&timer_app);
                crate::meeting::window::show_meeting_copilot(&timer_app);
            }
        }
    });
    *guard = Some(RunningCapture {
        capture,
        workers,
        timer_stop,
        timer,
        silence,
        started,
    });
    Ok(loopback_active)
}

/// Stop the capture pipeline. Idempotent: a no-op when nothing is running.
fn stop_meeting_capture(app: &AppHandle) {
    let runtime = app.state::<MeetingRuntime>();
    let running = { runtime.running.lock().unwrap().take() };
    if let Some(RunningCapture {
        capture,
        workers,
        timer_stop,
        timer,
        ..
    }) = running
    {
        timer_stop.store(true, Ordering::Relaxed);
        // Drop the CaptureHandle first → audio streams stop → no more sends → the per-source
        // channels close → each worker's `rx.into_iter()` ends, flushes its trailing partial,
        // and the thread exits.
        drop(capture);
        for w in workers {
            if let Err(e) = w.join() {
                log::warn!("meeting: worker thread join failed: {e:?}");
            }
        }
        let _ = timer.join();
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn start_meeting(app: AppHandle, sess: State<'_, MeetingSession>) -> Result<(), String> {
    start_meeting_inner(&app, &sess).await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_meeting(
    app: AppHandle,
    sess: State<'_, MeetingSession>,
    binding: State<'_, melori_consult::client_meeting::ClientBinding>,
) -> Result<LiveTranscript, String> {
    stop_meeting_inner(&app, &sess, &binding).await
}

pub async fn stop_meeting_inner(
    app: &AppHandle,
    sess: &MeetingSession,
    binding: &melori_consult::client_meeting::ClientBinding,
) -> Result<LiveTranscript, String> {
    stop_meeting_capture(app);
    let marks = sess.marks();
    let fin = guard_stop(sess);
    if binding.get().is_some() {
        let engine = app.state::<Arc<crate::consult::engine::EngineManager>>();
        let segments = crate::consult::client_meeting::segments_json(&fin.segments, &marks);
        let duration_ms = fin.segments.iter().map(|s| s.end_ms).max().unwrap_or(0);
        if let Err(error) = crate::consult::client_meeting::push_transcript(
            &engine,
            binding,
            &segments,
            duration_ms,
        )
        .await
        {
            log::warn!("meeting: transcript upload failed for client session: {error}");
        }
    }
    let _ = MeetingStateEvent {
        active: false,
        loopback_active: fin.loopback_active,
    }
    .emit(app);
    Ok(fin)
}

pub async fn start_meeting_inner(app: &AppHandle, sess: &MeetingSession) -> Result<(), String> {
    let started_at = chrono::Utc::now().to_rfc3339();
    let loopback_active = start_meeting_capture(app)?;
    if let Err(e) = guard_start(sess, started_at, loopback_active) {
        stop_meeting_capture(app);
        return Err(e);
    }
    let _ = (MeetingStateEvent {
        active: true,
        loopback_active,
    })
    .emit(app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn generate_client_note(
    engine: State<'_, Arc<crate::consult::engine::EngineManager>>,
    binding: State<'_, melori_consult::client_meeting::ClientBinding>,
    sess: State<'_, MeetingSession>,
    template_id: String,
) -> Result<crate::consult::client_meeting::ClientNote, String> {
    let segments =
        crate::consult::client_meeting::segments_json(&sess.snapshot().segments, &sess.marks());
    crate::consult::client_meeting::generate_note(&engine, &binding, &template_id, Some(&segments))
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn ask_meeting(
    engine: State<'_, Arc<crate::consult::engine::EngineManager>>,
    binding: State<'_, melori_consult::client_meeting::ClientBinding>,
    sess: State<'_, MeetingSession>,
    question: String,
    kind: String,
) -> Result<crate::consult::client_meeting::AskView, String> {
    let snap = sess.snapshot();
    let segments = crate::consult::client_meeting::segments_json(&snap.segments, &sess.marks());
    crate::consult::client_meeting::ask(
        &engine,
        &binding,
        &question,
        &kind,
        &segments,
        sess.elapsed_ms(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub fn mark_moment(sess: State<'_, MeetingSession>) -> Result<u64, String> {
    if !sess.is_active() {
        return Err("no active meeting".into());
    }
    let elapsed = sess.elapsed_ms();
    sess.add_mark(elapsed);
    Ok(elapsed)
}

#[tauri::command]
#[specta::specta]
pub fn reset_silence(app: AppHandle) -> Result<(), String> {
    let runtime = app.state::<MeetingRuntime>();
    if let Some(running) = runtime.running.lock().unwrap().as_ref() {
        running
            .silence
            .lock()
            .unwrap()
            .reset(running.started.elapsed().as_millis() as u64);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_auto_stop_silence_setting(app: AppHandle, minutes: u32) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_stop_silence_min = minutes;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn meeting_status(sess: State<'_, MeetingSession>) -> MeetingStatus {
    status_of(&sess)
}

/// Reveal the copilot panel (create-once). Reveal only — never starts capture.
#[tauri::command]
#[specta::specta]
pub fn show_meeting_copilot(app: AppHandle) {
    crate::meeting::window::show_meeting_copilot(&app);
}

/// Hide the copilot panel (kept alive for re-reveal).
#[tauri::command]
#[specta::specta]
pub fn hide_meeting_copilot(app: AppHandle) {
    crate::meeting::window::hide_meeting_copilot(&app);
}

/// Persist that the user has acknowledged the one-time recording-consent notice.
#[tauri::command]
#[specta::specta]
pub fn set_meeting_consent_acked(app: AppHandle) {
    let mut settings = get_settings(&app);
    settings.meeting_consent_acked = true;
    write_settings(&app, settings);
}

/// Persist the wellbeing sidecar base URL (where the psych-council stream is consulted).
#[tauri::command]
#[specta::specta]
pub fn set_wellbeing_url(app: AppHandle, url: String) {
    let mut settings = get_settings(&app);
    settings.wellbeing_url = url;
    write_settings(&app, settings);
}

/// Privacy/destination info for the active LLM provider (no secrets).
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct LlmDestination {
    pub configured: bool,
    pub label: String,
    pub is_local: bool,
}

/// Empty replies are retried (reasoning models sometimes answer only in their reasoning).
const ANALYSIS_ATTEMPTS: u32 = 3;

/// Analyze the current transcript into a Brief (Business) or SOAP (Session).
/// Uses melori's LLM (Settings → Engine and language model); no transcript is persisted.
#[tauri::command]
#[specta::specta]
pub async fn analyze_meeting(
    app: AppHandle,
    sess: State<'_, MeetingSession>,
    mode: AnalysisMode,
) -> Result<MeetingAnalysis, String> {
    let transcript = format_transcript(&sess.snapshot().segments);
    if transcript.trim().is_empty() {
        return Err("nothing to analyze yet".into());
    }

    // melori's LLM (Settings → Engine and language model), not echo's post-processing
    // provider; a client meeting needs the client's `council` consent first.
    let settings = crate::settings::get_settings(&app);
    let (base_url, model) = melori_consult::analysis_policy::analysis_target(
        &settings.llm_base_url,
        &settings.llm_model,
    )?;
    let engine = app.state::<Arc<crate::consult::engine::EngineManager>>();
    let binding = app.state::<melori_consult::client_meeting::ClientBinding>();
    let permissions =
        crate::consult::client_meeting::bound_client_permissions(&engine, &binding).await?;
    melori_consult::analysis_policy::analysis_allowed(permissions.as_deref())?;
    let provider = echo_config::PostProcessProvider {
        id: "melori".into(),
        label: "melori".into(),
        base_url,
        allow_base_url_edit: false,
        models_endpoint: None,
        supports_structured_output: true,
    };
    // same key and timeout the engine gets (env, never stored in settings)
    let api_key = std::env::var("MELORI_LLM_API_KEY").unwrap_or_default();
    let timeout_secs = std::env::var("MELORI_LLM_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600);

    // write the note in the language of the session, not the prompt's English
    let language = melori_consult::language::transcript_language(&transcript);
    let system_prompt = format!(
        "{}\n\nWrite every field in {}, the language of the session.",
        prompt_for(mode),
        melori_consult::language::language_name(language)
    );
    let attempts = async {
        for attempt in 1..=ANALYSIS_ATTEMPTS {
            let reply = crate::llm_client::send_chat_completion_with_schema(
                &provider,
                api_key.clone(),
                &model,
                transcript.clone(),
                Some(system_prompt.clone()),
                Some(schema_for(mode)),
                None,
                None,
            )
            .await?;
            match reply {
                Some(json) if !json.trim().is_empty() => return Ok(json),
                _ => log::warn!("analysis: empty reply from the language model (attempt {attempt}/{ANALYSIS_ATTEMPTS})"),
            }
        }
        Err(format!(
            "empty analysis from LLM after {ANALYSIS_ATTEMPTS} attempts"
        ))
    };
    let json = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), attempts)
        .await
        .map_err(|_| format!("language model did not answer within {timeout_secs} s"))??;

    parse_analysis(mode, &json)
}

/// Where would an analysis be sent? (active provider label + locality)
#[tauri::command]
#[specta::specta]
pub fn meeting_llm_destination(app: AppHandle) -> LlmDestination {
    let settings = crate::settings::get_settings(&app);
    match melori_consult::analysis_policy::analysis_target(
        &settings.llm_base_url,
        &settings.llm_model,
    ) {
        Ok((base_url, model)) => LlmDestination {
            configured: true,
            label: format!("{model} ({base_url})"),
            is_local: is_local_url(&base_url),
        },
        Err(_) => LlmDestination {
            configured: false,
            label: String::new(),
            is_local: false,
        },
    }
}

/// Save the meeting as a markdown file under <capture_folder>/meetings/.
/// Deterministic metadata always written; LLM topical tags are best-effort (never block the save).
#[tauri::command]
#[specta::specta]
pub async fn save_meeting(
    app: AppHandle,
    sess: State<'_, MeetingSession>,
    mode: AnalysisMode,
    analysis: Option<MeetingAnalysis>,
) -> Result<String, String> {
    use crate::meeting::persist::{
        compute_meta, format_session_markdown, parse_tags, session_filename, tags_prompt,
        tags_schema,
    };

    let snap = sess.snapshot();
    let transcript = format_transcript(&snap.segments);
    if transcript.trim().is_empty() {
        return Err("nothing to save yet".into());
    }

    let settings = crate::settings::get_settings(&app);
    let folder = settings.capture_folder.trim().to_string();
    if folder.is_empty() {
        return Err("set a capture folder in Settings".into());
    }

    let now = chrono::Local::now();
    let mut meta = compute_meta(&snap, mode, now.to_rfc3339());

    // Best-effort tags — never blocks/fails the save.
    if let Some(provider) = settings.active_post_process_provider() {
        let api_key = settings
            .post_process_api_keys
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();
        let model = settings
            .post_process_models
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();
        if !model.is_empty() {
            if let Ok(Some(json)) = crate::llm_client::send_chat_completion_with_schema(
                provider,
                api_key,
                &model,
                transcript.clone(),
                Some(tags_prompt().to_string()),
                Some(tags_schema()),
                None,
                None,
            )
            .await
            {
                meta.tags = parse_tags(&json);
            }
        }
    }

    let md = format_session_markdown(&meta, &transcript, analysis.as_ref());
    let dir = std::path::Path::new(&folder).join("meetings");
    let name = session_filename(now, meta.meeting_type);
    let path = crate::capture::write_capture(&dir, &name, &md)
        .map_err(|e| format!("failed to write session: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Stream the wellbeing psychology council on `situation` (a SOAP, or the server transcript
/// when None). Emits a CouncilEvent per SSE frame. Loopback/practitioner; nothing persisted.
#[tauri::command]
#[specta::specta]
pub async fn consult_council(
    app: AppHandle,
    engine: State<'_, Arc<crate::consult::engine::EngineManager>>,
    binding: State<'_, melori_consult::client_meeting::ClientBinding>,
    sess: State<'_, MeetingSession>,
    situation: Option<String>,
    specialist_ids: Option<Vec<String>>,
) -> Result<(), String> {
    use crate::meeting::council::{parse_sse_frame, split_frames, CouncilEvent};
    use futures_util::StreamExt;

    let situation = match situation {
        Some(s) if !s.trim().is_empty() => s,
        _ => {
            let t = format_transcript(&sess.snapshot().segments);
            if t.trim().is_empty() {
                return Err("nothing to consult on".into());
            }
            t
        }
    };

    let settings = crate::settings::get_settings(&app);
    let echo_base = settings
        .wellbeing_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    let mut body = serde_json::json!({
        "situation": situation,
        "specialist_ids": [],
        "mode": "practitioner",
        "ground_in_records": false,
    });

    let request = if let Some(client_binding) = binding.get() {
        let base = engine
            .url()
            .ok_or_else(|| "consult engine unavailable".to_string())?;
        body["client_id"] = serde_json::Value::String(client_binding.client_id);
        // the session's language comes from the transcript, even when the council reads
        // the SOAP note (which may have been written from an English prompt)
        let spoken = format_transcript(&sess.snapshot().segments);
        body["language"] = serde_json::Value::String(
            melori_consult::language::transcript_language(&spoken).to_string(),
        );
        if let Some(ids) = specialist_ids.filter(|ids| !ids.is_empty()) {
            body["specialist_ids"] = serde_json::json!(ids);
        }
        body["session_id"] = serde_json::Value::String(client_binding.session_id);
        let url = format!("{}/api/psych-council/stream", base.trim_end_matches('/'));
        // A council is ~12 sequential LLM calls streamed over SSE: bound the connect and the
        // silence between chunks, never the whole stream (a total timeout cuts it mid-council).
        let request = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .read_timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("council client unavailable: {e}"))?
            .post(&url)
            .header("X-Melori-Token", engine.token());
        request
    } else {
        let url = format!("{echo_base}/api/psych-council/stream");
        reqwest::Client::new().post(&url)
    };

    let resp = request.json(&body).send().await.map_err(|e| {
        format!("council unavailable (is the wellbeing sidecar running at {echo_base}?): {e}")
    })?;
    if !resp.status().is_success() {
        return Err(format!("council request failed: {}", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    // the engine reports an LLM failure as an `error` frame followed by `done`; the
    // command used to return Ok regardless, so a failed council looked like a success
    let mut failure: Option<String> = None;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("council stream error: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        let (frames, rest) = split_frames(&buf);
        buf = rest;
        for f in frames {
            if let Some(frame) = parse_sse_frame(&f) {
                if let crate::meeting::council::CouncilFrame::Error { detail } = &frame {
                    failure = Some(detail.clone());
                }
                let _ = (CouncilEvent { frame }).emit(&app);
            }
        }
    }
    match failure {
        Some(detail) => Err(format!("council failed: {detail}")),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::session::MeetingSession;

    #[test]
    fn double_start_is_rejected_and_status_reflects_state() {
        let sess = MeetingSession::default();
        assert!(guard_start(&sess, "t0".into(), true).is_ok());
        assert!(guard_start(&sess, "t1".into(), true).is_err());
        let st = status_of(&sess);
        assert!(st.active);
        assert!(st.loopback_active);
    }

    #[test]
    fn stop_when_inactive_is_safe() {
        let sess = MeetingSession::default();
        let fin = guard_stop(&sess);
        assert!(!fin.active);
        assert_eq!(fin.segments.len(), 0);
    }
}
