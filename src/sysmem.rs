use std::path::Path;

const HEADROOM_MB: u64 = 256;

/// `MemAvailable` de /proc/meminfo, en Mo.
pub fn parse_available_mb(meminfo: &str) -> Option<u64> {
    let line = meminfo.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

pub fn available_mb() -> Option<u64> {
    parse_available_mb(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

/// Un modèle de `file_mb` Mo tient-il en RAM ? `factor_tenths` = RAM réelle / taille du fichier, ×10
/// (mesuré pour Whisper `small` : 891 Mo de pic pour 466 Mo de fichier, soit 19). RAM inconnue : on tente.
pub fn model_fits(file_mb: u64, factor_tenths: u64, available_mb: Option<u64>) -> bool {
    available_mb.is_none_or(|a| file_mb * factor_tenths / 10 + HEADROOM_MB <= a)
}

pub fn required_mb(file_mb: u64, factor_tenths: u64) -> u64 {
    file_mb * factor_tenths / 10 + HEADROOM_MB
}

/// Espace disque libre (Mo) sur le volume qui contient `path` (ou son premier parent existant).
pub fn free_disk_mb(path: &Path) -> Option<u64> {
    let mut dir = path;
    while !dir.exists() { dir = dir.parent()?; }
    let c = std::ffi::CString::new(dir.to_str()?).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` est une chaîne C valide et `st` une structure statvfs allouée par nous.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 { return None; }
    Some(st.f_bavail as u64 * st.f_frsize as u64 / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mem_available_in_mb() {
        let m = "MemTotal:       32000000 kB\nMemFree:  100 kB\nMemAvailable:   24160000 kB\n";
        assert_eq!(parse_available_mb(m), Some(23593));
        assert_eq!(parse_available_mb("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn model_fits_uses_measured_factor_and_headroom() {
        // Whisper small f16 : 466 Mo de fichier, ×1,9 + 256 de marge ≈ 1141 Mo requis.
        assert!(model_fits(466, 19, Some(1200)));
        assert!(!model_fits(466, 19, Some(1000)));
        // large-v2 (≈3100 Mo) sur 4 Go libres : refusé.
        assert!(!model_fits(3100, 19, Some(4000)));
        assert!(model_fits(3100, 19, None), "RAM inconnue : on ne bloque pas");
        assert_eq!(required_mb(466, 19), 1141);
    }

    #[test]
    fn reads_real_system_values() {
        assert!(available_mb().is_some_and(|m| m > 0));
        assert!(free_disk_mb(Path::new("/tmp/does/not/exist/yet")).is_some_and(|m| m > 0));
    }
}
