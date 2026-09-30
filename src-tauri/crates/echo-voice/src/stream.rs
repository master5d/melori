//! Streaming readback driver (Epic C, Lever 1). Chunks text and synthesizes each
//! chunk in order through the VoiceRouter, emitting each result via `on_chunk`.
//! Tauri-free and SAPI-free: a chunk the cloned router cannot produce is emitted
//! as `ChunkOutput::Failed { text }` so the host app can speak it on its SAPI floor.
use std::sync::atomic::{AtomicBool, Ordering};

use crate::chunker::{chunk_text, clean_markdown};
use crate::router::VoiceRouter;
use crate::{SynthOpts, VoiceError, VoiceProfile};

/// Default readback chunk budget (chars). Packs ~1–2 sentences per chunk.
pub const READBACK_MAX_CHARS: usize = 200;

/// Per-chunk output, emitted in sentence order.
pub enum ChunkOutput {
    /// Cloned-tier WAV bytes (RIFF) for this chunk.
    Wav(Vec<u8>),
    /// The cloned router could not produce this chunk; the host should speak
    /// `text` on its SAPI floor. `err` is for logging.
    Failed { text: String, err: VoiceError },
}

/// Synthesize `text` as ordered chunks. See module docs.
pub fn synthesize_stream(
    text: &str,
    profile: &VoiceProfile,
    router: &VoiceRouter,
    opts: &SynthOpts,
    max_chars: usize,
    cancel: &AtomicBool,
    on_chunk: &mut dyn FnMut(ChunkOutput),
) -> Result<&'static str, VoiceError> {
    let clean = clean_markdown(text);
    let chunks = chunk_text(&clean, max_chars);
    let mut first_ok: Option<&'static str> = None;
    for chunk in chunks {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match router.synthesize(&chunk, profile, opts) {
            Ok((wav, id)) => {
                first_ok.get_or_insert(id);
                on_chunk(ChunkOutput::Wav(wav));
            }
            Err(err) => {
                on_chunk(ChunkOutput::Failed { text: chunk, err });
            }
        }
    }
    Ok(first_ok.unwrap_or("none"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::{chunk_text as ct, clean_markdown as cm};
    use crate::router::VoiceRouter;
    use crate::{LanguageHint, VoiceEngine, VoiceTier};

    fn prof() -> VoiceProfile {
        VoiceProfile {
            id: "p".into(),
            display_name: "P".into(),
            language_hint: LanguageHint::En,
            created: "n".into(),
            notes: String::new(),
            refs: vec![],
        }
    }

    /// Engine that echoes the chunk text back as the "wav" bytes, so tests can read
    /// the exact chunk sequence off `on_chunk`.
    struct EchoText;
    impl VoiceEngine for EchoText {
        fn id(&self) -> &'static str {
            "voxcpm_local"
        }
        fn synthesize(
            &self,
            t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            Ok(t.as_bytes().to_vec())
        }
    }
    struct Boom;
    impl VoiceEngine for Boom {
        fn id(&self) -> &'static str {
            "boom"
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            Err(VoiceError::Engine("boom".into()))
        }
    }
    fn local_router(engine: Box<dyn VoiceEngine>) -> VoiceRouter {
        VoiceRouter::new(
            VoiceTier::LocalOnly,
            None,
            Some(engine),
            None,
            Box::new(|| false),
        )
    }

    #[test]
    fn emits_chunks_in_order_and_returns_engine_id() {
        let router = local_router(Box::new(EchoText));
        let text = "One two. Three four. Five six.";
        let cancel = AtomicBool::new(false);
        let mut got: Vec<String> = Vec::new();
        let id = synthesize_stream(
            text,
            &prof(),
            &router,
            &SynthOpts::default(),
            12,
            &cancel,
            &mut |o| match o {
                ChunkOutput::Wav(b) => got.push(String::from_utf8(b).unwrap()),
                ChunkOutput::Failed { .. } => panic!("unexpected failure"),
            },
        )
        .unwrap();
        assert_eq!(id, "voxcpm_local");
        assert_eq!(got, ct(&cm(text), 12));
        assert!(got.len() >= 2, "expected multiple chunks, got {got:?}");
    }

    #[test]
    fn cancel_stops_before_next_chunk() {
        let router = local_router(Box::new(EchoText));
        let text = "One two. Three four. Five six.";
        let cancel = AtomicBool::new(false);
        let mut count = 0usize;
        synthesize_stream(
            text,
            &prof(),
            &router,
            &SynthOpts::default(),
            12,
            &cancel,
            &mut |_o| {
                count += 1;
                cancel.store(true, Ordering::Relaxed);
            },
        )
        .unwrap();
        assert_eq!(count, 1, "cancel after chunk 1 must stop further synthesis");
    }

    #[test]
    fn router_error_becomes_failed_with_chunk_text() {
        let router = local_router(Box::new(Boom));
        let text = "One two. Three four.";
        let cancel = AtomicBool::new(false);
        let mut fails: Vec<String> = Vec::new();
        let id = synthesize_stream(
            text,
            &prof(),
            &router,
            &SynthOpts::default(),
            12,
            &cancel,
            &mut |o| match o {
                ChunkOutput::Failed { text, .. } => fails.push(text),
                ChunkOutput::Wav(_) => panic!("unexpected wav"),
            },
        )
        .unwrap();
        assert_eq!(fails, ct(&cm(text), 12));
        assert_eq!(id, "none");
    }

    #[test]
    fn empty_text_emits_nothing() {
        let router = local_router(Box::new(EchoText));
        let cancel = AtomicBool::new(false);
        let mut n = 0usize;
        let id = synthesize_stream(
            "   ",
            &prof(),
            &router,
            &SynthOpts::default(),
            12,
            &cancel,
            &mut |_o| {
                n += 1;
            },
        )
        .unwrap();
        assert_eq!(n, 0);
        assert_eq!(id, "none");
    }
}
