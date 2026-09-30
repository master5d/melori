use std::{
    io::Error,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SizedSample,
};

use crate::{
    audio::{AudioVisualiser, FrameResampler},
    constants,
    vad::{self, VadFrame},
    VoiceActivityDetector,
};

use super::health::DeviceHealthState;
use super::monitor::MonitorBuf;
use super::spectrum::SpectrumAnalyzer;

/// How long `run_consumer` will wait for a chunk before waking up to look at its
/// command channel anyway. Short enough that `close()` is never perceptibly slow
/// on a dead device; long enough that a healthy stream (buffers every ~10–20 ms)
/// essentially never hits it.
const CONSUMER_POLL: Duration = Duration::from_millis(100);

enum Cmd {
    Start,
    Stop(mpsc::Sender<Vec<f32>>),
    Peek(mpsc::Sender<Vec<f32>>),
    Shutdown,
}

enum AudioChunk {
    Samples(Vec<f32>),
    EndOfStream,
}

pub struct AudioRecorder {
    device: Option<Device>,
    cmd_tx: Option<mpsc::Sender<Cmd>>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    meter_cb: Option<Arc<dyn Fn(f32, f32) + Send + Sync + 'static>>,
    gain: Option<Arc<AtomicU32>>,
    monitor: Option<Arc<MonitorBuf>>,
    #[allow(clippy::type_complexity)]
    spectrum: Option<(
        Arc<AtomicBool>,
        Arc<dyn Fn(Vec<f32>, u32) + Send + Sync + 'static>,
    )>,
    health: Option<Arc<DeviceHealthState>>,
}

