use anyhow::Result;
use crate::domain::model::AudioSamples;

#[cfg_attr(test, mockall::automock)]
// Not Send: cpal::Stream is thread-local on some platforms.
pub trait AudioRecorder {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<AudioSamples>;
}
