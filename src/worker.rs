//! Hilo de trabajo: posee el motor aria2, el registry y la lógica de descargas.
//! Todo el texto ya formateado viaja al front-end GTK vía eventos.

use serde_json::json;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::engine::{Engine, EngineState};
use crate::i18n::{format_eta, format_size, format_speed, status_text, tr};
use crate::models::{clamp_connections, derive_name_from_url, Entry, Settings};
use crate::paths;
use crate::rpc::{get_f64, get_i64, mc_err, mc_get, Rpc};

// ------------------------------------------------------------------ mensajes

#[derive(Debug)]
pub enum Cmd {
    Add { urls: Vec<String>, dir: String, filename: Option<String>, connections: i32 },
    Pause { id: String },
    Resume { id: String },
    Delete { id: String, delete_file: bool },
    OpenFolder { id: String },
    SetSettings { max_concurrent: i32, default_connections: i32, default_dir: Option<String>, minimize_to_tray: bool },
    CleanupCompleted,
    /// Cierre de ventana: el worker decide (ocultar a segundo plano o apagar).
    HideRequested,
    /// La UI informa de su visibilidad (para notificar completadas solo si oculta).
    SetHidden(bool),
    Shutdown,
}

#[derive(Clone)]
pub struct ModelRow {
    pub id: String,
    pub name: String,
    pub urls: Vec<String>,
    pub dir: String,
    pub added_text: String,
    pub completed: bool,
}

#[derive(Clone)]
pub struct RowInfo {
    pub id: String,
    pub status_text: String,
    pub badge_hex: String,
    pub can_pause: bool,
    pub can_resume: bool,
    pub fraction: f64,
    pub fraction_known: bool,
    pub size_text: String,
    pub speed_text: String,
    pub threads_text: String,
    pub eta_text: String,
    pub action: String,
    pub error_text: String,
}

#[derive(Clone)]
pub struct EngineInfo {
    /// missing | starting | running | error
    pub state: &'static str,
    pub banner: String,
}

#[derive(Clone)]
pub enum Event {
    /// Cambios estructurales (alta/baja/completadas): (re)crear tarjetas.
    Model(Vec<ModelRow>),
    /// Tick de progreso; rows None = no tocar tarjetas (motor caído).
    Statuses { rows: Option<Vec<RowInfo>>, global_speed: String, heartbeat: String, engine: EngineInfo },
    Flash(String),
    /// Ocultar la ventana principal (modo segundo plano).
    HideWindow,
    Bye,
}

// ------------------------------------------------------------------ estado

pub struct State {
    pub entries: Vec<Entry>,
    pub settings: Settings,
    /// Ventana principal oculta (minimizada a segundo plano).
    pub hidden: bool,
    dirty: bool,
}

impl State {
    fn load() -> Self {
        let txt = std::fs::read_to_string(paths::registry()).unwrap_or_default();
        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct Disk {
            entries: Vec<Entry>,
            settings: Settings,
        }
        let d: Disk = serde_json::from_str(&txt).unwrap_or_default();
        State {
            entries: d.entries,
            settings: if txt.is_empty() { Settings::default() } else { d.settings },
            hidden: false,
            dirty: false,
        }
    }

    fn save(&mut self) {
        let txt = serde_json::json!({ "settings": self.settings, "entries": self.entries }).to_string();
        let _ = std::fs::write(paths::registry(), txt);
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }
}

// ------------------------------------------------------------------ bucle principal

