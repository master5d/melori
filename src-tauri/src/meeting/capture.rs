// ── C1.0 SPIKE FINDING (compile-time only; runtime is the user's manual check) ──
// (a) `cargo check --tests --lib` COMPILES the cpal-0.16 loopback path with zero
//     errors and zero warnings on THIS new code (only pre-existing, unrelated
//     warnings elsewhere). cpal 0.16's `build_input_stream` accepted the default
//     OUTPUT (render) device + its `default_output_config().into()` StreamConfig
//     as an INPUT stream — i.e. the WASAPI loopback API surface is viable here.
// (b) Chosen C1.3 capture path: cpal-loopback (the maintained cpal 0.16 WASAPI
//     loopback), PENDING the user's manual runtime check (Step 4). Only fall back
//     to a direct `windows`-crate WASAPI loopback impl if that runtime check
//     returns a silent (peak == 0.0) stream.
// (c) Sample-format handling: NONE needed at compile time. The spike assumes the
//     default output config delivers `f32` samples (the common WASAPI shared-mode
//     format) and the `data: &[f32]` callback type-checked. If the user's runtime
//     check panics with a sample-format mismatch, match `config.sample_format()`
//     and dispatch to `build_input_stream::<i16|u16|f32, _, _>` with a per-format
//     conversion to f32 before computing the peak. Not required to compile.
// ────────────────────────────────────────────────────────────────────────────────
//! Dual capture: mic (`me`) + WASAPI loopback (`others`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SizedSample};

use crate::audio_toolkit::audio::FrameResampler;
use crate::meeting::Source;

/// Re-chunks variable-length cpal callback buffers into fixed-size frames. Pure + tested.
pub struct FrameChunker {
    frame: usize,
    acc: Vec<f32>,
}
impl FrameChunker {
    pub fn new(frame: usize) -> Self {
        Self {
            frame,
            acc: Vec::new(),
        }
    }
    pub fn push(&mut self, src: &[f32], mut emit: impl FnMut(&[f32])) {
        self.acc.extend_from_slice(src);
        while self.acc.len() >= self.frame {
            let rest = self.acc.split_off(self.frame);
            emit(&self.acc);
            self.acc = rest;
        }
    }
}

/// Frame duration that yields `frame_samples` at `engine_hz` — what `FrameResampler` expects.
fn frame_duration(engine_hz: usize, frame_samples: usize) -> Duration {
    Duration::from_millis((frame_samples as u64 * 1000) / engine_hz as u64)
}

/// A running pair of capture streams. Dropping it stops capture (RAII).
pub struct CaptureHandle {
    pub loopback_active: bool,
    _streams: Vec<cpal::Stream>, // kept alive; dropped = stopped
}

/// Open mic (`Me`) and, when available, loopback (`Others`). Each source's resampled, fixed-size
/// frames are delivered to `on_frame(source, frame)`. Loopback failure is non-fatal (returns
/// `loopback_active=false`, mic-only).
pub fn start_capture(
    engine_hz: usize,
    frame_samples: usize,
    mic_name: Option<&str>,
    on_frame: impl Fn(Source, &[f32]) + Send + Sync + 'static,
) -> anyhow::Result<CaptureHandle> {
    let on_frame: Arc<dyn Fn(Source, &[f32]) + Send + Sync + 'static> = Arc::new(on_frame);
    let host = crate::audio_toolkit::get_cpal_host();
    let mut streams: Vec<cpal::Stream> = Vec::new();

    // ── Mic (Source::Me) — the microphone chosen in settings, as dictation uses it;
    // the system default only when none is chosen or the chosen one is gone. ──
    let mic_device = match mic_name.and_then(find_input_device) {
        Some(device) => device,
        None => {
            if let Some(name) = mic_name {
                log::warn!("meeting capture: selected microphone {name:?} not found, using the system default");
            }
            host.default_input_device()
                .ok_or_else(|| anyhow::anyhow!("no default input device"))?
        }
    };
    let mic_config = mic_device
        .default_input_config()
        .map_err(|e| anyhow::anyhow!("failed to fetch mic input config: {e}"))?;
    log::info!(
        "meeting capture mic: device={:?} rate={} channels={} format={:?}",
        mic_device.name(),
        mic_config.sample_rate().0,
        mic_config.channels(),
        mic_config.sample_format()
    );
    let mic_stream = build_source_stream(
        &mic_device,
        &mic_config,
        Source::Me,
        engine_hz,
        frame_samples,
        on_frame.clone(),
    )
    .map_err(|e| anyhow::anyhow!("failed to build mic stream: {e}"))?;
    mic_stream
        .play()
        .map_err(|e| anyhow::anyhow!("failed to start mic stream: {e}"))?;
    streams.push(mic_stream);

    // ── Loopback (Source::Others) — default OUTPUT device opened as an INPUT
    //    stream (the C1.0 spike pattern). Non-fatal: on any failure we log and
    //    run mic-only. ──
    let loopback_active = match open_loopback(&host, engine_hz, frame_samples, on_frame.clone()) {
        Ok(stream) => {
            streams.push(stream);
            true
        }
        Err(e) => {
            log::warn!("meeting loopback capture unavailable, running mic-only: {e}");
            false
        }
    };

    Ok(CaptureHandle {
        loopback_active,
        _streams: streams,
    })
}

