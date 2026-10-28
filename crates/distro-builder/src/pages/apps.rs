//! Seite „Flatpak & Distrobox“: Flatpak-Quellen und -Apps sowie Distrobox-Container.

use std::collections::HashSet;
use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config::{self as C, DistroboxBox, FlatpakRemote};
use gtk::glib;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    // ---- Flatpak
    let fp = adw::PreferencesGroup::builder()
        .title("Flatpak")
        .description(
            "Apps aus Flathub oder anderen Flatpak-Quellen vorinstallieren. Sie werden beim Bauen geladen und beim ersten Start eingerichtet – fehlt etwas, lädt das System es nach, sobald Internet da ist.",
        )
        .build();
    page.add(&fp);
    let flatpak_row = ctx.switch_row(
        &fp,
        "Flatpak einrichten",
        None,
        |cfg| cfg.flatpak.enabled,
        |cfg, v| cfg.flatpak.enabled = v,
    );
    ctx.shared.borrow_mut().flatpak_row = Some(flatpak_row);
    ctx.switch_row(
        &fp,
        "Flathub als Quelle",
        Some("dl.flathub.org – die größte Flatpak-Quelle"),
        |cfg| cfg.flatpak.flathub,
        |cfg, v| cfg.flatpak.flathub = v,
    );
    ctx.switch_row(
        &fp,
        "App-Store mit Flatpak-Unterstützung",
        Some("KDE/LXQt: Discover, sonst GNOME Software"),
        |cfg| cfg.flatpak.store,
        |cfg, v| cfg.flatpak.store = v,
    );
    ctx.switch_row(
        &fp,
        "Apps beim Bauen mitliefern",
        Some("Braucht flatpak auf diesem Rechner. Aus = Download beim ersten Start"),
        |cfg| cfg.flatpak.preload,
        |cfg, v| cfg.flatpak.preload = v,
    );

    let apps = adw::PreferencesGroup::builder()
        .title("Flatpak-Apps")
        .build();
    page.add(&apps);
    let x86 = !C::is_image(&ctx.cfg.borrow());
    let mut preset_ids = HashSet::new();
    for preset in C::flatpak_presets() {
        if preset.x86_only && !x86 {
            continue;
        }
        preset_ids.insert(preset.id.to_string());
        let active = ctx.cfg.borrow().flatpak.apps.iter().any(|a| a == preset.id);
        let row = adw::SwitchRow::builder()
            .title(preset.label)
            .subtitle(preset.id)
            .active(active)
            .build();
        let ctx2 = ctx.clone();
        let id = preset.id.to_string();
        row.connect_active_notify(move |r| toggle_flatpak(&ctx2, &id, r.is_active()));
        apps.add(&row);
    }
    let preset_ids = Rc::new(preset_ids);

    let custom_grp = adw::PreferencesGroup::builder()
        .title("Weitere Flatpak-Apps")
        .description(
            "App-ID wie auf flathub.org, z. B. org.inkscape.Inkscape. Aus einer eigenen Quelle: quelle:app.id",
        )
        .build();
    page.add(&custom_grp);
    ctx.shared.borrow_mut().flatpak_custom_grp = Some(custom_grp.clone());
    let add = adw::EntryRow::builder()
        .title("App hinzufügen")
        .show_apply_button(true)
        .build();
    {
        let ctx2 = ctx.clone();
        let preset_ids = preset_ids.clone();
        add.connect_apply(move |r| {
            let entry = r.text().trim().to_string();
            let app = entry.rsplit(':').next().unwrap_or_default();
            if !C::flatpak_app_re().is_match(app) {
                ctx2.toast("Ungültige App-ID");
                return;
            }
            toggle_flatpak(&ctx2, &entry, true);
            r.set_text("");
            refresh_flatpak_custom(&ctx2, &preset_ids);
        });
    }
    custom_grp.add(&add);
    refresh_flatpak_custom(ctx, &preset_ids);

    let remotes_grp = adw::PreferencesGroup::builder()
        .title("Eigene Flatpak-Quellen")
        .description(
            "Name und Adresse einer .flatpakrepo-Datei, z. B. kde → https://distribute.kde.org/kdeapps.flatpakrepo",
        )
        .build();
    page.add(&remotes_grp);
    ctx.shared.borrow_mut().remotes_grp = Some(remotes_grp.clone());
    let rname = adw::EntryRow::builder().title("Name der Quelle").build();
    let rurl = adw::EntryRow::builder()
        .title("URL (.flatpakrepo)")
        .show_apply_button(true)
        .build();
    {
        let ctx2 = ctx.clone();
        let rname = rname.clone();
        rurl.connect_apply(move |rurl| {
            let name = rname.text().trim().to_string();
            let url = rurl.text().trim().to_string();
            if !C::remote_re().is_match(&name)
                || !(url.starts_with("http://") || url.starts_with("https://"))
            {
                ctx2.toast("Bitte Namen (a-z, 0-9, _ -) und URL angeben");
                return;
            }
            {
                let mut cfg = ctx2.cfg.borrow_mut();
                cfg.flatpak.remotes.retain(|r| r.name != name);
                cfg.flatpak.remotes.push(FlatpakRemote { name, url });
            }
            rname.set_text("");
            rurl.set_text("");
            refresh_remotes(&ctx2);
            ctx2.changed();
        });
    }
    remotes_grp.add(&rname);
    remotes_grp.add(&rurl);
    refresh_remotes(ctx);

    // ---- Distrobox
    let db = adw::PreferencesGroup::builder()
        .title("Distrobox")
        .description(
            "Andere Distributionen als Container im Terminal – mit Zugriff auf das Home-Verzeichnis. Die Container werden beim ersten Anmelden jedes Benutzers angelegt (braucht Internet).",
        )
        .build();
    page.add(&db);
    let distrobox_row = ctx.switch_row(
        &db,
        "Distrobox installieren",
        Some("Nutzt Podman (wird bei Bedarf mitinstalliert) oder Docker"),
        |cfg| cfg.distrobox.enabled,
        |cfg, v| cfg.distrobox.enabled = v,
    );
    ctx.shared.borrow_mut().distrobox_row = Some(distrobox_row);

    let boxes_grp = adw::PreferencesGroup::builder()
        .title("Container")
        .description("Starten mit 'distrobox enter &lt;name&gt;'")
        .build();
    let menu = gio::Menu::new();
    for (key, label, _image) in C::distrobox_presets() {
        if key == "arch" && !x86 {
            continue;
        }
        menu.append(Some(label), Some(&format!("win.box-preset::{key}")));
    }
    menu.append(Some("Eigener Container …"), Some("win.box-preset::custom"));
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("list-add-symbolic")
        .menu_model(&menu)
        .css_classes(["flat"])
        .tooltip_text("Container hinzufügen")
        .build();
    boxes_grp.set_header_suffix(Some(&menu_btn));
    page.add(&boxes_grp);
    ctx.shared.borrow_mut().boxes_grp = Some(boxes_grp);
    if ctx.window.lookup_action("box-preset").is_none() {
        let action = gio::SimpleAction::new("box-preset", Some(glib::VariantTy::STRING));
        let ctx2 = ctx.clone();
        action.connect_activate(move |_, v| {
            if let Some(key) = v.and_then(|v| v.str().map(|s| s.to_string())) {
                add_box_preset(&ctx2, &key);
            }
        });
        ctx.window.add_action(&action);
    }
    refresh_boxes(ctx);
    page
}

