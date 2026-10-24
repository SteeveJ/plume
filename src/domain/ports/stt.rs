use async_trait::async_trait;
use anyhow::Result;
use crate::domain::model::{AudioSamples, Transcription};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SpeechToText: Send + Sync {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription>;
    /// Called when recording starts: lets an unloaded model reload while the user speaks.
    fn warm(&self) {}
}

#[async_trait]
impl SpeechToText for Box<dyn SpeechToText> {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        (**self).transcribe(audio).await
    }
    fn warm(&self) { (**self).warm() }
}
