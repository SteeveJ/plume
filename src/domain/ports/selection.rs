use anyhow::Result;

/// Reads the text currently selected in the active application, without touching the keyboard.
#[cfg_attr(test, mockall::automock)]
pub trait SelectionReader: Send + Sync {
    /// `None`: nothing is selected.
    fn selected_text(&self) -> Result<Option<String>>;
}
