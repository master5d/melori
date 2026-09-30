//! A monotonic generation gate used to seal audio-playback barge-in races.
//! Pure `std` atomics — no audio deps — so it is unit-tested natively.
use std::sync::atomic::{AtomicU64, Ordering};

/// A token identifying one playback generation. Only the latest token issued by
/// a `PlaybackGate` is "current"; any older token is stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(u64);

/// Issues monotonically increasing generation tokens. `begin` starts a new
/// generation and hands back its token; `supersede` starts a new generation
/// without handing back a token (invalidating all outstanding tokens); the
/// caller checks a token with `is_current`.
pub struct PlaybackGate {
    current: AtomicU64,
}

impl PlaybackGate {
    pub fn new() -> Self {
        Self {
            current: AtomicU64::new(0),
        }
    }

    pub fn begin(&self) -> Generation {
        Generation(self.current.fetch_add(1, Ordering::SeqCst) + 1)
    }

    pub fn is_current(&self, gen: &Generation) -> bool {
        self.current.load(Ordering::SeqCst) == gen.0
    }

    pub fn supersede(&self) {
        self.current.fetch_add(1, Ordering::SeqCst);
    }
}

impl Default for PlaybackGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_token_is_current() {
        let gate = PlaybackGate::new();
        let g = gate.begin();
        assert!(gate.is_current(&g));
    }

    #[test]
    fn second_begin_makes_first_stale() {
        let gate = PlaybackGate::new();
        let g1 = gate.begin();
        let g2 = gate.begin();
        assert!(
            !gate.is_current(&g1),
            "old token must be stale after a new begin"
        );
        assert!(gate.is_current(&g2), "newest token must be current");
    }

    #[test]
    fn supersede_invalidates_outstanding_token() {
        let gate = PlaybackGate::new();
        let g = gate.begin();
        assert!(gate.is_current(&g));
        gate.supersede();
        assert!(!gate.is_current(&g), "token must be stale after supersede");
    }

    #[test]
    fn begin_after_supersede_yields_fresh_current_token() {
        let gate = PlaybackGate::new();
        let g_old = gate.begin();
        gate.supersede();
        let g_new = gate.begin();
        assert!(gate.is_current(&g_new));
        assert!(!gate.is_current(&g_old), "pre-supersede token stays stale");
    }

    #[test]
    fn tokens_are_monotonic_across_interleaving() {
        let gate = PlaybackGate::new();
        let a = gate.begin();
        gate.supersede();
        let b = gate.begin();
        gate.supersede();
        let c = gate.begin();
        // Distinct, strictly increasing underlying values; only the last is current.
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
        assert!(!gate.is_current(&a));
        assert!(!gate.is_current(&b));
        assert!(gate.is_current(&c));
    }
}
