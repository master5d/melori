use crate::audio_toolkit::{
    list_input_devices, vad::SmoothedVad, AudioRecorder, DeviceFacts, DeviceHealthState,
    HealthEvaluator, HealthStatus, MonitorBuf, SileroVad,
};
use crate::helpers::clamshell;
use crate::settings::{get_settings, AppSettings};
use crate::utils;
use log::{debug, error, info};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

fn set_mute(mute: bool) {
    // Expected behavior:
    // - Windows: works on most systems using standard audio drivers.
    // - Linux: works on many systems (PipeWire, PulseAudio, ALSA),
    //   but some distros may lack the tools used.
    // - macOS: works on most standard setups via AppleScript.
    // If unsupported, fails silently.

    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows::Win32::{
                Media::Audio::{
                    eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                    MMDeviceEnumerator,
                },
                System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
            };

            macro_rules! unwrap_or_return {
                ($expr:expr) => {
                    match $expr {
                        Ok(val) => val,
                        Err(_) => return,
                    }
                };
            }

            // Initialize the COM library for this thread.
            // If already initialized (e.g., by another library like Tauri), this does nothing.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let all_devices: IMMDeviceEnumerator =
                unwrap_or_return!(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL));
            let default_device =
                unwrap_or_return!(all_devices.GetDefaultAudioEndpoint(eRender, eMultimedia));
            let volume_interface = unwrap_or_return!(
                default_device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            );

            let _ = volume_interface.SetMute(mute, std::ptr::null());
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let mute_val = if mute { "1" } else { "0" };
        let amixer_state = if mute { "mute" } else { "unmute" };

        // Try multiple backends to increase compatibility
        // 1. PipeWire (wpctl)
        if Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 2. PulseAudio (pactl)
        if Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 3. ALSA (amixer)
        let _ = Command::new("amixer")
            .args(["set", "Master", amixer_state])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "set volume output muted {}",
            if mute { "true" } else { "false" }
        );
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }
}

const WHISPER_SAMPLE_RATE: usize = 16000;

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone, Debug)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

/* ──────────────────────────────────────────────────────────────── */

/// Payload for the `input-monitor-level` event: linear peak/RMS of the latest
/// input frame. dBFS/clip/peak-hold are computed frontend-side.
#[derive(Clone, serde::Serialize, specta::Type)]
pub struct InputMonitorLevel {
    pub peak: f32,
    pub rms: f32,
}

/// Payload for the `input-spectrum` event: one analyzer frame (dBFS per log band)
/// plus the capture rate the bands were computed for. Distinct from the pill's
/// `mic-level` (cosmetic 16-bucket) and from `input-monitor-level` (peak/RMS).
#[derive(Clone, serde::Serialize, specta::Type)]
pub struct InputSpectrum {
    pub bands: Vec<f32>,
    pub sample_rate: u32,
}

/// Wire mirror of `echo_audio::DeviceFacts`. It lives here, not in `echo-audio`,
/// because that crate has no serde dependency and must not gain one — the app
/// crate owns the wire format.
#[derive(Clone, serde::Serialize, specta::Type)]
pub struct DeviceFactsPayload {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

impl From<DeviceFacts> for DeviceFactsPayload {
    fn from(f: DeviceFacts) -> Self {
        Self {
            name: f.name,
            sample_rate: f.sample_rate,
            channels: f.channels,
            sample_format: f.sample_format,
        }
    }
}

/// Payload for the `input-device-health` event (Epic D2). `status` is one of
/// "ok" | "starting" | "silent" | "stalled" | "unavailable".
#[derive(Clone, serde::Serialize, specta::Type)]
pub struct InputDeviceHealth {
    pub status: String,
    pub facts: Option<DeviceFactsPayload>,
    /// 0 unless `status == "silent"`.
    pub silent_for_ms: u64,
    /// `Some` only when `status == "unavailable"`.
    pub reason: Option<String>,
}

impl InputDeviceHealth {
    fn new(status: HealthStatus, facts: Option<DeviceFacts>) -> Self {
        let (status, silent_for_ms, reason) = match status {
            HealthStatus::Ok => ("ok", 0, None),
            HealthStatus::Starting => ("starting", 0, None),
            HealthStatus::Silent { for_ms } => ("silent", for_ms, None),
            HealthStatus::Stalled => ("stalled", 0, None),
            HealthStatus::Unavailable { reason } => ("unavailable", 0, Some(reason)),
        };
        Self {
            status: status.to_string(),
            facts: facts.map(Into::into),
            silent_for_ms,
            reason,
        }
    }
}

/// How often the health ticker evaluates and emits.
const HEALTH_TICK_MS: u64 = 250;

/// One long-lived ticker. It is gated INSIDE its loop rather than being spawned
/// and killed per monitor session: a thread with no lifecycle is a thread whose
/// lifecycle cannot desynchronize, and every expensive bug in Epic C lived in
/// thread lifecycle rather than in thread work.
fn spawn_health_ticker(
    app: tauri::AppHandle,
    health: Arc<DeviceHealthState>,
    monitor_active: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut evaluator = HealthEvaluator::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(HEALTH_TICK_MS));

