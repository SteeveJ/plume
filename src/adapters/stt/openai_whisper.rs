use async_trait::async_trait;
use anyhow::{Context, Result};
use hound::{WavWriter, WavSpec, SampleFormat};
use reqwest::Client;
use std::io::Cursor;
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
            client: Client::new(),
            api_key: api_key.into(),
            model: "whisper-1".to_string(),
            lang: lang.into(),
            prompt,
        }
    }

    fn encode_wav(&self, audio: &AudioSamples) -> Result<Vec<u8>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate: audio.sample_rate,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut buf = Cursor::new(Vec::new());
        let mut writer = WavWriter::new(&mut buf, spec)?;
        // mix down to mono
        let step = audio.channels as usize;
        for chunk in audio.samples.chunks(step) {
            let mono = chunk.iter().sum::<f32>() / step as f32;
            writer.write_sample(mono)?;
        }
        writer.finalize()?;
        Ok(buf.into_inner())
    }
}

#[async_trait]
impl SpeechToText for OpenAiWhisper {
    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        let wav = self.encode_wav(audio).context("encode wav")?;

        let part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;
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
