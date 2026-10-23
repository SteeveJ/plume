use anyhow::Result;
use crate::domain::model::HotkeyEvent;
use std::sync::mpsc::Sender;

pub trait HotkeyListener: Send + Sync {
    /// Starts listening and sends events through `tx`. Blocking; run in a thread.
    fn listen(&self, tx: Sender<HotkeyEvent>) -> Result<()>;
}
