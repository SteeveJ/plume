use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AudioSamples {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioSamples {
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self { samples, sample_rate, channels }
    }

    pub fn duration(&self) -> Duration {
        let frames = self.samples.len() / self.channels as usize;
        Duration::from_secs_f64(frames as f64 / self.sample_rate as f64)
    }
}

#[derive(Debug, Clone)]
pub struct Transcription {
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    RecordStart,
    RecordStop,
}
