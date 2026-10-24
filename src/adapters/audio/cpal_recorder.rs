use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use std::sync::{Arc, Mutex};
use crate::domain::model::AudioSamples;
use crate::domain::ports::audio::AudioRecorder;

pub struct CpalRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    config: StreamConfig,
    sample_rate: u32,
    channels: u16,
}

impl CpalRecorder {
    /// Does not fail without a microphone: the device is re-read at every `start()` (headset plugged in later, default input changed).
    pub fn new() -> Result<Self> {
        let cfg = cpal::default_host().default_input_device().and_then(|d| d.default_input_config().ok());
        Ok(Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            config: cfg.as_ref().map_or(StreamConfig { channels: 1, sample_rate: cpal::SampleRate(16_000), buffer_size: cpal::BufferSize::Default }, |c| c.config()),
            sample_rate: cfg.as_ref().map_or(16_000, |c| c.sample_rate().0),
            channels: cfg.as_ref().map_or(1, |c| c.channels()),
        })
    }
}

/// Recording cap: 10 minutes (stuck key, forgotten toggle). At 48 kHz stereo 10 min is about 230 MB; without a cap it grows about 1.4 GB per hour until out-of-memory.
fn max_samples(sample_rate: u32, channels: u16) -> usize {
    sample_rate as usize * channels as usize * 600
}

/// Appends `data` only if there is room; returns `false` once the cap is reached.
fn append_capped(buf: &mut Vec<f32>, data: &[f32], max: usize) -> bool {
    if buf.len() + data.len() > max { return false; }
    buf.extend_from_slice(data);
    true
}

impl AudioRecorder for CpalRecorder {
    fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_input_device().context("no microphone found — plug one in or enable it")?;
        let cfg = device.default_input_config().context("the microphone cannot be used (busy or unsupported)")?;
        self.config = cfg.config();
        self.sample_rate = cfg.sample_rate().0;
        self.channels = cfg.channels();
        let buf = Arc::clone(&self.buffer);
        *buf.lock().unwrap() = Vec::new();
        let max_samples = max_samples(self.sample_rate, self.channels);

        let stream = device.build_input_stream(
            &self.config,
            move |data: &[f32], _| {
                append_capped(&mut buf.lock().unwrap(), data, max_samples);
            },
            |err| tracing::error!("audio stream error: {err}"),
            None,
        ).context("build input stream")?;

        stream.play().context("play stream")?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) -> Result<AudioSamples> {
        self.stream.take(); // drop → stops capture
        let samples = std::mem::take(&mut *self.buffer.lock().unwrap());
        Ok(AudioSamples::new(samples, self.sample_rate, self.channels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_is_ten_minutes_of_audio() {
        assert_eq!(max_samples(48_000, 2), 57_600_000);
        assert_eq!(max_samples(16_000, 1), 9_600_000);
    }

    #[test]
    fn buffer_never_grows_past_the_cap() {
        let mut buf = Vec::new();
        assert!(append_capped(&mut buf, &[0.1; 6], 10));
        assert!(!append_capped(&mut buf, &[0.1; 6], 10), "un bloc qui dépasse est refusé");
        assert!(append_capped(&mut buf, &[0.1; 4], 10), "un bloc qui tient exactement est accepté");
        assert_eq!(buf.len(), 10);
        assert!(!append_capped(&mut buf, &[0.1; 1], 10));
    }
}