pub fn run(cmds: Receiver<Cmd>, out: Sender<Event>) {
    paths::ensure_base();
    let mut st = State::load();
    let mut engine = Engine::new();
    let mut next_engine_try = Instant::now();
    let mut last_poll_err = String::new();

    // Carga: tras un cierre sin parada limpia nada vive en aria2 → "stopped".
    for e in st.entries.iter_mut() {
        if matches!(e.status.as_str(), "active" | "waiting") {
            e.status = "stopped".into();
        }
    }
    st.dirty = true;
    flush_model(&st, &out);

    loop {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let t = deadline.saturating_duration_since(Instant::now());
            if t.is_zero() {
                break;
            }
            match cmds.recv_timeout(t) {
                Ok(Cmd::Shutdown) => {
                    engine.stop_shutdown();
                    let _ = out.send(Event::Bye);
                    return;
                }
                Ok(Cmd::HideRequested) => {
                    if !st.settings.minimize_to_tray {
                        // ajuste desactivado ⇒ cerrar es apagar de verdad
                        engine.stop_shutdown();
                        let _ = out.send(Event::Bye);
                        return;
                    }
                    if !st.settings.tray_hint_shown {
                        st.settings.tray_hint_shown = true;
                        st.save();
                        desktop_notify(
                            &tr("Aria-XTRDownloader sigue activa", "Aria-XTRDownloader is still running"),
                            &tr(
                                "Seguirá descargando en segundo plano.\nVuelve a lanzar la aplicación para reabrirla.",
                                "It will keep downloading in the background.\nLaunch the app again to reopen it.",
                            ),
                        );
                    }
                    let _ = out.send(Event::HideWindow);
                }
                Ok(cmd) => handle_cmd(&cmd, &mut st, &mut engine, &out),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    engine.stop_shutdown();
                    return;
                }
            }
        }

        if !engine.running() && Instant::now() >= next_engine_try {
            if engine.ensure_started(&st.settings) {
                paths::ui_log_line(&format!("motor arrancado v{}", engine.version.clone().unwrap_or_default()));
            } else if engine.state == EngineState::Error {
                next_engine_try = Instant::now() + Duration::from_secs(30);
            }
        }

        if engine.running() {
            poll(&mut st, &engine, &out, &mut last_poll_err);
        } else {
            let _ = out.send(Event::Statuses {
                rows: None,
                global_speed: String::new(),
                heartbeat: String::new(),
                engine: engine_info(&engine),
            });
        }

        if st.dirty {
            st.save();
            st.dirty = false;
        }
    }
}

// ------------------------------------------------------------------ utilidades UI-side

fn flash(out: &Sender<Event>, msg: &str) {
    paths::ui_log_line(&format!("UI: {msg}"));
    let _ = out.send(Event::Flash(msg.to_string()));
}

fn flush_model(st: &State, out: &Sender<Event>) {
    let rows: Vec<ModelRow> = st
        .entries
        .iter()
        .map(|e| ModelRow {
            id: e.id.clone(),
            name: e.name.clone(),
            urls: e.urls.clone(),
            dir: e.dir.clone(),
            added_text: crate::i18n::added_text(e.added_at),
            completed: e.completed,
        })
        .collect();
    let _ = out.send(Event::Model(rows));
}

fn engine_info(engine: &Engine) -> EngineInfo {
    let (state, banner) = match engine.state {
        EngineState::Missing => ("missing", format!("⚠ {}", engine.last_error)),
        EngineState::Starting => ("starting", format!("⏳ {}", tr("Iniciando motor aria2…", "Starting aria2 engine…"))),
        EngineState::Running => (
            "running",
            format!(
                "✓ {} · RPC :{}{}",
                tr("Motor aria2 listo", "aria2 engine ready"),
                engine.port,
                engine.version.as_deref().map(|v| format!(" v{v}")).unwrap_or_default()
            ),
        ),
        EngineState::Error => ("error", format!("⚠ {} · {}", tr("Error del motor aria2", "aria2 engine error"), engine.last_error)),
    };
    EngineInfo { state, banner }
}

