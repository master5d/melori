//! Pure input-meter DSP for the Audio console: true peak/RMS from raw
//! time-domain samples, plus a dBFS conversion. Tauri-free (leaf crate),
//! so `cargo test -p echo-audio` runs on Windows.

/// Linear peak (`max |s|`) and RMS (`sqrt(mean(s^2))`) of a frame of samples.
/// Empty input → `(0.0, 0.0)`. RMS is accumulated in f64 to avoid drift on
/// long frames, then narrowed to f32.
pub fn frame_peak_rms(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut peak = 0.0f32;
    let mut sum_sq = 0.0f64;
    for &s in samples {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum_sq += (s as f64) * (s as f64);
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    (peak, rms)
}

/// Convert a linear magnitude to dBFS (`20*log10`), floored at -100 dB.
/// `x <= 0` → -100.0.
pub fn linear_to_dbfs(x: f32) -> f32 {
    if x <= 0.0 {
        return -100.0;
    }
    let db = 20.0 * x.log10();
    if db < -100.0 {
        -100.0
    } else {
        db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn silence_is_zero() {
        let (p, r) = frame_peak_rms(&[0.0f32; 256]);
        assert_eq!(p, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(frame_peak_rms(&[]), (0.0, 0.0));
    }

    #[test]
    fn full_scale_sine_peak_and_rms() {
        let n = 2048usize;
        let sine: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 8.0 * i as f32 / n as f32).sin())
            .collect();
        let (p, r) = frame_peak_rms(&sine);
        assert!(approx(p, 1.0, 0.01), "peak was {p}");
        assert!(approx(r, 0.707, 0.01), "rms was {r}");
    }

    #[test]
    fn dc_offset_peak_equals_rms() {
        let (p, r) = frame_peak_rms(&[0.5f32; 512]);
        assert!(approx(p, 0.5, 1e-6), "peak {p}");
        assert!(approx(r, 0.5, 1e-6), "rms {r}");
    }

    #[test]
    fn dbfs_reference_points() {
        assert!(approx(linear_to_dbfs(1.0), 0.0, 1e-4));
        assert!(approx(linear_to_dbfs(0.5), -6.02, 0.02));
        assert_eq!(linear_to_dbfs(0.0), -100.0);
        assert_eq!(linear_to_dbfs(-0.3), -100.0);
        assert_eq!(linear_to_dbfs(1e-9), -100.0);
    }
}
