//! Agent Bridge: localhost HTTP API letting agents ask the user questions
//! through a floating panel and journal the answers locally.
//! Design: docs/superpowers/specs/2026-06-12-agent-bridge-design.md
pub mod server;
pub mod state;
pub mod storage;
pub mod token;
pub mod window;
