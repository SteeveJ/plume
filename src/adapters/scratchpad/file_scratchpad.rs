use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
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
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&self.path) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn appends_entries_to_a_private_file() {
        let p = std::env::temp_dir().join(format!("plume-scratch-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let s = FileScratchpad::new(p.clone());
        s.append("hello");
        s.append("world");
        let body = std::fs::read_to_string(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_file(&p);
        assert_eq!((body.lines().count(), mode), (2, 0o600));
        assert!(body.contains("hello") && body.contains("world"));
    }
}
