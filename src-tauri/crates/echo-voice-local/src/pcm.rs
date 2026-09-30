use echo_voice::VoiceError;
use std::io::Cursor;

pub fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, VoiceError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut w = hound::WavWriter::new(&mut buf, spec)
            .map_err(|e| VoiceError::Engine(format!("wav writer: {e}")))?;
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            w.write_sample(v)
                .map_err(|e| VoiceError::Engine(format!("wav sample: {e}")))?;
        }
        w.finalize()
            .map_err(|e| VoiceError::Engine(format!("wav finalize: {e}")))?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_riff_wav_with_expected_size() {
        let s = vec![0.0f32, 0.5, -0.5, 1.0, -1.0];
        let wav = pcm_f32_to_wav(&s, 22050).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + s.len() * 2);
    }
    #[test]
    fn clamps_out_of_range() {
        assert_eq!(pcm_f32_to_wav(&[2.0, -2.0], 16000).unwrap().len(), 44 + 4);
    }
}
