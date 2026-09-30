//! Echo Meeting Copilot — C1: continuous capture + live transcript.
//! Design: docs/superpowers/specs/2026-06-22-meeting-capture-c1-design.md
pub mod analyze;
pub mod capture;
pub mod council;
pub mod persist;
pub mod session;
pub mod transcribe;
pub mod window;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Which side of the meeting a segment came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Me,
    Others,
}
