//! Pure IPA phoneme segmentation + Needleman-Wunsch alignment for the Course
//! pronunciation trainer. No Tauri/runtime deps — unit-tested on Windows.

use echo_config::{PhonemeCell, PhonemeReport, PhonemeStatus, WordPhonemes};

/// One aligned position between the reference and the spoken phoneme sequence.
#[derive(Debug, Clone, PartialEq)]
pub enum AlignOp {
    Match(String),
    Sub { expected: String, said: String },
    Ins(String),
    Del(String),
}

/// Split an espeak `--ipa` string into phoneme units. Stress marks (ˈ ˌ) and
/// whitespace are dropped; length (ː ˑ), nasal (̃) and tie (͡) attach to the
/// current unit (a tie also joins the following base into the same unit).
pub fn segment_ipa(ipa: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut join_next = false;
    for ch in ipa.chars() {
        if ch == 'ˈ' || ch == 'ˌ' || ch.is_whitespace() {
            continue; // suprasegmental / separator — not a phoneme
        }
        if ch == 'ː' || ch == 'ˑ' || ch == '\u{0303}' {
            if let Some(last) = out.last_mut() {
                last.push(ch);
            }
            continue;
        }
        if ch == '\u{0361}' {
            // combining double inverted breve (tie) — attach + join the next base
            if let Some(last) = out.last_mut() {
                last.push(ch);
            }
            join_next = true;
            continue;
        }
        if join_next {
            if let Some(last) = out.last_mut() {
                last.push(ch);
            }
            join_next = false;
        } else {
            out.push(ch.to_string());
        }
    }
    out
}

