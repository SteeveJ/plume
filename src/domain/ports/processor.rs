use async_trait::async_trait;
use anyhow::Result;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait TextProcessor: Send + Sync {
    async fn process(&self, raw: &str) -> Result<String>;
}
