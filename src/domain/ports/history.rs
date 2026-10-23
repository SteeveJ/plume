use anyhow::Result;

pub trait HistoryStore: Send {
    fn save(&mut self, raw: &str, text: &str, duration_ms: u64) -> Result<()>;
}
