use anyhow::Result;
use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind};
use crate::domain::ports::selection::SelectionReader;

/// Selected text = the desktop's "primary" selection (the middle-click one). It is read without typing anything: a simulated Ctrl+C
/// would interrupt a program running in a terminal. Limit: it keeps the last selected text even if the application
/// has deselected it since; the notification therefore shows the text read before replacing it.
pub struct PrimarySelection;

impl SelectionReader for PrimarySelection {
    fn selected_text(&self) -> Result<Option<String>> {
        let text = Clipboard::new()?.get().clipboard(LinuxClipboardKind::Primary).text().ok();
        Ok(text.filter(|t| !t.trim().is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arboard::SetExtLinux;

    #[test]
    fn reads_back_what_was_selected_and_treats_blank_as_nothing() {
        // Without a display server (continuous integration) the test has nothing to check.
        let Ok(mut cb) = Clipboard::new() else { return };
        if cb.set().clipboard(LinuxClipboardKind::Primary).text("  texte à réécrire ").is_err() { return; }
        assert_eq!(PrimarySelection.selected_text().unwrap().as_deref(), Some("  texte à réécrire "));
        let _ = cb.set().clipboard(LinuxClipboardKind::Primary).text("   ");
        assert_eq!(PrimarySelection.selected_text().unwrap(), None);
    }
}
