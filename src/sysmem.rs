use std::path::{Path, PathBuf};

const HEADROOM_MB: u64 = 256;

/// `MemAvailable` from /proc/meminfo, in MB.
pub fn parse_available_mb(meminfo: &str) -> Option<u64> {
    let line = meminfo.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Memory this process can still use: the smaller of the machine's `MemAvailable` and what its cgroup limits leave
/// (a container, a systemd unit with `MemoryMax`, Flatpak...). `/proc/meminfo` alone reports the whole machine.
pub fn available_mb() -> Option<u64> {
    let machine = std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| parse_available_mb(&m));
    let limited = std::fs::read_to_string("/proc/self/cgroup").ok().and_then(|c| cgroup_headroom_mb_at(Path::new("/sys/fs/cgroup"), &c));
    match (machine, limited) {
        (Some(m), Some(l)) => Some(m.min(l)),
        (m, l) => m.or(l),
    }
}

/// Room left under the tightest `memory.max` between the process's cgroup (v2) and the root, in MB; `None` when nothing is limited.
/// Reclaimable page cache (`inactive_file` + `active_file`: a model file read twice moves to the active list) is not counted as
/// used: the kernel frees it before refusing memory.
pub fn cgroup_headroom_mb_at(root: &Path, proc_self_cgroup: &str) -> Option<u64> {
    let rel = proc_self_cgroup.lines().find_map(|l| l.strip_prefix("0::"))?;
    let mut dir: PathBuf = root.join(rel.trim().trim_start_matches('/'));
    let mut tightest: Option<u64> = None;
    while dir.starts_with(root) {
        if let Some(room) = headroom_of(&dir) { tightest = Some(tightest.map_or(room, |t| t.min(room))); }
        if dir == root || !dir.pop() { break; }
    }
    tightest
}

fn headroom_of(dir: &Path) -> Option<u64> {
    let num = |file: &str| std::fs::read_to_string(dir.join(file)).ok()?.trim().parse::<u64>().ok();
    let max = num("memory.max")?; // "max" (no limit) does not parse: None
    let current = num("memory.current")?;
    let stat = std::fs::read_to_string(dir.join("memory.stat")).unwrap_or_default();
    let field = |name: &str| stat.lines().find_map(|l| l.strip_prefix(name)?.trim().parse::<u64>().ok()).unwrap_or(0);
    let reclaimable = field("inactive_file ") + field("active_file ");
    Some(max.saturating_sub(current.saturating_sub(reclaimable)) / (1024 * 1024))
}

/// Does a model of `file_mb` MB fit in RAM? `factor_tenths` = real RAM / file size, ×10
/// (measured for Whisper `small`: 891 MB peak for a 466 MB file, i.e. 19). Unknown RAM: try anyway.
pub fn model_fits(file_mb: u64, factor_tenths: u64, available_mb: Option<u64>) -> bool {
    available_mb.is_none_or(|a| file_mb * factor_tenths / 10 + HEADROOM_MB <= a)
}

/// Run-time RAM / model file size, ×10, for a speech model. On the CPU the weights and the compute buffers live in RAM
/// (measured: 891 MB peak for a 466 MB file, so 19). In a GPU build they live in VRAM and RAM only holds the file while it
/// is read (measured with the 1.6 GB `turbo`: 480 MB after loading, 611 MB peak, so about 4); 6 keeps a margin.
pub const fn speech_model_ram_factor_tenths() -> u64 {
    if cfg!(any(feature = "cuda", feature = "vulkan")) { 6 } else { 19 }
}

pub fn required_mb(file_mb: u64, factor_tenths: u64) -> u64 {
    file_mb * factor_tenths / 10 + HEADROOM_MB
}

/// Free disk space (MB) on the volume containing `path` (or its first existing parent).
pub fn free_disk_mb(path: &Path) -> Option<u64> {
    let mut dir = path;
    while !dir.exists() { dir = dir.parent()?; }
    let c = std::ffi::CString::new(dir.to_str()?).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `st` a statvfs structure allocated by us.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 { return None; }
    Some(st.f_bavail as u64 * st.f_frsize as u64 / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_builds_need_far_less_ram_for_the_same_model() {
        let factor = speech_model_ram_factor_tenths();
        assert_eq!(factor, if cfg!(any(feature = "cuda", feature = "vulkan")) { 6 } else { 19 });
        // `turbo` (1.6 GB file) in a 2 GB cgroup: refused on the CPU, accepted on the GPU where it sits in VRAM.
        let fits = model_fits(1624, factor, Some(1900));
        assert_eq!(fits, cfg!(any(feature = "cuda", feature = "vulkan")));
    }

    #[test]
    fn parses_mem_available_in_mb() {
        let m = "MemTotal:       32000000 kB\nMemFree:  100 kB\nMemAvailable:   24160000 kB\n";
        assert_eq!(parse_available_mb(m), Some(23593));
        assert_eq!(parse_available_mb("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn model_fits_uses_measured_factor_and_headroom() {
        // Whisper small f16: 466 MB file, ×1.9 + 256 headroom ≈ 1141 MB required.
        assert!(model_fits(466, 19, Some(1200)));
        assert!(!model_fits(466, 19, Some(1000)));
        // large-v2 (≈3100 MB) on 4 GB free: refused.
        assert!(!model_fits(3100, 19, Some(4000)));
        assert!(model_fits(3100, 19, None), "RAM inconnue : on ne bloque pas");
        assert_eq!(required_mb(466, 19), 1141);
    }

    fn fake_cgroup(root: &Path, dir: &str, max: &str, current: u64, inactive_file: u64, active_file: u64) {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("memory.max"), format!("{max}\n")).unwrap();
        std::fs::write(d.join("memory.current"), format!("{current}\n")).unwrap();
        std::fs::write(d.join("memory.stat"), format!("anon 1\ninactive_file {inactive_file}\nactive_file {active_file}\n")).unwrap();
    }

    #[test]
    fn a_cgroup_limit_caps_what_a_process_may_use() {
        const MB: u64 = 1024 * 1024;
        let root = std::env::temp_dir().join(format!("plume-cg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        fake_cgroup(&root, "app", "max", 5000 * MB, 0, 0);          // no limit here, high use does not matter
        fake_cgroup(&root, "app/svc", "2147483648", 1024 * MB, 0, 0); // 2 GB limit, 1 GB used
        let line = "0::/app/svc\n";
        assert_eq!(cgroup_headroom_mb_at(&root, line), Some(1024));
        // Page cache that the kernel can drop is not "used".
        fake_cgroup(&root, "app/svc", "2147483648", 1024 * MB, 256 * MB, 256 * MB); // 256 inactive + 256 active
        assert_eq!(cgroup_headroom_mb_at(&root, line), Some(1536));
        // A tighter limit on a parent wins.
        fake_cgroup(&root, "app", "1073741824", 512 * MB, 0, 0);
        assert_eq!(cgroup_headroom_mb_at(&root, line), Some(512));
        // No limit anywhere, or cgroup v1 (no "0::" line): nothing to report.
        assert_eq!(cgroup_headroom_mb_at(&root, "0::/nowhere\n"), None);
        assert_eq!(cgroup_headroom_mb_at(&root, "4:memory:/app/svc\n"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_real_system_values() {
        assert!(available_mb().is_some_and(|m| m > 0));
        assert!(free_disk_mb(Path::new("/tmp/does/not/exist/yet")).is_some_and(|m| m > 0));
    }
}
