//! Per-source VAD-gated utterance accumulation → ASR → tagged Segment.
use crate::meeting::session::Segment;
use crate::meeting::Source;

/// ASR seam: real impl delegates to MoonshineStreaming; tests inject a fake.
pub trait MeetingAsr {
    fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<String>;
}

/// Boolean voice-activity seam (real impl wraps SmoothedVad; tests inject a stub).
pub trait VadGate {
    fn is_voice(&mut self, frame: &[f32]) -> bool;
}

/// Accumulates voice frames into utterances. `(start_ms, end_ms, pcm)`.
pub struct Utterancer {
    vad: Box<dyn VadGate + Send>,
    /// Carried for C1.4's capture path, which sizes cpal frames from this. Unused by the
    /// utterancer logic itself (frames arrive pre-sized), hence the allow.
    #[allow(dead_code)]
    frame_samples: usize,
    frame_ms: u64,
    hangover_frames: usize,
    buf: Vec<f32>,
    elapsed_ms: u64,
    utt_start_ms: u64,
    in_speech: bool,
    silence_run: usize,
}

impl Utterancer {
    pub fn new(
        vad: Box<dyn VadGate + Send>,
        frame_samples: usize,
        frame_ms: u64,
        hangover_frames: usize,
    ) -> Self {
        Self {
            vad,
            frame_samples,
            frame_ms,
            hangover_frames,
            buf: Vec::new(),
            elapsed_ms: 0,
            utt_start_ms: 0,
            in_speech: false,
            silence_run: 0,
        }
    }

    pub fn speaking(&self) -> bool {
        self.in_speech
    }

    /// Push one fixed-size frame; returns a closed utterance `(start_ms, end_ms, pcm)` when a
    /// speech→silence boundary (hangover frames of silence) completes it.
    pub fn push(&mut self, frame: &[f32]) -> Option<(u64, u64, Vec<f32>)> {
        let voice = self.vad.is_voice(frame);
        let frame_start = self.elapsed_ms;
        self.elapsed_ms += self.frame_ms;

        if voice {
            if !self.in_speech {
                self.in_speech = true;
                self.utt_start_ms = frame_start;
                self.buf.clear();
            }
            self.silence_run = 0;
            self.buf.extend_from_slice(frame);
            return None;
        }
        if self.in_speech {
            self.silence_run += 1;
            if self.silence_run >= self.hangover_frames {
                let out = (
                    self.utt_start_ms,
                    frame_start,
                    std::mem::take(&mut self.buf),
                );
                self.in_speech = false;
                self.silence_run = 0;
                return Some(out);
            }
        }
        None
    }

    /// Surface any in-progress utterance (call on stop).
    pub fn flush(&mut self) -> Option<(u64, u64, Vec<f32>)> {
        if self.in_speech && !self.buf.is_empty() {
            self.in_speech = false;
            Some((
                self.utt_start_ms,
                self.elapsed_ms,
                std::mem::take(&mut self.buf),
            ))
        } else {
            None
        }
    }
}

