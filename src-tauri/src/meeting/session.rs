//! In-memory live transcript. No persistence (that's C2), no network.
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::meeting::Source;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Segment {
    pub id: u64,
    pub source: Source,
    pub speaker: Option<String>, // always None in C1 (per-speaker diarization deferred)
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
pub struct LiveTranscript {
    pub started_at: String, // rfc3339
    pub active: bool,
    pub loopback_active: bool,
    pub segments: Vec<Segment>,
}

#[derive(Default)]
pub struct MeetingSession {
    inner: Mutex<LiveTranscript>,
    next_id: Mutex<u64>,
    started: Mutex<Option<Instant>>,
    marks: Mutex<Vec<u64>>,
}

impl MeetingSession {
    pub fn start(&self, started_at: String, loopback_active: bool) {
        let mut t = self.inner.lock().unwrap();
        *t = LiveTranscript {
            started_at,
            active: true,
            loopback_active,
            segments: Vec::new(),
        };
        *self.next_id.lock().unwrap() = 0;
        *self.started.lock().unwrap() = Some(Instant::now());
        self.marks.lock().unwrap().clear();
    }

    /// Returns the new segment id, or 0 when the session is inactive (segment ignored).
    pub fn append(&self, source: Source, start_ms: u64, end_ms: u64, text: String) -> u64 {
        let mut t = self.inner.lock().unwrap();
        if !t.active {
            return 0;
        }
        let mut nid = self.next_id.lock().unwrap();
        *nid += 1;
        let id = *nid;
        t.segments.push(Segment {
            id,
            source,
            speaker: None,
            start_ms,
            end_ms,
            text,
        });
        id
    }

    pub fn snapshot(&self) -> LiveTranscript {
        self.inner.lock().unwrap().clone()
    }

    pub fn add_mark(&self, elapsed_ms: u64) {
        if self.is_active() {
            self.marks.lock().unwrap().push(elapsed_ms);
        }
    }

    pub fn marks(&self) -> Vec<u64> {
        self.marks.lock().unwrap().clone()
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started
            .lock()
            .unwrap()
            .map(|started| started.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn is_active(&self) -> bool {
        self.inner.lock().unwrap().active
    }

    pub fn finish(&self) -> LiveTranscript {
        let mut t = self.inner.lock().unwrap();
        t.active = false;
        *self.started.lock().unwrap() = None;
        t.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::Source;

    #[test]
    fn append_assigns_monotonic_ids_and_snapshots() {
        let s = MeetingSession::default();
        s.start("2026-06-22T10:00:00Z".into(), true);
        let id1 = s.append(Source::Me, 0, 500, "hello".into());
        let id2 = s.append(Source::Others, 600, 900, "hi".into());
        assert_eq!((id1, id2), (1, 2));
        let snap = s.snapshot();
        assert!(snap.active);
        assert!(snap.loopback_active);
        assert_eq!(snap.segments.len(), 2);
        assert_eq!(snap.segments[0].source, Source::Me);
        assert_eq!(snap.segments[1].text, "hi");
    }

    #[test]
    fn finish_deactivates_and_returns_final() {
        let s = MeetingSession::default();
        s.start("2026-06-22T10:00:00Z".into(), false);
        s.append(Source::Me, 0, 100, "x".into());
        let fin = s.finish();
        assert!(!fin.active);
        assert!(!fin.loopback_active);
        assert_eq!(fin.segments.len(), 1);
        assert!(!s.snapshot().active);
    }

    #[test]
    fn append_when_inactive_is_ignored() {
        let s = MeetingSession::default();
        let id = s.append(Source::Me, 0, 1, "nope".into());
        assert_eq!(id, 0);
        assert_eq!(s.snapshot().segments.len(), 0);
    }
}
