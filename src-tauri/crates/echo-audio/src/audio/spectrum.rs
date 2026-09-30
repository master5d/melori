//! The Audio-console spectrum analyzer (Epic D1) — an honest analyzer, distinct
//! from the overlay pill's `AudioVisualiser`.
//!
//! The pill's visualiser is built as `AudioVisualiser::new(rate, 512, 16, 400.0,
//! 4000.0)`: it covers only 400–4000 Hz (mains hum is BELOW that range and
//! sibilance is ABOVE it), its 512-point window gives 93.75 Hz per bin at 48 kHz
//! (50 and 60 Hz are not separable even in principle), and its dB is clamped to
//! -55..-8 and then curve-shaped. It is a good pill visual and is left untouched;
//! this module is the analyzer.
//!
//! Tauri-free (std + rustfft only), so `cargo test -p echo-audio` runs on Windows.
use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::Arc;

/// FFT window. 4096 @ 48 kHz = 11.72 Hz per bin — fine enough to show a hum spike.
pub const WINDOW: usize = 4096;
/// Hop between frames (50 % overlap) → ~24 frames/s at 48 kHz.
pub const HOP: usize = 2048;
/// Number of log-spaced output bands.
pub const BANDS: usize = 128;
/// Lowest displayed frequency.
pub const F_MIN: f32 = 20.0;
/// Highest displayed frequency, before the Nyquist clamp.
pub const F_MAX: f32 = 20_000.0;
/// dBFS floor.
pub const DB_FLOOR: f32 = -100.0;

/// The top of the analysed range for a given rate: 20 kHz, or Nyquist if lower.
fn f_max_for(sample_rate: u32) -> f32 {
    F_MAX.min(sample_rate as f32 / 2.0)
}

/// Geometric (log) center frequency of each band.
///
/// THE single source of truth for the frequency axis: the frontend fetches these
/// and does no frequency math of its own, so the drawn axis cannot drift from the
/// bands that were actually computed.
pub fn band_centers(sample_rate: u32) -> Vec<f32> {
    let ratio = f_max_for(sample_rate) / F_MIN;
    (0..BANDS)
        .map(|i| F_MIN * ratio.powf((i as f32 + 0.5) / BANDS as f32))
        .collect()
}

pub struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    band_bins: Vec<(usize, usize)>,
    scratch: Vec<Complex32>,
    buffer: Vec<f32>,
}

