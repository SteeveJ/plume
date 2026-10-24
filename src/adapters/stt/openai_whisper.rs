use async_trait::async_trait;
use anyhow::{Context, Result};
use reqwest::Client;
use crate::domain::model::{AudioSamples, Transcription};
use crate::domain::ports::stt::SpeechToText;
use super::shared_vocab::SharedVocab;

pub struct OpenAiWhisper {
    client: Client,
    api_key: String,
    model: String,
    lang: String,
    prompt: SharedVocab,
}

impl OpenAiWhisper {
    pub fn new(api_key: impl Into<String>, lang: impl Into<String>, prompt: SharedVocab) -> Self {
        Self {
            client: crate::adapters::http::client(std::time::Duration::from_secs(60)),
            api_key: api_key.into(),
            model: "whisper-1".to_string(),
            lang: lang.into(),
            prompt,
        }
    }

    fn encode_audio(&self, audio: &AudioSamples) -> Result<(Vec<u8>, &'static str, &'static str)> {
        #[cfg(feature = "opus")]
        match super::opus_enc::encode_ogg_opus(audio) {
            Ok(b) => return Ok((b, "audio.ogg", "audio/ogg")),
            Err(e) => tracing::warn!("opus encode failed, fallback WAV: {e:#}"),
        }
        Ok((super::audio_util::encode_wav_16k(audio).context("encode wav")?, "audio.wav", "audio/wav"))
    }
}

#[async_trait]
impl SpeechToText for OpenAiWhisper {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        let (bytes, name, mime) = self.encode_audio(audio)?;

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name)
            .mime_str(mime)?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "json");
        if !self.lang.is_empty() {
            form = form.text("language", self.lang.clone());
        }
        let prompt = self.prompt.lock().unwrap().clone();
        if !prompt.is_empty() {
            form = form.text("prompt", prompt);
        }

        let resp = self.client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("whisper request")?
            .error_for_status()
            .context("whisper status")?;

        let body: serde_json::Value = resp.json().await?;
        let text = body["text"].as_str().unwrap_or("").trim().to_string();

        Ok(Transcription { text })
    }
}
