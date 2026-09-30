//! Kitten TTS Mini (B-24, интейк #138): третий локальный движок, EN-only.
//!
//! Пайплайн снят ЗАМЕРОМ с эталонного python-пайплайна (kitten_golden.json,
//! зверь 2026-08-24), а не чтением кода: бандл-espeak Echo выдаёт байт-в-байт
//! ту же IPA, что python-phonemizer (`həlˈoʊ wˈɜːld … kwˈɔlᵻɾi` — вплоть до `ᵻ`),
//! поэтому словарь и сборка ids проверяются золотыми векторами.
//!
//! Tensor contract (FIXED — снят с ONNX-сессии):
//! - input `input_ids`: int64  [1, T]  = [0] + ids(IPA+пунктуация) + [10, 0]
//! - input `style`:     f32    [1,256] = строка №min(len(text), 399) матрицы голоса
//! - input `speed`:     f32    [1]
//! - output `waveform`: f32    [N] → срез последних 5000 сэмплов (артефакт хвоста)
//!
//! espeak режет вывод по знакам препинания на строки и сами знаки опускает —
//! поэтому текст сегментируется ЗДЕСЬ (знак известен из исходника), espeak зовётся
//! на сегмент, знак приклеивается обратно: питоновский phonemizer с
//! `preserve_punctuation` делает то же самое.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use echo_voice::VoiceError;

/// Частота выхода модели (СНЯТА с эталона, не настройка).
pub const KITTEN_SAMPLE_RATE: u32 = 24000;
/// Хвостовой артефакт генерации: python-эталон делает `[..., :-5000]`.
const TAIL_TRIM_SAMPLES: usize = 5000;
/// Строк стиля в матрице голоса (стиль выбирается ПО ДЛИНЕ текста — трюк StyleTTS2).
const STYLE_ROWS: usize = 400;
const STYLE_DIM: usize = 256;

/// Словарь символов — ТОЧНАЯ копия python `TextCleaner` (порядок = id).
/// Менять нельзя: id жёстко зашиты в веса модели.
fn symbol_table() -> HashMap<char, i64> {
    let pad = "$";
    let punctuation = ";:,.!?¡¿—…\"«»“” ";
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let letters_ipa = "ɑɐɒæɓʙβɔɕçɗɖðʤəɘɚɛɜɝɞɟʄɡɠɢʛɦɧħɥʜɨɪʝɭɬɫɮʟɱɯɰŋɳɲɴøɵɸθœɶʘɹɺɾɻʀʁɽʂʃʈʧʉʊʋⱱʌɣɤʍχʎʏʑʐʒʔʡʕʢǀǁǂǃˈˌːˑʼʴʰʱʲʷˠˤ˞↓↑→↗↘'̩'ᵻ";
    let mut map = HashMap::new();
    for (i, c) in pad
        .chars()
        .chain(punctuation.chars())
        .chain(letters.chars())
        .chain(letters_ipa.chars())
        .enumerate()
    {
        // При дубликате символа (в python-строке ' повторяется) выигрывает ПЕРВЫЙ
        // id — как в python-цикле, который перезаписывает; порядок обхода тот же,
        // поэтому последний. Совпадение с эталоном закреплено golden-тестом.
        map.insert(c, i as i64);
    }
    map
}

/// Сегментация текста по знакам препинания, которые espeak молча съедает:
/// `[("Hello world", Some(',')), (" this is a probe", Some('.'))]`.
pub(crate) fn segment_by_punct(text: &str) -> Vec<(String, Option<char>)> {
    let puncts = [',', '.', '!', '?', ';', ':', '…'];
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if puncts.contains(&ch) {
            out.push((std::mem::take(&mut cur), Some(ch)));
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push((cur, None));
    }
    out
}

/// Сборка input_ids из фонемной строки (IPA с пунктуацией и пробелами).
/// Неизвестные словарю символы молча пропускаются — как python `except KeyError: pass`.
pub(crate) fn ids_from_phonemes(phonemes: &str, table: &HashMap<char, i64>) -> Vec<i64> {
    let mut ids = Vec::with_capacity(phonemes.chars().count() + 3);
    ids.push(0);
    for ch in phonemes.chars() {
        if let Some(&id) = table.get(&ch) {
            ids.push(id);
        }
    }
    ids.push(10);
    ids.push(0);
    ids
}

/// Голоса Kitten: имя → матрица стиля [400][256] f32.
///
/// Формат файла `voices.bin` — самодельный плоский (magic KVOX1), собирается
/// конвертером из апстримного voices.npz: npz = zip+npy, тащить zip-парсер в
/// крейт ради одного файла — лишняя зависимость.
pub struct KittenVoices {
    voices: HashMap<String, Vec<f32>>, // len = 400*256
}

