use std::rc::Rc;

use adw::prelude::*;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let grp = adw::PreferencesGroup::builder()
        .title("Grafischer Installer")
        .description(
            "Ein eigener GTK-Installer: Festplatte wählen, Benutzer anlegen, fertig. Er installiert die Pakete aus dem Internet (pacstrap) und übernimmt dein Branding. Benötigt eine Desktop-Umgebung.",
        )
        .build();
    page.add(&grp);
    ctx.switch_row(
        &grp,
        "Installer einbinden",
        None,
        |cfg| cfg.installer.enabled,
        |cfg, v| cfg.installer.enabled = v,
    );
    ctx.switch_row(
        &grp,
        "Installer beim Start des Live-Systems öffnen",
        None,
        |cfg| cfg.installer.autostart,
        |cfg, v| cfg.installer.autostart = v,
    );
    ctx.switch_row(
        &grp,
        "zram-Swap aktivieren",
        Some("Komprimierter Swap im RAM"),
        |cfg| cfg.installer.zram,
        |cfg, v| cfg.installer.zram = v,
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
    let locales: Vec<(String, String)> =
        ctx.locales.iter().map(|l| (l.clone(), l.clone())).collect();
    ctx.combo_row(
        &d,
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
        &d,
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
        &d,
        "Tastaturlayout",
        &layouts,
        |cfg| cfg.installer.keyboard.clone(),
        |cfg, v| cfg.installer.keyboard = v,
        None,
        true,
    );
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

    page
}
