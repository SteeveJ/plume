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
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device().context("no input device")?;
        let cfg = device.default_input_config().context("no input config")?;
        Ok(Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            config: cfg.config(),
            sample_rate: cfg.sample_rate().0,
            channels: cfg.channels(),
        })
    }
}

impl AudioRecorder for CpalRecorder {
    fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_input_device().context("no input device")?;
        let buf = Arc::clone(&self.buffer);
        buf.lock().unwrap().clear();

        let stream = device.build_input_stream(
            &self.config,
            move |data: &[f32], _| {
                buf.lock().unwrap().extend_from_slice(data);
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
        let samples = self.buffer.lock().unwrap().clone();
        Ok(AudioSamples::new(samples, self.sample_rate, self.channels))
    }
}
