//! Pure PCM primitives for Lever-2 streaming: preamble validation, byte→i16 decode
//! with odd-byte carry, and the jitter buffer with silence-fill. No I/O, no audio dep.
use crate::VoiceError;
use std::collections::VecDeque;

pub const PCM_SAMPLE_RATE: u32 = 24000;
pub const PCM_CHANNELS: u8 = 1;
pub const PCM_BITS: u8 = 16;
pub const PCM_PREAMBLE_MAGIC: &[u8; 4] = b"EPCM";
pub const PCM_PREAMBLE_LEN: usize = 10;

/// Validate the 10-byte stream preamble: magic + sample_rate(u32 le) + channels + bits.
pub fn parse_preamble(bytes: &[u8; 10]) -> Result<(), VoiceError> {
    if &bytes[0..4] != PCM_PREAMBLE_MAGIC {
        return Err(VoiceError::Engine("bad PCM stream magic".into()));
    }
    let sr = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if sr != PCM_SAMPLE_RATE || bytes[8] != PCM_CHANNELS || bytes[9] != PCM_BITS {
        return Err(VoiceError::Engine(format!(
            "unexpected PCM format: sr={sr} ch={} bits={}",
            bytes[8], bytes[9]
        )));
    }
    Ok(())
}

/// Accumulates raw little-endian s16 bytes across reads, carrying a trailing odd byte.
pub struct PcmDecoder {
    carry: Vec<u8>,
}
impl PcmDecoder {
    pub fn new() -> Self {
        Self { carry: Vec::new() }
    }
    /// Append `bytes`, drain complete little-endian i16 pairs into `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<i16>) {
        self.carry.extend_from_slice(bytes);
        let pairs = self.carry.len() / 2;
        for i in 0..pairs {
            let lo = self.carry[i * 2];
            let hi = self.carry[i * 2 + 1];
            out.push(i16::from_le_bytes([lo, hi]));
        }
        // keep the trailing odd byte (if any)
        let consumed = pairs * 2;
        self.carry.drain(0..consumed);
    }
}
impl Default for PcmDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Assembles a PCM stream: consumes the 10-byte preamble (validating format) then
/// decodes s16le PCM (carrying odd bytes). Feed arbitrary byte slices in any split.
pub struct PcmStreamAssembler {
    header: Vec<u8>,
    header_done: bool,
    dec: PcmDecoder,
}
impl PcmStreamAssembler {
    pub fn new() -> Self {
        Self {
            header: Vec::new(),
            header_done: false,
            dec: PcmDecoder::new(),
        }
    }
    /// Feed `bytes`; append any decoded i16 samples to `out`. Returns Err on a bad preamble.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<i16>) -> Result<(), VoiceError> {
        let mut data = bytes;
        if !self.header_done {
            let need = PCM_PREAMBLE_LEN - self.header.len();
            let take = need.min(data.len());
            self.header.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.header.len() < PCM_PREAMBLE_LEN {
                return Ok(()); // still accumulating the preamble
            }
            let mut head = [0u8; 10];
            head.copy_from_slice(&self.header[..PCM_PREAMBLE_LEN]);
            parse_preamble(&head)?;
            self.header_done = true;
        }
        if !data.is_empty() {
            self.dec.push(data, out);
        }
        Ok(())
    }
}
impl Default for PcmStreamAssembler {
    fn default() -> Self {
        Self::new()
    }
}

