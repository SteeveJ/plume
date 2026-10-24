use async_trait::async_trait;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use crate::domain::ports::processor::TextProcessor;

pub struct AnthropicProcessor {
    client: Client,
    api_key: String,
    model: String,
    system_prompt: String,
}

impl AnthropicProcessor {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self { client: crate::adapters::http::client(std::time::Duration::from_secs(30)), api_key: api_key.into(), model: model.into(), system_prompt: system_prompt.into() }
    }
}

#[async_trait]
impl TextProcessor for AnthropicProcessor {
    async fn process(&self, raw: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": format!("{}\n\n{raw}", self.system_prompt)}]
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await.context("anthropic request")?
            .error_for_status().context("anthropic status")?;

        let data: serde_json::Value = resp.json().await?;
        Ok(data["content"][0]["text"]
            .as_str().unwrap_or(raw).trim().to_string())
    }
}
