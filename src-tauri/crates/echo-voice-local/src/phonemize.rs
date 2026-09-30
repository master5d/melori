use echo_voice::VoiceError;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Phonemizer {
    pub bin: PathBuf,
    pub data_dir: PathBuf,
    pub timeout: Duration,
}

pub(crate) fn phonemes_to_piper_ids(phonemes: &str, map: &HashMap<String, Vec<i64>>) -> Vec<i64> {
    let mut ids = Vec::new();
    let push = |ids: &mut Vec<i64>, k: &str| {
        if let Some(v) = map.get(k) {
            ids.extend(v.iter().copied());
        }
    };
    push(&mut ids, "^"); // BOS
    let mut has_phoneme = false;
    for ch in phonemes.chars() {
        let key = ch.to_string();
        if map.contains_key(&key) {
            push(&mut ids, "_"); // pad before each phoneme
            push(&mut ids, &key); // phoneme
            has_phoneme = true;
        }
    }
    if has_phoneme {
        push(&mut ids, "_"); // trailing pad after last phoneme
    }
    push(&mut ids, "$"); // EOS
    ids
}

impl Phonemizer {
    /// Сырой espeak IPA для целого куска текста (нормализованный пробелами).
    /// Нужен движкам с посимвольным словарём (Kitten): им нужна СТРОКА фонем,
    /// а не piper-ids. espeak опускает пунктуацию — сегментация лежит на вызывающем.
    pub fn text_to_ipa(&self, text: &str, espeak_voice: &str) -> Result<String, VoiceError> {
        let out = self.run_espeak(text, espeak_voice)?;
        Ok(out.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    fn run_espeak(&self, text: &str, espeak_voice: &str) -> Result<String, VoiceError> {
        let mut child = Command::new(&self.bin)
            .args([
                "-q",
                "--ipa",
                "--path",
                &self.data_dir.to_string_lossy(),
                "-v",
                espeak_voice,
                text,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| VoiceError::Engine(format!("phonemize spawn: {e}")))?;
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if start.elapsed() > self.timeout {
                        let _ = child.kill();
                        return Err(VoiceError::Engine("phonemize timeout".into()));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(VoiceError::Engine(format!("phonemize wait: {e}"))),
            }
        }
        let mut out = String::new();
        child
            .stdout
            .take()
            .ok_or_else(|| VoiceError::Engine("phonemize stdout not captured".into()))?
            .read_to_string(&mut out)
            .map_err(|e| VoiceError::Engine(format!("phonemize read: {e}")))?;
        Ok(out)
    }

    pub fn text_to_ids(
        &self,
        text: &str,
        espeak_voice: &str,
        map: &HashMap<String, Vec<i64>>,
    ) -> Result<Vec<i64>, VoiceError> {
        let out = self.run_espeak(text, espeak_voice)?;
        let normalized = out.split_whitespace().collect::<Vec<_>>().join(" ");
        Ok(phonemes_to_piper_ids(&normalized, map))
    }

    /// Per-word IPA for `text` using `espeak -q --ipa`. One espeak call per
    /// whitespace-separated word so the returned Vec lines up 1:1 with the
    /// word split. Returns an error if espeak can't be run.
    pub fn text_to_words_ipa(
        &self,
        text: &str,
        espeak_voice: &str,
    ) -> Result<Vec<String>, VoiceError> {
        let mut out = Vec::new();
        for word in text.split_whitespace() {
            out.push(self.word_to_ipa(word, espeak_voice)?);
        }
        Ok(out)
    }

    fn word_to_ipa(&self, word: &str, espeak_voice: &str) -> Result<String, VoiceError> {
        let mut child = Command::new(&self.bin)
            .args([
                "-q",
                "--ipa",
                "--path",
                &self.data_dir.to_string_lossy(),
                "-v",
                espeak_voice,
                word,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| VoiceError::Engine(format!("phonemize spawn: {e}")))?;
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if start.elapsed() > self.timeout {
                        let _ = child.kill();
                        return Err(VoiceError::Engine("phonemize timeout".into()));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(VoiceError::Engine(format!("phonemize wait: {e}"))),
            }
        }
        let mut s = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout
                .read_to_string(&mut s)
                .map_err(|e| VoiceError::Engine(format!("phonemize read: {e}")))?;
        }
        Ok(s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn map() -> HashMap<String, Vec<i64>> {
        HashMap::from([
            ("_".into(), vec![0]),
            ("^".into(), vec![1]),
            ("$".into(), vec![2]),
            ("p".into(), vec![5]),
            ("r".into(), vec![6]),
        ])
    }

    #[test]
    fn piper_layout_wraps_and_interleaves_pad() {
        // ^ _ p _ r _ $  -> [1,0,5,0,6,0,2]
        assert_eq!(
            phonemes_to_piper_ids("pr", &map()),
            vec![1, 0, 5, 0, 6, 0, 2]
        );
    }

    #[test]
    fn unknown_phonemes_skipped() {
        assert_eq!(
            phonemes_to_piper_ids("pxr", &map()),
            vec![1, 0, 5, 0, 6, 0, 2]
        );
    }

    // The Windows subprocess path (spawning the real piper-phonemize .exe) is validated
    // at integration/live-smoke; direct .exe spawn on Windows works fine — only batch-file
    // stubs don't (CreateProcess cannot execute .cmd directly).
    #[cfg(unix)]
    #[test]
    fn spawns_stub_and_returns_ids() {
        let bin = "tests/stub_phonemize.sh";
        let p = Phonemizer {
            bin: bin.into(),
            data_dir: "tests".into(),
            timeout: Duration::from_secs(5),
        };
        let ids = p.text_to_ids("привет", "ru", &map()).unwrap();
        assert!(!ids.is_empty() && ids.first() == Some(&1) && ids.last() == Some(&2));
    }
}