/// Needleman-Wunsch alignment (match +1, mismatch -1, gap -1) with backtrace into
/// Match / Sub / Ins / Del ops. `Ins` = a phoneme present only in the spoken
/// side; `Del` = a reference phoneme the speaker omitted.
pub fn align_phonemes(reference: &[String], spoken: &[String]) -> Vec<AlignOp> {
    let n = reference.len();
    let m = spoken.len();
    // dp[i][j] = best score aligning reference[..i] with spoken[..j].
    let mut dp = vec![vec![0i32; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = -(i as i32);
    }
    for j in 0..=m {
        dp[0][j] = -(j as i32);
    }
    for i in 1..=n {
        for j in 1..=m {
            let diag = dp[i - 1][j - 1]
                + if reference[i - 1] == spoken[j - 1] {
                    1
                } else {
                    -1
                };
            let up = dp[i - 1][j] - 1; // deletion (consume reference)
            let left = dp[i][j - 1] - 1; // insertion (consume spoken)
            dp[i][j] = diag.max(up).max(left);
        }
    }
    // Backtrace.
    let mut ops = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0
            && j > 0
            && dp[i][j]
                == dp[i - 1][j - 1]
                    + if reference[i - 1] == spoken[j - 1] {
                        1
                    } else {
                        -1
                    }
        {
            if reference[i - 1] == spoken[j - 1] {
                ops.push(AlignOp::Match(reference[i - 1].clone()));
            } else {
                ops.push(AlignOp::Sub {
                    expected: reference[i - 1].clone(),
                    said: spoken[j - 1].clone(),
                });
            }
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] - 1 {
            ops.push(AlignOp::Del(reference[i - 1].clone()));
            i -= 1;
        } else {
            ops.push(AlignOp::Ins(spoken[j - 1].clone()));
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Matched phonemes / reference length (Match+Sub+Del), as 0..=100.
pub fn phoneme_accuracy(ops: &[AlignOp]) -> u8 {
    let mut matched = 0usize;
    let mut ref_len = 0usize;
    for op in ops {
        match op {
            AlignOp::Match(_) => {
                matched += 1;
                ref_len += 1;
            }
            AlignOp::Sub { .. } | AlignOp::Del(_) => ref_len += 1,
            AlignOp::Ins(_) => {}
        }
    }
    if ref_len == 0 {
        return 0;
    }
    ((matched as f32 / ref_len as f32) * 100.0).round() as u8
}

/// Build the per-word → per-phoneme report from the plain words + their IPA for
/// both sides. Words are paired by index; an extra reference word yields all
/// `Del` cells, an extra spoken word yields an `Ins`-only entry.
pub fn build_phoneme_report(
    ref_words: &[String],
    ref_ipa: &[String],
    spoken_words: &[String],
    spoken_ipa: &[String],
) -> PhonemeReport {
    let mut words = Vec::new();
    let mut all_ops: Vec<AlignOp> = Vec::new();
    let count = ref_words.len().max(spoken_words.len());
    for i in 0..count {
        let r_ipa = ref_ipa.get(i).map(|s| segment_ipa(s)).unwrap_or_default();
        let s_ipa = spoken_ipa
            .get(i)
            .map(|s| segment_ipa(s))
            .unwrap_or_default();
        let ops = align_phonemes(&r_ipa, &s_ipa);
        let cells = ops
            .iter()
            .map(|op| match op {
                AlignOp::Match(p) => PhonemeCell {
                    ipa: p.clone(),
                    status: PhonemeStatus::Ok,
                    said: None,
                },
                AlignOp::Sub { expected, said } => PhonemeCell {
                    ipa: expected.clone(),
                    status: PhonemeStatus::Sub,
                    said: Some(said.clone()),
                },
                AlignOp::Ins(said) => PhonemeCell {
                    ipa: said.clone(),
                    status: PhonemeStatus::Ins,
                    said: Some(said.clone()),
                },
                AlignOp::Del(expected) => PhonemeCell {
                    ipa: expected.clone(),
                    status: PhonemeStatus::Del,
                    said: None,
                },
            })
            .collect();
        let text = ref_words
            .get(i)
            .or_else(|| spoken_words.get(i))
            .cloned()
            .unwrap_or_default();
        words.push(WordPhonemes { text, cells });
        all_ops.extend(ops);
    }
    let overall = phoneme_accuracy(&all_ops);
    let note = if overall >= 90 {
        "Great pronunciation!".to_string()
    } else if overall >= 60 {
        "Close — check the highlighted sounds.".to_string()
    } else {
        "Keep practicing the highlighted sounds.".to_string()
    };
    PhonemeReport {
        overall,
        words,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_simple_word_dropping_stress() {
        // espeak en-us "think" ≈ "θˈɪŋk" — stress mark dropped, 4 phonemes.
        assert_eq!(segment_ipa("θˈɪŋk"), vec!["θ", "ɪ", "ŋ", "k"]);
    }

    #[test]
    fn keeps_length_and_tie_attached() {
        assert_eq!(segment_ipa("ɑː"), vec!["ɑː"]); // length attaches to base
        assert_eq!(segment_ipa("t͡ʃ"), vec!["t͡ʃ"]); // tie joins into one unit
    }

    #[test]
    fn align_flags_a_substitution() {
        let r = vec![
            "θ".to_string(),
            "ɪ".to_string(),
            "ŋ".to_string(),
            "k".to_string(),
        ];
        let s = vec![
            "s".to_string(),
            "ɪ".to_string(),
            "ŋ".to_string(),
            "k".to_string(),
        ];
        let ops = align_phonemes(&r, &s);
        assert_eq!(ops.len(), 4);
        assert!(
            matches!(&ops[0], AlignOp::Sub { expected, said } if expected == "θ" && said == "s")
        );
        assert!(matches!(&ops[1], AlignOp::Match(p) if p == "ɪ"));
    }

    #[test]
    fn align_flags_insertion_and_deletion() {
        let ins = align_phonemes(
            &["a".to_string(), "b".to_string()],
            &["a".to_string(), "x".to_string(), "b".to_string()],
        );
        assert!(ins.iter().any(|o| matches!(o, AlignOp::Ins(p) if p == "x")));

        let del = align_phonemes(
            &["a".to_string(), "x".to_string(), "b".to_string()],
            &["a".to_string(), "b".to_string()],
        );
        assert!(del.iter().any(|o| matches!(o, AlignOp::Del(p) if p == "x")));
    }

    #[test]
    fn accuracy_counts_matches_over_reference_length() {
        // 3 match, 1 sub → reference length 4 → 75%.
        let ops = vec![
            AlignOp::Match("a".into()),
            AlignOp::Match("b".into()),
            AlignOp::Match("c".into()),
            AlignOp::Sub {
                expected: "d".into(),
                said: "e".into(),
            },
        ];
        assert_eq!(phoneme_accuracy(&ops), 75);
        assert_eq!(phoneme_accuracy(&[]), 0);
    }

    #[test]
    fn builds_report_flagging_the_wrong_sound() {
        use echo_config::PhonemeStatus;
        let report = build_phoneme_report(
            &["think".to_string()],
            &["θɪŋk".to_string()],
            &["sink".to_string()],
            &["sɪŋk".to_string()],
        );
        assert_eq!(report.words.len(), 1);
        assert_eq!(report.words[0].text, "think");
        assert_eq!(report.words[0].cells[0].status, PhonemeStatus::Sub);
        assert_eq!(report.words[0].cells[0].said.as_deref(), Some("s"));
        assert_eq!(report.words[0].cells[1].status, PhonemeStatus::Ok);
        assert_eq!(report.overall, 75); // 3/4 phonemes matched
    }

    #[test]
    fn extra_reference_word_becomes_deletions() {
        let report = build_phoneme_report(
            &["a".to_string(), "b".to_string()],
            &["eɪ".to_string(), "biː".to_string()],
            &["a".to_string()],
            &["eɪ".to_string()],
        );
        assert_eq!(report.words.len(), 2);
        use echo_config::PhonemeStatus;
        assert!(report.words[1]
            .cells
            .iter()
            .all(|c| c.status == PhonemeStatus::Del));
    }
}