fn build_row(e: &Entry, done: i64, total: i64, speed: f64, conns: i32, retry_hint: bool, slots_free: bool) -> RowInfo {
    let status = e.status.as_str();
    let status_text = if status == "waiting" && retry_hint {
        tr("Reintentando…", "Retrying…").to_string()
    } else {
        status_text(status)
    };
    let (badge_hex, can_pause, can_resume) = match status {
        "active" => ("#22C55E", true, false),
        "waiting" => (if retry_hint { "#F59E0B" } else { "#A78BFA" }, true, false),
        "paused" => ("#F59E0B", false, true),
        "error" => ("#EF4444", false, true),
        _ => ("#6B728A", false, true),
    };
    let fraction_known = total > 0;
    let fraction = if fraction_known { (done as f64 / total as f64).clamp(0.0, 1.0) } else { 0.0 };
    let eta_text = if fraction_known {
        if speed > 1.0 && done < total {
            format_eta((total - done) as f64 / speed)
        } else {
            "—".into()
        }
    } else {
        String::new()
    };
    let size_text = if fraction_known {
        format!("{} / {}", format_size(done as f64), format_size(total as f64))
    } else {
        format_size(done as f64)
    };
    let action = match status {
        "active" => {
            if total == 0 {
                tr("Descargando (tamaño aún desconocido)…", "Downloading (size still unknown)…")
            } else if speed > 0.0 {
                tr("Recibiendo datos", "Receiving data")
            } else {
                tr("Conexiones abiertas, esperando datos…", "Holding connections, waiting for data…")
            }
        }
        "waiting" => {
            if retry_hint {
                if total == 0 && done == 0 {
                    tr("Reintento: resolviendo host/URL…", "Retry: resolving host/URL…")
                } else {
                    tr("Reintentando…", "Retrying…")
                }
            } else if slots_free {
                tr("Preparando (DNS / redirecciones / reservando espacio)…", "Preparing (DNS / redirects / space allocation)…")
            } else {
                tr("En cola, esperando hueco libre", "Queued, waiting for a free slot")
            }
        }
        "paused" => tr("Pausada por el usuario", "Paused by user"),
        "stopped" => tr("Detenida — pulsa ▶ para reanudar", "Stopped — press ▶ to resume"),
        "error" => tr("Detenida por error — pulsa ▶ para reintentar", "Stopped by error — press ▶ to retry"),
        _ => "",
    };
    RowInfo {
        id: e.id.clone(),
        status_text: status_text.to_string(),
        badge_hex: badge_hex.to_string(),
        can_pause,
        can_resume,
        fraction,
        fraction_known,
        size_text,
        speed_text: format_speed(speed),
        threads_text: format!("{}/{} {}", conns, e.connections, tr("hilos", "threads")),
        eta_text,
        action: action.to_string(),
        error_text: if status == "error" { e.error.clone().unwrap_or_default() } else { String::new() },
    }
}

// ------------------------------------------------------------------ sondeo

const STATUS_FIELDS: &[&str] = &[
    "status",
    "totalLength",
    "completedLength",
    "downloadSpeed",
    "connections",
    "errorMessage",
];

