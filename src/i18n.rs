//! Localización mínima: español por defecto, inglés si el sistema está en inglés.

use gtk::glib;
use std::sync::OnceLock;

static IS_EN: OnceLock<bool> = OnceLock::new();

pub fn is_english() -> bool {
    *IS_EN.get_or_init(detect)
}

fn detect() -> bool {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(l) = std::env::var(var) {
            let l = l.to_lowercase();
            if !l.is_empty() && l != "c" && l != "posix" && !l.starts_with("c.") {
                return l.starts_with("en");
            }
        }
    }
    false
}

/// Traducción inline: tr("es", "en")
pub fn tr<'a>(es: &'a str, en: &'a str) -> &'a str {
    if is_english() { en } else { es }
}

pub fn status_text(key: &str) -> String {
    match key {
        "active" => tr("Descargando", "Downloading").to_string(),
        "waiting" => tr("En cola", "Waiting").to_string(),
        "paused" => tr("Pausada", "Paused").to_string(),
        "stopped" => tr("Detenida", "Stopped").to_string(),
        "error" => tr("Error", "Error").to_string(),
        "complete" => tr("Completada", "Completed").to_string(),
        other => other.to_string(),
    }
}

pub fn format_size(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut b = bytes;
    let mut i = 0;
    while b >= 1024.0 && i < UNITS.len() - 1 {
        b /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", b, UNITS[i])
}

pub fn format_speed(bps: f64) -> String {
    format!("{}/s", format_size(bps))
}

pub fn format_eta(secs: f64) -> String {
    if secs >= 3600.0 {
        format!("{}h {:02}m", (secs / 3600.0) as i64, ((secs % 3600.0) / 60.0) as i64)
    } else if secs >= 60.0 {
        format!("{}m {:02}s", (secs / 60.0) as i64, (secs as i64) % 60)
    } else {
        format!("{}s", secs as i64)
    }
}

/// Cadena "hh:mm:ss" local (para el latido del sondeo).
pub fn clock() -> String {
    glib::DateTime::now_local()
        .ok()
        .and_then(|d| d.format("%H:%M:%S").ok().map(String::from))
        .unwrap_or_default()
}

/// Fecha local "dd/mm/aaaa hh:mm" desde epoch-segundos.
pub fn added_text(epoch: i64) -> String {
    glib::DateTime::from_unix_local(epoch)
        .ok()
        .and_then(|d| {
            d.format("%d/%m/%Y %H:%M")
                .ok()
                .map(|s| String::from_utf8_lossy(s.as_bytes()).into_owned())
        })
        .unwrap_or_default()
}
