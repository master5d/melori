pub mod cloned;
pub mod local_readback;
pub mod stream_source;

use crate::audio_toolkit::playback_gate::{Generation, PlaybackGate};
use crate::tts::cloned::ClonedVoiceEngine;
use crate::tts::local_readback::LocalReadbackEngine;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// RAII guard: emits `tts-playback` `{active:true}` on construction and
/// `{active:false}` on drop, so every playback exit path (normal end,
/// fallback-ladder failure, barge-in `sink.stop()`) symmetrically clears the
/// shell's OUT meter. Must be constructed only after the barge-in
/// publish-guard confirms this worker is current — never at function entry —
/// so a superseded worker never emits a spurious `true`.
struct TtsActiveGuard(AppHandle);
impl TtsActiveGuard {
    fn start(app: AppHandle) -> Self {
        let _ = app.emit("tts-playback", serde_json::json!({"active": true}));
        Self(app)
    }
}
impl Drop for TtsActiveGuard {
    fn drop(&mut self) {
        let _ = self
            .0
            .emit("tts-playback", serde_json::json!({"active": false}));
    }
}

#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VoiceInfo {
    pub id: String,
    pub display_name: String,
    pub language: String,
}

pub trait TtsEngine: Send + Sync {
    fn list_voices(&self) -> Result<Vec<VoiceInfo>, String>;
    fn synthesize(&self, text: &str, voice_id: Option<&str>, rate: f32) -> Result<Vec<u8>, String>;
}

/// Pick a voice whose language matches the text's script. If >=30% of the
/// alphabetic characters are Cyrillic, prefer a `ru*` voice; otherwise an
/// `en*` voice. Falls back to the first available voice.
pub fn pick_voice_for_text<'a>(text: &str, voices: &'a [VoiceInfo]) -> Option<&'a VoiceInfo> {
    let alpha: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha.is_empty() {
        return voices.first();
    }

    let cyrillic = alpha
        .iter()
        .filter(|&&c| ('\u{0400}'..='\u{04FF}').contains(&c))
        .count();
    let is_russian = (cyrillic as f32 / alpha.len() as f32) >= 0.3;

    let prefix = if is_russian { "ru" } else { "en" };
    voices
        .iter()
        .find(|v| v.language.to_lowercase().starts_with(prefix))
        .or_else(|| voices.first())
}

pub struct TtsManager {
    engine: Option<Arc<dyn TtsEngine>>,
    current_sink: Arc<Mutex<Option<Arc<Sink>>>>,
    current_cancel: Mutex<Option<Arc<AtomicBool>>>,
    cloned: Mutex<Option<Arc<ClonedVoiceEngine>>>,
    local: Mutex<Option<Arc<LocalReadbackEngine>>>,
    gate: Arc<PlaybackGate>,
    speak_lock: Mutex<()>,
    app_handle: AppHandle,
}

impl TtsManager {
    pub fn new(app_handle: AppHandle) -> Self {
        #[cfg(windows)]
        {
            Self {
                engine: Some(Arc::new(windows::WindowsTts::new())),
                current_sink: Arc::new(Mutex::new(None)),
                current_cancel: Mutex::new(None),
                cloned: Mutex::new(None),
                local: Mutex::new(None),
                gate: Arc::new(PlaybackGate::new()),
                speak_lock: Mutex::new(()),
                app_handle,
            }
        }
        #[cfg(not(windows))]
        {
            Self {
                engine: None,
                current_sink: Arc::new(Mutex::new(None)),
                current_cancel: Mutex::new(None),
                cloned: Mutex::new(None),
                local: Mutex::new(None),
                gate: Arc::new(PlaybackGate::new()),
                speak_lock: Mutex::new(()),
                app_handle,
            }
        }
    }

    /// Install the cloned-voice engine (called at app setup when settings enable it).
    pub fn set_cloned_engine(&self, engine: Option<ClonedVoiceEngine>) {
        *self.cloned.lock().unwrap() = engine.map(Arc::new);
    }

