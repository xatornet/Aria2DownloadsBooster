mod engine;
mod i18n;
mod models;
mod paths;
mod rpc;
mod tray;
mod window;
mod worker;

use gtk::gio::prelude::*;

fn main() {
    let app = gtk::Application::builder()
        .application_id("io.ariaxtr.downloader")
        .build();

    app.connect_activate(|app| {
        // Instancia única: si ya hay ventana (quizá oculta en segundo plano), restaurarla.
        if window::already_open() {
            window::present_existing();
            return;
        }
        // UI → worker: mpsc de comandos. worker → UI: mpsc de eventos drenado por timeout.
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (ev_tx, ev_rx) = std::sync::mpsc::channel::<worker::Event>();
        std::thread::spawn(move || worker::run(cmd_rx, ev_tx));
        window::build(app, ev_rx, cmd_tx);
    });

    app.run();
}
