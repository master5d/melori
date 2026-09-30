//! Device health (Epic D2). A pure state machine plus the shared state the
//! capture thread writes into.
//!
//! Deliberately free of cpal and tauri: that is what lets it run under
//! `cargo test` on Windows, where app-crate tests are blocked by the native ML
//! DLL. All health *policy* lives here; the app crate only transports it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// No frame for this long → `Stalled`.
///
/// Deliberately not 1000 ms: cpal's first input callback lands one buffer period
/// after `stream.play()` returns (~10–200 ms typically, longer on Bluetooth/USB),
/// and a flickering false alarm is worse than a two-second detection delay.
pub const STALL_MS: u64 = 2_000;

/// Pre-gain peak strictly below this, continuously for `SILENCE_MS` → `Silent`.
pub const SILENCE_DBFS: f32 = -60.0;

/// How long the peak must stay below `SILENCE_DBFS` before we call it silence.
pub const SILENCE_MS: u64 = 5_000;

/// Process-start reference for the monotonic millisecond clock the atomics hold.
/// An `Instant` cannot live in an atomic, so we store elapsed milliseconds
/// against a fixed epoch instead.
fn epoch() -> &'static Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now)
}

/// Milliseconds since process start. Monotonic; never goes backwards.
pub fn now_ms() -> u64 {
    epoch().elapsed().as_millis() as u64
}

/// What the device ACTUALLY delivers — the negotiated stream config, not the
/// advertised capability ranges. `open()` already computes this and, before
/// Epic D2, only wrote it to the log.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFacts {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HealthStatus {
    Ok,
    /// The stream is open but has not produced its first frame yet (or the
    /// device's negotiated facts are not in yet). Within the `STALL_MS` grace
    /// this is the honest answer: we do not know that the device works, and
    /// saying `Ok` here made a slow Bluetooth mic and a dead one look identical.
    Starting,
    /// The stream is alive but the device has been delivering digital silence.
    Silent {
        for_ms: u64,
    },
    /// Frames stopped arriving — the stream is alive as far as we asked, but the
    /// device is not producing.
    Stalled,
    /// cpal reported a stream error (e.g. the device was unplugged).
    Unavailable {
        reason: String,
    },
}

/// An immutable read of `DeviceHealthState` at one instant.
pub struct HealthSnapshot {
    pub now_ms: u64,
    pub last_frame_ms: u64,
    /// The PRE-gain peak. See `run_consumer`: reading the post-gain peak would
    /// make a −24 dB gain cut look like a dead microphone.
    pub peak: f32,
    pub error: Option<String>,
    /// Has THIS stream delivered a frame yet? `begin_open` clears it.
    pub frames_seen: bool,
    /// Are the negotiated facts for THIS stream in yet? They are written at open,
    /// before frames flow, so "no facts" means the open is still in flight — and
    /// it also fences a straggler callback from the outgoing stream, which would
    /// otherwise set `frames_seen` on a stream whose identity we do not have.
    pub facts_known: bool,
}

/// Turns snapshots into a status. Stateful only in that silence must be
/// *continuous*, so it remembers when the quiet started.
#[derive(Default)]
pub struct HealthEvaluator {
    silent_since: Option<u64>,
}