    /// Install the local whole-WAV readback engine (offline Piper floor).
    pub fn set_local_engine(&self, engine: Option<LocalReadbackEngine>) {
        *self.local.lock().unwrap() = engine.map(Arc::new);
    }

    pub fn list_voices(&self) -> Result<Vec<VoiceInfo>, String> {
        if let Some(c) = self.cloned.lock().unwrap().as_ref() {
            return c.list_voices();
        }
        self.engine
            .as_ref()
            .ok_or_else(|| "no TTS engine available on this platform".to_string())?
            .list_voices()
    }

    pub fn speak(&self, text: String, voice_id: Option<String>, rate: f32) -> Result<(), String> {
        // Serialize the barge-in prologue so the cancel-token publish and gate.begin()
        // stay atomic against a concurrent speak() (two call sites: agent bridge + tts command).
        let _prologue = self.speak_lock.lock().unwrap();

        // Barge-in: cancel + stop any current playback, and invalidate outstanding workers.
        self.stop()?;

        // Publish a fresh cancel token on the caller thread so a `stop()` that lands before
        // this worker even opens its sink still flips a flag the worker later observes.
        let cancel = Arc::new(AtomicBool::new(false));
        *self.current_cancel.lock().unwrap() = Some(cancel.clone());
        let gen = self.gate.begin();

        // Snapshot everything the worker needs; return immediately. The health() probe,
        // routing, device open, sink publish, and playback all run on the worker thread.
        let cloned = self.cloned.lock().unwrap().clone();
        let local = self.local.lock().unwrap().clone();
        let engine = self.engine.clone();
        let current_sink = self.current_sink.clone();
        let gate = self.gate.clone();
        let app = self.app_handle.clone();

        std::thread::spawn(move || {
            Self::run_playback(
                text,
                voice_id,
                rate,
                cloned,
                local,
                engine,
                current_sink,
                gate,
                gen,
                cancel,
                app,
            );
        });
        Ok(())
    }

    /// The single playback worker: probe health, route (cloned PCM ladder vs SAPI whole),
    /// open the audio device, publish the sink only if still current (barge-in seal), play.
    #[allow(clippy::too_many_arguments)]
    fn run_playback(
        text: String,
        voice_id: Option<String>,
        rate: f32,
        cloned: Option<Arc<ClonedVoiceEngine>>,
        local: Option<Arc<LocalReadbackEngine>>,
        engine: Option<Arc<dyn TtsEngine>>,
        current_sink: Arc<Mutex<Option<Arc<Sink>>>>,
        gate: Arc<PlaybackGate>,
        gen: Generation,
        cancel: Arc<AtomicBool>,
        app: AppHandle,
    ) {
        use crate::tts::stream_source::{
            end_stream, new_shared_buf, push_samples, wait_prebuffer, PcmStreamSource,
        };
        const PREBUFFER_SAMPLES: usize = 4800; // ~200ms @ 24kHz mono

        // In cloned mode the requested id is a cloned profile id, not a Windows SAPI voice —
        // so on the SAPI floor it must NOT be forwarded (would error "voice not found" →
        // silent no-audio). Pass None there; only the pure-system path forwards the user's id.
        let sapi_voice_id: Option<String> = if cloned.is_some() {
            None
        } else {
            voice_id.clone()
        };

        // Publish-guard (the barge-in seal): publish this sink to current_sink only if this
        // worker's generation is still current. A superseded worker returns without playing —
        // the correct barge-in outcome.
        let publish = |sink: &Arc<Sink>| -> bool {
            let mut lock = current_sink.lock().unwrap();
            if gate.is_current(&gen) {
                *lock = Some(sink.clone());
                true
            } else {
                false
            }
        };

        let stream_eligible = cloned
            .as_ref()
            .map(|c| c.stream_eligible())
            .unwrap_or(false);

        if stream_eligible {
            let cloned = cloned.expect("stream_eligible implies a cloned engine is present");
            let cloned_for_ladder = cloned.clone();
            let sapi = engine.clone();

            let stream =
                match OutputStreamBuilder::from_default_device().and_then(|b| b.open_stream()) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("TTS stream open failed: {e}");
                        return;
                    }
                };
            let sink = Arc::new(Sink::connect_new(stream.mixer()));
            if !publish(&sink) {
                log::debug!("TTS worker superseded before publish; skipping playback");
                return;
            }
            let _tts_active = TtsActiveGuard::start(app.clone());

