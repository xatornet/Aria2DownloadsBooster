//! Modelo de datos idéntico en espíritu al de la app Windows (registry.json).

use serde::{Deserialize, Serialize};

pub fn clamp_connections(n: i32) -> i32 {
    n.clamp(1, 16)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub max_concurrent_downloads: i32,
    pub default_connections: i32,
    pub default_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_concurrent_downloads: 3,
            default_connections: 16,
            default_dir: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: String,
    pub gid: Option<String>,
    pub urls: Vec<String>,
    pub dir: String,
    pub out: Option<String>,
    pub name: String,
    /// active | waiting | paused | stopped | error | complete
    pub status: String,
    pub total: i64,
    pub completed: bool,
    pub error: Option<String>,
    pub connections: i32,
    /// epoch-segundos de alta
    pub added_at: i64,
}

impl Entry {
    pub fn new(urls: Vec<String>, dir: String, out: Option<String>, name: String, connections: i32) -> Self {
        Entry {
            id: crate::paths::random_hex(8),
            gid: None,
            urls,
            dir,
            out,
            name,
            status: "stopped".into(),
            total: 0,
            completed: false,
            error: None,
            connections,
            added_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }
}

/// Deriva el nombre de archivo desde la URL principal (igual que la app Windows).
pub fn derive_name_from_url(url: &str) -> String {
    let mut seg = url.split(['?', '#']).next().unwrap_or(url);
    seg = seg.trim_end_matches('/');
    seg = seg.rsplit('/').next().unwrap_or(seg);
    let decoded = percent_decode(seg);
    if decoded.is_empty() {
        "descarga".to_string()
    } else {
        decoded
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(n) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