/// Drive one source's frames through the utterancer + ASR, emitting tagged Segments.
/// `frames` yields fixed-size PCM frames already at the engine sample rate. This is the
/// PULL-based convenience used by tests; the live capture path (C1.4) drives a per-source
/// `Utterancer` directly from the cpal callback (push model).
#[allow(clippy::too_many_arguments)]
pub fn run_source(
    source: Source,
    vad: Box<dyn VadGate + Send>,
    asr: &dyn MeetingAsr,
    frame_samples: usize,
    frame_ms: u64,
    hangover_frames: usize,
    frames: impl Iterator<Item = Vec<f32>>,
    mut emit: impl FnMut(&Segment),
) {
    let mut u = Utterancer::new(vad, frame_samples, frame_ms, hangover_frames);
    fn emit_utt(
        source: Source,
        asr: &dyn MeetingAsr,
        start_ms: u64,
        end_ms: u64,
        pcm: Vec<f32>,
        emit: &mut dyn FnMut(&Segment),
    ) {
        match asr.transcribe(&pcm) {
            Ok(text) if !text.trim().is_empty() => {
                let seg = Segment {
                    id: 0,
                    source,
                    speaker: None,
                    start_ms,
                    end_ms,
                    text,
                };
                emit(&seg);
            }
            Ok(_) => {}
            Err(e) => eprintln!("meeting asr failed on a chunk (dropped): {e}"),
        }
    }
    for f in frames {
        if let Some((s, e, pcm)) = u.push(&f) {
            emit_utt(source, asr, s, e, pcm, &mut emit);
        }
    }
    if let Some((s, e, pcm)) = u.flush() {
        emit_utt(source, asr, s, e, pcm, &mut emit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeAsr;
    impl MeetingAsr for FakeAsr {
        fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<String> {
            Ok(format!("utt:{}", pcm.len()))
        }
    }

    struct ThresholdVad;
    impl VadGate for ThresholdVad {
        fn is_voice(&mut self, frame: &[f32]) -> bool {
            frame.iter().copied().fold(0.0f32, f32::max) > 0.5
        }
    }

    fn frame(val: f32, n: usize) -> Vec<f32> {
        vec![val; n]
    }

    #[test]
    fn emits_one_segment_per_utterance_with_timestamps() {
        let mut u = Utterancer::new(Box::new(ThresholdVad), 16, 30, 1);
        let mut out = Vec::new();
        for f in [
            frame(0.0, 16),
            frame(1.0, 16),
            frame(1.0, 16),
            frame(0.0, 16),
            frame(0.0, 16),
        ] {
            if let Some(seg) = u.push(&f) {
                out.push(seg);
            }
        }
        if let Some(seg) = u.flush() {
            out.push(seg);
        }
        assert_eq!(out.len(), 1, "expected exactly one utterance, got {out:?}");
        let (start_ms, end_ms, pcm) = &out[0];
        assert!(end_ms > start_ms);
        assert_eq!(pcm.len(), 32, "two 16-sample voice frames accumulated");
    }

    #[test]
    fn flush_emits_trailing_partial() {
        let mut u = Utterancer::new(Box::new(ThresholdVad), 16, 30, 5);
        u.push(&frame(1.0, 16));
        let seg = u.flush().expect("trailing partial flushed");
        assert_eq!(seg.2.len(), 16);
    }

    #[test]
    fn brief_gap_within_hangover_does_not_split_utterance() {
        // hangover=3: a single silence frame in the middle of speech must NOT close the utterance;
        // voice resumes and the whole thing is one utterance closed only by 3 trailing silences.
        let mut u = Utterancer::new(Box::new(ThresholdVad), 16, 30, 3);
        let seq = [
            frame(1.0, 16), // voice
            frame(0.0, 16), // gap (silence_run=1 < 3)
            frame(1.0, 16), // voice resumes → silence_run resets
            frame(0.0, 16), // silence_run=1
            frame(0.0, 16), // silence_run=2
            frame(0.0, 16), // silence_run=3 → close
        ];
        let mut closed = Vec::new();
        for f in seq {
            if let Some(seg) = u.push(&f) {
                closed.push(seg);
            }
        }
        assert_eq!(
            closed.len(),
            1,
            "brief mid-speech gap must not split the utterance"
        );
        // two voice frames (16 each) accumulated across the gap = 32 samples
        assert_eq!(closed[0].2.len(), 32);
        assert!(
            u.flush().is_none(),
            "no trailing partial after a clean close"
        );
    }

    #[test]
    fn hangover_of_two_needs_exactly_two_silence_frames() {
        // hangover=2: one silence frame must NOT close; the second one does.
        let mut u = Utterancer::new(Box::new(ThresholdVad), 16, 30, 2);
        assert!(u.push(&frame(1.0, 16)).is_none()); // voice, open
        assert!(
            u.push(&frame(0.0, 16)).is_none(),
            "one silence frame must not close (hangover=2)"
        );
        let closed = u
            .push(&frame(0.0, 16))
            .expect("second silence frame closes the utterance");
        assert_eq!(closed.2.len(), 16);
    }

    #[test]
    fn run_source_appends_tagged_segments() {
        use crate::meeting::session::MeetingSession;
        use crate::meeting::Source;
        let sess = MeetingSession::default();
        sess.start("t0".into(), true);
        let frames = vec![frame(1.0, 16), frame(1.0, 16), frame(0.0, 16)];
        let mut emitted = Vec::new();
        run_source(
            Source::Others,
            Box::new(ThresholdVad),
            &FakeAsr,
            16,
            30,
            1,
            frames.into_iter(),
            |seg| {
                emitted.push(seg.clone());
                sess.append(seg.source, seg.start_ms, seg.end_ms, seg.text.clone());
            },
        );
        let snap = sess.snapshot();
        assert_eq!(snap.segments.len(), 1);
        assert_eq!(snap.segments[0].source, Source::Others);
        assert!(snap.segments[0].text.starts_with("utt:"));
    }
}