/// Open the default output (render) device as a loopback input stream and start it.
fn open_loopback(
    host: &cpal::Host,
    engine_hz: usize,
    frame_samples: usize,
    on_frame: Arc<dyn Fn(Source, &[f32]) + Send + Sync + 'static>,
) -> anyhow::Result<cpal::Stream> {
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no default output device for loopback"))?;
    let config = device
        .default_output_config()
        .map_err(|e| anyhow::anyhow!("failed to fetch output config: {e}"))?;
    log::info!(
        "meeting capture loopback: device={:?} rate={} channels={} format={:?}",
        device.name(),
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );
    let stream = build_source_stream(
        &device,
        &config,
        Source::Others,
        engine_hz,
        frame_samples,
        on_frame,
    )
    .map_err(|e| anyhow::anyhow!("failed to build loopback input stream: {e}"))?;
    stream
        .play()
        .map_err(|e| anyhow::anyhow!("failed to start loopback stream: {e}"))?;
    Ok(stream)
}

/// Match `config.sample_format()` and dispatch to the typed `build_input_stream` builder,
/// converting non-f32 samples to f32 before resample/chunk (mirrors recorder.rs).
fn build_source_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    source: Source,
    engine_hz: usize,
    frame_samples: usize,
    on_frame: Arc<dyn Fn(Source, &[f32]) + Send + Sync + 'static>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let in_hz = config.sample_rate().0 as usize;
    let channels = config.channels() as usize;
    match config.sample_format() {
        cpal::SampleFormat::U8 => build_typed_stream::<u8>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        cpal::SampleFormat::I8 => build_typed_stream::<i8>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        cpal::SampleFormat::I16 => build_typed_stream::<i16>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        cpal::SampleFormat::U16 => build_typed_stream::<u16>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        cpal::SampleFormat::I32 => build_typed_stream::<i32>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        cpal::SampleFormat::F32 => build_typed_stream::<f32>(
            device,
            config,
            source,
            in_hz,
            channels,
            engine_hz,
            frame_samples,
            on_frame,
        ),
        other => {
            log::error!("meeting capture: unsupported sample format {other:?}");
            Err(cpal::BuildStreamError::StreamConfigNotSupported)
        }
    }
}

/// Build a typed input stream whose callback downmixes to mono f32, resamples to `engine_hz`,
/// re-chunks to `frame_samples`, and delivers each frame to `on_frame(source, frame)`.
/// The per-stream `FrameResampler` + `FrameChunker` need `&mut` across cpal's `Fn` callback,
/// so they live behind a `Mutex` captured by the `move` closure (the standard cpal pattern).
#[allow(clippy::too_many_arguments)]
fn build_typed_stream<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    source: Source,
    in_hz: usize,
    channels: usize,
    engine_hz: usize,
    frame_samples: usize,
    on_frame: Arc<dyn Fn(Source, &[f32]) + Send + Sync + 'static>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: Sample + SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let resampler = FrameResampler::new(in_hz, engine_hz, frame_duration(engine_hz, frame_samples));
    let chunker = FrameChunker::new(frame_samples);
    let state = Mutex::new((resampler, chunker, Vec::<f32>::new()));

    let stream_cb = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let (resampler, chunker, mono) = &mut *guard;

        // Downmix interleaved samples to mono f32 (mirrors recorder.rs).
        mono.clear();
        if channels <= 1 {
            mono.extend(data.iter().map(|&s| s.to_sample::<f32>()));
        } else {
            mono.reserve(data.len() / channels);
            for frame in data.chunks_exact(channels) {
                let sum: f32 = frame.iter().map(|&s| s.to_sample::<f32>()).sum();
                mono.push(sum / channels as f32);
            }
        }

        let mono_snapshot = std::mem::take(mono);
        resampler.push(&mono_snapshot, |resampled| {
            chunker.push(resampled, |frame| on_frame(source, frame));
        });
        *mono = mono_snapshot;
    };

    device.build_input_stream(
        &config.clone().into(),
        stream_cb,
        |err| log::error!("meeting capture stream error: {err}"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_chunk_to_fixed_size() {
        let mut out: Vec<Vec<f32>> = Vec::new();
        let mut r = FrameChunker::new(4);
        r.push(&[1.0, 2.0, 3.0], |f| out.push(f.to_vec()));
        r.push(&[4.0, 5.0, 6.0, 7.0, 8.0], |f| out.push(f.to_vec()));
        assert_eq!(
            out,
            vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]]
        );
    }

    #[test]
    fn chunker_holds_partial_until_full() {
        let mut out: Vec<Vec<f32>> = Vec::new();
        let mut r = FrameChunker::new(4);
        r.push(&[1.0, 2.0], |f| out.push(f.to_vec()));
        assert!(
            out.is_empty(),
            "2 samples < frame size 4 → nothing emitted yet"
        );
        r.push(&[3.0, 4.0, 5.0], |f| out.push(f.to_vec()));
        assert_eq!(out, vec![vec![1.0, 2.0, 3.0, 4.0]]); // one full frame, 5.0 retained
    }
}

fn find_input_device(name: &str) -> Option<cpal::Device> {
    let devices = crate::audio_toolkit::list_input_devices().ok()?;
    let names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
    let index = pick_by_name(&names, name)?;
    devices.into_iter().nth(index).map(|d| d.device)
}

/// Index of the device whose name matches the setting exactly.
fn pick_by_name(names: &[String], wanted: &str) -> Option<usize> {
    names.iter().position(|n| n == wanted)
}

#[cfg(test)]
mod mic_selection_tests {
    use super::pick_by_name;

    #[test]
    fn the_selected_microphone_is_found_by_exact_name() {
        let names = vec![
            "Microphone (Camo)".to_string(),
            "Surface Stereo Microphones".to_string(),
        ];
        assert_eq!(pick_by_name(&names, "Surface Stereo Microphones"), Some(1));
    }

    #[test]
    fn a_missing_microphone_falls_back_to_the_default() {
        let names = vec!["Microphone (Camo)".to_string()];
        assert_eq!(pick_by_name(&names, "Surface Stereo Microphones"), None);
    }
}
