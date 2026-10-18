//! Interfaz GTK4 nativa: ventana principal, tarjetas de descarga y diálogos.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use gtk::glib;
use gtk::prelude::*;

use crate::i18n::tr;
use crate::worker::{Cmd, Event, ModelRow, RowInfo};

pub struct RowWidgets {
    badge: gtk::Label,
    progress: gtk::ProgressBar,
    size: gtk::Label,
    action: gtk::Label,
    speed: gtk::Label,
    threads: gtk::Label,
    eta: gtk::Label,
    error: gtk::Label,
    pause_btn: gtk::Button,
    resume_btn: gtk::Button,
}

fn meta(label: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(label));
    l.add_css_class("meta");
    l
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(r) = list.row_at_index(0) {
        list.remove(&r);
    }
}

/// escape para markup Pango
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn make_row(cmd_tx: &Sender<Cmd>, mr: &ModelRow) -> (gtk::ListBoxRow, RowWidgets) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("card");
    card.set_margin_top(6);
    card.set_margin_bottom(2);
    card.set_margin_start(8);
    card.set_margin_end(8);

    // ── línea 1: nombre + badge + detalles
    let line1 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(&mr.name));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    name.set_hexpand(true);
    name.add_css_class("name");
    let badge = gtk::Label::new(None);
    let info = gtk::Button::builder()
        .label("ⓘ")
        .has_frame(false)
        .tooltip_text(tr("detalles", "details"))
        .build();
    line1.append(&name);
    line1.append(&badge);
    line1.append(&info);

    let progress = gtk::ProgressBar::new();

    // ── línea 2: tamaño · acción · velocidad · hilos · eta
    let line2 = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let size = meta("");
    let action = meta("");
    action.set_hexpand(true);
    action.set_xalign(0.0);
    action.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let speed = meta("");
    let threads = meta("");
    let eta = meta("");
    line2.append(&size);
    line2.append(&action);
    line2.append(&speed);
    line2.append(&threads);
    line2.append(&eta);

    let error = meta("");
    error.add_css_class("err");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);

    // ── detalles desplegables
    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .build();
    let det = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let urls_l = meta(&format!("{} {}", tr("URLs:", "URLs:"), mr.urls.join("  ·  ")));
    urls_l.set_wrap(true);
    urls_l.set_selectable(true);
    urls_l.set_xalign(0.0);
    let dir_l = meta(&format!("{} {}", tr("Carpeta:", "Folder:"), mr.dir));
    dir_l.set_selectable(true);
    dir_l.set_xalign(0.0);
    let added_l = meta(&format!("{} {}", tr("Añadida:", "Added:"), mr.added_text));
    det.append(&urls_l);
    det.append(&dir_l);
    det.append(&added_l);
    revealer.set_child(Some(&det));
    {
        let rev = revealer.clone();
        info.connect_clicked(move |_| {
            rev.set_reveal_child(!rev.reveals_child());
        });
    }

    // ── línea 3: acciones
    let line3 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line3.set_halign(gtk::Align::End);
    let open_btn = gtk::Button::builder()
        .label(format!("📁 {}", tr("Abrir carpeta", "Open folder")))
        .build();
    let pause_btn = gtk::Button::builder().label(format!("⏸ {}", tr("Pausar", "Pause"))).build();
    let resume_btn = gtk::Button::builder().label(format!("▶ {}", tr("Reanudar", "Resume"))).build();
    let del_btn = gtk::Button::builder().label(format!("🗑 {}", tr("Eliminar", "Delete"))).build();
    del_btn.add_css_class("destructive");
    line3.append(&open_btn);
    line3.append(&pause_btn);
    line3.append(&resume_btn);
    line3.append(&del_btn);

    card.append(&line1);
    card.append(&progress);
    card.append(&line2);
    card.append(&error);
    card.append(&revealer);
    card.append(&line3);

    let row = gtk::ListBoxRow::builder().child(&card).build();
    row.set_activatable(false);

    let id = mr.id.clone();
    {
        let t = cmd_tx.clone();
        let id = id.clone();
        open_btn.connect_clicked(move |_| {
            let _ = t.send(Cmd::OpenFolder { id: id.clone() });
        });
    }
    {
        let t = cmd_tx.clone();
        let id = id.clone();
        pause_btn.connect_clicked(move |_| {
            let _ = t.send(Cmd::Pause { id: id.clone() });
        });
    }
    {
        let t = cmd_tx.clone();
        let id = id.clone();
        resume_btn.connect_clicked(move |_| {
            let _ = t.send(Cmd::Resume { id: id.clone() });
        });
    }
    {
        let t = cmd_tx.clone();
        let id = id.clone();
        del_btn.connect_clicked(move |b| {
            // confirmación con diálogo; si no hay ventana aún, borrar directo
            match b.root().and_then(|w| w.downcast::<gtk::Window>().ok()) {
                Some(win) => dialog_delete(&win, t.clone(), id.clone()),
                None => {
                    let _ = t.send(Cmd::Delete { id: id.clone(), delete_file: false });
                }
            }
        });
    }

    let widgets = RowWidgets {
        badge,
        progress,
        size,
        action,
        speed,
        threads,
        eta,
        error,
        pause_btn,
        resume_btn,
    };
    (row, widgets)
}

