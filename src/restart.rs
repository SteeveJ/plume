use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use crate::config::APP_NAME;

/// Pids des démons Plume (hors fenêtre de réglages, hors assistant de redémarrage et hors `me`).
pub fn find_daemon_pids(proc_root: &Path, me: u32) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(proc_root) else { return Vec::new() };
    entries.flatten().filter_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        if pid == me { return None; }
        let comm = std::fs::read_to_string(e.path().join("comm")).ok()?;
        if comm.trim() != APP_NAME { return None; }
        let cmdline = std::fs::read(e.path().join("cmdline")).ok()?;
        let is_helper = cmdline.split(|b| *b == 0).any(|a| a == b"--settings" || a == b"--restart");
        (!is_helper).then_some(pid)
    }).collect()
}

/// Lance l'assistant `plume --restart` (sans shell) ; il arrête le démon puis en relance un neuf.
pub fn request_restart() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).arg("--restart")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
}

/// Arrête l'ancien démon (SIGTERM, puis SIGKILL après 3 s) et démarre la même version du programme.
pub fn restart_daemon() -> Result<()> {
    let proc_root = Path::new("/proc");
    let pids = find_daemon_pids(proc_root, std::process::id());
    for pid in &pids {
        // SAFETY: simple appel système `kill` sur un pid numérique.
        unsafe { libc::kill(*pid as i32, libc::SIGTERM); }
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while pids.iter().any(|p| proc_root.join(p.to_string()).exists()) {
        if Instant::now() >= deadline {
            for pid in &pids { unsafe { libc::kill(*pid as i32, libc::SIGKILL); } }
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let exe = std::env::current_exe().context("current exe")?;
    Command::new(exe).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().context("start plume")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_proc(root: &Path, pid: &str, comm: &str, cmdline: &[&str]) {
        let d = root.join(pid);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("comm"), format!("{comm}\n")).unwrap();
        std::fs::write(d.join("cmdline"), cmdline.join("\0")).unwrap();
    }

    #[test]
    fn finds_only_real_daemons() {
        let root = std::env::temp_dir().join(format!("plume-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        fake_proc(&root, "100", "plume", &["plume"]);
        fake_proc(&root, "101", "plume", &["plume", "--settings"]);
        fake_proc(&root, "102", "bash", &["bash"]);
        fake_proc(&root, "103", "plume", &["plume", "--restart"]);
        fake_proc(&root, "999", "plume", &["plume"]);
        std::fs::create_dir_all(root.join("self")).unwrap();
        let mut pids = find_daemon_pids(&root, 999);
        let _ = std::fs::remove_dir_all(&root);
        pids.sort();
        assert_eq!(pids, vec![100]);
    }
}
