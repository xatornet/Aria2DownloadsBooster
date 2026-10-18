//! Watcher SNI de prueba (mock del host del escritorio) para validar la bandeja sin
//! un escritorio real, p. ej. bajo `dbus-run-session` en WSL/CI:
//!
//! ```sh
//! dbus-run-session -- bash -c 'cargo run --example sni-host & ./target/debug/aria-xtrdownloader &'
//! ```
use std::time::Duration;

use zbus::blocking::Connection;

#[derive(Debug)]
struct Watcher {
    items: std::sync::Mutex<Vec<String>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        println!("[sni-host] registrado: {service}");
        self.items.lock().unwrap().push(service.to_string());
    }

    fn unregister_status_notifier_item(&self, service: &str) {
        println!("[sni-host] desregistrado: {service}");
        self.items.lock().unwrap().retain(|s| s != service);
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_running(&self) -> bool {
        true
    }
}

fn main() {
    let conn = Connection::session().expect("DBus de sesión");
    conn.request_name("org.kde.StatusNotifierWatcher")
        .expect("nombre de watcher");
    let watcher = Watcher { items: std::sync::Mutex::new(Vec::new()) };
    conn.object_server()
        .at("/StatusNotifierWatcher", watcher)
        .expect("objeto watcher");
    println!("[sni-host] watcher listo");
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
