use std::rc::Rc;

use adw::prelude::*;

use distro_builder_core::config as C;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    if C::is_image(&ctx.cfg.borrow()) {
        return image_page(ctx);
    }
    let page = adw::PreferencesPage::new();

    // ---- Art des Mediums
    let mode_grp = adw::PreferencesGroup::builder()
        .title("Art des Mediums")
        .description(
            "„Nur Live-System“ startet direkt vom USB-Stick ohne Installer – z. B. für eine Rettungs-, Kiosk-, Präsentations- oder Gaming-ISO.",
        )
        .build();
    page.add(&mode_grp);
    let modes = vec![
        (
            "installer".to_string(),
            "Live-System mit grafischem Installer".to_string(),
        ),
        ("live".to_string(), "Nur Live-System".to_string()),
    ];
    let mode_row = ctx.combo_row(
        &mode_grp,
        "Medium",
        &modes,
        |cfg| {
            if cfg.installer.enabled {
                "installer".into()
            } else {
                "live".into()
            }
        },
        |cfg, v| {
            cfg.installer.enabled = v == "installer";
            // Der Installer läuft per „sudo -n“ – dafür muss sudo ohne Passwort gehen
            if cfg.installer.enabled {
                cfg.live.sudo = "nopasswd".into();
            }
        },
        None,
        false,
    );
    ctx.switch_row(
        &mode_grp,
        "zram-Swap aktivieren",
        Some("Komprimierter Swap im RAM"),
        |cfg| cfg.installer.zram,
        |cfg, v| cfg.installer.zram = v,
    );

    // ---- Live-Benutzer
    let live_grp = adw::PreferencesGroup::builder()
        .title("Live-Benutzer")
        .description(
            "Mit diesem Konto startet das Live-System. Das Passwort wird nur als SHA-512-Hash gespeichert – weder im Projekt noch in der ISO steht es im Klartext.",
        )
        .build();
    page.add(&live_grp);
    ctx.entry_row(
        &live_grp,
        "Benutzername",
        |cfg| cfg.live.username.clone(),
        |cfg, v| cfg.live.username = v,
        None,
    );
    let pw_status = adw::ActionRow::builder()
        .title("Passwort")
        .css_classes(["property"])
        .build();
    let pw_clear = gtk::Button::builder()
        .label("Entfernen")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    pw_status.add_suffix(&pw_clear);
    let update_status: Rc<dyn Fn()> = {
        let ctx2 = ctx.clone();
        let pw_status = pw_status.clone();
        let pw_clear = pw_clear.clone();
        Rc::new(move || {
            let set = !ctx2.cfg.borrow().live.password_hash.is_empty();
            pw_status.set_subtitle(if set {
                "gesetzt (gespeichert als SHA-512-Hash)"
            } else {
                "keins – Anmeldung ohne Passwort"
            });
            pw_clear.set_visible(set);
        })
    };
    update_status();
    let pw_entry = adw::PasswordEntryRow::builder()
        .title("Neues Passwort setzen")
        .show_apply_button(true)
        .build();
    {
        let ctx2 = ctx.clone();
        let update_status = update_status.clone();
        pw_entry.connect_apply(move |r| {
            let text = r.text().to_string();
            if text.is_empty() {
                ctx2.toast("Kein Passwort eingegeben – zum Entfernen „Entfernen“ nutzen");
                return;
            }
            ctx2.cfg.borrow_mut().live.password_hash =
                distro_builder_core::passwd::hash_password(&text);
            r.set_text("");
            update_status();
            ctx2.toast("Live-Passwort gesetzt");
            ctx2.changed();
        });
    }
    {
        let ctx2 = ctx.clone();
        let update_status = update_status.clone();
        pw_clear.connect_clicked(move |_| {
            ctx2.cfg.borrow_mut().live.password_hash.clear();
            update_status();
            ctx2.changed();
        });
    }
    live_grp.add(&pw_entry);
    live_grp.add(&pw_status);
    let sudo_modes: Vec<(String, String)> = C::sudo_modes()
        .into_iter()
        .map(|(k, l)| (k.to_string(), l.to_string()))
        .collect();
    let sudo_row = ctx.combo_row(
        &live_grp,
        "Administratorrechte",
        &sudo_modes,
        |cfg| cfg.live.sudo.clone(),
        |cfg, v| cfg.live.sudo = v,
        Some("Mit Installer immer „sudo ohne Passwort“"),
        false,
    );
    ctx.switch_row(
        &live_grp,
        "Automatisch anmelden",
        Some("Desktop bzw. Konsole startet ohne Anmeldemaske"),
        |cfg| cfg.live.autologin,
        |cfg, v| cfg.live.autologin = v,
    );
    ctx.switch_row(
        &live_grp,
        "Komplett in den RAM laden (copytoram)",
        Some("Startet langsamer, danach kann der USB-Stick entfernt werden"),
        |cfg| cfg.live.copytoram,
        |cfg, v| cfg.live.copytoram = v,
    );

    let grp = adw::PreferencesGroup::builder()
        .title("Grafischer Installer")
        .description(
            "Ein eigener GTK-Installer: Festplatte wählen, Benutzer anlegen, fertig. Er kopiert das System offline von der ISO oder installiert es aus dem Internet und übernimmt dein Branding. Benötigt eine Desktop-Umgebung.",
        )
        .build();
    page.add(&grp);
    ctx.switch_row(
        &grp,
        "Installer beim Start des Live-Systems öffnen",
        None,
        |cfg| cfg.installer.autostart,
        |cfg, v| cfg.installer.autostart = v,
    );

    let src = adw::PreferencesGroup::builder()
        .title("Installation")
        .description(
            "Offline kopiert der Installer das fertige System direkt von der ISO – ohne Internet, inklusive aller Pakete, KI-Modelle und Images.",
        )
        .build();
    page.add(&src);
    let sources = vec![
        ("offline".to_string(), "Von der ISO (offline)".to_string()),
        (
            "online".to_string(),
            "Aus dem Internet (pacstrap)".to_string(),
        ),
    ];
    ctx.combo_row(
        &src,
        "Standard-Installationsquelle",
        &sources,
        |cfg| cfg.installer.source.clone(),
        |cfg, v| cfg.installer.source = v,
        None,
        false,
    );
    ctx.switch_row(
        &src,
        "Online-Installation zur Auswahl anbieten",
        None,
        |cfg| cfg.installer.allow_online,
        |cfg, v| cfg.installer.allow_online = v,
    );
    ctx.switch_row(
        &src,
        "Verschlüsselung (LUKS2) vorauswählen",
        Some("Der Benutzer kann es im Installer trotzdem ändern"),
        |cfg| cfg.installer.encrypt,
        |cfg, v| cfg.installer.encrypt = v,
    );
    let loaders = vec![
        ("grub".to_string(), "GRUB".to_string()),
        (
            "systemd-boot".to_string(),
            "systemd-boot (nur UEFI)".to_string(),
        ),
    ];
    ctx.combo_row(
        &src,
        "Standard-Bootloader",
        &loaders,
        |cfg| cfg.installer.bootloader.clone(),
        |cfg, v| cfg.installer.bootloader = v,
        None,
        false,
    );

    let swap = adw::SpinRow::new(
        Some(&gtk::Adjustment::new(0.0, 0.0, 128.0, 1.0, 1.0, 0.0)),
        1.0,
        0,
    );
    swap.set_title("Standard-Swap-Datei (GB)");
    swap.set_subtitle("0 = keine (zram reicht meist)");
    swap.set_value(ctx.cfg.borrow().installer.swap_gb as f64);
    {
        let ctx2 = ctx.clone();
        swap.connect_value_notify(move |r| {
            ctx2.cfg.borrow_mut().installer.swap_gb = r.value() as u32;
        });
    }
    src.add(&swap);

    let d = adw::PreferencesGroup::builder()
        .title("Standardwerte")
        .description("Vorauswahl im Installer, gilt auch für die Tastatur im Live-System")
        .build();
    page.add(&d);
    region_rows(ctx, &d);
    let filesystems = vec![
        ("ext4".to_string(), "ext4".to_string()),
        (
            "btrfs".to_string(),
            "btrfs (Subvolumes, Kompression)".to_string(),
        ),
    ];
    ctx.combo_row(
        &d,
        "Dateisystem",
        &filesystems,
        |cfg| cfg.installer.filesystem.clone(),
        |cfg, v| cfg.installer.filesystem = v,
        None,
        false,
    );

    let w = adw::PreferencesGroup::builder()
        .title("Willkommenstext")
        .build();
    page.add(&w);
    ctx.textview_row(
        &w,
        |cfg| cfg.installer.welcome.clone(),
        |cfg, v| cfg.installer.welcome = v,
        70,
        false,
    );

    let installer_groups = [grp, src, w];
    let sync: Rc<dyn Fn()> = {
        let ctx2 = ctx.clone();
        Rc::new(move || {
            let with_installer = ctx2.cfg.borrow().installer.enabled;
            for g in &installer_groups {
                g.set_visible(with_installer);
            }
            // sudo-Wahl nur ohne Installer frei (der Installer braucht sudo ohne Passwort)
            sudo_row.set_sensitive(!with_installer);
            let idx = C::sudo_modes()
                .iter()
                .position(|(k, _)| *k == ctx2.cfg.borrow().live.sudo)
                .unwrap_or(0);
            if sudo_row.selected() as usize != idx {
                sudo_row.set_selected(idx as u32);
            }
        })
    };
    sync();
    mode_row.connect_selected_notify(move |_| sync());
    page
}

