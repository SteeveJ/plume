use anyhow::{Context, Result};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use opus::{Application, Bitrate, Channels, Encoder};
use crate::domain::model::AudioSamples;
use super::audio_util::{to_mono_16k, RATE};

const FRAME: usize = 320; // 20 ms à 16 kHz
const SERIAL: u32 = 0x504c_554d;

pub fn encode_ogg_opus(audio: &AudioSamples) -> Result<Vec<u8>> {
    let pcm = to_mono_16k(audio);
    let mut enc = Encoder::new(RATE, Channels::Mono, Application::Voip).context("opus encoder")?;
    enc.set_bitrate(Bitrate::Bits(24_000)).context("opus bitrate")?;
    let pre_skip = enc.get_lookahead().context("opus lookahead")? as u16 * 3; // en unités 48 kHz

    let mut out = Vec::new();
    let mut w = PacketWriter::new(&mut out);

    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1);
    head.push(1);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&RATE.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes());
    head.push(0);
    w.write_packet(head, SERIAL, PacketWriteEndInfo::EndPage, 0)?;

    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    let vendor = b"plume";
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    w.write_packet(tags, SERIAL, PacketWriteEndInfo::EndPage, 0)?;

    let total = pcm.len();
    let frames = total.div_ceil(FRAME).max(1);
    for n in 0..frames {
        let start = n * FRAME;
        let mut frame = [0f32; FRAME];
        let end = (start + FRAME).min(total);
        if start < end {
            frame[..end - start].copy_from_slice(&pcm[start..end]);
        }
        let packet = enc.encode_vec_float(&frame, 4000).context("opus encode")?;
        let last = n + 1 == frames;
        let (info, absgp) = if last {
            (PacketWriteEndInfo::EndStream, pre_skip as u64 + total as u64 * 3)
        } else {
            (PacketWriteEndInfo::NormalPacket, pre_skip as u64 + ((n + 1) * FRAME) as u64 * 3)
        };
        w.write_packet(packet, SERIAL, info, absgp)?;
    }
    drop(w);
    Ok(out)
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
    fn ogg_opus_headers_and_size() {
        let a = sine(48_000, 1.0);
        let ogg = encode_ogg_opus(&a).unwrap();
        assert_eq!(&ogg[..4], b"OggS");
        assert!(ogg.windows(8).any(|w| w == b"OpusHead"));
        assert!(ogg.windows(8).any(|w| w == b"OpusTags"));
        assert!(ogg.len() < 48_000 * 2 / 4, "ogg={} doit être << wav 16-bit", ogg.len());
    }

    #[test]
    fn empty_audio_does_not_panic() {
        assert!(encode_ogg_opus(&AudioSamples::new(vec![], 16_000, 1)).is_ok());
    }
}
