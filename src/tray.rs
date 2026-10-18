//! Bandeja de notificaciones vía SNI (org.kde.StatusNotifierItem) + menú com.canonical.dbusmenu.
//! Implementación propia sobre D-Bus con zbus en modo blocking (hilo dedicado; zbus levanta su
//! propio runtime interno). Necesaria porque GTK4 eliminó GtkStatusIcon; SNI es el estándar que
//! consumen KDE/XFCE/Cinnamon/LXQt/Hyprland+waybar y GNOME (extensión "AppIndicator/KStatusNotifier").

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, Type, Value};

use crate::i18n::tr;
use crate::worker::TrayEvent;

const ITEM_OPEN: i32 = 1;
const ITEM_QUIT: i32 = 2;

fn label_open() -> String {
    tr("Abrir Aria-XTRDownloader", "Open Aria-XTRDownloader").to_string()
}
fn label_quit() -> String {
    tr("Salir", "Quit").to_string()
}

// ---------------------------------------------------------------- manejo del hilo

pub struct TrayHandle {
    quit: Arc<AtomicBool>,
    joiner: Mutex<Option<JoinHandle<()>>>,
}

impl TrayHandle {
    pub fn spawn(events: Sender<TrayEvent>) -> Self {
        let quit = Arc::new(AtomicBool::new(false));
        let q2 = quit.clone();
        let joiner = std::thread::spawn(move || {
            if let Err(e) = run(&q2, events) {
                crate::paths::ui_log_line(&format!("tray: sin bandeja ({e})"));
            }
        });
        TrayHandle { quit, joiner: Mutex::new(Some(joiner)) }
    }

    /// Libera la conexión D-Bus (el host retira el icono) y une el hilo.
    pub fn shutdown(&self) {
        self.quit.store(true, Ordering::SeqCst);
        if let Some(j) = self.joiner.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}

fn run(quit: &AtomicBool, events: Sender<TrayEvent>) -> zbus::Result<()> {
    let conn = Connection::session()?;

    let service = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    conn.request_name(service.as_str())?;

    conn.object_server().at("/StatusNotifierItem", Sni::new(events.clone()))?;
    conn.object_server().at("/StatusNotifierItem/Menu", Menu::new(events.clone()))?;

    // Registro ante el watcher (el host del escritorio). Reintenta si aún no está listo,
    // p. ej. porque la sesión gráfica arranca después que la app.
    let mut warned = false;
    while !quit.load(Ordering::SeqCst) {
        let res = conn.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &service.as_str(),
        );
        match res {
            Ok(_) => {
                crate::paths::ui_log_line("tray: icono registrado en el host");
                break;
            }
            Err(e) => {
                if !warned {
                    crate::paths::ui_log_line(&format!("tray: watcher no disponible aún ({e})"));
                    warned = true;
                }
                let until = Instant::now() + Duration::from_secs(2);
                while Instant::now() < until && !quit.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    // Mantener viva la conexión hasta que el worker apague la bandeja.
    while !quit.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(300));
    }
    Ok(())
}

// --------------------------------------------------------------------- SNI

#[derive(Debug)]
struct Sni {
    events: Sender<TrayEvent>,
}

impl Sni {
    fn new(events: Sender<TrayEvent>) -> Self {
        Sni { events }
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Sni {
    #[zbus(property)]
    fn category(&self) -> String {
        "ApplicationStatus".into()
    }

    #[zbus(property)]
    fn id(&self) -> String {
        "aria-xtrdownloader".into()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        "Aria-XTRDownloader".into()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        "aria-xtrdownloader".into()
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        vec![]
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        vec![]
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        vec![]
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> String {
        String::new()
    }

    /// Firma (sa(iiay)ss) de la spec: (icono, pixmaps, título, descripción).
    #[zbus(property)]
    fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
        (
            "aria-xtrdownloader".into(),
            vec![],
            "Aria-XTRDownloader".into(),
            tr(
                "Descargando en segundo plano. Clic para abrir.",
                "Downloading in the background. Click to open.",
            )
            .to_string(),
        )
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> ObjectPath<'static> {
        ObjectPath::try_from("/StatusNotifierItem/Menu").unwrap()
    }

    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.events.send(TrayEvent::ShowWindow);
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.events.send(TrayEvent::ShowWindow);
    }

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}
}

// ---------------------------------------------------------------- dbusmenu

type MenuProps = HashMap<String, Value<'static>>;

/// Nodo del menú: firma exacta (ia{sv}av) de com.canonical.dbusmenu GetLayout.
/// (hojas: children vacío ⇒ av sin elementos).
#[derive(Debug, Serialize, Type)]
struct MenuNode {
    id: i32,
    properties: MenuProps,
    children: Vec<Value<'static>>,
}

impl From<MenuNode> for Value<'static> {
    /// Hoja como variante (los elementos de `av` deben ser variantes).
    fn from(n: MenuNode) -> Self {
        let mut b = zbus::zvariant::StructureBuilder::default();
        b = b.add_field(Value::from(n.id));
        b = b.add_field(Value::from(n.properties));
        b = b.add_field(Value::from(n.children));
        Value::Structure(b.build().expect("menu leaf (structura estática, no puede fallar)"))
    }
}

fn item_props(id: i32) -> MenuProps {
    let label = match id {
        ITEM_OPEN => label_open(),
        _ => label_quit(),
    };
    let mut m = MenuProps::new();
    m.insert("type".into(), Value::from("standard"));
    m.insert("visible".into(), Value::from(true));
    m.insert("enabled".into(), Value::from(true));
    m.insert("label".into(), Value::from(label));
    m
}

/// Hoja envuelta en variante: los elementos de `av` deben ser variantes.
fn leaf(id: i32) -> Value<'static> {
    Value::from(MenuNode { id, properties: item_props(id), children: vec![] })
}

#[derive(Debug)]
struct Menu {
    events: Sender<TrayEvent>,
}

impl Menu {
    fn new(events: Sender<TrayEvent>) -> Self {
        Menu { events }
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".into()
    }

    fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, MenuNode) {
        let mut root_props = MenuProps::new();
        root_props.insert("children-display".into(), Value::from("submenu"));
        (
            1,
            MenuNode {
                id: 0,
                properties: root_props,
                children: vec![leaf(ITEM_OPEN), leaf(ITEM_QUIT)],
            },
        )
    }

    fn about_to_show(&self, _id: i32) -> bool {
        true
    }

    fn get_group_properties(&self, ids: Vec<i32>) -> Vec<(i32, MenuProps)> {
        ids.into_iter().map(|id| (id, item_props(id))).collect()
    }

    fn get_properties(&self, id: i32) -> MenuProps {
        item_props(id)
    }

    fn event(&self, id: i32, event_id: String, _data: Value<'_>, _timestamp: u32) {
        if event_id == "clicked" {
            match id {
                ITEM_OPEN => {
                    let _ = self.events.send(TrayEvent::ShowWindow);
                }
                ITEM_QUIT => {
                    let _ = self.events.send(TrayEvent::Quit);
                }
                _ => {}
            }
        }
    }

    fn event_send_result(&self, _id: i32, _event_id: String, _data: Value<'_>) {}
}