impl AudioRecorder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AudioRecorder {
            device: None,
            cmd_tx: None,
            worker_handle: None,
            vad: None,
            level_cb: None,
            meter_cb: None,
            gain: None,
            monitor: None,
            spectrum: None,
            health: None,
        })
    }

    pub fn with_vad(mut self, vad: Box<dyn VoiceActivityDetector>) -> Self {
        self.vad = Some(Arc::new(Mutex::new(vad)));
        self
    }

    pub fn with_level_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.level_cb = Some(Arc::new(cb));
        self
    }

    pub fn with_meter_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(f32, f32) + Send + Sync + 'static,
    {
        self.meter_cb = Some(Arc::new(cb));
        self
    }

    pub fn with_gain(mut self, gain: Arc<AtomicU32>) -> Self {
        self.gain = Some(gain);
        self
    }

    pub fn with_monitor(mut self, monitor: Arc<MonitorBuf>) -> Self {
        self.monitor = Some(monitor);
        self
    }

    /// Attach the Audio-console spectrum analyzer. `gate` is checked BEFORE the
    /// FFT runs, so the analyzer costs nothing while the Audio section is closed.
    pub fn with_spectrum<F>(mut self, gate: Arc<AtomicBool>, cb: F) -> Self
    where
        F: Fn(Vec<f32>, u32) + Send + Sync + 'static,
    {
        self.spectrum = Some((gate, Arc::new(cb)));
        self
    }

    /// Attach the Audio-console device-health state (Epic D2). The consumer
    /// records the PRE-gain peak into it every frame; `open()` publishes the
    /// negotiated device config; cpal's error callback writes stream failures.
    pub fn with_health(mut self, health: Arc<DeviceHealthState>) -> Self {
        self.health = Some(health);
        self
    }

    pub fn open(&mut self, device: Option<Device>) -> Result<(), Box<dyn std::error::Error>> {
        if self.worker_handle.is_some() {
            return Ok(()); // already open
        }

        let (sample_tx, sample_rx) = mpsc::channel::<AudioChunk>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (init_tx, init_rx) = mpsc::sync_channel::<Result<(), String>>(1);

        let host = crate::get_cpal_host();
        let device = match device {
            Some(dev) => dev,
            None => host
                .default_input_device()
                .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "No input device found"))?,
        };

        let thread_device = device.clone();
        let vad = self.vad.clone();
        // Move the optional level callback into the worker thread
        let level_cb = self.level_cb.clone();
        let meter_cb = self.meter_cb.clone();
        let gain = self.gain.clone();
        let monitor = self.monitor.clone();
        let spectrum = self.spectrum.clone();
        // Seed the frame clock and drop the previous stream's facts. The error
        // slot is NOT cleared here — see `DeviceHealthState::begin_open`. It is
        // cleared below, only once the worker reports the stream is actually
        // running, so a Reconnect against a still-dead device never opens an
        // `Ok` window for the meter to un-blank into.
        if let Some(h) = &self.health {
            h.begin_open();
        }
        let health = self.health.clone();

        let worker = std::thread::spawn(move || {
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_flag_for_stream = stop_flag.clone();
            let init_result = (|| -> Result<(cpal::Stream, u32), String> {
                let config = AudioRecorder::get_preferred_config(&thread_device)
                    .map_err(|e| format!("Failed to fetch preferred config: {e}"))?;

                let sample_rate = config.sample_rate().0;
                let channels = config.channels() as usize;

                log::info!(
                    "Using device: {:?}\nSample rate: {}\nChannels: {}\nFormat: {:?}",
                    thread_device.name(),
                    sample_rate,
                    channels,
                    config.sample_format()
                );

                if let Some(h) = &health {
                    h.set_facts(super::health::DeviceFacts {
                        name: thread_device.name().unwrap_or_else(|_| "Unknown".into()),
                        sample_rate,
                        channels: channels as u16,
                        sample_format: format!("{:?}", config.sample_format()),
                    });
                }

                let stream = match config.sample_format() {
                    cpal::SampleFormat::U8 => AudioRecorder::build_stream::<u8>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                        health.clone(),
                    )
                    .map_err(|e| format!("Failed to build input stream: {e}"))?,
                    cpal::SampleFormat::I8 => AudioRecorder::build_stream::<i8>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                        health.clone(),
                    )
                    .map_err(|e| format!("Failed to build input stream: {e}"))?,
                    cpal::SampleFormat::I16 => AudioRecorder::build_stream::<i16>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                        health.clone(),
                    )
                    .map_err(|e| format!("Failed to build input stream: {e}"))?,
                    cpal::SampleFormat::I32 => AudioRecorder::build_stream::<i32>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                        health.clone(),
                    )
                    .map_err(|e| format!("Failed to build input stream: {e}"))?,
                    cpal::SampleFormat::F32 => AudioRecorder::build_stream::<f32>(
                        &thread_device,
                        &config,
                        sample_tx,
                        channels,
                        stop_flag_for_stream,
                        health.clone(),
                    )
                    .map_err(|e| format!("Failed to build input stream: {e}"))?,
                    sample_format => {
                        return Err(format!("Unsupported sample format: {sample_format:?}"));
                    }
                };

                stream
                    .play()
                    .map_err(|e| format!("Failed to start microphone stream: {e}"))?;

                Ok((stream, sample_rate))
            })();

            match init_result {
                Ok((stream, sample_rate)) => {
                    let _ = init_tx.send(Ok(()));
                    // Keep the stream alive while we process samples.
                    run_consumer(
                        sample_rate,
                        vad,
                        sample_rx,
                        cmd_rx,
                        level_cb,
                        meter_cb,
                        gain,
                        monitor,
                        spectrum,
                        health,
                        stop_flag,
                    );
                    drop(stream);
                }
                Err(error_message) => {
                    log::error!("{error_message}");
                    let _ = init_tx.send(Err(error_message));
                }
            }
        });

        match init_rx.recv() {
            Ok(Ok(())) => {
                // THE confirmation point: the worker built the stream and
                // `play()` returned. Only now — with the stream proven open and
                // running — may the previous failure be forgotten. This is the
                // single clearer of the error slot in the whole codebase.
                if let Some(h) = &self.health {
                    h.clear_error();
                }
                self.device = Some(device);
                self.cmd_tx = Some(cmd_tx);
                self.worker_handle = Some(worker);
                Ok(())
            }
            Ok(Err(error_message)) => {
                let _ = worker.join();
                let kind = if is_microphone_access_denied(&error_message) {
                    std::io::ErrorKind::PermissionDenied
                } else {
                    std::io::ErrorKind::Other
                };
                Err(Box::new(Error::new(kind, error_message)))
            }
            Err(recv_error) => {
                let _ = worker.join();
                Err(Box::new(Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to initialize microphone worker: {recv_error}"),
                )))
            }
        }
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Start)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Stop(resp_tx))?;
        }
        Ok(resp_rx.recv()?) // wait for the samples
    }

    pub fn peek(&self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Peek(resp_tx))?;
        }
        Ok(resp_rx.recv()?)
    }

    pub fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
        self.device = None;
        Ok(())
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        sample_tx: mpsc::Sender<AudioChunk>,
        channels: usize,
        stop_flag: Arc<AtomicBool>,
        health: Option<Arc<DeviceHealthState>>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError>
    where
        T: Sample + SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let mut output_buffer = Vec::new();
        let mut eos_sent = false;

        let stream_cb = move |data: &[T], _: &cpal::InputCallbackInfo| {
            if stop_flag.load(Ordering::Relaxed) {
                if !eos_sent {
                    let _ = sample_tx.send(AudioChunk::EndOfStream);
                    eos_sent = true;
                }
                return;
            }
            eos_sent = false;

            output_buffer.clear();

            if channels == 1 {
                output_buffer.extend(data.iter().map(|&sample| sample.to_sample::<f32>()));
            } else {
                let frame_count = data.len() / channels;
                output_buffer.reserve(frame_count);

                for frame in data.chunks_exact(channels) {
                    let mono_sample = frame
                        .iter()
                        .map(|&sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / channels as f32;
                    output_buffer.push(mono_sample);
                }
            }

            if sample_tx
                .send(AudioChunk::Samples(output_buffer.clone()))
                .is_err()
            {
                log::error!("Failed to send samples");
            }
        };

        device.build_input_stream(
            &config.clone().into(),
            stream_cb,
            move |err| {
                // Not a dead end any more: this is how a mid-session unplug
                // (StreamError::DeviceNotAvailable) reaches the UI.
                log::error!("Stream error: {}", err);
                if let Some(h) = &health {
                    h.set_error(err.to_string());
                }
            },
            None,
        )
    }

    fn get_preferred_config(
        device: &cpal::Device,
    ) -> Result<cpal::SupportedStreamConfig, Box<dyn std::error::Error>> {
        // Use the device's native/default sample rate and let the FrameResampler
        // in run_consumer() downsample to 16kHz. This avoids forcing hardware into
        // a non-native rate which can cause issues on some devices (Bluetooth
        // codecs, certain ALSA drivers, etc.).
        let default_config = device.default_input_config()?;
        let target_rate = default_config.sample_rate();

        // Try to find the best sample format at the device's default rate
        let supported_configs = match device.supported_input_configs() {
            Ok(configs) => configs,
            Err(e) => {
                log::warn!("Could not enumerate input configs ({e}), using device default");
                return Ok(default_config);
            }
        };
        let mut best_config: Option<cpal::SupportedStreamConfigRange> = None;

        for config_range in supported_configs {
            if config_range.min_sample_rate() <= target_rate
                && config_range.max_sample_rate() >= target_rate
            {
                match best_config {
                    None => best_config = Some(config_range),
                    Some(ref current) => {
                        // Prioritize F32 > I16 > I32 > others
                        let score = |fmt: cpal::SampleFormat| match fmt {
                            cpal::SampleFormat::F32 => 4,
                            cpal::SampleFormat::I16 => 3,
                            cpal::SampleFormat::I32 => 2,
                            _ => 1,
                        };

                        if score(config_range.sample_format()) > score(current.sample_format()) {
                            best_config = Some(config_range);
                        }
                    }
                }
            }
        }

        if let Some(config) = best_config {
            return Ok(config.with_sample_rate(target_rate));
        }

        // Fall back to device default if no config matched (exotic/virtual devices)
        log::warn!(
            "No supported config matched device default rate {:?}, using default config",
            target_rate
        );
        Ok(default_config)
    }
}