// ------------------------------------------------------------------ diálogos

fn dialog_shell(parent: &gtk::Window, title: &str) -> gtk::Window {
    let win = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(560)
        .resizable(false)
        .build();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 10);
    bx.set_margin_top(14);
    bx.set_margin_bottom(14);
    bx.set_margin_start(16);
    bx.set_margin_end(16);
    win.set_child(Some(&bx));
    win
}

fn buttons_row(cancel: &gtk::Button, ok: &gtk::Button) -> gtk::Box {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    r.set_halign(gtk::Align::End);
    r.append(cancel);
    r.append(ok);
    r
}

fn pick_folder(parent: &gtk::Window, entry: &gtk::Entry) {
    let fd = gtk::FileDialog::builder().title(tr("Guardar en", "Save to")).build();
    let e = entry.clone();
    let p = parent.clone();
    fd.select_folder(Some(&p), None::<&gtk::gio::Cancellable>, move |res| {
        if let Ok(f) = res {
            if let Some(path) = f.path() {
                e.set_text(&path.display().to_string());
            }
        }
    });
}

fn dialog_add(parent: &gtk::Window, cmd_tx: Sender<Cmd>, defaults: &Rc<RefCell<(String, i32, bool)>>) {
    let win = dialog_shell(parent, tr("Nueva descarga", "New download"));
    let bx = win.child().and_then(|c| c.downcast::<gtk::Box>().ok()).unwrap();

    bx.append(&meta(tr(
        "Enlace(s) de descarga — una URL por línea (varias URLs = espejos del mismo archivo)",
        "Download URL(s) — one per line (multiple URLs = mirrors of the same file)",
    )));
    let tv = gtk::TextView::new();
    tv.set_wrap_mode(gtk::WrapMode::WordChar);
    tv.set_size_request(-1, 92);
    let scroll = gtk::ScrolledWindow::builder().child(&tv).build();
    bx.append(&scroll);

    bx.append(&meta(tr("Guardar en", "Save to")));
    let dir_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let dir_entry = gtk::Entry::new();
    dir_entry.set_text(&defaults.borrow().0);
    dir_entry.set_hexpand(true);
    let browse = gtk::Button::builder().label(tr("Examinar…", "Browse…")).build();
    dir_row.append(&dir_entry);
    dir_row.append(&browse);
    bx.append(&dir_row);

    bx.append(&meta(tr("Nombre de archivo (opcional)", "File name (optional)")));
    let fn_entry = gtk::Entry::new();
    fn_entry.set_placeholder_text(Some(tr("Automático desde la URL", "Automatic from URL")));
    bx.append(&fn_entry);

    let conn_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    conn_row.append(&meta(tr("Conexiones (hilos)", "Connections (threads)")));
    let spin = gtk::SpinButton::with_range(1.0, 16.0, 1.0);
    spin.set_value(defaults.borrow().1 as f64);
    conn_row.append(&spin);
    bx.append(&conn_row);

    let cancel = gtk::Button::builder().label(tr("Cancelar", "Cancel")).build();
    let ok = gtk::Button::builder().label(tr("Descargar", "Download")).build();
    ok.add_css_class("suggested");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let w = win.clone();
        let t = cmd_tx.clone();
        let tv2 = tv.clone();
        let de = dir_entry.clone();
        let fe = fn_entry.clone();
        ok.connect_clicked(move |_| {
            let buf = tv2.buffer();
            let raw = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            let urls: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && l.contains("://"))
                .map(String::from)
                .collect();
            let dir = de.text().trim().to_string();
            if urls.is_empty() || dir.is_empty() {
                return;
            }
            let fname = fe.text().trim().to_string();
            let _ = t.send(Cmd::Add {
                urls,
                dir,
                filename: if fname.is_empty() { None } else { Some(fname) },
                connections: spin.value() as i32,
            });
            w.close();
        });
    }
    let browse2 = browse.clone();
    browse2.connect_clicked({
        let de = dir_entry.clone();
        let pw = win.clone();
        move |_| pick_folder(&pw, &de)
    });

    bx.append(&buttons_row(&cancel, &ok));
    win.present();
}