fn poll(st: &mut State, engine: &Engine, out: &Sender<Event>, last_poll_err: &mut String) {
    let Some(rpc) = engine.rpc() else { return };
    let targets: Vec<(String, String)> = st
        .entries
        .iter()
        .filter_map(|e| e.gid.clone().map(|g| (e.id.clone(), g)))
        .collect();

    let mut calls: Vec<(&str, Vec<serde_json::Value>)> = targets
        .iter()
        .map(|(_, g)| ("aria2.tellStatus", vec![json!(g), json!(STATUS_FIELDS)]))
        .collect();
    // tellActive SIEMPRE al final; tellWaiting está roto con rpc-secret: jamás usarlo.
    calls.push(("aria2.tellActive", vec![]));

    let res = match rpc.multicall(&calls) {
        Ok(r) => r,
        Err(ex) => {
            let m = format!("POLL ERROR: {ex}");
            if *last_poll_err != m {
                *last_poll_err = m.clone();
                paths::ui_log_line(&m);
            }
            return;
        }
    };
    let elems: Vec<serde_json::Value> = res.as_array().cloned().unwrap_or_default();
    let daemon_active = elems.last().and_then(mc_get).and_then(|v| v.as_array().map(|a| a.len())).unwrap_or(0);
    let slots_free = (daemon_active as i32) < st.settings.max_concurrent_downloads.max(1);

    let mut rows = Vec::new();
    let mut total_speed = 0f64;
    let mut has_active = false;
    let mut waiting = 0usize;
    let mut structural = false;

    for ((id, gid), el) in targets.iter().zip(elems.iter()) {
        let Some(idx) = st.find(id) else { continue };
        match mc_get(el) {
            Some(stv) => {
                let status = stv.get("status").and_then(|s| s.as_str()).unwrap_or("stopped").to_string();
                let total = get_i64(stv, "totalLength");
                let done = get_i64(stv, "completedLength");
                let speed = get_f64(stv, "downloadSpeed");
                let conns = get_i64(stv, "connections") as i32;
                let err = stv
                    .get("errorMessage")
                    .and_then(|e| e.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);

                let hidden_now = st.hidden;
                let e = &mut st.entries[idx];
                if status == "complete" {
                    if hidden_now {
                        desktop_notify(
                            &tr("Descarga completada", "Download completed"),
                            &format!("{} · {}", e.name, format_size(total as f64)),
                        );
                    }
                    e.completed = true;
                    e.status = "complete".into();
                    e.total = total;
                    e.error = None;
                    st.dirty = true;
                    structural = true;
                    let _ = rpc.call("aria2.forceRemove", &[json!(gid)]);
                    continue;
                }
                e.status = status.clone();
                e.error = err;
                e.total = total;
                if status == "active" {
                    total_speed += speed;
                    has_active = true;
                }
                if status == "waiting" {
                    waiting += 1;
                }
                let retry_hint = status == "waiting" && slots_free;
                rows.push(build_row(e, done, total, speed, conns, retry_hint, slots_free));
            }
            None => {
                // sub-llamada con error: gid desconocido (demonio muerto) u otro fallo
                let msg = mc_err(el).unwrap_or("").to_string();
                let e = &mut st.entries[idx];
                if msg.to_lowercase().contains("is not found") {
                    let was_downloading = matches!(e.status.as_str(), "active" | "waiting");
                    e.gid = None;
                    st.dirty = true;
                    if was_downloading {
                        // auto-sanear: re-añadir (aria2 continúa con su .aria2)
                        paths::ui_log_line(&format!("Gid perdido ({}): re-añadiendo automáticamente", e.name));
                        match readd(e, rpc) {
                            Ok(()) => flash(out, &format!("↻ {} {}", tr("Recuperada tras reinicio del motor", "Recovered after engine restart"), e.name)),
                            Err(ex2) => {
                                e.status = "error".into();
                                e.error = Some(ex2.to_string());
                                st.dirty = true;
                            }
                        }
                    } else {
                        e.status = "stopped".into();
                    }
                } else {
                    paths::ui_log_line(&format!("tellStatus gid={gid} error: {msg}"));
                }
                rows.push(build_row(e, 0, 0, 0.0, 0, false, slots_free));
            }
        }
    }

    if structural {
        flush_model(st, out);
    }
    let global_speed = if has_active {
        format!("{}: {}", tr("Velocidad total", "Total speed"), format_speed(total_speed))
    } else {
        String::new()
    };
    let heartbeat = format!("{} · aria2: {} act / {} cola", crate::i18n::clock(), daemon_active, waiting);
    let _ = out.send(Event::Statuses { rows: Some(rows), global_speed, heartbeat, engine: engine_info(engine) });
}

// ------------------------------------------------------------------ acciones sobre aria2