impl KittenVoices {
    pub fn load(path: &Path) -> Result<KittenVoices, VoiceError> {
        let data = std::fs::read(path).map_err(|e| VoiceError::Io(format!("read voices: {e}")))?;
        Self::parse(&data)
    }

    pub(crate) fn parse(data: &[u8]) -> Result<KittenVoices, VoiceError> {
        let bad = |m: &str| VoiceError::Engine(format!("voices.bin: {m}"));
        if data.len() < 9 || &data[0..5] != b"KVOX1" {
            return Err(bad("bad magic"));
        }
        let n = u32::from_le_bytes(data[5..9].try_into().unwrap()) as usize;
        let mut off = 9usize;
        let mut voices = HashMap::new();
        for _ in 0..n {
            if off + 2 > data.len() {
                return Err(bad("truncated name len"));
            }
            let name_len = u16::from_le_bytes(data[off..off + 2].try_into().unwrap()) as usize;
            off += 2;
            let name = std::str::from_utf8(
                data.get(off..off + name_len)
                    .ok_or_else(|| bad("truncated name"))?,
            )
            .map_err(|_| bad("name not utf8"))?
            .to_string();
            off += name_len;
            let want = STYLE_ROWS * STYLE_DIM * 4;
            let raw = data
                .get(off..off + want)
                .ok_or_else(|| bad("truncated matrix"))?;
            off += want;
            let floats: Vec<f32> = raw
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            voices.insert(name, floats);
        }
        Ok(KittenVoices { voices })
    }

    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.voices.keys().map(|s| s.as_str()).collect();
        v.sort();
        v
    }

    /// Строка стиля для голоса: №min(len(text), 399). Длина — в СИМВОЛАХ исходного
    /// текста (python `len(text)`), не в байтах и не в фонемах.
    pub fn style_row(&self, voice: &str, text_chars: usize) -> Result<&[f32], VoiceError> {
        let m = self
            .voices
            .get(voice)
            .ok_or_else(|| VoiceError::Engine(format!("kitten voice not found: {voice}")))?;
        let row = text_chars.min(STYLE_ROWS - 1);
        Ok(&m[row * STYLE_DIM..(row + 1) * STYLE_DIM])
    }
}

/// ort-сессия Kitten. Та же дисциплина, что у Piper `OnnxTts`.
pub struct KittenOnnx {
    session: Option<Mutex<ort::session::Session>>,
}

impl KittenOnnx {
    pub fn load(model_path: &Path) -> Result<KittenOnnx, VoiceError> {
        let session = ort::session::Session::builder()
            .map_err(|e| VoiceError::Engine(format!("ort builder: {e}")))?
            .commit_from_file(model_path)
            .map_err(|e| VoiceError::Engine(format!("ort load: {e}")))?;
        Ok(KittenOnnx {
            session: Some(Mutex::new(session)),
        })
    }

    #[cfg(test)]
    pub(crate) fn dummy() -> KittenOnnx {
        KittenOnnx { session: None }
    }

    pub fn infer(&self, ids: &[i64], style: &[f32], speed: f32) -> Result<Vec<f32>, VoiceError> {
        use ort::value::Tensor;
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| VoiceError::Engine("dummy kitten session".into()))?;
        let mut session = session.lock().unwrap();

        let input_ids = Tensor::from_array(([1usize, ids.len()], ids.to_vec()))
            .map_err(|e| VoiceError::Engine(format!("ids tensor: {e}")))?;
        let style_t = Tensor::from_array(([1usize, STYLE_DIM], style.to_vec()))
            .map_err(|e| VoiceError::Engine(format!("style tensor: {e}")))?;
        let speed_t = Tensor::from_array(([1usize], vec![speed]))
            .map_err(|e| VoiceError::Engine(format!("speed tensor: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids,
                "style" => style_t,
                "speed" => speed_t,
            ])
            .map_err(|e| VoiceError::Engine(format!("kitten run: {e}")))?;

        let wav = outputs["waveform"]
            .try_extract_tensor::<f32>()
            .map_err(|e| VoiceError::Engine(format!("waveform extract: {e}")))?
            .1
            .to_vec();
        // Хвостовой артефакт: эталон режет последние 5000 сэмплов всегда.
        let keep = wav.len().saturating_sub(TAIL_TRIM_SAMPLES);
        Ok(wav[..keep].to_vec())
    }
}