fn dialog_settings(parent: &gtk::Window, cmd_tx: Sender<Cmd>, defaults: &Rc<RefCell<(String, i32, bool)>>) {
    let defaults = defaults.clone();
    let win = dialog_shell(parent, tr("Ajustes", "Settings"));
    let bx = win.child().and_then(|c| c.downcast::<gtk::Box>().ok()).unwrap();

    bx.append(&meta(tr("Descargas simultáneas", "Simultaneous downloads")));
    let max_spin = gtk::SpinButton::with_range(1.0, 20.0, 1.0);
    max_spin.set_value(3.0);
    bx.append(&max_spin);

    bx.append(&meta(tr("Conexiones por descarga (máx. 16 hilos)", "Connections per download (max 16 threads)")));
    let conn_spin = gtk::SpinButton::with_range(1.0, 16.0, 1.0);
    conn_spin.set_value(defaults.borrow().1 as f64);
    bx.append(&conn_spin);

    bx.append(&meta(tr("Carpeta por defecto", "Default folder")));
    let dir_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let dir_entry = gtk::Entry::new();
    dir_entry.set_text(&defaults.borrow().0);
    dir_entry.set_hexpand(true);
    let browse = gtk::Button::builder().label(tr("Examinar…", "Browse…")).build();
    dir_row.append(&dir_entry);
    dir_row.append(&browse);
    bx.append(&dir_row);

    let tray_check = gtk::CheckButton::builder()
        .label(tr(
            "Minimizar a segundo plano al cerrar (sigue descargando; relanzar reabre la ventana)",
            "Minimize to background on close (keeps downloading; relaunch reopens the window)",
        ))
        .build();
    tray_check.set_active(defaults.borrow().2);
    bx.append(&tray_check);

    let cancel = gtk::Button::builder().label(tr("Cancelar", "Cancel")).build();
    let save = gtk::Button::builder().label(tr("Guardar", "Save")).build();
    save.add_css_class("suggested");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let w = win.clone();
        let de = dir_entry.clone();
        save.connect_clicked(move |_| {
            let conn = conn_spin.value() as i32;
            let dir = de.text().trim().to_string();
            let tray = tray_check.is_active();
            defaults.borrow_mut().0 = dir.clone();
            defaults.borrow_mut().1 = conn;
            defaults.borrow_mut().2 = tray;
            let _ = cmd_tx.send(Cmd::SetSettings {
                max_concurrent: max_spin.value() as i32,
                default_connections: conn,
                default_dir: if dir.is_empty() { None } else { Some(dir) },
                minimize_to_tray: tray,
            });
            w.close();
        });
    }
    browse.connect_clicked({
        let de = dir_entry.clone();
        let pw = win.clone();
        move |_| pick_folder(&pw, &de)
    });

    bx.append(&buttons_row(&cancel, &save));
    win.present();
}