fn readd(e: &mut Entry, rpc: &Rpc) -> Result<(), crate::rpc::AriaError> {
    let conn = clamp_connections(e.connections);
    let mut opts = serde_json::Map::new();
    opts.insert("dir".into(), json!(e.dir));
    if let Some(o) = &e.out {
        opts.insert("out".into(), json!(o));
    }
    opts.insert("continue".into(), json!("true"));
    opts.insert("maxConnectionPerServer".into(), json!(conn.to_string()));
    opts.insert("split".into(), json!(conn.to_string()));
    opts.insert("autoFileRenaming".into(), json!("false"));
    opts.insert("allowOverwrite".into(), json!("true"));

    let gid = rpc.call("aria2.addUri", &[json!(e.urls), serde_json::Value::Object(opts)])?;
    paths::ui_log_line(&format!("Re-add OK ({}) gid={} (continue=true)", e.name, gid.as_str().unwrap_or("?")));
    e.gid = gid.as_str().map(String::from);
    e.status = "waiting".into();
    e.error = None;
    Ok(())
}

fn force_pause(rpc: &Rpc, gid: &str) -> Result<(), crate::rpc::AriaError> {
    match rpc.call("aria2.forcePause", &[json!(gid)]) {
        Err(ex) if ex.message.to_lowercase().contains("paused now") => {
            // fase transitoria (arranque/handshake): reintenta una vez
            std::thread::sleep(Duration::from_millis(1200));
            rpc.call("aria2.forcePause", &[json!(gid)]).map(|_| ())
        }
        other => other.map(|_| ()),
    }
}

// ------------------------------------------------------------------ comandos de la UI

