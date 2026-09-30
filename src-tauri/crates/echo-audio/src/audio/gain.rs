//! Pure input-gain DSP for the Audio console: a linear gain multiply with a hard
//! clamp, and a dB→linear conversion. Tauri-free (leaf crate), so
//! `cargo test -p echo-audio` runs on Windows.

/// Convert a decibel gain to a linear multiplier. `0 dB → 1.0`.
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Multiply each sample by `linear` and clamp to `[-1.0, 1.0]`, in place.
/// Unity (`linear == 1.0`) early-returns, so the hot path pays nothing at the
/// default. Empty slice → no-op.
pub fn apply_gain(samples: &mut [f32], linear: f32) {
    if linear == 1.0 {
        return;
    }
    for s in samples.iter_mut() {
        *s = (*s * linear).clamp(-1.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn db_to_linear_reference_points() {
        assert!(approx(db_to_linear(0.0), 1.0, 1e-6));
        assert!(approx(db_to_linear(6.0), 1.995, 0.005));
        assert!(approx(db_to_linear(-6.0), 0.501, 0.005));
        assert!(approx(db_to_linear(20.0), 10.0, 1e-3));
        assert!(approx(db_to_linear(-24.0), 0.0631, 0.001));
    }

    #[test]
    fn unity_is_identity() {
        let mut buf = [0.1f32, -0.2, 0.3, -0.4];
        let orig = buf;
        apply_gain(&mut buf, 1.0);
        assert_eq!(buf, orig);
    }

    #[test]
    fn doubles_below_full_scale() {
        let mut buf = [0.1f32, -0.2, 0.25];
        apply_gain(&mut buf, 2.0);
        assert!(approx(buf[0], 0.2, 1e-6));
        assert!(approx(buf[1], -0.4, 1e-6));
        assert!(approx(buf[2], 0.5, 1e-6));
    }

    #[test]
    fn clamps_past_full_scale() {
        let mut buf = [0.8f32, -0.9];
        apply_gain(&mut buf, 2.0);
        assert_eq!(buf[0], 1.0);
        assert_eq!(buf[1], -1.0);
    }

    #[test]
    fn zero_gain_is_silence() {
        let mut buf = [0.5f32, -0.5, 1.0];
        apply_gain(&mut buf, 0.0);
        assert_eq!(buf, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn empty_is_noop() {
        let mut buf: [f32; 0] = [];
        apply_gain(&mut buf, 2.0);
        assert_eq!(buf.len(), 0);
    }
}