            if !monitor_active.load(Ordering::SeqCst) {
                // Drop the accumulated silence clock while nobody is watching.
                // Otherwise a `silent_since` from a previous visit to the Audio
                // console would make a freshly reopened console report `Silent`
                // instantly — the same reason D1's analyzer resets on a closed
                // gate.
                evaluator = HealthEvaluator::new();
                continue;
            }

            // ONE read: status inputs and facts under a single lock. Sampling them
            // separately let a device switch slip between the two calls and emit a
            // tick of {status: "ok", facts: null} — a healthy device with no
            // identity (Epic D2 ticket T1).
            let (snapshot, facts) = health.read();
            let status = evaluator.evaluate(snapshot);
            let _ = app.emit_to(
                "main",
                "input-device-health",
                InputDeviceHealth::new(status, facts),
            );
        }
    });
}

fn create_audio_recorder(
    vad_path: &str,
    app_handle: &tauri::AppHandle,
    is_recording: Arc<Mutex<bool>>,
    monitor_active: Arc<AtomicBool>,
    input_gain: Arc<AtomicU32>,
    monitor: Arc<MonitorBuf>,
    capture_sample_rate: Arc<AtomicU32>,
    device_health: Arc<DeviceHealthState>,
) -> Result<AudioRecorder, anyhow::Error> {
    let silero = SileroVad::new(vad_path, 0.3)
        .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {}", e))?;
    let smoothed_vad = SmoothedVad::new(Box::new(silero), 15, 15, 2);

    // Recorder with VAD plus a spectrum-level callback that forwards updates to
    // the frontend.
    let recorder = AudioRecorder::new()
        .map_err(|e| anyhow::anyhow!("Failed to create AudioRecorder: {}", e))?
        .with_vad(Box::new(smoothed_vad))
        .with_level_callback({
            let app_handle = app_handle.clone();
            let is_recording = is_recording.clone();
            move |levels| {
                let recording = *is_recording.lock().unwrap();
                utils::emit_levels(&app_handle, &levels, recording);
            }
        })
        .with_meter_callback({
            let app_handle = app_handle.clone();
            let is_recording = is_recording.clone();
            let monitor_active = monitor_active.clone();
            move |peak, rms| {
                let to_main =
                    *is_recording.lock().unwrap() || monitor_active.load(Ordering::Relaxed);
                if to_main {
                    let _ = app_handle.emit_to(
                        "main",
                        "input-monitor-level",
                        InputMonitorLevel { peak, rms },
                    );
                }
            }
        })
        .with_gain(input_gain)
        .with_monitor(monitor)
        .with_spectrum(monitor_active.clone(), {
            let app_handle = app_handle.clone();
            let rate_cache = capture_sample_rate.clone();
            move |bands, sample_rate| {
                // The rate arrives with the frame — cache it so `get_spectrum_freqs`
                // can answer without touching the audio thread.
                rate_cache.store(sample_rate, Ordering::Relaxed);
                let _ = app_handle.emit_to(
                    "main",
                    "input-spectrum",
                    InputSpectrum { bands, sample_rate },
                );
            }
        })
        .with_health(device_health);

    Ok(recorder)
}

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: tauri::AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    did_mute: Arc<Mutex<bool>>,
    /// Serializes mute TRANSITIONS (apply vs remove) with each other — and with
    /// nothing else. `set_mute` shells out to `wpctl`/`amixer`/AppleScript on
    /// Linux/macOS (a process spawn, tens of ms). Holding `is_open` across that
    /// made every stream-lifecycle caller — device switch, monitor, Reconnect —
    /// wait on a subprocess. This lock lets the OS call happen with `is_open`
    /// released while still admitting exactly one mute transition at a time.
    mute_lock: Arc<Mutex<()>>,
    close_generation: Arc<AtomicU64>,
    monitor_active: Arc<AtomicBool>,
    input_gain: Arc<AtomicU32>,
    monitor_buf: Arc<MonitorBuf>,
    monitor_volume: Arc<AtomicU32>,
    passthrough_enabled: Arc<AtomicBool>,
    capture_sample_rate: Arc<AtomicU32>,
    device_health: Arc<DeviceHealthState>,
    /// Stop flag + join handle for the monitor output thread, consolidated into
    /// one field so they can never desynchronize (see `monitor_lifecycle`).
    monitor_thread: Arc<Mutex<Option<(Arc<AtomicBool>, JoinHandle<()>)>>>,
    /// Serializes the check-then-act toggle/device-change sequences in
    /// `set_passthrough`, `set_monitor_output_device`, and the passthrough-aware
    /// branch of `update_selected_device` — held for the whole body of each so
    /// Tauri's unordered command dispatch can never interleave two spawns/stops
    /// of the monitor output thread. The monitor thread itself touches no
    /// manager locks, so there is no lock-ordering hazard.
    monitor_lifecycle: Arc<Mutex<()>>,
}