fn trash_button() -> gtk::Button {
    gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(["flat"])
        .valign(gtk::Align::Center)
        .build()
}

fn toggle_flatpak(ctx: &Rc<Ctx>, app_id: &str, active: bool) {
    let enable = {
        let mut cfg = ctx.cfg.borrow_mut();
        let apps = &mut cfg.flatpak.apps;
        if active && !apps.iter().any(|a| a == app_id) {
            apps.push(app_id.to_string());
        } else if !active {
            apps.retain(|a| a != app_id);
        }
        active && !cfg.flatpak.enabled
    };
    if enable {
        // set_active löst den Schalter-Handler aus, der cfg.flatpak.enabled setzt
        let row = ctx.shared.borrow().flatpak_row.clone();
        if let Some(row) = row {
            row.set_active(true);
        }
        ctx.toast("Flatpak wurde aktiviert");
    }
    ctx.changed();
}

fn refresh_flatpak_custom(ctx: &Rc<Ctx>, preset_ids: &Rc<HashSet<String>>) {
    let grp = ctx.shared.borrow().flatpak_custom_grp.clone();
    let Some(grp) = grp else { return };
    for row in std::mem::take(&mut ctx.shared.borrow_mut().flatpak_custom_rows) {
        grp.remove(&row);
    }
    let custom: Vec<String> = ctx
        .cfg
        .borrow()
        .flatpak
        .apps
        .iter()
        .filter(|a| !preset_ids.contains(*a))
        .cloned()
        .collect();
    let mut rows = Vec::new();
    for app in custom {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&app))
            .build();
        let rm = trash_button();
        let ctx2 = ctx.clone();
        let preset_ids = preset_ids.clone();
        rm.connect_clicked(move |_| {
            toggle_flatpak(&ctx2, &app, false);
            refresh_flatpak_custom(&ctx2, &preset_ids);
        });
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().flatpak_custom_rows = rows;
}