            // --- Tier 1: PCM streaming ---
            let buf = new_shared_buf();
            let produced_any = Arc::new(AtomicBool::new(false));
            // Hoisted so we can join the pre-buffer helper before playback is awaited.
            let prebuffer_handle;
            let stream_res = {
                let buf_push = buf.clone();
                let produced_flag = produced_any.clone();
                let mut on_pcm = |pcm: &[i16]| {
                    produced_flag.store(true, Ordering::Relaxed);
                    push_samples(&buf_push, pcm);
                };
                // Start playback once pre-buffered: spawn a helper that waits then appends.
                let buf_play = buf.clone();
                let sink_play = sink.clone();
                let cancel_play = cancel.clone();
                prebuffer_handle = std::thread::spawn(move || {
                    wait_prebuffer(&buf_play, PREBUFFER_SAMPLES);
                    if !cancel_play.load(Ordering::Relaxed) {
                        sink_play.append(PcmStreamSource::new(buf_play));
                    }
                });
                cloned.synthesize_stream_pcm(&text, voice_id.as_deref(), &cancel, &mut on_pcm)
            };
            // Unblocks wait_prebuffer (sets `ended`) so the helper can return even
            // when total audio is < PREBUFFER_SAMPLES. Must run before the join below.
            end_stream(&buf);
            let produced_any = produced_any.load(Ordering::Relaxed);

            match stream_res {
                Ok(()) => {}
                Err(e) if !produced_any && !cancel.load(Ordering::Relaxed) => {
                    // --- Tier 2: Lever-1 batch cloned (whole reply, per-chunk WAV append) ---
                    // Note: the pre-buffer helper still appends a PcmStreamSource over the
                    // now-`end`ed, empty `buf`; it yields `None` immediately (a no-op) so it
                    // does not interfere with the Tier-2/Tier-3 fallback audio queued here.
                    log::warn!("PCM stream failed pre-audio ({e}); falling to batch cloned");
                    let append = |bytes: Vec<u8>| {
                        if let Ok(src) = Decoder::new(Cursor::new(bytes)) {
                            sink.append(src);
                        }
                    };
                    let mut on_chunk = |out: echo_voice::stream::ChunkOutput| {
                        if cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        match out {
                            echo_voice::stream::ChunkOutput::Wav(b) => append(b),
                            echo_voice::stream::ChunkOutput::Failed { text, .. } => {
                                if let Some(sapi) = sapi.as_ref() {
                                    if let Ok(w) = sapi.synthesize(&text, None, 1.0) {
                                        append(w);
                                    }
                                }
                            }
                        }
                    };
                    let batch = cloned_for_ladder.speak_stream(
                        &text,
                        voice_id.as_deref(),
                        &cancel,
                        &mut on_chunk,
                    );
                    if batch.is_err() {
                        // --- Tier 3: SAPI whole-utterance floor ---
                        if let Some(sapi) = sapi.as_ref() {
                            if let Ok(w) = sapi.synthesize(&text, None, 1.0) {
                                if let Ok(src) = Decoder::new(Cursor::new(w)) {
                                    sink.append(src);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("PCM stream failed mid-utterance ({e}); stopping (no restart)");
                }
            }

            // Ensure the pre-buffer helper has appended its source (or skipped on cancel)
            // BEFORE we await playout / drop the OutputStream. `end_stream` above lets
            // wait_prebuffer return even for sub-PREBUFFER_SAMPLES replies, so this join
            // never deadlocks; without it a very short reply's audio could be dropped.
            let _ = prebuffer_handle.join();

            if !cancel.load(Ordering::Relaxed) {
                sink.sleep_until_end();
            }
        } else {
            // --- Whole-utterance route: local Piper first, then the SAPI floor. ---
            let wav_bytes = local
                .as_ref()
                .and_then(|l| {
                    l.synthesize_whole_wav(&text, rate)
                        .map_err(|e| log::info!("local readback unavailable, SAPI fallback: {e}"))
                        .ok()
                })
                .or_else(|| {
                    let engine = engine.as_ref()?;
                    engine
                        .synthesize(&text, sapi_voice_id.as_deref(), rate)
                        .map_err(|e| log::error!("SAPI synth failed: {e}"))
                        .ok()
                });
            let wav_bytes = match wav_bytes {
                Some(b) => b,
                None => {
                    log::error!("no TTS engine produced audio (local + SAPI unavailable)");
                    return;
                }
            };
            let stream =
                match OutputStreamBuilder::from_default_device().and_then(|b| b.open_stream()) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("TTS stream open failed: {e}");
                        return;
                    }
                };
            let sink = Arc::new(Sink::connect_new(stream.mixer()));
            if !publish(&sink) {
                log::debug!("TTS worker superseded before publish; skipping playback");
                return;
            }
            let _tts_active = TtsActiveGuard::start(app.clone());
            match Decoder::new(Cursor::new(wav_bytes)) {
                Ok(src) => sink.append(src),
                Err(e) => {
                    log::error!("TTS decode failed: {e}");
                    return;
                }
            }
            if !cancel.load(Ordering::Relaxed) {
                sink.sleep_until_end();
            }
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        // Invalidate outstanding workers first, so any worker checking the gate after this
        // returns observes the newer generation and declines to publish.
        self.gate.supersede();
        if let Some(cancel) = self.current_cancel.lock().unwrap().take() {
            cancel.store(true, Ordering::Relaxed);
        }
        let mut sink_lock = self.current_sink.lock().map_err(|e| e.to_string())?;
        if let Some(sink) = sink_lock.take() {
            sink.stop();
        }
        Ok(())
    }
}

#[cfg(test)]
mod voice_pick_tests {
    use super::*;

