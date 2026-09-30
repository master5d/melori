//! A rodio Source fed by a shared StreamBuf jitter buffer (Lever-2 PCM streaming).
//! The HTTP thread pushes samples; the audio callback pulls via `next`. Underruns
//! silence-fill (StreamBuf::next_sample), so the callback never blocks or ends early.
use echo_voice::pcm::{StreamBuf, PCM_CHANNELS, PCM_SAMPLE_RATE};
use std::sync::{Arc, Condvar, Mutex};

/// Shared handle: (buffer, condvar-for-prebuffer-wait).
pub type SharedBuf = Arc<(Mutex<StreamBuf>, Condvar)>;

pub fn new_shared_buf() -> SharedBuf {
    Arc::new((Mutex::new(StreamBuf::new()), Condvar::new()))
}

/// Push samples from the streaming callback; notify any pre-buffer waiter.
pub fn push_samples(buf: &SharedBuf, s: &[i16]) {
    let (m, cv) = &**buf;
    m.lock().unwrap().push(s);
    cv.notify_all();
}

/// Mark the stream ended; notify waiters.
pub fn end_stream(buf: &SharedBuf) {
    let (m, cv) = &**buf;
    m.lock().unwrap().end();
    cv.notify_all();
}

/// Block until the buffer holds >= `min_samples` OR the stream ended.
pub fn wait_prebuffer(buf: &SharedBuf, min_samples: usize) {
    let (m, cv) = &**buf;
    let mut g = m.lock().unwrap();
    while g.len() < min_samples && !g.is_ended() {
        g = cv.wait(g).unwrap();
    }
}

pub struct PcmStreamSource {
    buf: SharedBuf,
}
impl PcmStreamSource {
    pub fn new(buf: SharedBuf) -> Self {
        Self { buf }
    }
}
impl Iterator for PcmStreamSource {
    // This rodio fork's Source is `Iterator<Item = Sample>` where `Sample = f32`
    // (samples are normalized floats). Convert the queued i16 to normalized f32.
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.buf
            .0
            .lock()
            .unwrap()
            .next_sample()
            .map(|s| s as f32 / 32768.0)
    }
}
impl rodio::Source for PcmStreamSource {
    // The pinned fork (master5d/rodio @ fed3029) renames `current_frame_len` to
    // `current_span_len` and returns the `ChannelCount`/`SampleRate` type aliases
    // (u16/u32). Match that exact signature so the app crate builds.
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        PCM_CHANNELS as rodio::ChannelCount
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        PCM_SAMPLE_RATE
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
