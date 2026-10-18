//! Ciclo de vida del demonio aria2c (paridad con EngineService.cs de la app Windows).
//! En Arch se usa el aria2c del sistema (`sudo pacman -S aria2`); la variable
//! A2DB_ENGINE permite señalar un binario alternativo.

use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::i18n::tr;
use crate::models::Settings;
use crate::paths;
use crate::rpc::Rpc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Missing,
    Starting,
    Running,
    Error,
}

pub struct Engine {
    pub state: EngineState,
    pub version: Option<String>,
    pub last_error: String,
    pub port: u16,
    child: Option<Child>,
    rpc: Option<Rpc>,
    secret: String,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            state: EngineState::Missing,
            version: None,
            last_error: String::new(),
            port: 0,
            child: None,
            rpc: None,
            secret: format!("a2db-{}", paths::random_hex(16)),
        }
    }

    pub fn running(&self) -> bool {
        self.state == EngineState::Running && self.rpc.is_some()
    }

    pub fn rpc(&self) -> Option<&Rpc> {
        self.rpc.as_ref()
    }

    /// Búsqueda del binario: A2DB_ENGINE → PATH → /usr/bin/aria2c.
    pub fn binary() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("A2DB_ENGINE") {
            let pb = PathBuf::from(&p);
            if is_executable(&pb) {
                return Some(pb);
            }
        }
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(':') {
                let cand = PathBuf::from(dir).join("aria2c");
                if is_executable(&cand) {
                    return Some(cand);
                }
            }
        }
        let sys = PathBuf::from("/usr/bin/aria2c");
        if is_executable(&sys) {
            return Some(sys);
        }
        None
    }

    /// Arranca el demonio si no corre. Devuelve true si está operativo.
    pub fn ensure_started(&mut self, settings: &Settings) -> bool {
        if self.running() {
            return true;
        }
        let Some(bin) = Self::binary() else {
            self.state = EngineState::Missing;
            self.last_error =
                "aria2c no encontrado — instálalo con: sudo pacman -S aria2".to_string();
            return false;
        };

        self.state = EngineState::Starting;
        self.last_error.clear();
        kill_orphan();

        for port in 6800u16..=6804 {
            let mut cmd = Command::new(&bin);
            cmd.args(build_args(settings, port, &self.secret))
                .stdout(Stdio::null())
                .stderr(Stdio::piped());

            // Garantía de vida: aria2c recibe SIGTERM si este proceso muere
            // (kill -9, crash, logout). Equivalente al Job Object de Windows.
            unsafe {
                cmd.pre_exec(|| {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }

            let Ok(mut child) = cmd.spawn() else { continue };

            // Captura stderr para diagnóstico (aria2c escribe ahí sus errores de arranque)
            if let Some(err) = child.stderr.take() {
                std::thread::spawn(move || {
                    let reader = std::io::BufReader::new(err);
                    for line in reader.lines().map_while(Result::ok) {
                        if !line.trim().is_empty() {
                            paths::ui_log_line(&format!("aria2c: {line}"));
                        }
                    }
                });
            }

            let rpc = Rpc::new(port, &self.secret);
            let deadline = Instant::now() + Duration::from_millis(2500);
            let mut version = None;
            while Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
                if let Some(v) = rpc.try_get_version() {
                    version = Some(v);
                    break;
                }
                if child.try_wait().map(|s| s.is_some()).unwrap_or(true) {
                    break;
                }
            }

            match version {
                Some(v) => {
                    let _ = std::fs::write(paths::pid_file(), child.id().to_string());
                    self.rpc = Some(rpc);
                    self.port = port;
                    self.version = Some(v);
                    self.state = EngineState::Running;
                    self.child = Some(child);
                    return true;
                }
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }

        self.state = EngineState::Error;
        self.last_error = format!(
            "{} ~/.local/share/Aria2DownloadsBooster/aria2c.log",
            tr("No se pudo arrancar aria2c (puertos 6800-6804). Revisa",
               "Could not start aria2c (ports 6800-6804). Check")
        );
        self.rpc = None;
        false
    }

    /// Parada ordenada: aria2.shutdown (guarda sesión); si aguantar, matar.
    pub fn stop_shutdown(&mut self) {
        if let Some(rpc) = &self.rpc {
            let _ = rpc.call("aria2.shutdown", &[]);
        }
        if let Some(child) = self.child.as_mut() {
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(150))
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        self.rpc = None;
        self.child = None;
        self.state = EngineState::Missing;
        let _ = std::fs::remove_file(paths::pid_file());
    }
}

fn is_executable(p: &std::path::Path) -> bool {
    if !p.is_file() {
        return false;
    }
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Matar el demonio huérfano de una sesión anterior (cierre brusco).
fn kill_orphan() {
    let Ok(txt) = std::fs::read_to_string(paths::pid_file()) else { return };
    let Ok(pid): Result<i32, _> = txt.trim().parse() else { return };
    // solo matar si el proceso es realmente aria2c
    if let Ok(comm) = std::fs::read(format!("/proc/{pid}/comm")) {
        if String::from_utf8_lossy(&comm).starts_with("aria2c") {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let _ = std::fs::remove_file(paths::pid_file());
}

/// Argumentos del demonio. La sesión de aria2 es respaldo; el registry de la app
/// es la fuente de verdad para reanudar (por eso no se usa --input-file).
fn build_args(s: &Settings, port: u16, secret: &str) -> Vec<String> {
    vec![
        "--enable-rpc".into(),
        "--rpc-listen-all=false".into(),
        format!("--rpc-listen-port={port}"),
        format!("--rpc-secret={secret}"),
        format!(
            "--max-concurrent-downloads={}",
            s.max_concurrent_downloads.clamp(1, 20)
        ),
        // Máxima paralelización: hasta 16 conexiones por servidor
        "--max-connection-per-server=16".into(),
        "--split=16".into(),
        "--min-split-size=1M".into(),
        "--continue=true".into(),
        "--auto-file-renaming=false".into(),
        "--allow-overwrite=true".into(),
        // Linux: prealloc funciona sin privilegios (fallocate); en Windows haría
        // falta "none" — ver el bug del zero-fill en la rama windows.
        "--file-allocation=prealloc".into(),
        "--disk-cache=32M".into(),
        "--timeout=60".into(),
        "--connect-timeout=30".into(),
        "--max-tries=5".into(),
        "--retry-wait=3".into(),
        "--console-log-level=warn".into(),
        "--log-level=warn".into(),
        format!("--log={}", paths::engine_log().display()),
        format!("--save-session={}", paths::session().display()),
        "--follow-torrent=true".into(),
    ]
}