    fn voices() -> Vec<VoiceInfo> {
        vec![
            VoiceInfo {
                id: "en-1".into(),
                display_name: "David".into(),
                language: "en-US".into(),
            },
            VoiceInfo {
                id: "ru-1".into(),
                display_name: "Irina".into(),
                language: "ru-RU".into(),
            },
        ]
    }

    #[test]
    fn russian_text_picks_ru_voice() {
        let v = voices();
        let picked = pick_voice_for_text("Привет, как дела?", &v).unwrap();
        assert_eq!(picked.id, "ru-1");
    }

    #[test]
    fn english_text_picks_en_voice() {
        let v = voices();
        let picked = pick_voice_for_text("Hello, how are you?", &v).unwrap();
        assert_eq!(picked.id, "en-1");
    }

    #[test]
    fn no_alpha_falls_back_to_first() {
        let v = voices();
        let picked = pick_voice_for_text("12345 !!!", &v).unwrap();
        assert_eq!(picked.id, "en-1");
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Real WinRT synthesis — needs installed voices, so opt-in only:
    /// cargo test --lib tts -- --ignored --nocapture
    #[test]
    #[ignore]
    fn windows_tts_synthesizes_wav() {
        let engine = windows::WindowsTts::new();

        let voices = engine.list_voices().expect("list_voices failed");
        assert!(!voices.is_empty(), "no Windows voices installed");
        println!(
            "voices: {:?}",
            voices.iter().map(|v| &v.display_name).collect::<Vec<_>>()
        );

        let wav = engine
            .synthesize("Echo speech engine online. Эхо на связи.", None, 1.0)
            .expect("synthesize failed");
        assert!(wav.len() > 44, "WAV too small: {} bytes", wav.len());
        assert_eq!(&wav[0..4], b"RIFF", "not a WAV container");
        println!("synthesized {} bytes", wav.len());
    }
}
