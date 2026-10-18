mod engine;
mod i18n;
mod models;
mod paths;
mod rpc;
mod window;
mod worker;

use gtk::glib;

fn main() {
    let app = gtk::Application::builder()
        .application_id("io.ariaxtr.downloader")
        .build();

    app.connect_activate(|app| {
        // UI ← worker: canal de eventos glib (线程-safe); UI → worker: mpsc de comandos
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (ui_tx, ui_rx) = glib::MainContext::channel::<worker::Event>(glib::Priority::default());
        std::thread::spawn(move || worker::run(cmd_rx, ui_tx.clone()));
        window::build(app, ui_rx, cmd_tx);
    });

    app.run();
}
