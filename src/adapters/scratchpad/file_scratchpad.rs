use std::io::Write;
use std::path::PathBuf;
use crate::domain::ports::scratchpad::ScratchpadWriter;

pub struct FileScratchpad {
    path: PathBuf,
}

impl FileScratchpad {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ScratchpadWriter for FileScratchpad {
    fn append(&self, text: &str) {
        let timestamp = utc_hms();
        let entry = format!("[{timestamp}] {text}\n");
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = f.write_all(entry.as_bytes());
        }
        tracing::info!("appended to {}", self.path.display());
    }
}

fn utc_hms() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{:02}:{:02}:{:02}", secs / 3600 % 24, secs / 60 % 60, secs % 60)
}
