//! Audio-console monitor / passthrough output (Epic C). A dedicated thread owns
//! the rodio output stream (`!Send`) and plays a `MonitorSource` fed by the
//! shared `MonitorBuf` the capture consumer pushes into. The thread parks until
//! its stop flag is set, then drops the stream to end playback.
use crate::audio_toolkit::{apply_monitor, MonitorBuf, MonitorRamp};
use log::{error, info};
use rodio::Sink;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::Emitter;

/// Tauri event emitted when the monitor output thread ends abnormally (device
/// open failure, or the capture sample rate never became known) — the spec's
/// promised "graceful stop + warning toast" path. Payload is a user-safe
/// message string; the frontend maps it to a toast and flips the toggle off.
const MONITOR_ERROR_EVENT: &str = "monitor-error";

/// Fade-in duration for the mute-ramp (soft start, no instant howl).
const RAMP_MS: u64 = 300;
/// Max time to wait for the capture rate to be known before giving up.
const RATE_WAIT_MS: u64 = 2_000;

/// A rodio `Source` that pulls captured samples from a `MonitorBuf`, applies a
/// fade-in ramp × monitor volume, and clamps. Mono (the capture frame is
/// downmixed); rodio resamples to the output device. Never returns `None`
/// (underrun → silence), so the stream stays alive until the thread drops it.
struct MonitorSource {
    buf: Arc<MonitorBuf>,
    ramp: MonitorRamp,
    volume: Arc<AtomicU32>,
    sample_rate: u32,
}

impl Iterator for MonitorSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // Underrun: emit silence WITHOUT advancing the ramp. rodio pulls in
        // realtime whether or not the buffer has data, so if the output thread
        // starts pulling before cpal's first capture callback lands (warm-up is
        // ~10-200ms, longer on Bluetooth/USB), advancing the ramp against zeros
        // would burn the whole fade-in down to silence and let the first REAL
        // audio arrive at factor ~= 1.0 — the anti-howl soft start would be
        // gone. Freezing here means the ramp only fades in audible signal.
        let Some(s) = self.buf.next_sample() else {
            return Some(0.0);
        };
        let factor = self.ramp.factor();
        let volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
        Some(apply_monitor(s, factor, volume))
    }
}

impl rodio::Source for MonitorSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        1
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Emit `monitor-error` to the main window with a user-facing message. Match
/// the `input-monitor-level` emit pattern in `managers/audio.rs`: same
/// `Emitter` trait, same `"main"` window label.
pub(crate) fn emit_monitor_error(app: &tauri::AppHandle, message: &str) {
    let _ = app.emit_to("main", MONITOR_ERROR_EVENT, message.to_string());
}

/// Spawn the monitor output thread. It waits for the capture rate to be known,
/// opens the output stream on `device`, plays a fresh-ramp `MonitorSource`, and
/// parks until `stop` is set. On rate-wait timeout or output-open failure, it
/// logs AND emits `monitor-error` so the frontend can surface a warning toast
/// and flip the toggle off instead of silently leaving passthrough "on" with
/// no audio (the spec's device-disconnect edge case).
pub fn spawn_monitor_thread(
    app: tauri::AppHandle,
    buf: Arc<MonitorBuf>,
    volume: Arc<AtomicU32>,
    device: Option<String>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        // Wait for the capture consumer to publish the sample rate.
        let mut waited = 0;
        while buf.sample_rate() == 0 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
            waited += 10;
            if waited >= RATE_WAIT_MS {
                let msg = "Monitor: capture sample rate never became known; aborting";
                error!("{msg}");
                emit_monitor_error(&app, msg);
                return;
            }
        }
        let sample_rate = buf.sample_rate();

        // A mid-session output-device loss (headphones yanked) surfaces ONLY
        // through cpal's error callback — the open-time Result cannot see it.
        // Stop this thread and reuse the monitor-error pipe Epic C already built:
        // the frontend listens, flips the toggle off, and toasts.
        let on_error: Arc<dyn Fn(String) + Send + Sync> = {
            let app = app.clone();
            let stop = stop.clone();
            Arc::new(move |msg: String| {
                stop.store(true, Ordering::SeqCst);
                emit_monitor_error(&app, &msg);
            })
        };

        let stream_handle =
            match crate::platform::audio_feedback::resolve_output_stream(device, Some(on_error)) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("Monitor: failed to open output stream: {e}");
                    error!("{msg}");
                    emit_monitor_error(&app, &msg);
                    return;
                }
            };
        let source = MonitorSource {
            buf,
            ramp: MonitorRamp::new(sample_rate as u64 * RAMP_MS / 1000),
            volume,
            sample_rate,
        };
        let sink = Sink::connect_new(stream_handle.mixer());
        sink.append(source);
        info!("Monitor passthrough started at {sample_rate} Hz");

        // Park until stopped; then drop the sink + stream to end playback.
        while !stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
        }
        drop(sink);
        drop(stream_handle);
        info!("Monitor passthrough stopped");
    })
}