impl AudioRecordingManager {
    /* ---------- construction ------------------------------------------------ */

    pub fn new(app: &tauri::AppHandle) -> Result<Self, anyhow::Error> {
        let settings = get_settings(app);
        let mode = if settings.always_on_microphone {
            MicrophoneMode::AlwaysOn
        } else {
            MicrophoneMode::OnDemand
        };

        let manager = Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            mode: Arc::new(Mutex::new(mode.clone())),
            app_handle: app.clone(),

            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            did_mute: Arc::new(Mutex::new(false)),
            mute_lock: Arc::new(Mutex::new(())),
            close_generation: Arc::new(AtomicU64::new(0)),
            monitor_active: Arc::new(AtomicBool::new(false)),
            input_gain: Arc::new(AtomicU32::new(
                crate::audio_toolkit::db_to_linear(settings.input_gain.clamp(-24.0, 24.0))
                    .to_bits(),
            )),
            monitor_buf: Arc::new(MonitorBuf::new()),
            monitor_volume: Arc::new(AtomicU32::new(
                settings.monitor_volume.clamp(0.0, 1.0).to_bits(),
            )),
            passthrough_enabled: Arc::new(AtomicBool::new(false)),
            capture_sample_rate: Arc::new(AtomicU32::new(0)),
            device_health: Arc::new(DeviceHealthState::new()),
            monitor_thread: Arc::new(Mutex::new(None)),
            monitor_lifecycle: Arc::new(Mutex::new(())),
        };