fn refresh_remotes(ctx: &Rc<Ctx>) {
    let grp = ctx.shared.borrow().remotes_grp.clone();
    let Some(grp) = grp else { return };
    for row in std::mem::take(&mut ctx.shared.borrow_mut().remote_rows) {
        grp.remove(&row);
    }
    let remotes = ctx.cfg.borrow().flatpak.remotes.clone();
    let mut rows = Vec::new();
    for (i, rem) in remotes.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&rem.name))
            .subtitle(glib::markup_escape_text(&rem.url))
            .build();
        let rm = trash_button();
        let ctx2 = ctx.clone();
        rm.connect_clicked(move |_| {
            ctx2.cfg.borrow_mut().flatpak.remotes.remove(i);
            refresh_remotes(&ctx2);
            ctx2.changed();
        });
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().remote_rows = rows;
}

fn refresh_boxes(ctx: &Rc<Ctx>) {
    let grp = ctx.shared.borrow().boxes_grp.clone();
    let Some(grp) = grp else { return };
    for row in std::mem::take(&mut ctx.shared.borrow_mut().box_rows) {
        grp.remove(&row);
    }
    let boxes = ctx.cfg.borrow().distrobox.boxes.clone();
    let mut rows = Vec::new();
    if boxes.is_empty() {
        let row = adw::ActionRow::builder()
            .title("Keine Container")
            .subtitle("Mit + eine Vorlage (Ubuntu, Fedora, Debian …) oder ein eigenes Image wählen")
            .build();
        grp.add(&row);
        rows.push(row);
    }
    for (i, b) in boxes.iter().enumerate() {
        let mut parts = vec![b.image.clone()];
        if !b.packages.is_empty() {
            parts.push(b.packages.clone());
        }
        if !b.export.is_empty() {
            parts.push(format!("Apps: {}", b.export));
        }
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&b.name))
            .subtitle(glib::markup_escape_text(&parts.join(" · ")))
            .build();
        let edit = gtk::Button::builder()
            .icon_name("document-edit-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        {
            let ctx2 = ctx.clone();
            edit.connect_clicked(move |_| box_dialog(&ctx2, Some(i)));
        }
        let rm = trash_button();
        {
            let ctx2 = ctx.clone();
            rm.connect_clicked(move |_| {
                ctx2.cfg.borrow_mut().distrobox.boxes.remove(i);
                refresh_boxes(&ctx2);
                ctx2.changed();
            });
        }
        row.add_suffix(&edit);
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().box_rows = rows;
}

