//! Pure text/linguistic logic for Echo (heuristics, voice commands, transcript
//! formatting, custom-word correction). No Tauri, no I/O — testable anywhere.
pub mod heuristics;
pub mod phoneme;
pub mod rewrite;
pub mod subtitle_window;
pub mod text;
pub mod transcript_format;
pub mod voice_commands;

#[cfg(test)]
mod heuristics_tests;
