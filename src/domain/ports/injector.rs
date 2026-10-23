use anyhow::Result;

#[cfg_attr(test, mockall::automock)]
pub trait TextInjector: Send + Sync {
    fn inject(&self, text: &str) -> Result<()>;
}