impl SpectrumAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(WINDOW);

        // Hann window. Its coherent gain is 0.5 — the `4/N` scale in `compute`
        // depends on exactly this window.
        let window: Vec<f32> = (0..WINDOW)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / WINDOW as f32).cos()))
            .collect();

        // Genuinely geometric (log) band edges — NOT the `.powi(2)` pseudo-log the
        // pill's visualiser uses.
        let ratio = f_max_for(sample_rate) / F_MIN;
        let max_bin = WINDOW / 2;
        let mut band_bins = Vec::with_capacity(BANDS);
        for i in 0..BANDS {
            let f_lo = F_MIN * ratio.powf(i as f32 / BANDS as f32);
            let f_hi = F_MIN * ratio.powf((i + 1) as f32 / BANDS as f32);
            let mut lo = (f_lo * WINDOW as f32 / sample_rate as f32).round() as usize;
            let mut hi = (f_hi * WINDOW as f32 / sample_rate as f32).round() as usize;
            lo = lo.min(max_bin.saturating_sub(1));
            if hi <= lo {
                hi = lo + 1; // every band owns at least one bin
            }
            hi = hi.min(max_bin);
            band_bins.push((lo, hi));
        }

        Self {
            fft,
            window,
            band_bins,
            scratch: vec![Complex32::new(0.0, 0.0); WINDOW],
            buffer: Vec::with_capacity(WINDOW * 2),
        }
    }

    /// Feed captured samples. Returns one frame of `BANDS` dBFS values once a full
    /// window is available, draining only `HOP` samples so consecutive frames
    /// overlap by 50 %.
    pub fn feed(&mut self, samples: &[f32]) -> Option<Vec<f32>> {
        self.buffer.extend_from_slice(samples);
        if self.buffer.len() < WINDOW {
            return None;
        }
        let bands = self.compute();
        self.buffer.drain(..HOP);
        // Bound the buffer: an unusually large cpal chunk must not grow it forever.
        if self.buffer.len() > WINDOW * 2 {
            let excess = self.buffer.len() - WINDOW;
            self.buffer.drain(..excess);
        }
        Some(bands)
    }

    /// Drop the partial buffer (used when the analyzer's gate closes, so reopening
    /// the Audio section does not flush stale audio).
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    fn compute(&mut self) -> Vec<f32> {
        // Remove DC, apply the Hann window.
        let mean = self.buffer[..WINDOW].iter().sum::<f32>() / WINDOW as f32;
        for i in 0..WINDOW {
            self.scratch[i] = Complex32::new((self.buffer[i] - mean) * self.window[i], 0.0);
        }
        self.fft.process(&mut self.scratch);

        // A full-scale sine peaks at |X[k]| = N * coherent_gain / 2 = N/4 for Hann,
        // so `mag * 4 / N` normalizes it to 1.0 → exactly 0 dBFS.
        let scale = 4.0 / WINDOW as f32;

        let mut out = Vec::with_capacity(BANDS);
        for &(lo, hi) in &self.band_bins {
            // MAX bin, not the mean: mains hum is a NARROW tone, and averaging it
            // across a band would smear it into nothing — the very thing we must show.
            let mut peak = 0.0f32;
            for k in lo..hi {
                let m = self.scratch[k].norm();
                if m > peak {
                    peak = m;
                }
            }
            let amp = peak * scale;
            out.push(if amp <= 0.0 {
                DB_FLOOR
            } else {
                (20.0 * amp.log10()).max(DB_FLOOR)
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate `n` samples of a sine at `freq` with amplitude `amp`.
    fn sine(freq: f32, amp: f32, rate: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    /// Index of the loudest band in a frame.
    fn argmax(bands: &[f32]) -> usize {
        let mut best = 0;
        for (i, &v) in bands.iter().enumerate() {
            if v > bands[best] {
                best = i;
            }
        }
        best
    }

    #[test]
    fn full_scale_on_bin_sine_reads_zero_dbfs() {
        // At 48000/4096 the bin width is exactly 11.71875 Hz, so bin 85 sits at
        // 996.09375 Hz. An exact-bin sine is periodic in the window → no leakage,
        // so this pins the `4/N` normalization tightly.
        let rate = 48_000;
        let freq = 11.71875 * 85.0;
        let mut an = SpectrumAnalyzer::new(rate);
        let bands = an.feed(&sine(freq, 1.0, rate, WINDOW)).expect("one frame");
        let peak = bands[argmax(&bands)];
        assert!(
            (peak - 0.0).abs() < 0.5,
            "full-scale on-bin sine should read ~0 dBFS, got {peak}"
        );
    }

    #[test]
    fn off_bin_sine_loses_at_most_scalloping() {
        // 1000 Hz falls BETWEEN bins → Hann scalloping costs up to ~1.4 dB.
        // Assert the honest bound rather than hiding the loss.
        let rate = 48_000;
        let mut an = SpectrumAnalyzer::new(rate);
        let bands = an
            .feed(&sine(1000.0, 1.0, rate, WINDOW))
            .expect("one frame");
        let peak = bands[argmax(&bands)];
        assert!(
            peak >= -2.0,
            "off-bin full-scale sine should stay within scalloping loss, got {peak}"
        );
    }

    #[test]
    fn half_amplitude_drops_six_db() {
        let rate = 48_000;
        let freq = 11.71875 * 85.0;
        let mut an = SpectrumAnalyzer::new(rate);
        let bands = an.feed(&sine(freq, 0.5, rate, WINDOW)).expect("one frame");
        let peak = bands[argmax(&bands)];
        assert!(
            (peak - (-6.02)).abs() < 0.5,
            "half amplitude should read ~-6 dBFS, got {peak}"
        );
    }

    #[test]
    fn mains_hum_peaks_below_100_hz() {
        let rate = 48_000;
        let mut an = SpectrumAnalyzer::new(rate);
        let bands = an.feed(&sine(60.0, 0.8, rate, WINDOW)).expect("one frame");
        let centers = band_centers(rate);
        let loudest = centers[argmax(&bands)];
        assert!(
            loudest < 100.0,
            "a 60 Hz tone should peak in a sub-100 Hz band, got {loudest} Hz"
        );
    }

    #[test]
    fn silence_sits_on_the_floor() {
        let rate = 48_000;
        let mut an = SpectrumAnalyzer::new(rate);
        let bands = an.feed(&vec![0.0f32; WINDOW]).expect("one frame");
        assert_eq!(bands.len(), BANDS);
        assert!(
            bands.iter().all(|&b| b == DB_FLOOR),
            "silence must sit exactly on the dBFS floor"
        );
    }

    #[test]
    fn short_feed_yields_no_frame() {
        let mut an = SpectrumAnalyzer::new(48_000);
        assert!(an.feed(&vec![0.1f32; WINDOW - 1]).is_none());
    }

    #[test]
    fn feed_drains_only_hop_so_frames_overlap() {
        // A full window yields a frame; feeding only HOP more must yield a SECOND
        // frame — proving feed() drains HOP (50% overlap) rather than clear()ing.
        let rate = 48_000;
        let mut an = SpectrumAnalyzer::new(rate);
        assert!(an.feed(&sine(1000.0, 0.5, rate, WINDOW)).is_some());
        assert!(
            an.feed(&sine(1000.0, 0.5, rate, HOP)).is_some(),
            "the retained WINDOW-HOP samples plus HOP new ones must complete a window"
        );
    }

    #[test]
    fn band_centers_span_the_log_axis() {
        let centers = band_centers(48_000);
        assert_eq!(centers.len(), BANDS);
        for w in centers.windows(2) {
            assert!(w[1] > w[0], "band centers must increase monotonically");
        }
        assert!(centers[0] > 20.0 && centers[0] < 25.0);
        let last = centers[BANDS - 1];
        assert!(last > 19_000.0 && last < 20_000.0, "last center was {last}");
    }

    #[test]
    fn band_centers_clamp_to_nyquist_on_low_rate_devices() {
        // A 16 kHz device has an 8 kHz Nyquist — the axis must compress, not lie.
        let centers = band_centers(16_000);
        assert_eq!(centers.len(), BANDS);
        for w in centers.windows(2) {
            assert!(w[1] > w[0], "band centers must increase monotonically");
        }
        let last = centers[BANDS - 1];
        assert!(last > 7_500.0 && last < 8_000.0, "last center was {last}");
    }
}
