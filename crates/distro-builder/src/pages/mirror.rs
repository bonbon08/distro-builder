//! „Eigener Spiegelserver“ auf der Repositories-Seite: Einstellungen, eingebauter Server,
//! vollständiger Spiegel, eigenes Paket-Repository und Server-Paket.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use distro_builder_core::config as C;
use distro_builder_core::mirror as M;
use gtk::glib;

use crate::ctx::Ctx;

pub fn add_groups(ctx: &Rc<Ctx>, page: &adw::PreferencesPage) {
    let grp = adw::PreferencesGroup::builder()
        .title("Eigener Spiegelserver")
        .description(
            "Ein Spiegel für dich selbst: Build, Live-System und installierte Systeme laden Pakete zuerst von dort. Der Caching-Proxy speichert nur, was wirklich gebraucht wird – jeder weitere Rechner und jeder weitere ISO-Build lädt dann aus dem LAN.",
        )
        .build();
    page.add(&grp);
    ctx.switch_row(
        &grp,
        "Distro nutzt den eigenen Spiegel",
        Some("Wird in pacman.conf vor den offiziellen Mirrors eingetragen"),
        |cfg| cfg.mirror_server.enabled,
        |cfg, v| cfg.mirror_server.enabled = v,
    );
    let modes: Vec<(String, String)> = C::mirror_modes()
        .into_iter()
        .map(|(k, l)| (k.to_string(), l.to_string()))
        .collect();
    let mode_row = ctx.combo_row(
        &grp,
        "Art",
        &modes,
        |cfg| cfg.mirror_server.mode.clone(),
        |cfg, v| cfg.mirror_server.mode = v,
        None,
        false,
    );
    let url_row = ctx.entry_row(
        &grp,
        "Adresse, z. B. http://192.168.1.10:8080",
        |cfg| cfg.mirror_server.url.clone(),
        |cfg, v| cfg.mirror_server.url = v,
        None,
    );
    let here = gtk::Button::builder()
        .label("Dieser Rechner")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .tooltip_text("LAN-Adresse dieses Rechners mit dem Port unten eintragen")
        .build();
    {
        let ctx2 = ctx.clone();
        let url_row = url_row.clone();
        here.connect_clicked(move |_| {
            let port = ctx2.cfg.borrow().mirror_server.port;
            url_row.set_text(&M::suggested_url(port));
        });
    }
    url_row.add_suffix(&here);
    let port = adw::SpinRow::new(
        Some(&gtk::Adjustment::new(8080.0, 1.0, 65535.0, 1.0, 100.0, 0.0)),
        1.0,
        0,
    );
    port.set_title("Port des eingebauten Servers");
    port.set_value(ctx.cfg.borrow().mirror_server.port as f64);
    {
        let ctx2 = ctx.clone();
        port.connect_value_notify(move |r| {
            ctx2.cfg.borrow_mut().mirror_server.port = r.value() as u16;
            ctx2.changed();
        });
    }
    grp.add(&port);
    ctx.switch_row(
        &grp,
        "Offizielle Mirrors als Ausweich",
        Some("Ist der eigene Spiegel nicht erreichbar, lädt pacman von dort"),
        |cfg| cfg.mirror_server.fallback,
        |cfg, v| cfg.mirror_server.fallback = v,
    );
    let upstream_row = ctx.entry_row(
        &grp,
        "Quelle des Proxys (leer = Standard)",
        |cfg| cfg.mirror_server.upstream.clone(),
        |cfg, v| cfg.mirror_server.upstream = v,
        None,
    );
    upstream_row.set_tooltip_text(Some(&format!(
        "pacman-Format mit $repo und $arch. Standard: {}",
        C::mirror_upstream(&ctx.cfg.borrow())
    )));
    let rsync_row = ctx.entry_row(
        &grp,
        "rsync-Quelle",
        |cfg| cfg.mirror_server.rsync_upstream.clone(),
        |cfg, v| cfg.mirror_server.rsync_upstream = v,
        None,
    );

    // ---- Server
    let srv = adw::PreferencesGroup::builder()
        .title("Server")
        .description(
            "Der eingebaute Server läuft, solange Distro Builder offen ist. Dauerhaft: „Server-Paket exportieren“ (nginx/Docker) oder distro-builder --mirror-server projekt.json",
        )
        .build();
    page.add(&srv);
    let status = adw::ActionRow::builder()
        .title("Eingebauter Server")
        .build();
    let toggle = gtk::Button::builder()
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();
    let open = gtk::Button::builder()
        .icon_name("web-browser-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .tooltip_text("Im Browser öffnen")
        .build();
    status.add_suffix(&open);
    status.add_suffix(&toggle);
    srv.add(&status);
    let last = adw::ActionRow::builder()
        .title("Letzte Anfrage")
        .subtitle("–")
        .css_classes(["property"])
        .build();
    srv.add(&last);
    let sync_row = adw::ActionRow::builder()
        .title("Vollständigen Spiegel synchronisieren")
        .subtitle("rsync der genutzten Repos – beim ersten Mal 80 GB und mehr")
        .build();
    let sync_btn = gtk::Button::builder()
        .label("Jetzt synchronisieren")
        .valign(gtk::Align::Center)
        .build();
    sync_row.add_suffix(&sync_btn);
    srv.add(&sync_row);
    let kit_row = adw::ActionRow::builder()
        .title("Server-Paket exportieren")
        .subtitle("nginx-Konfiguration, compose.yaml, Sync-Skript und systemd-Timer für einen eigenen Server")
        .build();
    let kit_btn = gtk::Button::builder()
        .label("Exportieren")
        .valign(gtk::Align::Center)
        .build();
    kit_row.add_suffix(&kit_btn);
    srv.add(&kit_row);

    {
        let ctx2 = ctx.clone();
        toggle.connect_clicked(move |_| toggle_server(&ctx2));
    }
    {
        let ctx2 = ctx.clone();
        open.connect_clicked(move |_| {
            let port = ctx2
                .mirror
                .borrow()
                .as_ref()
                .map(|h| h.port)
                .unwrap_or(ctx2.cfg.borrow().mirror_server.port);
            let _ = std::process::Command::new("xdg-open")
                .arg(format!("http://localhost:{port}/"))
                .spawn();
        });
    }
    {
        let ctx2 = ctx.clone();
        sync_btn.connect_clicked(move |_| run_full_sync(&ctx2));
    }
    {
        let ctx2 = ctx.clone();
        kit_btn.connect_clicked(move |_| {
            let cfg = ctx2.cfg.borrow().clone();
            match M::write_server_kit(&cfg) {
                Ok(dir) => {
                    ctx2.toast("Server-Paket exportiert");
                    let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                }
                Err(e) => ctx2.error("Export fehlgeschlagen", &e.to_string()),
            }
        });
    }

    // ---- eigenes Repo
    let own = adw::PreferencesGroup::builder()
        .title("Eigenes Paket-Repository")
        .description(
            "Eigene Pakete (z. B. selbst gebaute AUR-Pakete) als Repository der Distro. Der Spiegel liefert es mit aus; die Distro bindet es vor den offiziellen Repos ein.",
        )
        .build();
    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .css_classes(["flat"])
        .tooltip_text("Pakete hinzufügen (.pkg.tar.zst)")
        .build();
    own.set_header_suffix(Some(&add_btn));
    page.add(&own);
    ctx.switch_row(
        &own,
        "Eigenes Repository ausliefern",
        Some(&format!(
            "[{}] – mit SigLevel „Optional TrustAll“",
            ctx.cfg.borrow().general.id
        )),
        |cfg| cfg.mirror_server.own_repo,
        |cfg, v| cfg.mirror_server.own_repo = v,
    );
    {
        let ctx2 = ctx.clone();
        add_btn.connect_clicked(move |_| pick_packages(&ctx2));
    }

    {
        let mut shared = ctx.shared.borrow_mut();
        shared.mirror_status = Some(status);
        shared.mirror_toggle = Some(toggle);
        shared.mirror_last = Some(last);
        shared.own_pkg_grp = Some(own);
    }
    refresh_status(ctx);
    refresh_packages(ctx);

    // Proxy- und rsync-Zeilen je nach Art ein-/ausblenden
    let sync_vis: Rc<dyn Fn()> = {
        let ctx2 = ctx.clone();
        Rc::new(move || {
            let full = ctx2.cfg.borrow().mirror_server.mode == "full";
            upstream_row.set_visible(!full);
            rsync_row.set_visible(full);
            sync_row.set_visible(full);
        })
    };
    sync_vis();
    mode_row.connect_selected_notify(move |_| sync_vis());
}

pub fn refresh_status(ctx: &Rc<Ctx>) {
    let shared = ctx.shared.borrow();
    let (Some(status), Some(toggle)) = (&shared.mirror_status, &shared.mirror_toggle) else {
        return;
    };
    let cfg = ctx.cfg.borrow();
    let paths = M::mirror_paths(&cfg);
    let size = M::dir_size(if cfg.mirror_server.mode == "full" {
        &paths.full
    } else {
        &paths.cache
    });
    match ctx.mirror.borrow().as_ref() {
        Some(h) => {
            status.set_subtitle(&format!(
                "läuft auf {} – {} gespeichert, {} ausgeliefert",
                M::suggested_url(h.port),
                M::human_size(size),
                M::human_size(h.served_bytes.load(std::sync::atomic::Ordering::Relaxed))
            ));
            toggle.set_label("Stoppen");
            toggle.set_css_classes(&["destructive-action"]);
        }
        None => {
            status.set_subtitle(&format!("gestoppt – {} gespeichert", M::human_size(size)));
            toggle.set_label("Starten");
            toggle.set_css_classes(&["suggested-action"]);
        }
    }
}

fn toggle_server(ctx: &Rc<Ctx>) {
    // Borrow vor dem if-let beenden: refresh_status liest ctx.mirror erneut
    let running = ctx.mirror.borrow_mut().take();
    if let Some(handle) = running {
        handle.stop();
        ctx.toast("Spiegelserver gestoppt");
        refresh_status(ctx);
        return;
    }
    start(ctx);
}

/// Startet den eingebauten Spiegelserver (Fehler als Dialog).
pub fn start(ctx: &Rc<Ctx>) {
    let cfg = ctx.cfg.borrow().clone();
    let (tx, rx) = async_channel::unbounded::<String>();
    let log: M::LogFn = Arc::new(move |line| {
        let _ = tx.send_blocking(line);
    });
    match M::start_server(M::ServerConfig::from_config(&cfg), log) {
        Ok(handle) => {
            let url = M::suggested_url(handle.port);
            *ctx.mirror.borrow_mut() = Some(handle);
            ctx.toast(&format!("Spiegelserver läuft auf {url}"));
            ctx.log(&format!("Spiegelserver gestartet: {url}"));
            let ctx2 = ctx.clone();
            glib::spawn_future_local(async move {
                while let Ok(line) = rx.recv().await {
                    if let Some(row) = &ctx2.shared.borrow().mirror_last {
                        row.set_subtitle(&glib::markup_escape_text(&line));
                    }
                    ctx2.log(&format!("Spiegel: {line}"));
                    refresh_status(&ctx2);
                }
            });
        }
        Err(e) => ctx.error(
            "Spiegelserver konnte nicht starten",
            &format!(
                "{e}\n\nLäuft auf dem Port schon etwas anderes? Dann einen anderen Port wählen."
            ),
        ),
    }
    refresh_status(ctx);
}

fn run_full_sync(ctx: &Rc<Ctx>) {
    let cfg = ctx.cfg.borrow().clone();
    let (errors, _) = C::validate(&cfg);
    if let Some(e) = errors
        .iter()
        .find(|e| e.contains("rsync") || e.contains("Spiegel"))
    {
        ctx.error("Bitte korrigieren", e);
        return;
    }
    let kit = match M::write_server_kit(&cfg) {
        Ok(k) => k,
        Err(e) => {
            ctx.error("Sync-Skript konnte nicht erzeugt werden", &e.to_string());
            return;
        }
    };
    let target = M::mirror_paths(&cfg).full;
    ctx.toast("Synchronisation läuft – Fortschritt auf der Seite „Bauen“");
    ctx.log(&format!(
        "Synchronisiere vollständigen Spiegel nach {}",
        target.display()
    ));
    let ctx2 = ctx.clone();
    ctx.run_streaming(
        vec![
            "env".into(),
            format!("MIRROR_DIR={}", target.display()),
            "bash".into(),
            kit.join("sync-mirror.sh").to_string_lossy().into_owned(),
        ],
        |ctx, line| ctx.log(line),
        move |_ctx, rc| {
            ctx2.toast(if rc == 0 {
                "Spiegel ist aktuell"
            } else {
                "Synchronisation fehlgeschlagen – siehe Protokoll"
            });
            refresh_status(&ctx2);
        },
    );
}

fn pick_packages(ctx: &Rc<Ctx>) {
    let dlg = gtk::FileDialog::builder()
        .title("Pakete hinzufügen")
        .build();
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Arch-Pakete (*.pkg.tar.*)"));
    filter.add_pattern("*.pkg.tar.*");
    dlg.set_default_filter(Some(&filter));
    let ctx2 = ctx.clone();
    dlg.open_multiple(Some(&ctx.window), None::<&gio::Cancellable>, move |res| {
        let Ok(list) = res else { return };
        let files: Vec<PathBuf> = (0..list.n_items())
            .filter_map(|i| list.item(i).and_downcast::<gio::File>())
            .filter_map(|f| f.path())
            .collect();
        if files.is_empty() {
            return;
        }
        let cfg = ctx2.cfg.borrow().clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let mut lines = Vec::new();
            let result = M::add_packages(&cfg, &files, &mut |l| lines.push(l.to_string()));
            let _ = tx.send_blocking((result.map_err(|e| e.to_string()), lines));
        });
        let ctx3 = ctx2.clone();
        glib::spawn_future_local(async move {
            if let Ok((result, lines)) = rx.recv().await {
                for l in &lines {
                    ctx3.log(l);
                }
                match result {
                    Ok(n) => {
                        if !ctx3.cfg.borrow().mirror_server.own_repo {
                            ctx3.cfg.borrow_mut().mirror_server.own_repo = true;
                            ctx3.rebuild();
                        }
                        ctx3.toast(&format!("{n} Paket(e) hinzugefügt"));
                    }
                    Err(e) => ctx3.error("Pakete konnten nicht hinzugefügt werden", &e),
                }
                refresh_packages(&ctx3);
                ctx3.changed();
            }
        });
    });
}