/// Полный пайплайн текст→ids: сегментация по пунктуации → espeak IPA на сегмент
/// (через переданный колбэк — сам espeak живёт в `phonemize.rs`) → знак + пробел.
pub(crate) fn phonemize_text(
    text: &str,
    mut espeak_ipa: impl FnMut(&str) -> Result<String, VoiceError>,
) -> Result<String, VoiceError> {
    let mut out = String::new();
    for (chunk, punct) in segment_by_punct(text) {
        let chunk = chunk.trim();
        if !chunk.is_empty() {
            let ipa = espeak_ipa(chunk)?;
            // espeak может отдать несколько строк — внутри сегмента это пробелы
            out.push_str(
                ipa.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .as_str(),
            );
        }
        if let Some(p) = punct {
            // Пробел И ПЕРЕД знаком: препроцессор эталона расставляет пробелы
            // вокруг пунктуации, и золотые ids содержат [' ', ',', ' '] —
            // печать фонемной строки этот пробел скрадывала, ids не врут.
            out.push(' ');
            out.push(p);
        }
        out.push(' ');
    }
    // Хвостового пробела в эталонных ids нет — финальный знак завершает строку.
    while out.ends_with(' ') {
        out.pop();
    }
    Ok(out)
}

use crate::pcm::pcm_f32_to_wav;
use crate::phonemize::Phonemizer;
use echo_voice::{LanguageHint, SynthOpts, VoiceEngine, VoiceProfile};

/// Голос по умолчанию (Jasper в терминах апстрима — тот, что владелец одобрил
/// ухом в пилоте #138). Настройка выбора голоса — хвост B-24, не блокер.
pub const DEFAULT_KITTEN_VOICE: &str = "expr-voice-3-m";

/// Kitten как `VoiceEngine`. EN-ONLY: Ru/Mixed честно возвращают Err, и цепочка
/// движков (`selected → fallback → SAPI`) уходит к Piper/Supertonic — движок,
/// не знающий языка, не имеет права его изображать (урок bake-off: необученные
/// движки на чужом языке мусорят, а не деградируют).
pub struct KittenEngine {
    onnx: KittenOnnx,
    voices: KittenVoices,
    phon: Phonemizer,
    voice: String,
    table: HashMap<char, i64>,
}

impl KittenEngine {
    /// `model_dir` ждёт `model.onnx` + `voices.bin` (бандл kitten-tts-mini-en).
    pub fn load(
        model_dir: &Path,
        phonemizer_bin: &Path,
        phonemizer_data_dir: &Path,
        voice: &str,
    ) -> Result<KittenEngine, VoiceError> {
        let onnx = KittenOnnx::load(&model_dir.join("model.onnx"))?;
        let voices = KittenVoices::load(&model_dir.join("voices.bin"))?;
        let voice = if voices.names().contains(&voice) {
            voice.to_string()
        } else {
            DEFAULT_KITTEN_VOICE.to_string()
        };
        Ok(KittenEngine {
            onnx,
            voices,
            phon: Phonemizer {
                bin: phonemizer_bin.into(),
                data_dir: phonemizer_data_dir.into(),
                timeout: std::time::Duration::from_secs(5),
            },
            voice,
            table: symbol_table(),
        })
    }
}