/// Sprache, Zeitzone und Tastatur (Installer-Vorgabe bzw. Einstellung des Images).
fn region_rows(ctx: &Rc<Ctx>, d: &adw::PreferencesGroup) {
    let locales: Vec<(String, String)> =
        ctx.locales.iter().map(|l| (l.clone(), l.clone())).collect();
    ctx.combo_row(
        d,
        "Sprache",
        &locales,
        |cfg| cfg.installer.locale.clone(),
        |cfg, v| cfg.installer.locale = v,
        None,
        true,
    );
    let timezones: Vec<(String, String)> = ctx
        .timezones
        .iter()
        .map(|l| (l.clone(), l.clone()))
        .collect();
    ctx.combo_row(
        d,
        "Zeitzone",
        &timezones,
        |cfg| cfg.installer.timezone.clone(),
        |cfg, v| cfg.installer.timezone = v,
        None,
        true,
    );
    let layouts: Vec<(String, String)> =
        ctx.layouts.iter().map(|l| (l.clone(), l.clone())).collect();
    ctx.combo_row(
        d,
        "Tastaturlayout",
        &layouts,
        |cfg| cfg.installer.keyboard.clone(),
        |cfg, v| cfg.installer.keyboard = v,
        None,
        true,
    );
}

/// ARM/Raspberry Pi: kein Installer, sondern ein fertig eingerichtetes Image.
fn image_page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let label = C::arch_info(&ctx.cfg.borrow()).label;
    let grp = adw::PreferencesGroup::builder()
        .title("Fertiges System")
        .description(format!(
            "Für {label} gibt es keinen Installer: Das Image enthält ein fertig eingerichtetes System. Beim ersten Start wächst die Partition auf die Größe der SD-Karte bzw. des Datenträgers."
        ))
        .build();
    page.add(&grp);
    ctx.entry_row(
        &grp,
        "Benutzername",
        |cfg| cfg.image.username.clone(),
        |cfg, v| cfg.image.username = v,
        None,
    );
    let pw = adw::PasswordEntryRow::builder()
        .title("Startpasswort")
        .text(ctx.cfg.borrow().image.password.clone())
        .build();
    {
        let ctx2 = ctx.clone();
        pw.connect_changed(move |r| {
            ctx2.cfg.borrow_mut().image.password = r.text().to_string();
            ctx2.changed();
        });
    }
    grp.add(&pw);
    ctx.switch_row(
        &grp,
        "Passwort beim ersten Anmelden ändern",
        None,
        |cfg| cfg.image.expire_password,
        |cfg, v| cfg.image.expire_password = v,
    );
    ctx.switch_row(
        &grp,
        "Automatisch anmelden",
        Some("Nur mit Desktop-Umgebung"),
        |cfg| cfg.image.autologin,
        |cfg, v| cfg.image.autologin = v,
    );
    ctx.switch_row(
        &grp,
        "zram-Swap aktivieren",
        Some("Komprimierter Swap im RAM"),
        |cfg| cfg.installer.zram,
        |cfg, v| cfg.installer.zram = v,
    );
    let size = adw::SpinRow::new(
        Some(&gtk::Adjustment::new(8.0, 3.0, 256.0, 1.0, 1.0, 0.0)),
        1.0,
        0,
    );
    size.set_title("Image-Größe (GB)");
    size.set_subtitle("Platz für System, Modelle und Apps – wächst beim ersten Start automatisch");
    size.set_value(ctx.cfg.borrow().image.size_gb as f64);
    {
        let ctx2 = ctx.clone();
        size.connect_value_notify(move |r| {
            ctx2.cfg.borrow_mut().image.size_gb = r.value() as u32;
            ctx2.changed();
        });
    }
    grp.add(&size);

    let d = adw::PreferencesGroup::builder()
        .title("Region")
        .description("Sprache, Zeitzone und Tastatur des Systems")
        .build();
    page.add(&d);
    region_rows(ctx, &d);
    page
}