pub fn is_microphone_access_denied(error_message: &str) -> bool {
    let normalized = error_message.to_lowercase();
    normalized.contains("access is denied")
        || normalized.contains("permission denied")
        || normalized.contains("0x80070005")
}

pub fn is_no_input_device_error(error_message: &str) -> bool {
    let normalized = error_message.to_lowercase();
    normalized.contains("no input device found")
        || (normalized.contains("failed to fetch preferred config")
            && normalized.contains("coreaudio"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_access_is_denied() {
        assert!(is_microphone_access_denied("Access is denied"));
    }

    #[test]
    fn detects_permission_denied() {
        assert!(is_microphone_access_denied("permission denied"));
    }

    #[test]
    fn detects_windows_error_code() {
        assert!(is_microphone_access_denied("WASAPI error: 0x80070005"));
    }

    #[test]
    fn does_not_match_unrelated_errors() {
        assert!(!is_microphone_access_denied("device not found"));
    }

    #[test]
    fn detects_no_input_device() {
        assert!(is_no_input_device_error("No input device found"));
    }

    #[test]
    fn detects_coreaudio_config_error() {
        assert!(is_no_input_device_error(
            "Failed to fetch preferred config: A backend-specific error has occurred: An unknown error unknown to the coreaudio-rs API occurred"
        ));
    }

    #[test]
    fn does_not_match_other_errors_for_no_device() {
        assert!(!is_no_input_device_error("permission denied"));
        assert!(!is_no_input_device_error("device not found"));
    }

    #[test]
    fn health_peak_is_measured_before_gain() {
        use super::super::health::DeviceHealthState;
        use std::sync::atomic::AtomicU32;

        let (sample_tx, sample_rx) = mpsc::channel();
        let (_cmd_tx, cmd_rx) = mpsc::channel();

        let health = Arc::new(DeviceHealthState::new());
        health.begin_open();

        // The input-gain slider's minimum: -24 dB = x0.063. If the health tap read
        // the POST-gain frame, a 0.5 peak would arrive as ~0.0316 (-30 dBFS) and,
        // with a quieter real mic, would fall under the -60 dBFS silence floor —
        // the app would call a healthy microphone dead.
        let gain = Arc::new(AtomicU32::new(
            super::super::gain::db_to_linear(-24.0).to_bits(),
        ));

        sample_tx
            .send(AudioChunk::Samples(vec![0.5, -0.25, 0.1]))
            .unwrap();
        drop(sample_tx); // closes the channel, so run_consumer's loop breaks

        run_consumer(
            48_000,
            None, // vad
            sample_rx,
            cmd_rx,
            None, // level_cb
            None, // meter_cb
            Some(gain),
            None, // monitor
            None, // spectrum
            Some(health.clone()),
            Arc::new(AtomicBool::new(false)),
        );

        let peak = health.snapshot().peak;
        assert!(
            (peak - 0.5).abs() < 1e-6,
            "health must see the RAW peak 0.5, not the gain-attenuated one; got {peak}"
        );
    }

    /// The `stalled` deadlock (Epic D2 Critical).
    ///
    /// `stalled` means, by construction, "no chunk for >= 2 s AND no cpal error".
    /// No cpal error means cpal's processing thread never broke, so its data
    /// callback still owns `sample_tx` — the channel is NOT disconnected, it is
    /// merely silent. If the consumer blocks on a bare `recv()`, `Cmd::Shutdown`
    /// can never be observed, `close()` joins the worker forever, and every lock
    /// it holds (`is_open`, `did_mute`, `recorder`, `monitor_lifecycle`) is held
    /// for the life of the process — which is exactly the state the Reconnect
    /// button is offered in.
    ///
    /// The live sender below is load-bearing: drop it and the channel
    /// disconnects, the loop breaks for the WRONG reason, and this test would
    /// pass against the broken code, proving nothing.
    #[test]
    fn shutdown_is_observed_when_no_audio_ever_arrives() {
        let (sample_tx, sample_rx) = mpsc::channel::<AudioChunk>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

        // No samples. Ever. Just the shutdown command.
        cmd_tx.send(Cmd::Shutdown).unwrap();

        let (done_tx, done_rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            run_consumer(
                48_000,
                None, // vad
                sample_rx,
                cmd_rx,
                None, // level_cb
                None, // meter_cb
                None, // gain
                None, // monitor
                None, // spectrum
                None, // health
                Arc::new(AtomicBool::new(false)),
            );
            let _ = done_tx.send(());
        });

        let returned = done_rx.recv_timeout(Duration::from_secs(5)).is_ok();

        // Only now — after the assertion's evidence has been gathered — may the
        // producer side go away.
        drop(sample_tx);

        assert!(
            returned,
            "run_consumer must observe Cmd::Shutdown while the device is silent \
             but connected; it did not return within 5s (the `stalled` deadlock)"
        );
    }
}

fn run_consumer(
    in_sample_rate: u32,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    sample_rx: mpsc::Receiver<AudioChunk>,
    cmd_rx: mpsc::Receiver<Cmd>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    meter_cb: Option<Arc<dyn Fn(f32, f32) + Send + Sync + 'static>>,
    gain: Option<Arc<AtomicU32>>,
    monitor: Option<Arc<MonitorBuf>>,
    #[allow(clippy::type_complexity)] spectrum: Option<(
        Arc<AtomicBool>,
        Arc<dyn Fn(Vec<f32>, u32) + Send + Sync + 'static>,
    )>,
    health: Option<Arc<DeviceHealthState>>,
    stop_flag: Arc<AtomicBool>,
) {
    if let Some(m) = &monitor {
        m.set_sample_rate(in_sample_rate);
    }

    let mut frame_resampler = FrameResampler::new(
        in_sample_rate as usize,
        constants::WHISPER_SAMPLE_RATE as usize,
        Duration::from_millis(30),
    );

    let mut processed_samples = Vec::<f32>::new();
    let mut recording = false;

    // ---------- spectrum visualisation setup ---------------------------- //
    const BUCKETS: usize = 16;
    const WINDOW_SIZE: usize = 512;
    let mut visualizer = AudioVisualiser::new(
        in_sample_rate,
        WINDOW_SIZE,
        BUCKETS,
        400.0,  // vocal_min_hz
        4000.0, // vocal_max_hz
    );

    // The Audio-console spectrum analyzer (Epic D1). Built with the real capture
    // rate; only when someone is listening.
    let mut spectrum_analyzer = spectrum
        .as_ref()
        .map(|_| SpectrumAnalyzer::new(in_sample_rate));

    fn handle_frame(
        samples: &[f32],
        recording: bool,
        vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
        out_buf: &mut Vec<f32>,
    ) {
        if !recording {
            return;
        }

        if let Some(vad_arc) = vad {
            let mut det = vad_arc.lock().unwrap();
            match det.push_frame(samples).unwrap_or(VadFrame::Speech(samples)) {
                VadFrame::Speech(buf) => out_buf.extend_from_slice(buf),
                VadFrame::Noise => {}
            }
        } else {
            out_buf.extend_from_slice(samples);
        }
    }

    loop {
        // BOUNDED wait, not `recv()`. A stalled device is a device whose cpal
        // callback is still alive (no error was reported) and therefore still
        // owns `sample_tx` — the channel is not disconnected, only silent. A
        // blocking `recv()` there parks this thread forever, `Cmd::Shutdown` is
        // never observed, and `AudioRecorder::close()` joins us for the life of
        // the process while holding `is_open`/`did_mute`/`recorder`. Waking
        // every POLL_MS is what lets the command drain below run even when no
        // audio is arriving.
        match sample_rx.recv_timeout(CONSUMER_POLL) {
            Ok(AudioChunk::Samples(mut raw)) => {
                // ---------- device health (Epic D2) ---------------------- //
                // PRE-GAIN, and it must stay here. The gain slider goes to -24 dB
                // (x0.063): read post-gain, a healthy mic at -40 dBFS would measure
                // -64 dBFS, fall under the -60 dBFS silence floor, and the app would
                // report a working microphone as dead. The meter below is post-gain on
                // purpose (it shows what enters the pipeline); health watches the
                // device. Do not "tidy" this to sit with the other taps.
                if let Some(h) = &health {
                    let (peak, _rms) = super::meter::frame_peak_rms(&raw);
                    h.record_frame(peak);
                }

                // ---------- input gain (first stage: preamp) ------------- //
                if let Some(g) = &gain {
                    super::gain::apply_gain(&mut raw, f32::from_bits(g.load(Ordering::Relaxed)));
                }

                // ---------- spectrum processing ------------------------- //
                if let Some(buckets) = visualizer.feed(&raw) {
                    if let Some(cb) = &level_cb {
                        cb(buckets);
                    }
                }

                // ---------- meter (true peak/RMS from raw) -------------- //
                if let Some(cb) = &meter_cb {
                    let (peak, rms) = super::meter::frame_peak_rms(&raw);
                    cb(peak, rms);
                }

                // ---------- monitor / passthrough tap (post-gain) ------- //
                if let Some(m) = &monitor {
                    m.push(&raw);
                }

                // ---------- spectrum analyzer (post-gain, gated on the Audio section) --- //
                if let (Some((gate, cb)), Some(an)) = (&spectrum, &mut spectrum_analyzer) {
                    if gate.load(Ordering::Relaxed) {
                        if let Some(bands) = an.feed(&raw) {
                            cb(bands, in_sample_rate);
                        }
                    } else {
                        // Gate closed (Audio section not open): drop the partial buffer so
                        // reopening the section does not flush stale audio. The FFT never runs.
                        an.reset();
                    }
                }

                // ---------- existing pipeline --------------------------- //
                frame_resampler.push(&raw, &mut |frame: &[f32]| {
                    handle_frame(frame, recording, &vad, &mut processed_samples)
                });
            }

            // The producer's stop sentinel. There is no frame to process, but we
            // must NOT `continue` — that would skip the command drain below, which
            // is precisely the shape of the bug this restructure removes. Falling
            // through costs one `try_recv` and closes the same hole here.
            Ok(AudioChunk::EndOfStream) => {}

            // No audio right now. This is the stalled-device case and it is
            // survivable BY DESIGN: fall through to the command drain.
            Err(mpsc::RecvTimeoutError::Timeout) => {}

            // The producer is genuinely gone (cpal's callback dropped the sender).
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Command drain — runs on EVERY iteration, chunk or no chunk. This is
        // the whole point of the bounded wait above.
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::Start => {
                    stop_flag.store(false, Ordering::Relaxed);
                    processed_samples.clear();
                    recording = true;
                    visualizer.reset();
                    if let Some(v) = &vad {
                        v.lock().unwrap().reset();
                    }
                }
                Cmd::Stop(reply_tx) => {
                    recording = false;
                    stop_flag.store(true, Ordering::Relaxed);

                    // Drain all remaining audio until the producer confirms end-of-stream.
                    // The cpal callback sees the stop flag, sends EndOfStream, and goes
                    // silent — guaranteeing every captured sample is in the channel
                    // ahead of the sentinel.
                    loop {
                        match sample_rx.recv_timeout(Duration::from_secs(2)) {
                            Ok(AudioChunk::Samples(remaining)) => {
                                frame_resampler.push(&remaining, &mut |frame: &[f32]| {
                                    handle_frame(frame, true, &vad, &mut processed_samples)
                                });
                            }
                            Ok(AudioChunk::EndOfStream) => break,
                            Err(_) => {
                                log::warn!("Timed out waiting for EndOfStream from audio callback");
                                break;
                            }
                        }
                    }

                    frame_resampler.finish(&mut |frame: &[f32]| {
                        handle_frame(frame, true, &vad, &mut processed_samples)
                    });

                    let _ = reply_tx.send(std::mem::take(&mut processed_samples));

                    // Resume the audio callback so the consumer loop can continue
                    // receiving chunks (important for always-on microphone mode).
                    stop_flag.store(false, Ordering::Relaxed);
                }
                Cmd::Peek(reply_tx) => {
                    let _ = reply_tx.send(processed_samples.clone());
                }
                Cmd::Shutdown => {
                    stop_flag.store(true, Ordering::Relaxed);
                    return;
                }
            }
        }
    }
}