        // Always-on?  Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        spawn_health_ticker(
            app.clone(),
            manager.device_health.clone(),
            manager.monitor_active.clone(),
        );

        Ok(manager)
    }

    /* ---------- helper methods --------------------------------------------- */

    fn get_effective_microphone_device(&self, settings: &AppSettings) -> Option<cpal::Device> {
        // Check if we're in clamshell mode and have a clamshell microphone configured
        let use_clamshell_mic = if let Ok(is_clamshell) = clamshell::is_clamshell() {
            is_clamshell && settings.clamshell_microphone.is_some()
        } else {
            false
        };

        let device_name = if use_clamshell_mic {
            settings.clamshell_microphone.as_ref().unwrap()
        } else {
            settings.selected_microphone.as_ref()?
        };

        // Find the device by name
        match list_input_devices() {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.name == *device_name)
                .map(|d| d.device),
            Err(e) => {
                debug!("Failed to list devices, using default: {}", e);
                None
            }
        }
    }

    fn schedule_lazy_close(&self) {
        let gen = self.close_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let app = self.app_handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(STREAM_IDLE_TIMEOUT);
            let rm = app.state::<Arc<AudioRecordingManager>>();
            // Hold state lock across the check AND close to serialize against
            // try_start_recording, preventing a race where the stream is closed
            // under an active recording.
            let state = rm.state.lock().unwrap();
            if rm.close_generation.load(Ordering::SeqCst) == gen
                && matches!(*state, RecordingState::Idle)
                && !rm.monitor_active.load(Ordering::SeqCst)
            {
                // stop_microphone_stream does not acquire the state lock,
                // so holding it here is safe (no deadlock).
                info!(
                    "Closing idle microphone stream after {:?}",
                    STREAM_IDLE_TIMEOUT
                );
                rm.stop_microphone_stream();
            }
        });
    }

    /* ---------- microphone life-cycle -------------------------------------- */

    /// Applies mute if mute_while_recording is enabled and stream is open.
    ///
    /// LOCK ORDER: `mute_lock` → (`is_open`) → (`did_mute`), each taken and
    /// RELEASED in turn; `set_mute` runs holding only `mute_lock`.
    ///
    /// The previous version held `is_open` (and `did_mute`) across `set_mute`,
    /// which spawns `wpctl`/`amixer`/AppleScript on Linux/macOS — so a mute made
    /// every stream-lifecycle caller wait on a subprocess. Dropping `is_open`
    /// around the OS call alone would reintroduce the TOCTOU that hold was there
    /// to prevent: the stream can close mid-call, and we would leave the OS
    /// microphone muted with no recording behind it. So the check is not merely
    /// dropped — it is repeated afterwards, and a mute that lost the race is
    /// undone immediately. `mute_lock` guarantees no other mute transition can
    /// interleave with that repair.
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        if !settings.mute_while_recording {
            return;
        }

        let _mute = self.mute_lock.lock().unwrap();
        if !*self.is_open.lock().unwrap() {
            return;
        }

        set_mute(true); // may shell out — no manager lock held but `mute_lock`
        *self.did_mute.lock().unwrap() = true;

        if !*self.is_open.lock().unwrap() {
            // The stream closed while we were muting. `stop_microphone_stream`
            // already ran its unmute (or is waiting on `mute_lock` and will find
            // nothing to do), so this repair is the only thing standing between
            // the user and a permanently muted microphone.
            set_mute(false);
            *self.did_mute.lock().unwrap() = false;
            debug!("Mute applied then undone — the stream closed mid-call");
            return;
        }
        debug!("Mute applied");
    }

    /// Removes mute if it was applied.
    ///
    /// `did_mute` is flipped BEFORE the OS call and the guard is dropped: holding
    /// it across `set_mute` would put a subprocess spawn inside a lock that
    /// `stop_microphone_stream` needs. `mute_lock` is what makes that safe — it
    /// admits one mute transition at a time, so nothing can observe (or act on)
    /// the window between the flag and the OS state.
    pub fn remove_mute(&self) {
        let _mute = self.mute_lock.lock().unwrap();
        let was_muted = {
            let mut did_mute_guard = self.did_mute.lock().unwrap();
            std::mem::replace(&mut *did_mute_guard, false)
        };
        if was_muted {
            set_mute(false);
            debug!("Mute removed");
        }
    }

    pub fn preload_vad(&self) -> Result<(), anyhow::Error> {
        let mut recorder_opt = self.recorder.lock().unwrap();
        if recorder_opt.is_none() {
            let vad_path = self
                .app_handle
                .path()
                .resolve(
                    "resources/models/silero_vad_v4.onnx",
                    tauri::path::BaseDirectory::Resource,
                )
                .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {}", e))?;
            *recorder_opt = Some(create_audio_recorder(
                vad_path.to_str().unwrap(),
                &self.app_handle,
                self.is_recording.clone(),
                self.monitor_active.clone(),
                self.input_gain.clone(),
                self.monitor_buf.clone(),
                self.capture_sample_rate.clone(),
                self.device_health.clone(),
            )?);
        }
        Ok(())
    }

    /// Record an open-time failure into the health error slot and hand the
    /// error straight back to the caller. Every error exit of
    /// `start_microphone_stream` funnels through here, so the OPEN-TIME write
    /// happens in exactly one place. (cpal's stream-error callback in
    /// `echo-audio`'s `build_stream` is the other writer of the slot, but it
    /// only fires for a stream that already started — the two never race for
    /// the same attempt.) Do not call this from anywhere but
    /// `start_microphone_stream`'s own error exits; `set_monitor` and
    /// `reconnect_input_device` fail only *through* this function and must
    /// not duplicate the write (see their comments).
    ///
    /// The matching CLEAR is `DeviceHealthState::clear_error`, and it does not
    /// live in this file at all: it runs inside `AudioRecorder::open`, the
    /// moment the capture worker confirms the stream is built and playing. See
    /// the comment on the `begin_open()` call below.
    fn record_open_failure(&self, e: anyhow::Error) -> anyhow::Error {
        self.device_health.set_error(e.to_string());
        e
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        let mut open_flag = self.is_open.lock().unwrap();
        if *open_flag {
            debug!("Microphone stream already active");
            return Ok(());
        }

        // Seed the health state HERE, under the same `is_open` guard that just
        // confirmed the stream is not already open — not in `set_monitor`
        // (fix wave 1's location). That check-then-seed used to be two steps
        // (`set_monitor` read `is_open` into a local, dropped the guard, then
        // reset) with a dictation start able to land in the gap: it would open
        // the stream between the read and this call, so `start_microphone_stream`
        // would see `*open_flag == true` and early-return above WITHOUT calling
        // `rec.open()` — but the seed had already cleared `facts` to `None`,
        // and nothing left to republish them. Net effect: a healthy, metering
        // stream whose health card reported `status: "ok"` with `facts: null`
        // forever. Doing it here makes the check and the seed atomic — one lock
        // acquisition, no TOCTOU window — because this is the one place that is
        // genuinely about to open a stream.
        //
        // It also closes the stale-`last_frame_ms` window properly instead of
        // merely bounding it: seeding `last_frame_ms = now` here means the
        // `STALL_MS` grace covers the settings read, device enumeration, and
        // VAD-load work below, rather than starting up to 2s ahead of it (as
        // it would if it ran earlier in `set_monitor`, before this lock).
        //
        // WRITER / CLEARER, accurately (this used to claim "one writer, one
        // clearer, both inside the same `is_open` critical section" — which was
        // never true, because `AudioRecorder::open` reset the state a second
        // time, one layer down, and any deferred-clear fix applied to only one
        // of the two would be silently undone by the other):
        //
        //   * `begin_open()` — called HERE and, idempotently, again inside
        //     `AudioRecorder::open`. It seeds `last_frame_ms`/`last_peak` and
        //     drops stale facts. It does NOT touch the error slot. That is the
        //     fail-closed property: while an open attempt is in flight against
        //     a device that was unavailable, the device is still unavailable
        //     until proven otherwise, so the card stays `unavailable` for the
        //     whole attempt and the frontend's `signalLost` never flickers
        //     false. Without it, Reconnect on a dead device produced a
        //     one-tick `Ok` window in which the meter un-blanked and repainted
        //     the levels captured just before the device died.
        //   * WRITE — `record_open_failure`, at every error exit below (plus
        //     cpal's own stream-error callback, which only fires for a stream
        //     that already started).
        //   * CLEAR — `DeviceHealthState::clear_error`, called from exactly one
        //     place: `AudioRecorder::open`, once the capture worker confirms the
        //     stream was built and `play()`ed. A *successful* reopen forgets the
        //     old failure; a *failed* one keeps showing it until
        //     `record_open_failure` replaces it with the new reason.
        //
        // Every error exit below MUST go through `record_open_failure` (never
        // write `device_health` directly, and never add a second writer in a
        // caller). `try_start_recording` (dictation) is also a caller of this
        // function; before fix wave 3 it never wrote the error slot, so a
        // dictation attempt on an already-diagnosed-unavailable device would
        // clear the error here and then fail below with nothing recording the
        // new failure — the ticker would read empty-error + freshly-seeded
        // `last_frame_ms` and flip the card from a correct "unavailable:
        // <reason>" to "ok", then decay to a reasonless "stalled". The deferred
        // clear now makes that structurally impossible, and `record_open_failure`
        // keeps the reason accurate. This does not hurt dictation: the 250ms
        // ticker only emits while `monitor_active` is true.
        self.device_health.begin_open();

        let start_time = Instant::now();

        // Don't mute immediately - caller will handle muting after audio feedback
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        *did_mute_guard = false;

        // Get the selected device from settings, considering clamshell mode
        let settings = get_settings(&self.app_handle);
        let selected_device = self.get_effective_microphone_device(&settings);

        // Pre-flight check: if no device was selected/configured AND no devices
        // exist at all, fail early with a clear error instead of letting cpal
        // produce a cryptic backend-specific message.
        if selected_device.is_none() {
            let has_any_device = list_input_devices()
                .map(|devices| !devices.is_empty())
                .unwrap_or(false);
            if !has_any_device {
                return Err(self.record_open_failure(anyhow::anyhow!("No input device found")));
            }
        }

        // Ensure VAD is loaded if it wasn't for whatever reason
        self.preload_vad()
            .map_err(|e| self.record_open_failure(e))?;

        let mut recorder_opt = self.recorder.lock().unwrap();
        if let Some(rec) = recorder_opt.as_mut() {
            rec.open(selected_device).map_err(|e| {
                self.record_open_failure(anyhow::anyhow!("Failed to open recorder: {}", e))
            })?;
        }

        *open_flag = true;
        // This timing covers through cpal's stream.play() returning — i.e. the
        // point cpal surfaces as "stream running." It does NOT guarantee the
        // host audio device is producing samples yet; the first input callback
        // fires asynchronously one buffer period later (hardware dependent,
        // typically ~10–200ms on macOS, longer on Bluetooth/USB).
        info!(
            "Microphone stream initialized in {:?}",
            start_time.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        {
            let mut open_flag = self.is_open.lock().unwrap();
            if !*open_flag {
                return;
            }

            if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
                // If still recording, stop first.
                if *self.is_recording.lock().unwrap() {
                    let _ = rec.stop();
                    *self.is_recording.lock().unwrap() = false;
                }
                let _ = rec.close();
            }

            *open_flag = false;
        } // `is_open` released BEFORE the unmute: `set_mute` shells out, and this
          // function used to make every other lifecycle caller wait on it.

        // Ordering is deliberate: the flag is already down, so a concurrent
        // `apply_mute` either sees a closed stream and does nothing, or is
        // mid-call and will undo its own mute in its post-check. Either way the
        // microphone does not stay muted.
        self.remove_mute();
        debug!("Microphone stream stopped");
    }

    /* ---------- mode switching --------------------------------------------- */

    pub fn update_mode(&self, new_mode: MicrophoneMode) -> Result<(), anyhow::Error> {
        let cur_mode = self.mode.lock().unwrap().clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*self.state.lock().unwrap(), RecordingState::Idle)
                    && !self.monitor_active.load(Ordering::SeqCst)
                {
                    self.close_generation.fetch_add(1, Ordering::SeqCst);
                    self.stop_microphone_stream();
                }
            }
            (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn) => {
                self.close_generation.fetch_add(1, Ordering::SeqCst);
                self.start_microphone_stream()?;
            }
            _ => {}
        }

        *self.mode.lock().unwrap() = new_mode;
        Ok(())
    }

    /* ---------- recording --------------------------------------------------- */

    pub fn try_start_recording(&self, binding_id: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Idle = *state {
            // Ensure microphone is open in on-demand mode
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                // Cancel any pending lazy close
                self.close_generation.fetch_add(1, Ordering::SeqCst);
                if let Err(e) = self.start_microphone_stream() {
                    let msg = format!("{e}");
                    error!("Failed to open microphone stream: {msg}");
                    return Err(msg);
                }
            }

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                if rec.start().is_ok() {
                    *self.is_recording.lock().unwrap() = true;
                    *state = RecordingState::Recording {
                        binding_id: binding_id.to_string(),
                    };
                    debug!("Recording started for binding {binding_id}");
                    return Ok(());
                }
            }
            Err("Recorder not available".to_string())
        } else {
            Err("Already recording".to_string())
        }
    }

    /// Enable/disable the Audio-console monitor. While active, the input stream
    /// is held open and `input-monitor-level` is emitted every frame even when
    /// not recording. Disabling schedules the normal idle close.
    pub fn set_monitor(&self, active: bool) -> Result<(), anyhow::Error> {
        if active {
            // The health-state reset (seeding `last_frame_ms`, clearing stale
            // error/facts) now lives inside `start_microphone_stream`, under
            // its `is_open` guard, so the check and the reset are atomic. See
            // the comment there for why — it used to live here, gated on a
            // racily-read `is_open` local, which left a window for a
            // concurrent dictation start to open the stream and turn the
            // reset into a hollowed-out "ok" card with `facts: null`.
            self.monitor_active.store(active, Ordering::SeqCst);

            // Cancel any pending lazy close, then ensure the stream is open.
            self.close_generation.fetch_add(1, Ordering::SeqCst);
            if let Err(e) = self.start_microphone_stream() {
                // `start_microphone_stream` itself writes the health error
                // slot on every one of its error exits (fix wave 3,
                // `record_open_failure`) — cpal's stream-error callback only
                // fires for a stream that already started, so without that
                // in-function write an open-time failure (no device, VAD load
                // failure, cpal build/play error) would leave the slot empty
                // and the ticker reporting "stalled" with no reason forever.
                // Nothing to do here but propagate; do NOT re-write
                // `device_health` here — see the "one writer, one clearer"
                // invariant on `reset()`.
                //
                // `monitor_active` above is deliberately left `true` on this
                // failure path — do NOT roll it back here. It is the only
                // reason the 250ms ticker keeps running after an open
                // failure, which is what lets it deliver the "unavailable"
                // status + reason `start_microphone_stream` already wrote.
                // Resetting the gate to `false` on failure would silently
                // delete this entire open-failure reporting path, and no
                // test would catch it (app-crate tests cannot run natively on
                // this box).
                return Err(e);
            }
        } else {
            self.monitor_active.store(active, Ordering::SeqCst);
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                self.schedule_lazy_close();
            }
        }
        Ok(())
    }

    /// Update the live capture gain. `db` is converted to a linear multiplier
    /// and stored in the atomic the consumer reads each frame — no reopen.
    pub fn set_input_gain(&self, db: f32) {
        self.input_gain.store(
            crate::audio_toolkit::db_to_linear(db).to_bits(),
            Ordering::SeqCst,
        );
    }

    /// Enable/disable live monitoring (passthrough). Enabling ensures the
    /// capture stream is open (reusing the Epic-A monitor), activates the buffer
    /// tap, and spawns the output thread with a fresh mute-ramp. Disabling stops
    /// the output thread and deactivates the tap; the capture stays under the
    /// Epic-A meter's control. Session-only — never persisted.
    pub fn set_passthrough(&self, enabled: bool) -> Result<(), anyhow::Error> {
        // Serialize the whole check-then-act sequence: Tauri dispatches sync
        // commands on a thread pool with no ordering guarantee, so rapid
        // toggling (or a device change racing an enable) could otherwise spawn
        // two output threads and orphan one — see FIX 3 / final-fix-brief.
        let _lifecycle = self.monitor_lifecycle.lock().unwrap();
        if enabled {
            if self.passthrough_enabled.load(Ordering::SeqCst) {
                return Ok(()); // already on
            }
            // Ensure the capture stream is running so frames flow into the buffer.
            self.set_monitor(true)?;
            self.monitor_buf.set_active(true);

            let stop = Arc::new(AtomicBool::new(false));
            let device = get_settings(&self.app_handle).monitor_output_device;
            let handle = crate::monitor::spawn_monitor_thread(
                self.app_handle.clone(),
                self.monitor_buf.clone(),
                self.monitor_volume.clone(),
                device,
                stop.clone(),
            );
            *self.monitor_thread.lock().unwrap() = Some((stop, handle));
            self.passthrough_enabled.store(true, Ordering::SeqCst);
        } else {
            self.passthrough_enabled.store(false, Ordering::SeqCst);
            self.monitor_buf.set_active(false);
            self.stop_monitor_thread();
        }
        Ok(())
    }

    /// Stop and join the monitor output thread if running.
    fn stop_monitor_thread(&self) {
        if let Some((stop, handle)) = self.monitor_thread.lock().unwrap().take() {
            stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
        }
    }

    /// Update the live monitor volume (linear 0..1). Read per sample by the
    /// monitor Source — no reopen.
    pub fn set_monitor_volume(&self, volume: f32) {
        // f32::clamp propagates NaN (both `NaN < min` and `NaN > max` are
        // false) — sanitize first so a non-finite volume never reaches
        // apply_monitor and poisons the output samples.
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.monitor_volume
            .store(volume.to_bits(), Ordering::SeqCst);
    }

    /// Change the monitor output device. If passthrough is active, restart the
    /// output thread on the new device (a fresh ramp); otherwise the new device
    /// is picked up on the next enable.
    pub fn set_monitor_output_device(&self, device: Option<String>) -> Result<(), anyhow::Error> {
        // See set_passthrough: serialize check-then-act against concurrent
        // enable/disable/device-change so the stop+respawn pair below can never
        // interleave with another lifecycle transition.
        let _lifecycle = self.monitor_lifecycle.lock().unwrap();
        if self.passthrough_enabled.load(Ordering::SeqCst) {
            self.stop_monitor_thread();
            let stop = Arc::new(AtomicBool::new(false));
            let handle = crate::monitor::spawn_monitor_thread(
                self.app_handle.clone(),
                self.monitor_buf.clone(),
                self.monitor_volume.clone(),
                device,
                stop.clone(),
            );
            *self.monitor_thread.lock().unwrap() = Some((stop, handle));
        }
        Ok(())
    }

    pub fn is_passthrough_enabled(&self) -> bool {
        self.passthrough_enabled.load(Ordering::SeqCst)
    }

    /// The capture rate of the live stream, or 0 if it has never opened. Set from
    /// the spectrum callback (the rate arrives with the data — no extra plumbing).
    pub fn capture_sample_rate(&self) -> u32 {
        self.capture_sample_rate.load(Ordering::Relaxed)
    }

    pub fn update_selected_device(&self) -> Result<(), anyhow::Error> {
        // Serialize against set_passthrough/set_monitor_output_device: this
        // method also stops+respawns the monitor output thread below when
        // passthrough is live, so it shares the same lifecycle lock (FIX 3/5).
        let _lifecycle = self.monitor_lifecycle.lock().unwrap();

        // MonitorSource caches sample_rate at spawn time (rodio treats the span
        // as fixed), so a live passthrough session must be restarted around an
        // input-device swap or it keeps declaring the OLD rate to rodio —
        // audible pitch-shift until the user re-toggles. Force the respawned
        // thread to wait for the new rate, and clear stale pre-switch samples
        // out of the buffer so they don't survive the swap.
        let passthrough_was_active = self.passthrough_enabled.load(Ordering::SeqCst);
        if passthrough_was_active {
            self.monitor_buf.set_sample_rate(0);
            self.stop_monitor_thread();
            self.monitor_buf.set_active(false);
        }

        // If currently open, restart the microphone stream to use the new device
        if *self.is_open.lock().unwrap() {
            self.close_generation.fetch_add(1, Ordering::SeqCst);
            self.stop_microphone_stream();
            if let Err(e) = self.start_microphone_stream() {
                // Capture is gone — passthrough can't survive. Tear it down and
                // tell the UI, instead of leaving the toggle ON with a dead
                // monitor (the "silent lie" the monitor-error plumbing exists
                // to eliminate).
                if passthrough_was_active {
                    self.passthrough_enabled.store(false, Ordering::SeqCst);
                    self.monitor_buf.set_active(false);
                    crate::monitor::emit_monitor_error(
                        &self.app_handle,
                        &format!("Monitor stopped: capture device unavailable: {e}"),
                    );
                }
                return Err(e);
            }
        }

        if passthrough_was_active {
            self.monitor_buf.set_active(true);
            let stop = Arc::new(AtomicBool::new(false));
            let device = get_settings(&self.app_handle).monitor_output_device;
            let handle = crate::monitor::spawn_monitor_thread(
                self.app_handle.clone(),
                self.monitor_buf.clone(),
                self.monitor_volume.clone(),
                device,
                stop.clone(),
            );
            *self.monitor_thread.lock().unwrap() = Some((stop, handle));
        }
        Ok(())
    }

    /// Manual recovery from the Audio console's "Reconnect" button. Deliberately
    /// manual: auto-reconnect and auto-fallback-to-default were both rejected in
    /// the spec, because silently moving the user to a different microphone — or
    /// reaching into someone else's stream lifecycle — is the exact defect class
    /// that produced Epic C's Critical.
    ///
    /// Benign TOCTOU: `open` is read and the lock released before branching, so
    /// no state corruption is possible (both branches re-check under lock). But
    /// if another thread opens the stream in that window, the `else` branch's
    /// `start_microphone_stream()` short-circuits on the now-true `is_open` flag
    /// and returns `Ok(())` — Reconnect reports success without having done
    /// anything. Do not "fix" this by holding a guard across the call below;
    /// that is exactly the deadlock this method was written to avoid (see the
    /// comment on `open`).
    pub fn reconnect_input_device(&self) -> Result<(), anyhow::Error> {
        // Read the flag into a local so the `is_open` guard is dropped at the end
        // of THIS statement. `update_selected_device` takes `monitor_lifecycle`
        // and then `is_open`; holding `is_open` across the call would invert that
        // order and deadlock.
        let open = *self.is_open.lock().unwrap();

        if open {
            // Already the exact recovery sequence: stop + start the mic stream and
            // respawn the monitor output thread (which caches the sample rate).
            // `update_selected_device`'s only fallible step is its own call to
            // `start_microphone_stream` (verified by reading it end to end —
            // every other branch returns `Ok(())` unconditionally), which
            // already writes the health error slot on every error exit (fix
            // wave 3, `record_open_failure`) — see the "one writer, one
            // clearer" invariant on `reset()`. No re-write needed here.
            self.update_selected_device()
        } else {
            // The stream never opened (or was torn down) — just open it.
            // `start_microphone_stream` already records the failure reason
            // into the health error slot on every error exit — see the "one
            // writer, one clearer" invariant on `reset()`. A failed Reconnect
            // still leaves the card showing "unavailable" with the NEW
            // reason, just written one level down instead of here.
            self.start_microphone_stream()
        }
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                *state = RecordingState::Idle;
                drop(state);

                // Optionally keep recording for a bit longer to capture trailing audio
                let settings = get_settings(&self.app_handle);
                if settings.extra_recording_buffer_ms > 0 {
                    debug!(
                        "Extra recording buffer: sleeping {}ms before stopping",
                        settings.extra_recording_buffer_ms
                    );
                    std::thread::sleep(Duration::from_millis(settings.extra_recording_buffer_ms));
                }

                let samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    match rec.stop() {
                        Ok(buf) => buf,
                        Err(e) => {
                            error!("stop() failed: {e}");
                            Vec::new()
                        }
                    }
                } else {
                    error!("Recorder not available");
                    Vec::new()
                };

                *self.is_recording.lock().unwrap() = false;

                // In on-demand mode, close the mic (lazily if the setting is enabled).
                // Skipped while the Audio-console monitor wants the stream open
                // (meter and/or passthrough) — closing here would silently kill
                // both without ever clearing monitor_active.
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand)
                    && !self.monitor_active.load(Ordering::SeqCst)
                {
                    if get_settings(&self.app_handle).lazy_stream_close {
                        self.schedule_lazy_close();
                    } else {
                        self.stop_microphone_stream();
                    }
                }

                // Pad if very short
                let s_len = samples.len();
                // debug!("Got {} samples", s_len);
                if s_len < WHISPER_SAMPLE_RATE && s_len > 0 {
                    let mut padded = samples;
                    padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
                    Some(padded)
                } else {
                    Some(samples)
                }
            }
            _ => None,
        }
    }
    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    pub fn peek_recording(&self) -> Option<Vec<f32>> {
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.peek().ok()
        } else {
            None
        }
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Recording { .. } = *state {
            *state = RecordingState::Idle;
            drop(state);

            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                let _ = rec.stop(); // Discard the result
            }

            *self.is_recording.lock().unwrap() = false;

            // In on-demand mode, close the mic (lazily if the setting is enabled).
            // Skipped while the Audio-console monitor wants the stream open (see
            // the identical guard in stop_recording).
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand)
                && !self.monitor_active.load(Ordering::SeqCst)
            {
                if get_settings(&self.app_handle).lazy_stream_close {
                    self.schedule_lazy_close();
                } else {
                    self.stop_microphone_stream();
                }
            }
        }
    }
}