impl HealthEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Strict precedence: `Unavailable` > `Stalled` > `Starting` > `Silent` > `Ok`.
    ///
    /// `Starting` sits BELOW `Stalled` on purpose: it is a grace, not a hiding
    /// place. A device that never delivers leaves `Starting` at `STALL_MS` and
    /// hardens into `Stalled` — the status that offers Reconnect.
    pub fn evaluate(&mut self, snap: HealthSnapshot) -> HealthStatus {
        if let Some(reason) = snap.error {
            self.silent_since = None;
            return HealthStatus::Unavailable { reason };
        }

        if snap.now_ms.saturating_sub(snap.last_frame_ms) >= STALL_MS {
            self.silent_since = None;
            return HealthStatus::Stalled;
        }

        // Nothing has arrived on THIS stream yet (or we do not even know what the
        // device negotiated). Say so, and do not let the silence clock run: a
        // stream that has not spoken is not a stream speaking silence.
        if !snap.frames_seen || !snap.facts_known {
            self.silent_since = None;
            return HealthStatus::Starting;
        }

        // A non-finite peak is treated as being BELOW the silence floor, not as
        // audio. Without the `is_finite` guard a NaN peak makes
        // `linear_to_dbfs` return NaN, `NaN < SILENCE_DBFS` is `false`, and the
        // else-branch below resets the silence clock and reports `Ok` — i.e. a
        // garbage sample would certify the microphone as healthy. Fail towards
        // the warning, never towards a false all-clear.
        if !snap.peak.is_finite() || super::meter::linear_to_dbfs(snap.peak) < SILENCE_DBFS {
            let since = *self.silent_since.get_or_insert(snap.now_ms);
            let for_ms = snap.now_ms.saturating_sub(since);
            if for_ms >= SILENCE_MS {
                return HealthStatus::Silent { for_ms };
            }
        } else {
            // Any audible frame restarts the clock — silence must be continuous.
            self.silent_since = None;
        }

        HealthStatus::Ok
    }
}

/// Shared health state: the capture thread writes, the app-crate ticker reads.
/// The hot path is two relaxed stores per frame, so the audio thread never
/// blocks and never allocates.
pub struct DeviceHealthState {
    last_frame_ms: AtomicU64,
    /// f32 bits.
    last_peak: AtomicU32,
    /// Cleared by `begin_open`, set by the first `record_frame` of a stream.
    frames_seen: AtomicBool,
    /// `error` and `facts` share ONE lock so a reader can take a consistent view
    /// of both. They used to be two mutexes read in two steps, and a `begin_open`
    /// landing between those steps published one tick of {Ok, facts: null} — a
    /// healthy device with no identity, on every device switch.
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    error: Option<String>,
    facts: Option<DeviceFacts>,
}

