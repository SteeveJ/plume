use async_trait::async_trait;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use crate::domain::ports::processor::TextProcessor;

pub struct OllamaProcessor {
    client: Client,
    base_url: String,
    model: String,
    system_prompt: String,
}

impl OllamaProcessor {
    pub fn new(model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            client: crate::adapters::http::client(std::time::Duration::from_secs(60)),
            base_url: "http://localhost:11434".to_string(),
            model: model.into(),
            system_prompt: system_prompt.into(),
        }
    }
}

#[async_trait]
impl TextProcessor for OllamaProcessor {
    async fn process(&self, raw: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "stream": false,
            "messages": [{
                "role": "user",
                "content": format!("{}\n\n{raw}", self.system_prompt)
            }]
        });

        let resp = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .context("ollama request")?
            .error_for_status()
            .context("ollama status")?;

        let data: serde_json::Value = resp.json().await?;
        let text = data["message"]["content"]
            .as_str()
            .unwrap_or(raw)
            .trim()
            .to_string();

        Ok(text)
    }
}
