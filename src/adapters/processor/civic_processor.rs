use async_trait::async_trait;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use crate::domain::ports::processor::TextProcessor;

pub struct CivicProcessor {
    client: Client,
    url: String,
    mode: String,
    tone: String,
    diplomacy: u8,
    language: String,
}

impl CivicProcessor {
    pub fn new(
        url: impl Into<String>,
        mode: impl Into<String>,
        tone: impl Into<String>,
        diplomacy: u8,
        language: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            url: url.into(),
            mode: mode.into(),
            tone: tone.into(),
            diplomacy,
            language: language.into(),
        }
    }
}

#[async_trait]
impl TextProcessor for CivicProcessor {
    async fn process(&self, text: &str) -> Result<String> {
        let body = json!({
            "text": text,
            "mode": self.mode,
            "tone": self.tone,
            "diplomacy": self.diplomacy,
            "language": self.language,
        });
        let resp: serde_json::Value = self.client
            .post(format!("{}/api/transform", self.url))
            .json(&body)
            .send().await.context("civic request")?
            .error_for_status().context("civic status")?
            .json().await?;
        Ok(resp["output_text"].as_str().unwrap_or(text).to_owned())
    }
}