impl Default for DeviceHealthState {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceHealthState {
    pub fn new() -> Self {
        Self {
            last_frame_ms: AtomicU64::new(0),
            last_peak: AtomicU32::new(0f32.to_bits()),
            frames_seen: AtomicBool::new(false),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Called at the START of an open attempt. It seeds the frame clock and the
    /// peak, and drops the previous stream's facts — but it deliberately does
    /// **NOT** clear the error slot. See `clear_error`.
    ///
    /// Seeding `last_frame_ms` with the open time is what gives the start-up
    /// grace: a device that is dead from the very first moment reaches `Stalled`
    /// `STALL_MS` after open, instead of sitting in `Ok` forever because
    /// `last_frame_ms` was 0.
    ///
    /// FAIL-CLOSED, and that is the whole point. The old `reset()` cleared the
    /// error slot here, so the instant the user pressed Reconnect on a dead
    /// device the evaluator saw "no error + fresh last_frame_ms" → `Ok`, the
    /// frontend's `signalLost` went false, and the meter un-blanked and repainted
    /// the levels captured just before the device died — as live audio, on the
    /// epic's own recovery path. While an open attempt is in flight against a
    /// device that was unavailable, the device IS still unavailable until proven
    /// otherwise; keeping the old error means there is no `Ok` window to un-blank
    /// into, and a failed reopen simply keeps the (correct) old status until
    /// `record_open_failure` overwrites it with the new reason.
    ///
    /// Idempotent: it is called by both `AudioRecordingManager::start_microphone_stream`
    /// and `AudioRecorder::open`, and running twice per attempt costs nothing.
    pub fn begin_open(&self) {
        // Everything this method touches is published under the inner lock, and
        // `read()` takes the same lock — so a reader sees the whole transition or
        // none of it, never half of the old stream and half of the new one.
        let mut inner = self.inner.lock().unwrap();
        let now = now_ms();
        self.last_frame_ms.store(now, Ordering::Relaxed);
        self.last_peak.store(0f32.to_bits(), Ordering::Relaxed);
        self.frames_seen.store(false, Ordering::Relaxed);
        inner.facts = None;
    }

    /// Clear the error slot. Called from EXACTLY ONE place — `AudioRecorder::open`,
    /// once the worker has reported that the stream was built and `play()`ed —
    /// i.e. only once the stream is confirmed open and running. That is what makes
    /// a *successful* reconnect drop the stale failure while a *failed* one keeps
    /// showing it.
    pub fn clear_error(&self) {
        self.inner.lock().unwrap().error = None;
    }

    /// Hot path — the audio thread. Three relaxed stores, no lock, no allocation.
    /// `peak` MUST be the pre-gain peak.
    pub fn record_frame(&self, peak: f32) {
        self.last_frame_ms.store(now_ms(), Ordering::Relaxed);
        self.last_peak.store(peak.to_bits(), Ordering::Relaxed);
        self.frames_seen.store(true, Ordering::Relaxed);
    }

    pub fn set_error(&self, reason: String) {
        self.inner.lock().unwrap().error = Some(reason);
    }

    pub fn set_facts(&self, facts: DeviceFacts) {
        self.inner.lock().unwrap().facts = Some(facts);
    }

    pub fn facts(&self) -> Option<DeviceFacts> {
        self.inner.lock().unwrap().facts.clone()
    }

    /// The ONE read the ticker should use: status inputs and facts taken under a
    /// single lock, so the pair it emits always describes the same stream.
    pub fn read(&self) -> (HealthSnapshot, Option<DeviceFacts>) {
        let inner = self.inner.lock().unwrap();
        let snap = HealthSnapshot {
            now_ms: now_ms(),
            last_frame_ms: self.last_frame_ms.load(Ordering::Relaxed),
            peak: f32::from_bits(self.last_peak.load(Ordering::Relaxed)),
            error: inner.error.clone(),
            frames_seen: self.frames_seen.load(Ordering::Relaxed),
            facts_known: inner.facts.is_some(),
        };
        (snap, inner.facts.clone())
    }

    pub fn snapshot(&self) -> HealthSnapshot {
        self.read().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A loud frame that just arrived.
    fn healthy(now: u64) -> HealthSnapshot {
        HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: 0.5,
            error: None,
            frames_seen: true,
            facts_known: true,
        }
    }

    #[test]
    fn loud_and_fresh_is_ok() {
        let mut ev = HealthEvaluator::new();
        assert_eq!(ev.evaluate(healthy(1_000)), HealthStatus::Ok);
    }

    #[test]
    fn error_outranks_everything() {
        let mut ev = HealthEvaluator::new();
        // Stale frames AND silent AND an error — the error must win.
        let snap = HealthSnapshot {
            now_ms: 100_000,
            last_frame_ms: 0,
            peak: 0.0,
            error: Some("device not available".into()),
            frames_seen: true,
            facts_known: true,
        };
        assert_eq!(
            ev.evaluate(snap),
            HealthStatus::Unavailable {
                reason: "device not available".into()
            }
        );
    }

    #[test]
    fn stall_outranks_silence() {
        let mut ev = HealthEvaluator::new();
        // Silent for ages AND no frames for longer than STALL_MS: Stalled wins,
        // because "the instrument is dead" is a different fact from "it is quiet".
        let snap = HealthSnapshot {
            now_ms: 100_000,
            last_frame_ms: 100_000 - STALL_MS,
            peak: 0.0,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        assert_eq!(ev.evaluate(snap), HealthStatus::Stalled);
    }

    #[test]
    fn frames_flowing_just_under_stall_is_not_stalled() {
        let mut ev = HealthEvaluator::new();
        let snap = HealthSnapshot {
            now_ms: 100_000,
            last_frame_ms: 100_000 - (STALL_MS - 1),
            peak: 0.5,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        assert_eq!(ev.evaluate(snap), HealthStatus::Ok);
    }

    #[test]
    fn silence_must_be_continuous_for_silence_ms() {
        let mut ev = HealthEvaluator::new();
        // Quiet from t=1000. Not yet Silent one millisecond before the threshold.
        let quiet = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: 0.0,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        assert_eq!(ev.evaluate(quiet(1_000)), HealthStatus::Ok);
        assert_eq!(ev.evaluate(quiet(1_000 + SILENCE_MS - 1)), HealthStatus::Ok);
        assert_eq!(
            ev.evaluate(quiet(1_000 + SILENCE_MS)),
            HealthStatus::Silent { for_ms: SILENCE_MS }
        );
    }

    #[test]
    fn silence_resets_the_instant_the_peak_rises() {
        let mut ev = HealthEvaluator::new();
        let quiet = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: 0.0,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        // Almost there...
        assert_eq!(ev.evaluate(quiet(1_000)), HealthStatus::Ok);
        assert_eq!(ev.evaluate(quiet(1_000 + SILENCE_MS - 1)), HealthStatus::Ok);
        // ...then the user speaks. The clock must start over, not resume.
        assert_eq!(ev.evaluate(healthy(1_000 + SILENCE_MS)), HealthStatus::Ok);
        assert_eq!(ev.evaluate(quiet(1_000 + SILENCE_MS + 1)), HealthStatus::Ok);
        assert_eq!(
            ev.evaluate(quiet(1_000 + 2 * SILENCE_MS)),
            HealthStatus::Ok,
            "the silence clock restarted at the loud frame, so this is only SILENCE_MS-1 of quiet"
        );
    }

    #[test]
    fn silent_for_ms_grows_while_quiet() {
        let mut ev = HealthEvaluator::new();
        let quiet = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: 0.0,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        ev.evaluate(quiet(0));
        assert_eq!(
            ev.evaluate(quiet(SILENCE_MS)),
            HealthStatus::Silent { for_ms: SILENCE_MS }
        );
        assert_eq!(
            ev.evaluate(quiet(SILENCE_MS + 3_000)),
            HealthStatus::Silent {
                for_ms: SILENCE_MS + 3_000
            }
        );
    }

    #[test]
    fn a_peak_exactly_at_the_threshold_is_not_silent() {
        let mut ev = HealthEvaluator::new();
        // -60 dBFS linear = 0.001. The predicate is STRICTLY below the floor.
        let at_floor = HealthSnapshot {
            now_ms: 0,
            last_frame_ms: 0,
            peak: 0.001,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        assert_eq!(ev.evaluate(at_floor), HealthStatus::Ok);
        let mut ev2 = HealthEvaluator::new();
        let below = HealthSnapshot {
            now_ms: 0,
            last_frame_ms: 0,
            peak: 0.0001,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        ev2.evaluate(below);
        assert_eq!(
            ev2.evaluate(HealthSnapshot {
                now_ms: SILENCE_MS,
                last_frame_ms: SILENCE_MS,
                peak: 0.0001,
                error: None,
                frames_seen: true,
                facts_known: true
            }),
            HealthStatus::Silent { for_ms: SILENCE_MS }
        );
    }

    #[test]
    fn a_non_finite_peak_is_never_reported_as_ok() {
        // NaN reaches the peak slot from a garbage/denormal frame. `linear_to_dbfs`
        // returns NaN, and `NaN < SILENCE_DBFS` is false — so without the guard the
        // evaluator falls into the else-branch, RESETS the silence clock, and
        // certifies a dead device as healthy.
        let mut ev = HealthEvaluator::new();
        let nan = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: f32::NAN,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        ev.evaluate(nan(0));
        assert_eq!(
            ev.evaluate(nan(SILENCE_MS)),
            HealthStatus::Silent { for_ms: SILENCE_MS },
            "a NaN peak must count as below the silence floor, not as audio"
        );

        // ...and it must not silently rearm the clock either: infinities are just
        // as broken as NaN.
        let mut ev2 = HealthEvaluator::new();
        let inf = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: f32::INFINITY,
            error: None,
            frames_seen: true,
            facts_known: true,
        };
        ev2.evaluate(inf(0));
        assert_eq!(
            ev2.evaluate(inf(SILENCE_MS)),
            HealthStatus::Silent { for_ms: SILENCE_MS }
        );
    }

    #[test]
    fn fresh_state_seeds_last_frame_so_a_new_stream_is_not_instantly_stalled() {
        let state = DeviceHealthState::new();
        state.begin_open();
        let snap = state.snapshot();
        // begin_open seeds last_frame_ms with the open time, so `now - last_frame`
        // is ~0 and the grace period is realized without a second field.
        assert!(
            snap.now_ms.saturating_sub(snap.last_frame_ms) < STALL_MS,
            "a freshly opened stream must not read as stalled"
        );
        let mut ev = HealthEvaluator::new();
        // CONTRACT CHANGE (T2): this used to assert `Ok`. It is not Ok — not one
        // frame has arrived and the device has not even told us what it
        // negotiated. `Starting` is the honest name for that, and it is what stops
        // the card from announcing "Signal present" over a stream that may be dead.
        assert_eq!(ev.evaluate(snap), HealthStatus::Starting);
    }

    #[test]
    fn record_frame_publishes_the_peak_and_error_slot_round_trips() {
        let state = DeviceHealthState::new();
        state.begin_open();
        state.record_frame(0.75);
        assert_eq!(state.snapshot().peak, 0.75);
        assert_eq!(state.snapshot().error, None);

        state.set_error("boom".into());
        assert_eq!(state.snapshot().error, Some("boom".into()));

        // A stream that is confirmed running must not inherit the previous
        // stream's failure.
        state.begin_open();
        state.clear_error();
        assert_eq!(state.snapshot().error, None);
        assert_eq!(state.facts(), None);
    }

    #[test]
    fn an_open_attempt_alone_does_not_clear_the_error_slot() {
        // The recovery-path defect: pressing Reconnect on a still-dead device
        // used to clear the error AND seed a fresh last_frame_ms, so the
        // evaluator returned `Ok` for the duration of the (doomed) attempt —
        // un-blanking the meter onto its pre-death levels. begin_open() must be
        // fail-closed: only a stream confirmed RUNNING may clear the error.
        let state = DeviceHealthState::new();
        state.set_error("The device is not available".into());

        state.begin_open(); // the attempt starts...
        assert_eq!(
            state.snapshot().error,
            Some("The device is not available".into()),
            "an in-flight open attempt must not clear the previous failure"
        );

        let mut ev = HealthEvaluator::new();
        assert!(
            matches!(
                ev.evaluate(state.snapshot()),
                HealthStatus::Unavailable { .. }
            ),
            "there must be no `Ok` window for the meter to un-blank into"
        );

        state.clear_error(); // ...and only now is the stream confirmed running
        assert_eq!(state.snapshot().error, None);
    }

    #[test]
    fn a_failed_reopen_keeps_reporting_unavailable_with_the_new_reason() {
        let state = DeviceHealthState::new();
        state.set_error("old reason".into());

        // Reconnect: attempt starts, fails; `record_open_failure` overwrites.
        state.begin_open();
        state.set_error("new reason".into());

        let mut ev = HealthEvaluator::new();
        assert_eq!(
            ev.evaluate(state.snapshot()),
            HealthStatus::Unavailable {
                reason: "new reason".into()
            }
        );
    }

    #[test]
    fn facts_round_trip() {
        let state = DeviceHealthState::new();
        let facts = DeviceFacts {
            name: "Blue Yeti".into(),
            sample_rate: 48_000,
            channels: 1,
            sample_format: "F32".into(),
        };
        state.set_facts(facts.clone());
        assert_eq!(state.facts(), Some(facts));
    }

    // ---- T2: the stream is open, but nothing has arrived yet ----------------

    fn facts_of(name: &str) -> DeviceFacts {
        DeviceFacts {
            name: name.into(),
            sample_rate: 48_000,
            channels: 1,
            sample_format: "F32".into(),
        }
    }

    #[test]
    fn pre_first_frame_reads_starting_not_signal_present() {
        // The reopen lie: for the whole STALL_MS grace the evaluator returned Ok
        // — "Signal present" — even though not one frame had arrived. A slow
        // Bluetooth mic and a dead one were rendered identically, and the user
        // was told the good news first.
        let state = DeviceHealthState::new();
        state.begin_open();
        state.set_facts(facts_of("Blue Yeti")); // negotiated at open, before frames
        let mut ev = HealthEvaluator::new();
        assert_eq!(ev.evaluate(state.read().0), HealthStatus::Starting);
    }

    #[test]
    fn the_first_frame_turns_starting_into_ok() {
        let state = DeviceHealthState::new();
        state.begin_open();
        state.set_facts(facts_of("Blue Yeti"));
        let mut ev = HealthEvaluator::new();
        assert_eq!(ev.evaluate(state.read().0), HealthStatus::Starting);
        state.record_frame(0.5);
        assert_eq!(ev.evaluate(state.read().0), HealthStatus::Ok);
    }

    #[test]
    fn a_device_that_never_delivers_still_reaches_stalled() {
        // Starting is a GRACE, not a hiding place: past STALL_MS with no frame
        // the verdict must harden into Stalled (which is what offers Reconnect).
        let mut ev = HealthEvaluator::new();
        let never_delivered = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: 0,
            peak: 0.0,
            error: None,
            frames_seen: false,
            facts_known: true,
        };
        assert_eq!(
            ev.evaluate(never_delivered(STALL_MS - 1)),
            HealthStatus::Starting
        );
        assert_eq!(
            ev.evaluate(never_delivered(STALL_MS)),
            HealthStatus::Stalled
        );
    }

    #[test]
    fn silence_is_only_judged_once_frames_actually_flow() {
        // A quiet pre-first-frame stream must not accrue a silence clock — the
        // device has not spoken yet, which is not the same as speaking silence.
        let mut ev = HealthEvaluator::new();
        let opening = |now: u64| HealthSnapshot {
            now_ms: now,
            last_frame_ms: now,
            peak: 0.0,
            error: None,
            frames_seen: false,
            facts_known: true,
        };
        ev.evaluate(opening(0));
        assert_eq!(ev.evaluate(opening(SILENCE_MS)), HealthStatus::Starting);
    }

    // ---- T1: one consistent read, never {Ok, facts: null} -------------------

    #[test]
    fn a_device_switch_never_reads_ok_with_no_facts() {
        // The two-phase read: the ticker sampled snapshot() and facts() as two
        // separate operations, so a begin_open() landing between them produced a
        // tick of {status: Ok, facts: null} — a healthy device with no identity.
        // read() takes both under one lock, and "facts not yet known" is itself
        // a reason to say Starting.
        let state = DeviceHealthState::new();
        state.begin_open();
        state.set_facts(facts_of("Old Mic"));
        state.record_frame(0.5);
        let mut ev = HealthEvaluator::new();
        let (snap, facts) = state.read();
        assert_eq!(ev.evaluate(snap), HealthStatus::Ok);
        assert_eq!(facts.map(|f| f.name), Some("Old Mic".into()));

        // ...the user switches devices: facts are dropped and the frame clock reseeds.
        state.begin_open();
        let (snap, facts) = state.read();
        assert_eq!(facts, None);
        assert_eq!(
            ev.evaluate(snap),
            HealthStatus::Starting,
            "no facts + no frames on the new stream is Starting, never Ok"
        );
    }

    #[test]
    fn a_stale_frame_from_the_old_stream_cannot_certify_the_new_one() {
        // cpal's callback for the OUTGOING stream can land just after begin_open.
        // frames_seen alone would then flip to true while the new stream has no
        // facts yet — so facts_known guards the Ok verdict as well.
        let state = DeviceHealthState::new();
        state.begin_open();
        state.set_facts(facts_of("Old Mic"));
        state.record_frame(0.5);

        state.begin_open(); // device switch; facts dropped
        state.record_frame(0.5); // a straggler frame from the old stream

        let mut ev = HealthEvaluator::new();
        let (snap, facts) = state.read();
        assert_eq!(facts, None);
        assert_eq!(ev.evaluate(snap), HealthStatus::Starting);
    }
}