fn handle_cmd(cmd: &Cmd, st: &mut State, engine: &mut Engine, out: &Sender<Event>) {
    match cmd {
        Cmd::Add { urls, dir, filename, connections } => {
            let u0 = urls.first().cloned().unwrap_or_default();
            if let Some(dup) = st
                .entries
                .iter()
                .find(|e| !e.completed && e.urls.first().is_some_and(|u| u.eq_ignore_ascii_case(&u0)))
            {
                flash(out, &format!("ℹ {} ({})", tr("Esa URL ya está en la lista", "That URL is already in the list"), dup.name));
                return;
            }
            if !engine.running() {
                engine.ensure_started(&st.settings);
            }
            let Some(rpc) = engine.rpc() else {
                flash(out, &format!("⚠ {}", engine.last_error));
                return;
            };
            let conn = clamp_connections(*connections);
            let out_name = filename
                .as_deref()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| derive_name_from_url(&u0));

            paths::ui_log_line(&format!("addUri → {u0} dir={dir} out={out_name} x{conn}"));
            let opts = json!({
                "dir": dir,
                "out": out_name,
                "continue": "true",
                "maxConnectionPerServer": conn.to_string(),
                "split": conn.to_string(),
                "autoFileRenaming": "false",
                "allowOverwrite": "true",
            });
            match rpc.call("aria2.addUri", &[json!(urls), opts]) {
                Ok(g) => {
                    paths::ui_log_line(&format!("addUri OK gid={}", g.as_str().unwrap_or("?")));
                    let mut entry = Entry::new(urls.clone(), dir.clone(), Some(out_name.clone()), out_name.clone(), conn);
                    entry.gid = g.as_str().map(String::from);
                    entry.status = "waiting".into();
                    st.entries.push(entry);
                    st.dirty = true;
                    flush_model(st, out);
                    flash(out, &format!("✔ {} {out_name}", tr("Añadida", "Added")));
                }
                Err(ex) => {
                    paths::ui_log_line(&format!("addUri ERROR: {ex}"));
                    flash(out, &format!("⚠ {} {ex}", tr("No se pudo añadir", "Could not add")));
                }
            }
        }
        Cmd::Pause { id } => {
            let Some(rpc) = engine.rpc() else { return };
            let Some(idx) = st.find(id) else { return };
            let gid = match st.entries[idx].gid.clone() {
                Some(g) => g,
                None => return,
            };
            match force_pause(rpc, &gid) {
                Ok(()) => {
                    st.entries[idx].status = "paused".into();
                    st.dirty = true;
                    flush_model(st, out);
                }
                Err(ex) if ex.is_unknown_gid() => {
                    st.entries[idx].gid = None;
                    st.entries[idx].status = "stopped".into();
                    st.dirty = true;
                    flush_model(st, out);
                }
                Err(ex) => flash(out, &format!("⚠ {} {ex}", tr("No se pudo pausar", "Could not pause"))),
            }
        }
        Cmd::Resume { id } => {
            if !engine.running() {
                engine.ensure_started(&st.settings);
            }
            let Some(rpc) = engine.rpc() else {
                flash(out, &format!("⚠ {}", engine.last_error));
                return;
            };
            let Some(idx) = st.find(id) else { return };
            if let Some(gid) = st.entries[idx].gid.clone() {
                match rpc.call("aria2.unpause", &[json!(gid)]) {
                    Ok(_) => {
                        st.entries[idx].status = "waiting".into();
                        st.dirty = true;
                        flush_model(st, out);
                        return;
                    }
                    Err(ex) if ex.is_unknown_gid() => { /* el demonio se reinició: re-añadir */ }
                    Err(ex) => {
                        flash(out, &format!("⚠ {} {ex}", tr("No se pudo reanudar", "Could not resume")));
                        return;
                    }
                }
            }
            match readd(&mut st.entries[idx], rpc) {
                Ok(()) => {
                    st.dirty = true;
                    flush_model(st, out);
                }
                Err(ex) => {
                    st.entries[idx].status = "error".into();
                    st.entries[idx].error = Some(ex.to_string());
                    st.dirty = true;
                    flash(out, &format!("⚠ {ex}"));
                }
            }
        }
        Cmd::Delete { id, delete_file } => {
            let Some(idx) = st.find(id) else { return };
            let e = st.entries.remove(idx);
            if let Some(gid) = &e.gid {
                if let Some(rpc) = engine.rpc() {
                    let _ = rpc.call("aria2.forceRemove", &[json!(gid)]);
                }
            }
            if *delete_file {
                if let Some(out_name) = &e.out {
                    let p = std::path::Path::new(&e.dir).join(out_name);
                    let _ = std::fs::remove_file(&p);
                    let mut ctrl = p.as_os_str().to_os_string();
                    ctrl.push(".aria2");
                    let _ = std::fs::remove_file(std::path::PathBuf::from(ctrl));
                }
            }
            paths::ui_log_line(&format!("Delete ({}) also_file={}", e.name, delete_file));
            st.dirty = true;
            flush_model(st, out);
        }
        Cmd::OpenFolder { id } => {
            let Some(idx) = st.find(id) else { return };
            let dir = st.entries[idx].dir.clone();
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
        Cmd::SetSettings { max_concurrent, default_connections, default_dir, minimize_to_tray } => {
            st.settings = Settings {
                max_concurrent_downloads: (*max_concurrent).clamp(1, 20),
                default_connections: clamp_connections(*default_connections),
                default_dir: default_dir.clone(),
                minimize_to_tray: *minimize_to_tray,
                tray_hint_shown: st.settings.tray_hint_shown,
            };
            if let Some(rpc) = engine.rpc() {
                let _ = rpc.call(
                    "aria2.changeGlobalOption",
                    &[json!({ "maxConcurrentDownloads": st.settings.max_concurrent_downloads.to_string() })],
                );
            }
            st.dirty = true;
            flash(out, &format!("✔ {}", tr("Ajustes guardados", "Settings saved")));
        }
        Cmd::CleanupCompleted => {
            st.entries.retain(|e| !e.completed);
            st.dirty = true;
            flush_model(st, out);
        }
        Cmd::SetHidden(h) => st.hidden = *h,
        Cmd::Shutdown | Cmd::HideRequested => unreachable!(),
    }
}

/// Notificación del escritorio (freedesktop). Silenciosa si notify-send no existe.
fn desktop_notify(title: &str, body: &str) {
    let r = std::process::Command::new("notify-send")
        .args(["--app-name", "Aria-XTRDownloader", title, body])
        .spawn();
    if let Err(e) = r {
        paths::ui_log_line(&format!("notify-send no disponible: {e}"));
    }
}
