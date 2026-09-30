use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Outcome {
    Answered(String),
    Dismissed,
    Timeout,
}

/// What the UI layer needs to know to show a question. Also returned by the
/// `agent_bridge_current` command so a freshly created panel window can pull
/// the active question on mount (the `agent-question` event may fire before
/// the webview is ready to listen — cold-window race).
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct QuestionEvent {
    pub id: i64,
    pub kind: String,
    pub question: String,
    pub options: Vec<String>,
    pub timeout_s: u64,
    pub speak: bool,
    pub source: String,
}

#[derive(Clone)]
pub struct BridgeState {
    pending: Arc<Mutex<HashMap<i64, Sender<Outcome>>>>,
    /// The question currently on screen (None between questions).
    current: Arc<Mutex<Option<QuestionEvent>>>,
    /// Serializes asks so the panel shows one question at a time.
    pub ask_serial: Arc<Mutex<()>>,
    /// Number of asks waiting for the serial lock (for the 429 cap).
    pub waiting: Arc<std::sync::atomic::AtomicUsize>,
}

pub struct PendingQuestion {
    id: i64,
    rx: Receiver<Outcome>,
    state: BridgeState,
}

impl BridgeState {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            current: Arc::new(Mutex::new(None)),
            ask_serial: Arc::new(Mutex::new(())),
            waiting: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Records the question currently shown by the panel.
    pub fn set_current(&self, ev: QuestionEvent) {
        *lock_ok(&self.current) = Some(ev);
    }

    /// The question currently on screen, if any (panel pulls this on mount).
    pub fn current(&self) -> Option<QuestionEvent> {
        lock_ok(&self.current).clone()
    }

    pub fn begin_question(&self, id: i64) -> PendingQuestion {
        let (tx, rx) = channel();
        let prev = lock_ok(&self.pending).insert(id, tx);
        debug_assert!(prev.is_none(), "duplicate question id {id}");
        PendingQuestion {
            id,
            rx,
            state: self.clone(),
        }
    }

    /// Returns false if the id is unknown (already resolved / timed out).
    pub fn resolve(&self, id: i64, outcome: Outcome) -> bool {
        if let Some(tx) = lock_ok(&self.pending).remove(&id) {
            self.clear_current_if(id);
            let _ = tx.send(outcome);
            true
        } else {
            false
        }
    }

    /// Clears the on-screen question if it is `id` (resolve and timeout paths).
    pub fn clear_current_if(&self, id: i64) {
        let mut cur = lock_ok(&self.current);
        if cur.as_ref().is_some_and(|c| c.id == id) {
            *cur = None;
        }
    }
}

/// Recover the guard even if a previous holder panicked — the bridge must not
/// wedge on a poisoned lock.
fn lock_ok<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl PendingQuestion {
    pub fn wait(self, timeout: Duration) -> Outcome {
        match self.rx.recv_timeout(timeout) {
            Ok(o) => o,
            Err(_) => {
                let removed = lock_ok(&self.state.pending).remove(&self.id);
                if removed.is_none() {
                    // resolve() won the race during our timeout and already
                    // sent the outcome — collect it instead of dropping it.
                    self.rx.try_recv().unwrap_or(Outcome::Timeout)
                } else {
                    Outcome::Timeout
                }
            }
        }
    }
}

impl Drop for PendingQuestion {
    fn drop(&mut self) {
        // Insurance against sender leaks if a holder unwinds without wait().
        lock_ok(&self.state.pending).remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn answer_resolves_waiting_ask() {
        let st = BridgeState::new();
        let pending = st.begin_question(7);
        let st2 = st.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            assert!(st2.resolve(7, Outcome::Answered("ok".into())));
        });
        let out = pending.wait(Duration::from_secs(2));
        assert!(matches!(out, Outcome::Answered(a) if a == "ok"));
    }

    #[test]
    fn timeout_when_nobody_answers() {
        let st = BridgeState::new();
        let pending = st.begin_question(1);
        let out = pending.wait(Duration::from_millis(50));
        assert!(matches!(out, Outcome::Timeout));
        // resolved/cleaned up: late answer is rejected
        assert!(!st.resolve(1, Outcome::Answered("late".into())));
    }

    #[test]
    fn resolve_unknown_id_is_false() {
        let st = BridgeState::new();
        assert!(!st.resolve(99, Outcome::Dismissed));
    }
}