fn dialog_delete(parent: &gtk::Window, cmd_tx: Sender<Cmd>, id: String) {
    let win = dialog_shell(parent, tr("Confirmar eliminación", "Confirm delete"));
    let bx = win.child().and_then(|c| c.downcast::<gtk::Box>().ok()).unwrap();
    bx.append(&gtk::Label::new(Some(tr(
        "¿Eliminar esta descarga de la lista?",
        "Remove this download from the list?",
    ))));
    let check = gtk::CheckButton::builder()
        .label(tr("Borrar también el archivo descargado/parcial", "Also delete downloaded/partial file"))
        .build();
    bx.append(&check);

    let cancel = gtk::Button::builder().label(tr("Cancelar", "Cancel")).build();
    let del = gtk::Button::builder().label(tr("Eliminar", "Delete")).build();
    del.add_css_class("destructive");
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }
    {
        let w = win.clone();
        del.connect_clicked(move |_| {
            let _ = cmd_tx.send(Cmd::Delete { id: id.clone(), delete_file: check.is_active() });
            w.close();
        });
    }
    bx.append(&buttons_row(&cancel, &del));
    win.present();
}

// ------------------------------------------------------------------ aplicación de estados

fn apply_row(ri: &RowInfo, w: &RowWidgets) {
    w.badge.set_markup(&format!(
        "<span foreground=\"{}\" weight=\"bold\">{}</span>",
        ri.badge_hex,
        esc(&ri.status_text)
    ));
    w.progress.set_show_text(ri.fraction_known);
    w.progress.set_fraction(if ri.fraction_known { ri.fraction } else { 0.0 });
    if ri.fraction_known {
        w.progress.set_text(Some(&format!("{:.1}%", ri.fraction * 100.0)));
    }
    w.size.set_text(&ri.size_text);
    w.action.set_text(&ri.action);
    w.speed.set_text(&ri.speed_text);
    w.threads.set_text(&ri.threads_text);
    w.eta.set_text(&ri.eta_text);
    if ri.error_text.is_empty() {
        w.error.set_visible(false);
    } else {
        w.error.set_text(&ri.error_text);
        w.error.set_visible(true);
    }
    w.pause_btn.set_visible(ri.can_pause);
    w.resume_btn.set_visible(ri.can_resume);
}

// ------------------------------------------------------------------ ventana principal

// Instancia única (GtkApplication): la primera activate construye; las siguientes
// simplemente traen la ventana al frente (restaurar desde segundo plano).
thread_local! {
    static EXISTING: RefCell<Option<(gtk::ApplicationWindow, Sender<Cmd>)>> = RefCell::new(None);
}

pub fn already_open() -> bool {
    EXISTING.with(|c| c.borrow().is_some())
}

pub fn present_existing() {
    EXISTING.with(|c| {
        if let Some((w, t)) = c.borrow().as_ref() {
            let _ = t.send(Cmd::SetHidden(false));
            w.present();
        }
    });
}

