use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config as C;

use crate::ctx::Ctx;
use gtk::glib;

/// Kernel, Grafiktreiber und Beschleunigung an die neue Plattform anpassen.
fn on_arch_changed(ctx: &Rc<Ctx>) {
    {
        let mut cfg = ctx.cfg.borrow_mut();
        cfg.packages.kernel = C::kernel(&cfg);
        if !C::gpus_for(&cfg)
            .iter()
            .any(|(k, _)| *k == cfg.packages.gpu)
        {
            cfg.packages.gpu = "mesa".into();
        }
        if C::is_image(&cfg) && matches!(cfg.ai.accel.as_str(), "cuda" | "rocm") {
            cfg.ai.accel = "cpu".into();
        }
    }
    ctx.rebuild();
    let desc = C::arch_info(&ctx.cfg.borrow()).desc;
    ctx.toast(desc);
}

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let plat = adw::PreferencesGroup::builder()
        .title("Zielplattform")
        .description(
            "x86_64 ergibt eine ISO mit Live-System und Installer. Für ARM und den Raspberry Pi entsteht ein fertig installiertes Disk-Image (Arch Linux ARM), das du direkt auf SD-Karte oder USB-Stick schreibst.",
        )
        .build();
    page.add(&plat);
    let arches: Vec<(String, String)> = C::arches()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.label.to_string()))
        .collect();
    let desc = C::arch_info(&ctx.cfg.borrow()).desc;
    let arch_row = ctx.combo_row(
        &plat,
        "Architektur",
        &arches,
        |cfg| cfg.build.arch.clone(),
        |cfg, v| cfg.build.arch = v,
        Some(desc),
        false,
    );
    {
        let ctx2 = ctx.clone();
        arch_row.connect_selected_notify(move |_| {
            // Erst nach dem Signal neu aufbauen – die Zeile selbst wird dabei ersetzt.
            let ctx3 = ctx2.clone();
            glib::idle_add_local_once(move || on_arch_changed(&ctx3));
        });
    }

    let grp = adw::PreferencesGroup::builder()
        .title("Deine Distribution")
        .description(
            "Diese Angaben landen in /etc/os-release, im Bootmenü, in fastfetch/neofetch und im Installer.",
        )
        .build();
    page.add(&grp);

    // Mirrors `self._auto_id` in ui.py: while the id field still matches the
    // auto-generated slug (or is empty), typing the name keeps re-slugging it.
    let auto_id: Rc<RefCell<String>> = Rc::new(RefCell::new({
        let cfg = ctx.cfg.borrow();
        if cfg.general.id == C::slugify(&cfg.general.name) {
            cfg.general.id.clone()
        } else {
            String::new()
        }
    }));

    let id_row = ctx.entry_row(
        &grp,
        "ID (kleingeschrieben, für Dateinamen)",
        |cfg| cfg.general.id.clone(),
        |cfg, v| cfg.general.id = v,
        None,
    );

    {
        let id_row = id_row.clone();
        let auto_id = auto_id.clone();
        ctx.entry_row(
            &grp,
            "Name der Distro",
            |cfg| cfg.general.name.clone(),
            |cfg, v| cfg.general.name = v,
            Some(Box::new(move |_ctx, text| {
                let current = id_row.text().to_string();
                if current.is_empty() || current == *auto_id.borrow() {
                    let slug = C::slugify(text);
                    *auto_id.borrow_mut() = slug.clone();
                    id_row.set_text(&slug);
                }
            })),
        );
    }
    // Re-order: Python builds "Name" before "ID"; move the name row above.
    grp.remove(&id_row);
    grp.add(&id_row);

    ctx.entry_row(
        &grp,
        "Version",
        |cfg| cfg.general.version.clone(),
        |cfg, v| cfg.general.version = v,
        None,
    );
    ctx.entry_row(
        &grp,
        "Standard-Hostname",
        |cfg| cfg.general.hostname.clone(),
        |cfg, v| cfg.general.hostname = v,
        None,
    );

    let grp2 = adw::PreferencesGroup::builder()
        .title("Herausgeber &amp; Links")
        .description("Optional")
        .build();
    page.add(&grp2);
    ctx.entry_row(
        &grp2,
        "Herausgeber",
        |cfg| cfg.general.publisher.clone(),
        |cfg, v| cfg.general.publisher = v,
        None,
    );
    ctx.entry_row(
        &grp2,
        "Webseite (HOME_URL)",
        |cfg| cfg.general.homepage.clone(),
        |cfg, v| cfg.general.homepage = v,
        None,
    );
    ctx.entry_row(
        &grp2,
        "Support-URL",
        |cfg| cfg.general.support_url.clone(),
        |cfg, v| cfg.general.support_url = v,
        None,
    );
    ctx.entry_row(
        &grp2,
        "Fehlerberichte-URL",
        |cfg| cfg.general.bug_url.clone(),
        |cfg, v| cfg.general.bug_url = v,
        None,
    );

    page
}