fn refresh_packages(ctx: &Rc<Ctx>) {
    let grp = ctx.shared.borrow().own_pkg_grp.clone();
    let Some(grp) = grp else { return };
    for row in std::mem::take(&mut ctx.shared.borrow_mut().own_pkg_rows) {
        grp.remove(&row);
    }
    let pkgs = M::list_packages(&ctx.cfg.borrow());
    let mut rows = Vec::new();
    if pkgs.is_empty() {
        let row = adw::ActionRow::builder()
            .title("Noch keine Pakete")
            .subtitle("Mit + fertige .pkg.tar.zst-Dateien hinzufügen (z. B. aus makepkg)")
            .build();
        grp.add(&row);
        rows.push(row);
    }
    for file in pkgs {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&M::package_name(&file)))
            .subtitle(glib::markup_escape_text(&file))
            .build();
        let rm = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        let ctx2 = ctx.clone();
        rm.connect_clicked(move |_| {
            let cfg = ctx2.cfg.borrow().clone();
            let mut lines = Vec::new();
            if let Err(e) = M::remove_package(&cfg, &file, &mut |l| lines.push(l.to_string())) {
                ctx2.error("Paket konnte nicht entfernt werden", &e.to_string());
            }
            for l in &lines {
                ctx2.log(l);
            }
            refresh_packages(&ctx2);
            ctx2.changed();
        });
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().own_pkg_rows = rows;
}
