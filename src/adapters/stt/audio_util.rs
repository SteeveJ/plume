use anyhow::Result;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Cursor;
use crate::domain::model::AudioSamples;

pub const RATE: u32 = 16_000;

// shortcut: box-filter resampling (no real low-pass), good enough for voice; upgrade if artifacts become audible.
pub fn to_mono_16k(audio: &AudioSamples) -> Vec<f32> {
    let step = audio.channels.max(1) as usize;
    let mono: Vec<f32> = audio.samples.chunks(step)
        .map(|c| (c.iter().sum::<f32>() / step as f32).clamp(-1.0, 1.0))
        .collect();
    if audio.sample_rate == RATE {
        return mono;
    }
    let ratio = audio.sample_rate as f64 / RATE as f64;
    let out_len = (mono.len() as f64 / ratio) as usize;
    (0..out_len).map(|i| {
        let a = (i as f64 * ratio) as usize;
        let b = (((i + 1) as f64 * ratio) as usize).clamp(a + 1, mono.len());
        mono[a..b].iter().sum::<f32>() / (b - a) as f32
    }).collect()
}

/// Mono 16-bit 16 kHz WAV: what Whisper uses internally, 3x smaller than at 48 kHz.
pub fn encode_wav_16k(audio: &AudioSamples) -> Result<Vec<u8>> {
    let spec = WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: SampleFormat::Int };
    let mut buf = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut buf, spec)?;
    for s in to_mono_16k(audio) {
        writer.write_sample((s * 32767.0) as i16)?;
    }
    writer.finalize()?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, secs: f32) -> AudioSamples {
        let n = (rate as f32 * secs) as usize;
        let s = (0..n).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5).collect();
        AudioSamples::new(s, rate, 1)
    }

    #[test]
    fn resample_length() {
        assert_eq!(to_mono_16k(&sine(48_000, 1.0)).len(), 16_000);
        assert_eq!(to_mono_16k(&sine(44_100, 1.0)).len(), 16_000);
    }

    #[test]
    fn stereo_is_mixed_down() {
        let a = AudioSamples::new(vec![0.2, 0.4, 0.2, 0.4], 16_000, 2);
        assert_eq!(to_mono_16k(&a), vec![0.3, 0.3]);
    }

    #[test]
    fn wav_is_16khz_and_small() {
        let wav = encode_wav_16k(&sine(48_000, 1.0)).unwrap();
        let r = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!((r.spec().sample_rate, r.spec().channels, r.len()), (16_000, 1, 16_000));
    }
}
