//! Pure DSP for the Audio-console monitor / passthrough path (Epic C).
//! `MonitorBuf` bridges the capture consumer (producer) and the rodio monitor
//! Source (consumer). `MonitorRamp` + `apply_monitor` shape the output: a
//! fade-in envelope times a monitor volume, hard-clamped. All Tauri-free so
//! `cargo test -p echo-audio` runs on Windows.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

/// Max samples buffered on the monitor path — bounds passthrough latency
/// (~170 ms @ 48 kHz, ~85 ms @ 96 kHz). Overrun drops the oldest so latency
/// never accumulates.
const CAP: usize = 8192;

/// A bounded f32 jitter buffer bridging the capture consumer and the rodio
/// monitor Source. `push` is a no-op when inactive, so the `run_consumer` tap
/// costs nothing when passthrough is off.
pub struct MonitorBuf {
    queue: Mutex<VecDeque<f32>>,
    active: AtomicBool,
    sample_rate: AtomicU32,
}

impl MonitorBuf {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(CAP)),
            active: AtomicBool::new(false),
            sample_rate: AtomicU32::new(0),
        }
    }

    /// Toggle the tap. Clearing on disable drops any stale buffered audio so a
    /// re-enable starts clean (no old samples flushed at fade-in).
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::SeqCst);
        if !active {
            self.queue.lock().unwrap().clear();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Relaxed);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Append captured samples. No-op when inactive (zero-cost hot path). Drops
    /// the oldest samples if over `CAP` so latency stays bounded.
    pub fn push(&self, samples: &[f32]) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let mut q = self.queue.lock().unwrap();
        q.extend(samples.iter().copied());
        while q.len() > CAP {
            q.pop_front();
        }
    }

    /// Pop the next sample, or `None` on underrun.
    pub fn next_sample(&self) -> Option<f32> {
        self.queue.lock().unwrap().pop_front()
    }
}

impl Default for MonitorBuf {
    fn default() -> Self {
        Self::new()
    }
}

/// A linear fade-in envelope: climbs 0.0 → 1.0 over `ramp_len` samples, then
/// holds 1.0. Each monitor session starts a fresh ramp so audio never jumps
/// straight to full volume (soft start, no click, no instant howl).
pub struct MonitorRamp {
    processed: u64,
    ramp_len: u64,
}

impl MonitorRamp {
    pub fn new(ramp_len: u64) -> Self {
        Self {
            processed: 0,
            ramp_len,
        }
    }

    /// Current ramp factor in `[0.0, 1.0]`; advances one sample per call.
    pub fn factor(&mut self) -> f32 {
        let f = if self.ramp_len == 0 {
            1.0
        } else {
            (self.processed as f32 / self.ramp_len as f32).min(1.0)
        };
        self.processed = self.processed.saturating_add(1);
        f
    }
}

/// Apply the monitor envelope to one sample: fade × volume, hard-clamped so the
/// output device never receives out-of-range samples.
pub fn apply_monitor(sample: f32, factor: f32, volume: f32) -> f32 {
    (sample * factor * volume).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_is_noop_when_inactive() {
        let buf = MonitorBuf::new();
        buf.push(&[0.5, 0.5, 0.5]);
        assert_eq!(buf.next_sample(), None);
    }

    #[test]
    fn push_appends_fifo_when_active() {
        let buf = MonitorBuf::new();
        buf.set_active(true);
        buf.push(&[0.1, 0.2]);
        assert_eq!(buf.next_sample(), Some(0.1));
        assert_eq!(buf.next_sample(), Some(0.2));
        assert_eq!(buf.next_sample(), None);
    }

    #[test]
    fn bounded_drops_oldest_on_overrun() {
        let buf = MonitorBuf::new();
        buf.set_active(true);
        let over: Vec<f32> = (0..(CAP as u32 + 3)).map(|i| i as f32).collect();
        buf.push(&over);
        // Oldest 3 (0.0, 1.0, 2.0) dropped; first surviving sample is 3.0.
        assert_eq!(buf.next_sample(), Some(3.0));
        let mut remaining = 1;
        while buf.next_sample().is_some() {
            remaining += 1;
        }
        assert_eq!(remaining, CAP);
    }

    #[test]
    fn set_inactive_clears_buffer() {
        let buf = MonitorBuf::new();
        buf.set_active(true);
        buf.push(&[0.3, 0.3]);
        buf.set_active(false);
        assert_eq!(buf.next_sample(), None);
    }

    #[test]
    fn sample_rate_roundtrips() {
        let buf = MonitorBuf::new();
        assert_eq!(buf.sample_rate(), 0);
        buf.set_sample_rate(48_000);
        assert_eq!(buf.sample_rate(), 48_000);
    }

    #[test]
    fn ramp_climbs_then_holds() {
        let mut ramp = MonitorRamp::new(4);
        assert_eq!(ramp.factor(), 0.0);
        assert_eq!(ramp.factor(), 0.25);
        assert_eq!(ramp.factor(), 0.5);
        assert_eq!(ramp.factor(), 0.75);
        assert_eq!(ramp.factor(), 1.0);
        assert_eq!(ramp.factor(), 1.0);
    }

    #[test]
    fn ramp_zero_len_is_immediately_unity() {
        let mut ramp = MonitorRamp::new(0);
        assert_eq!(ramp.factor(), 1.0);
    }

    #[test]
    fn apply_monitor_identity_and_scaling() {
        assert_eq!(apply_monitor(0.5, 1.0, 1.0), 0.5);
        assert_eq!(apply_monitor(0.5, 1.0, 0.5), 0.25);
        assert_eq!(apply_monitor(0.8, 0.5, 1.0), 0.4);
        assert_eq!(apply_monitor(0.0, 1.0, 1.0), 0.0);
    }

    #[test]
    fn apply_monitor_clamps_to_unit_range() {
        assert_eq!(apply_monitor(0.9, 1.0, 2.0), 1.0);
        assert_eq!(apply_monitor(-0.9, 1.0, 2.0), -1.0);
    }
}
