use crate::chunker::{chunk_text, clean_markdown};
use crate::router::VoiceRouter;
use crate::{SynthOpts, VoiceError, VoiceProfile};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub struct NarrateOpts {
    pub max_chars: usize,
    pub cache_dir: PathBuf,
}

pub fn narrate(
    router: &VoiceRouter,
    md: &str,
    profile: &VoiceProfile,
    opts: &NarrateOpts,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Vec<u8>, VoiceError> {
    std::fs::create_dir_all(&opts.cache_dir).map_err(|e| VoiceError::Io(e.to_string()))?;
    let clean = clean_markdown(md);
    let chunks = chunk_text(&clean, opts.max_chars);
    let total = chunks.len();
    let synth = SynthOpts::default();

    let mut wavs: Vec<Vec<u8>> = Vec::with_capacity(total);
    for (i, chunk) in chunks.iter().enumerate() {
        let mut hasher = Sha256::new();
        hasher.update(chunk.as_bytes());
        hasher.update(profile.id.as_bytes());
        let key = format!("{:x}", hasher.finalize());
        let cache_file = opts.cache_dir.join(format!("{key}.wav"));

        let bytes = if cache_file.exists() {
            std::fs::read(&cache_file).map_err(|e| VoiceError::Io(e.to_string()))?
        } else {
            let (b, _used) = router.synthesize(chunk, profile, &synth)?;
            std::fs::write(&cache_file, &b).map_err(|e| VoiceError::Io(e.to_string()))?;
            b
        };
        wavs.push(bytes);
        progress(i + 1, total);
    }
    concat_wavs(&wavs)
}

/// Concatenate PCM WAVs by splicing their data chunks. Assumes matching fmt
/// (the engine emits a consistent format); rewrites RIFF + data sizes.
fn concat_wavs(wavs: &[Vec<u8>]) -> Result<Vec<u8>, VoiceError> {
    if wavs.is_empty() {
        return Err(VoiceError::Engine("nothing to stitch (no chunks)".into()));
    }
    // Validate every chunk (including the first) BEFORE any slicing.
    for w in wavs {
        if w.len() < 44 || &w[0..4] != b"RIFF" {
            return Err(VoiceError::Engine("chunk is not a canonical WAV".into()));
        }
    }
    let header = &wavs[0][..44];
    let mut data = Vec::new();
    for w in wavs {
        data.extend_from_slice(&w[44..]);
    }
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(header);
    out.extend_from_slice(&data);
    // rewrite RIFF size (offset 4) = 36 + data_len; data size (offset 40) = data_len
    let data_len = data.len() as u32;
    out[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
    out[40..44].copy_from_slice(&data_len.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LanguageHint, VoiceEngine, VoiceTier};
    use tempfile::TempDir;

    // Minimal valid 44-byte WAV header + N data bytes (8kHz mono s16).
    fn wav(data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&8000u32.to_le_bytes());
        v.extend_from_slice(&16000u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    struct CountingShared(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl VoiceEngine for CountingShared {
        fn id(&self) -> &'static str {
            "voxcpm_local"
        }
        fn synthesize(
            &self,
            _t: &str,
            _p: &VoiceProfile,
            _o: &SynthOpts,
        ) -> Result<Vec<u8>, VoiceError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(wav(&[1, 2, 3, 4]))
        }
    }

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

    #[test]
    fn concat_two_wavs_sums_data() {
        let a = wav(&[1, 2, 3, 4]);
        let b = wav(&[5, 6]);
        let out = concat_wavs(&[a, b]).unwrap();
        // data chunk = 6 bytes, total RIFF size = 36 + 6
        assert_eq!(&out[0..4], b"RIFF");
        let riff_size = u32::from_le_bytes([out[4], out[5], out[6], out[7]]);
        assert_eq!(riff_size, 36 + 6);
    }

    #[test]
    fn concat_rejects_truncated_chunk() {
        let good = wav(&[1, 2, 3, 4]);
        let bad = vec![b'R', b'I', b'F']; // 3 bytes, < 44
        let r = concat_wavs(&[good, bad]);
        assert!(matches!(r, Err(VoiceError::Engine(_))));
    }

    #[test]
    fn narrate_caches_and_stitches() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let tmp = TempDir::new().unwrap();
        let counter1 = Arc::new(AtomicUsize::new(0));
        let router = VoiceRouter::new(
            VoiceTier::LocalOnly,
            None,
            Some(Box::new(CountingShared(counter1.clone()))),
            None,
            Box::new(|| false),
        );
        let opts = NarrateOpts {
            max_chars: 20,
            cache_dir: tmp.path().to_path_buf(),
        };
        let mut seen = Vec::new();
        let md = "One two three. Four five six.";
        let out = narrate(&router, md, &prof(), &opts, &mut |d, t| seen.push((d, t))).unwrap();
        assert!(out.starts_with(b"RIFF"));
        assert_eq!(seen.last().unwrap().0, seen.last().unwrap().1);
        assert!(
            counter1.load(Ordering::SeqCst) > 0,
            "first run must synthesize"
        );

        // Second run: fresh engine + counter, SAME cache dir → must reuse cache.
        let counter2 = Arc::new(AtomicUsize::new(0));
        let router2 = VoiceRouter::new(
            VoiceTier::LocalOnly,
            None,
            Some(Box::new(CountingShared(counter2.clone()))),
            None,
            Box::new(|| false),
        );
        let out2 = narrate(&router2, md, &prof(), &opts, &mut |_, _| {}).unwrap();
        assert_eq!(out, out2);
        assert_eq!(
            counter2.load(Ordering::SeqCst),
            0,
            "second run must be fully cached (engine never called)"
        );
    }
}
