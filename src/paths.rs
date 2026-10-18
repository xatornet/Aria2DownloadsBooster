//! Rutas de datos (XDG), con redirección opcional vía ARIA_XTR_DATA (paridad con la app Windows).

use std::path::PathBuf;

pub fn base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ARIA_XTR_DATA") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let data = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".local/share")
        });
    data.join("Aria-XTRDownloader")
}

pub fn ensure_base() {
    let _ = std::fs::create_dir_all(base_dir());
}

pub fn registry() -> PathBuf {
    base_dir().join("registry.json")
}
pub fn engine_log() -> PathBuf {
    base_dir().join("aria2c.log")
}
pub fn ui_log() -> PathBuf {
    base_dir().join("ui.log")
}
pub fn session() -> PathBuf {
    base_dir().join("aria2.session")
}
pub fn pid_file() -> PathBuf {
    base_dir().join("aria2c.pid")
}

/// Registro en ui.log (equivalente al UiLog.cs de la app Windows).
pub fn ui_log_line(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ui_log()) {
        use std::io::Write;
        let _ = writeln!(f, "{} {}", crate::i18n::clock(), msg);
    }
}

/// n bytes aleatorios en hex (de /dev/urandom).
pub fn random_hex(nbytes: usize) -> String {
    use std::io::Read;
    let mut buf = vec![0u8; nbytes];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        for b in buf.iter_mut() {
            *b = rand_fallback();
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn rand_fallback() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0x5bd1_e995);
    (ns ^ (ns >> 13)) as u8
}
