pub struct SilenceTimer {
    limit_ms: u64,
    last_voice_ms: Option<u64>,
    fired: bool,
}

impl SilenceTimer {
    pub fn new(limit_ms: u64) -> Self {
        Self {
            limit_ms,
            last_voice_ms: None,
            fired: false,
        }
    }
    pub fn on_voice(&mut self, now_ms: u64) {
        self.last_voice_ms = Some(now_ms);
        self.fired = false;
    }
    pub fn reset(&mut self, now_ms: u64) {
        self.on_voice(now_ms);
    }
    pub fn tick(&mut self, now_ms: u64) -> Option<u64> {
        if self.limit_ms == 0 || self.fired {
            return None;
        }
        let silent = now_ms.saturating_sub(self.last_voice_ms.unwrap_or(now_ms));
        if silent >= self.limit_ms {
            self.fired = true;
            Some(silent)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_fires_once_per_quiet_period() {
        let mut t = SilenceTimer::new(60_000);
        t.on_voice(0);
        assert_eq!(t.tick(59_000), None);
        assert_eq!(t.tick(60_000), Some(60_000));
        assert_eq!(t.tick(90_000), None);
        t.on_voice(100_000);
        assert_eq!(t.tick(150_000), None);
        assert_eq!(t.tick(160_000), Some(60_000));
    }
    #[test]
    fn reset_restarts_the_count_and_zero_disables() {
        let mut t = SilenceTimer::new(60_000);
        t.on_voice(0);
        t.reset(50_000);
        assert_eq!(t.tick(100_000), None);
        assert_eq!(t.tick(110_000), Some(60_000));
        let mut off = SilenceTimer::new(0);
        assert_eq!(off.tick(10_000_000), None);
    }
}