/// Jitter buffer feeding the audio Source. Silence-fills underruns.
pub struct StreamBuf {
    queue: VecDeque<i16>,
    ended: bool,
}
impl StreamBuf {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            ended: false,
        }
    }
    pub fn push(&mut self, s: &[i16]) {
        self.queue.extend(s.iter().copied());
    }
    pub fn end(&mut self) {
        self.ended = true;
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_ended(&self) -> bool {
        self.ended
    }
    /// Next sample for the audio callback: queued sample, else None if ended,
    /// else silence (0) so the callback never blocks and never ends early.
    pub fn next_sample(&mut self) -> Option<i16> {
        if let Some(s) = self.queue.pop_front() {
            Some(s)
        } else if self.ended {
            None
        } else {
            Some(0)
        }
    }
}
impl Default for StreamBuf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preamble(sr: u32, ch: u8, bits: u8) -> [u8; 10] {
        let mut p = [0u8; 10];
        p[0..4].copy_from_slice(PCM_PREAMBLE_MAGIC);
        p[4..8].copy_from_slice(&sr.to_le_bytes());
        p[8] = ch;
        p[9] = bits;
        p
    }

    #[test]
    fn valid_preamble_ok() {
        assert!(parse_preamble(&preamble(24000, 1, 16)).is_ok());
    }
    #[test]
    fn bad_magic_errs() {
        let mut p = preamble(24000, 1, 16);
        p[0] = b'X';
        assert!(matches!(parse_preamble(&p), Err(VoiceError::Engine(_))));
    }
    #[test]
    fn wrong_rate_errs() {
        assert!(matches!(
            parse_preamble(&preamble(48000, 1, 16)),
            Err(VoiceError::Engine(_))
        ));
    }

    #[test]
    fn decoder_carries_odd_byte_across_pushes() {
        // 3 samples = 6 bytes: 1, -1, 256. Delivered split so an odd byte carries.
        let s = [1i16, -1, 256];
        let mut bytes = Vec::new();
        for v in s {
            bytes.extend_from_slice(&v.to_le_bytes());
        } // 6 bytes
        let mut dec = PcmDecoder::new();
        let mut out = Vec::new();
        dec.push(&bytes[0..3], &mut out); // 3 bytes -> 1 sample + 1 carry
        assert_eq!(out, vec![1]);
        dec.push(&bytes[3..6], &mut out); // +3 bytes -> completes 2 more
        assert_eq!(out, vec![1, -1, 256]);
    }

    fn valid_stream_body() -> (Vec<u8>, Vec<i16>) {
        // valid preamble + 3 samples (1, -1, 256) little-endian
        let mut body = preamble(24000, 1, 16).to_vec();
        for v in [1i16, -1, 256] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        (body, vec![1, -1, 256])
    }

    #[test]
    fn assembler_whole_in_one_feed() {
        let (body, want) = valid_stream_body();
        let mut asm = PcmStreamAssembler::new();
        let mut out = Vec::new();
        asm.feed(&body, &mut out).unwrap();
        assert_eq!(out, want);
    }

    #[test]
    fn assembler_split_preamble_and_mid_sample_carry() {
        let (body, want) = valid_stream_body();
        let mut asm = PcmStreamAssembler::new();
        let mut out = Vec::new();
        // 1st feed: first 4 preamble bytes only (preamble spans feeds).
        asm.feed(&body[0..4], &mut out).unwrap();
        assert!(out.is_empty());
        // 2nd feed: remaining 6 preamble bytes + first 3 PCM bytes (odd -> mid-sample carry).
        asm.feed(&body[4..13], &mut out).unwrap();
        assert_eq!(out, vec![1]); // 1 sample decoded, 1 byte carried
                                  // 3rd feed: rest of the PCM payload.
        asm.feed(&body[13..], &mut out).unwrap();
        assert_eq!(out, want);
    }

    #[test]
    fn assembler_bad_magic_errs() {
        let mut body = preamble(24000, 1, 16).to_vec();
        body[0] = b'X';
        let mut asm = PcmStreamAssembler::new();
        let mut out = Vec::new();
        assert!(matches!(
            asm.feed(&body, &mut out),
            Err(VoiceError::Engine(_))
        ));
    }

    #[test]
    fn streambuf_returns_queued_then_silence_then_none() {
        let mut b = StreamBuf::new();
        b.push(&[7, 8]);
        assert_eq!(b.next_sample(), Some(7));
        assert_eq!(b.next_sample(), Some(8));
        assert_eq!(b.next_sample(), Some(0)); // underrun -> silence, not None
        b.end();
        assert_eq!(b.next_sample(), None); // ended + empty -> real end
    }
}