fn add_box_preset(ctx: &Rc<Ctx>, key: &str) {
    if !ctx.cfg.borrow().distrobox.enabled {
        let row = ctx.shared.borrow().distrobox_row.clone();
        if let Some(row) = row {
            row.set_active(true);
        }
        ctx.toast("Distrobox wurde aktiviert");
    }
    if key == "custom" {
        box_dialog(ctx, None);
        return;
    }
    let Some((_, label, image)) = C::distrobox_presets()
        .into_iter()
        .find(|(k, _, _)| *k == key)
    else {
        return;
    };
    if ctx
        .cfg
        .borrow()
        .distrobox
        .boxes
        .iter()
        .any(|b| b.name == key)
    {
        ctx.toast(&format!("{label} ist bereits vorhanden"));
        return;
    }
    ctx.cfg.borrow_mut().distrobox.boxes.push(DistroboxBox {
        name: key.to_string(),
        image: image.to_string(),
        packages: String::new(),
        export: String::new(),
    });
    refresh_boxes(ctx);
    ctx.changed();
}

fn box_dialog(ctx: &Rc<Ctx>, idx: Option<usize>) {
    let b = idx
        .and_then(|i| ctx.cfg.borrow().distrobox.boxes.get(i).cloned())
        .unwrap_or_default();
    let heading = if idx.is_some() {
        "Distrobox-Container"
    } else {
        "Eigener Distrobox-Container"
    };
    let dlg = adw::AlertDialog::builder().heading(heading).build();
    let lst = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .width_request(480)
        .build();
    let name = adw::EntryRow::builder()
        .title("Name (a-z, 0-9, _ . -)")
        .text(b.name.clone())
        .build();
    let image = adw::EntryRow::builder()
        .title("Image, z. B. docker.io/library/ubuntu:24.04")
        .text(b.image.clone())
        .build();
    let pkgs = adw::EntryRow::builder()
        .title("Zusätzliche Pakete im Container (optional)")
        .text(b.packages.clone())
        .build();
    let export = adw::EntryRow::builder()
        .title("Apps ins Menü übernehmen (optional)")
        .text(b.export.clone())
        .build();
    for w in [&name, &image, &pkgs, &export] {
        lst.append(w);
    }
    dlg.set_extra_child(Some(&lst));
    dlg.add_response("cancel", "Abbrechen");
    dlg.add_response("ok", "Speichern");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);

    let ctx2 = ctx.clone();
    dlg.connect_response(None, move |_dlg, response| {
        if response != "ok" {
            return;
        }
        let new = DistroboxBox {
            name: name.text().trim().to_string(),
            image: image.text().trim().to_string(),
            packages: pkgs.text().trim().to_string(),
            export: export.text().trim().to_string(),
        };
        if !C::box_re().is_match(&new.name) {
            ctx2.toast("Ungültiger Name (a-z, 0-9, _ . -)");
            return;
        }
        if !C::image_name_valid(&new.image) {
            ctx2.toast("Ungültiger Image-Name");
            return;
        }
        let taken = ctx2
            .cfg
            .borrow()
            .distrobox
            .boxes
            .iter()
            .enumerate()
            .any(|(j, other)| Some(j) != idx && other.name == new.name);
        if taken {
            ctx2.toast("Der Name ist bereits vergeben");
            return;
        }
        match idx {
            None => ctx2.cfg.borrow_mut().distrobox.boxes.push(new),
            Some(i) => ctx2.cfg.borrow_mut().distrobox.boxes[i] = new,
        }
        refresh_boxes(&ctx2);
        ctx2.changed();
    });
    dlg.present(Some(&ctx.window));
}