impl VoiceEngine for KittenEngine {
    fn id(&self) -> &'static str {
        "kitten_local"
    }

    fn synthesize(
        &self,
        text: &str,
        profile: &VoiceProfile,
        opts: &SynthOpts,
    ) -> Result<Vec<u8>, VoiceError> {
        if !matches!(profile.language_hint, LanguageHint::En) {
            return Err(VoiceError::Engine("kitten is en-only".into()));
        }
        if text.trim().is_empty() {
            return pcm_f32_to_wav(&[], KITTEN_SAMPLE_RATE);
        }
        let phonemes = phonemize_text(text, |chunk| self.phon.text_to_ipa(chunk, "en-us"))?;
        let ids = ids_from_phonemes(&phonemes, &self.table);
        let style = self.voices.style_row(&self.voice, text.chars().count())?;
        let speed = if opts.rate > 0.0 { opts.rate } else { 1.0 };
        let pcm = self.onnx.infer(&ids, style, speed)?;
        pcm_f32_to_wav(&pcm, KITTEN_SAMPLE_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Золотой вектор снят с python-пайплайна (kitten_golden.json, 2026-08-24):
    /// текст "Hello world, this is a quality probe." → фонемы → ids.
    const GOLDEN_PHONEMES: &str = "həlˈoʊ wˈɜːld , ðɪs ɪz ɐ kwˈɔlᵻɾi pɹˈoʊb .";
    const GOLDEN_IDS: &[i64] = &[
        0, 50, 83, 54, 156, 57, 135, 16, 65, 156, 87, 158, 54, 46, 16, 3, 16, 81, 102, 61, 16, 102,
        68, 16, 70, 16, 53, 65, 156, 76, 54, 177, 125, 51, 16, 58, 123, 156, 57, 135, 44, 16, 4,
        10, 0,
    ];

    #[test]
    fn golden_ids_match_python_pipeline() {
        let table = symbol_table();
        assert_eq!(ids_from_phonemes(GOLDEN_PHONEMES, &table), GOLDEN_IDS);
    }

    #[test]
    fn golden_phonemes_reassemble_from_segments() {
        // espeak-выход для двух сегментов золотой фразы — снят с бандл-espeak
        // Echo (совпал с python-phonemizer байт-в-байт).
        let mut calls = vec!["həlˈoʊ wˈɜːld", "ðɪs ɪz ɐ kwˈɔlᵻɾi pɹˈoʊb"].into_iter();
        let got = phonemize_text("Hello world, this is a quality probe.", |_chunk| {
            Ok(calls.next().unwrap().to_string())
        })
        .unwrap();
        assert_eq!(got, GOLDEN_PHONEMES);
    }

    #[test]
    fn segmenter_keeps_punct_and_tail() {
        assert_eq!(
            segment_by_punct("a, b. c"),
            vec![
                ("a".into(), Some(',')),
                (" b".into(), Some('.')),
                (" c".into(), None)
            ]
        );
    }

    #[test]
    fn unknown_symbols_are_skipped_like_python() {
        let table = symbol_table();
        // ъ нет в словаре — пропускается, не ошибка
        let ids = ids_from_phonemes("aъb", &table);
        assert_eq!(ids.len(), 2 + 2 + 1); // [0] a b [10 0]
    }

    #[test]
    fn voices_bin_round_trip_and_style_row() {
        let mut buf: Vec<u8> = b"KVOX1".to_vec();
        buf.extend_from_slice(&1u32.to_le_bytes());
        let name = b"expr-voice-3-m";
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(name);
        let mut matrix = vec![0f32; STYLE_ROWS * STYLE_DIM];
        matrix[7 * STYLE_DIM] = 42.0; // маркер строки 7
        matrix[(STYLE_ROWS - 1) * STYLE_DIM] = 9.0; // маркер последней строки
        for f in &matrix {
            buf.extend_from_slice(&f.to_le_bytes());
        }
        let v = KittenVoices::parse(&buf).unwrap();
        assert_eq!(v.names(), vec!["expr-voice-3-m"]);
        assert_eq!(v.style_row("expr-voice-3-m", 7).unwrap()[0], 42.0);
        // текст длиннее матрицы — клампится в последнюю строку, не паникует
        assert_eq!(v.style_row("expr-voice-3-m", 100_000).unwrap()[0], 9.0);
        assert!(v.style_row("nope", 1).is_err());
    }

    #[test]
    fn truncated_voices_bin_is_an_error_not_a_panic() {
        assert!(KittenVoices::parse(b"KVOX1").is_err());
        assert!(KittenVoices::parse(b"JUNK!....").is_err());
        let mut buf: Vec<u8> = b"KVOX1".to_vec();
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&(4u16).to_le_bytes());
        buf.extend_from_slice(b"name"); // матрицы нет вовсе
        assert!(KittenVoices::parse(&buf).is_err());
    }

    fn probe_profile(hint: echo_voice::LanguageHint) -> echo_voice::VoiceProfile {
        echo_voice::VoiceProfile {
            id: "p".into(),
            display_name: "p".into(),
            language_hint: hint,
            created: "now".into(),
            notes: String::new(),
            refs: vec![],
        }
    }

    #[test]
    fn ru_and_mixed_are_refused_before_any_synthesis() {
        use echo_voice::{LanguageHint, SynthOpts, VoiceEngine};
        let eng = KittenEngine {
            onnx: KittenOnnx::dummy(),
            voices: KittenVoices::parse({
                let mut b: Vec<u8> = b"KVOX1".to_vec();
                b.extend_from_slice(&0u32.to_le_bytes());
                &b.clone()
            })
            .unwrap(),
            phon: crate::phonemize::Phonemizer {
                bin: std::path::PathBuf::from("nonexistent"),
                data_dir: std::path::PathBuf::from("nonexistent"),
                timeout: std::time::Duration::from_secs(1),
            },
            voice: DEFAULT_KITTEN_VOICE.into(),
            table: symbol_table(),
        };
        for hint in [LanguageHint::Ru, LanguageHint::Mixed] {
            let r = eng.synthesize("привет", &probe_profile(hint), &SynthOpts::default());
            assert!(r.is_err(), "kitten must refuse non-EN, got Ok for {hint:?}");
        }
    }
}
