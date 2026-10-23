use async_trait::async_trait;
use anyhow::Result;
use crate::domain::model::{AudioSamples, Transcription};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SpeechToText: Send + Sync {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription>;
}

#[async_trait]
impl SpeechToText for Box<dyn SpeechToText> {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        (**self).transcribe(audio).await
    }
}