pub fn build(app: &gtk::Application, rx: std::sync::mpsc::Receiver<Event>, cmd_tx: Sender<Cmd>) {
    let win = gtk::ApplicationWindow::builder()
        .application(app)
        .title("⚡ Aria2 Downloads Booster")
        .default_width(1000)
        .default_height(740)
        .build();
    // Icono del tema (instalado por el paquete en hicolor/scalable/apps);
    // sin instalación queda el icono por defecto — sin coste.
    win.set_icon_name(Some("aria2-downloads-booster"));

    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    let display = gtk::prelude::WidgetExt::display(&win);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let hb = gtk::HeaderBar::builder().show_title_buttons(true).build();
    let title = gtk::Label::new(Some("⚡ Aria2 Downloads Booster"));
    title.add_css_class("title-4");
    hb.pack_start(&title);
    let btn_settings = gtk::Button::builder().label(tr("⚙ Ajustes", "⚙ Settings")).build();
    hb.pack_end(&btn_settings);
    let btn_add = gtk::Button::builder().label(tr("+ Nueva descarga", "+ New download")).build();
    btn_add.add_css_class("suggested");
    hb.pack_end(&btn_add);
    win.set_titlebar(Some(&hb));

    // etiquetas de estado globales
    let banner = gtk::Label::new(None);
    banner.add_css_class("meta");
    banner.set_wrap(true);
    banner.set_selectable(true);
    banner.set_xalign(0.0);
    banner.set_margin_start(12);
    banner.set_margin_end(12);
    banner.set_visible(false);

    let flash = gtk::Label::new(None);
    flash.add_css_class("flash");
    flash.set_wrap(true);
    flash.set_xalign(0.0);
    flash.set_margin_start(12);
    flash.set_visible(false);

    let speed_lbl = gtk::Label::new(None);
    speed_lbl.add_css_class("meta");
    speed_lbl.set_xalign(0.0);
    speed_lbl.set_margin_start(12);
    speed_lbl.set_visible(false);

    let heartbeat_lbl = gtk::Label::new(None);
    heartbeat_lbl.add_css_class("dim-label");
    heartbeat_lbl.set_xalign(0.0);
    heartbeat_lbl.set_margin_start(12);
    heartbeat_lbl.set_margin_bottom(4);

    // secciones
    let sv = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let head_active = gtk::Label::new(Some(&tr("Descargas", "Downloads")));
    head_active.add_css_class("title-5");
    head_active.set_xalign(0.0);
    head_active.set_margin_start(14);
    head_active.set_margin_top(8);
    let empty = gtk::Label::new(Some(&tr(
        "No hay descargas activas.\nPulsa «+ Nueva descarga» para empezar.",
        "No active downloads.\nPress «+ New download» to start one.",
    )));
    empty.add_css_class("dim-label");
    empty.set_margin_top(30);
    empty.set_visible(false);

    let list_active = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    list_active.add_css_class("cards");

    let done_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let head_done = gtk::Label::new(Some(&tr("Completadas", "Completed")));
    head_done.add_css_class("title-5");
    head_done.set_xalign(0.0);
    head_done.set_margin_start(14);
    head_done.set_margin_top(10);
    head_done.set_hexpand(true);
    let btn_cleanup = gtk::Button::builder().label(tr("Vaciar completadas", "Clear completed")).build();
    btn_cleanup.set_margin_end(10);
    btn_cleanup.set_valign(gtk::Align::Center);
    done_row.append(&head_done);
    done_row.append(&btn_cleanup);
    let list_done = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
    list_done.add_css_class("cards");

    sv.append(&head_active);
    sv.append(&empty);
    sv.append(&list_active);
    sv.append(&done_row);
    sv.append(&list_done);

    let scrolled = gtk::ScrolledWindow::builder().vexpand(true).child(&sv).build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
    root.append(&banner);
    root.append(&flash);
    root.append(&speed_lbl);
    root.append(&scrolled);
    root.append(&heartbeat_lbl);
    win.set_child(Some(&root));

    // valores por defecto para los diálogos (se actualizan al guardar ajustes)
    let defaults: Rc<RefCell<(String, i32, bool)>> = Rc::new(RefCell::new((
        std::env::var("HOME")
            .map(|h| format!("{h}/Downloads"))
            .unwrap_or_else(|_| {
                std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| ".".into())
            }),
        16,
        true,
    )));

    btn_add.connect_clicked({
        let w = win.clone();
        let t = cmd_tx.clone();
        let d = defaults.clone();
        move |_| dialog_add(&w.upcast_ref::<gtk::Window>(), t.clone(), &d)
    });
    btn_settings.connect_clicked({
        let w = win.clone();
        let t = cmd_tx.clone();
        let d = defaults.clone();
        move |_| dialog_settings(&w.upcast_ref::<gtk::Window>(), t.clone(), &d)
    });
    btn_cleanup.connect_clicked({
        let t = cmd_tx.clone();
        move |_| {
            let _ = t.send(Cmd::CleanupCompleted);
        }
    });

    // eventos del worker → widgets
    let map: Rc<RefCell<HashMap<String, RowWidgets>>> = Rc::new(RefCell::new(HashMap::new()));
    let app2 = app.clone();
    let flash2 = flash.clone();
    let banner2 = banner.clone();
    let speed2 = speed_lbl.clone();
    let heartbeat2 = heartbeat_lbl.clone();
    let empty2 = empty.clone();
    // dreno del canal del worker cada 25 ms (sin API de canal de glib: robusta entre versiones)
    let ev_cell = Rc::new(RefCell::new(rx));
    let cmd_tx_events = cmd_tx.clone();
    let win_ev = win.clone();
    let defaults_ev = defaults.clone();
    let quit_flag = Rc::new(std::cell::Cell::new(false));
    let quit_flag_ev = quit_flag.clone();
    let _attached = glib::timeout_add_local(std::time::Duration::from_millis(25), move || {
      loop {
        let ev = match ev_cell.borrow().try_recv() {
            Ok(ev) => ev,
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
        };
        match ev {
            Event::Model(rows) => {
                clear_list(&list_active);
                clear_list(&list_done);
                map.borrow_mut().clear();
                let mut n_active = 0usize;
                for mr in &rows {
                    let (row, w) = make_row(&cmd_tx_events, mr);
                    if mr.completed {
                        list_done.append(&row);
                    } else {
                        list_active.append(&row);
                        n_active += 1;
                    }
                    map.borrow_mut().insert(mr.id.clone(), w);
                }
                empty2.set_visible(n_active == 0);
            }
            Event::Statuses { rows, global_speed, heartbeat, engine } => {
                banner2.set_text(&engine.banner);
                banner2.set_visible(!engine.banner.is_empty());
                if global_speed.is_empty() {
                    speed2.set_visible(false);
                } else {
                    speed2.set_text(&global_speed);
                    speed2.set_visible(true);
                }
                heartbeat2.set_text(&heartbeat);
                if let Some(rs) = rows {
                    for ri in &rs {
                        if let Some(w) = map.borrow().get(&ri.id) {
                            apply_row(ri, w);
                        }
                    }
                }
            }
            Event::Flash(msg) => {
                flash2.set_text(&msg);
                flash2.set_visible(true);
                let f = flash2.clone();
                glib::timeout_add_local(std::time::Duration::from_secs(9), move || {
                    f.set_visible(false);
                    glib::ControlFlow::Break
                });
            }
            Event::HideWindow => {
                quit_flag_ev.set(false);
                let _ = cmd_tx_events.send(Cmd::SetHidden(true));
                win_ev.set_visible(false);
            }
            Event::PresentWindow => {
                quit_flag_ev.set(false);
                let _ = cmd_tx_events.send(Cmd::SetHidden(false));
                win_ev.present();
            }
            Event::Settings { dir, connections, minimize_to_tray: tray } => {
                // el worker manda los ajustes persistentes: diálogos siempre en verdad
                let mut d = defaults_ev.borrow_mut();
                d.0 = dir;
                d.1 = connections;
                d.2 = tray;
            }
            Event::Bye => app2.quit(),
        }
      }
      glib::ControlFlow::Continue
    });

    // cierre de ventana: el worker decide (ocultar a segundo plano o apagar según ajuste).
    win.connect_close_request({
        let t = cmd_tx.clone();
        let a = app.clone();
        let qf = quit_flag.clone();
        move |_| {
            qf.set(true);
            let _ = t.send(Cmd::HideRequested);
            // red de seguridad: si el worker cuelga, salir de verdad tras 8 s
            glib::timeout_add_local(std::time::Duration::from_millis(8000), {
                let a = a.clone();
                let qf2 = qf.clone();
                move || {
                    if qf2.get() {
                        a.quit();
                    }
                    glib::ControlFlow::Break
                }
            });
            glib::Propagation::Stop
        }
    });

    // Ctrl+Q con la ventana visible: salida real (deja de sondear y apaga el motor)
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let t = cmd_tx.clone();
        move |_, key, _code, mods| {
            if key == gtk::gdk::Key::q && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                let _ = t.send(Cmd::Shutdown);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    win.add_controller(keys);

    // registrar la instancia para que un relanzamiento la restaure
    EXISTING.with(|c| *c.borrow_mut() = Some((win.clone(), cmd_tx.clone())));

    win.present();
}
