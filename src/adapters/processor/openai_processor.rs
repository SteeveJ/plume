use async_trait::async_trait;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use crate::domain::ports::processor::TextProcessor;

pub struct OpenAiProcessor {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: String,
}

impl OpenAiProcessor {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self { client: Client::new(), api_key: api_key.into(), model: model.into(), system_prompt: system_prompt.into() }
    }
}

#[async_trait]
impl TextProcessor for OpenAiProcessor {
    async fn process(&self, raw: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": format!("{}\n\n{raw}", self.system_prompt)}]
        });

        let resp = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await.context("openai request")?
            .error_for_status().context("openai status")?;

        let data: serde_json::Value = resp.json().await?;
        Ok(data["choices"][0]["message"]["content"]
            .as_str().unwrap_or(raw).trim().to_string())
    }
}
