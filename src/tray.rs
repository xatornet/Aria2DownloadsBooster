//! Bandeja de notificaciones vía SNI (StatusNotifierItem + com.canonical.dbusmenu).
//!
//! Usa `ksni` (implementación de referencia del protocolo en Rust, probada en Plasma/KDE
//! durante años) en lugar de serializar D-Bus a mano: la iteración anterior con zbus directo
//! tumbaba plasmashell al leer el layout dbusmenu artesanal.
//!
//! Se puede desactivar por completo con la variable de entorno `A2DB_NO_TRAY=1`
//! (diagnóstico: si algo falla, comprobar que sin bandeja la app es estable).

use std::sync::mpsc::Sender;

use ksni::blocking::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Category, Status, ToolTip};

use crate::i18n::tr;
use crate::worker::TrayEvent;

pub(crate) struct XtrTray {
    events: Sender<TrayEvent>,
}

impl ksni::Tray for XtrTray {
    fn id(&self) -> String {
        "aria2-downloads-booster".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "Aria2 Downloads Booster".into()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        "aria2-downloads-booster".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "aria2-downloads-booster".into(),
            title: "Aria2 Downloads Booster".into(),
            description: tr(
                "Descargando en segundo plano. Clic para abrir.",
                "Downloading in the background. Click to open.",
            )
            .to_string(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        crate::paths::ui_log_line("tray: Activate → mostrar ventana");
        let _ = self.events.send(TrayEvent::ShowWindow);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        crate::paths::ui_log_line("tray: SecondaryActivate → mostrar ventana");
        let _ = self.events.send(TrayEvent::ShowWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx_open = self.events.clone();
        let tx_quit = self.events.clone();
        vec![
            StandardItem {
                label: tr("Abrir Aria2 Downloads Booster", "Open Aria2 Downloads Booster").to_string(),
                activate: Box::new(move |_this| {
                    crate::paths::ui_log_line("tray: menú Abrir → mostrar ventana");
                    let _ = tx_open.send(TrayEvent::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: tr("Salir", "Quit").to_string(),
                icon_name: "application-exit".into(),
                activate: Box::new(move |_this| {
                    crate::paths::ui_log_line("tray: menú Salir");
                    let _ = tx_quit.send(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// asa del servicio de bandeja: Live = icono activo; Dead = sin bandeja (p. ej. no hay
/// watcher SNI) — se reintenta si el usuario activa/desactiva el ajuste.
pub enum TrayHandle {
    Live(ksni::blocking::Handle<XtrTray>),
    Dead,
}

impl TrayHandle {
    pub fn spawn(events: Sender<TrayEvent>) -> Self {
        match (XtrTray { events }).spawn() {
            Ok(h) => {
                crate::paths::ui_log_line("tray: icono registrado en el host (ksni)");
                TrayHandle::Live(h)
            }
            Err(e) => {
                crate::paths::ui_log_line(&format!("tray: sin bandeja ({e})"));
                TrayHandle::Dead
            }
        }
    }

    /// Apaga el servicio SNI: el host retira el icono (libera el nombre de bus al cerrar).
    pub fn shutdown(&self) {
        if let TrayHandle::Live(h) = self {
            h.shutdown().wait();
        }
    }
}
