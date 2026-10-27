use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config as C;
use gtk::glib;

use crate::ctx::{Ctx, ARCH_ASCII, LOGO_MODES};

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let logo_grp = adw::PreferencesGroup::builder()
        .title("Logo")
        .description("Wird in fastfetch/neofetch, im Installer und als Systemlogo (os-release LOGO) verwendet.")
        .build();
    page.add(&logo_grp);
    ctx.file_row(
        &logo_grp,
        "Logo-Bild",
        "PNG, JPG oder SVG – quadratisch empfohlen",
        |cfg| cfg.branding.logo.clone(),
        |cfg, v| cfg.branding.logo = v,
    );
    let modes: Vec<(String, String)> = LOGO_MODES.iter().map(|(v, l)| (v.to_string(), l.to_string())).collect();
    ctx.combo_row(
        &logo_grp,
        "Logo in fastfetch / neofetch",
        &modes,
        |cfg| cfg.branding.logo_mode.clone(),
        |cfg, v| cfg.branding.logo_mode = v,
        None,
        false,
    );

    let color_row = adw::ActionRow::builder()
        .title("Akzentfarbe")
        .subtitle("ANSI_COLOR und Farbe der fastfetch-Schlüssel")
        .build();
    let rgba = gdk::RGBA::parse(&ctx.cfg.borrow().branding.color).unwrap_or(gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
    let dialog = gtk::ColorDialog::builder().with_alpha(false).build();
    let color_btn = gtk::ColorDialogButton::builder().dialog(&dialog).rgba(&rgba).valign(gtk::Align::Center).build();
    {
        let ctx2 = ctx.clone();
        color_btn.connect_rgba_notify(move |b| {
            let c = b.rgba();
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (c.red() * 255.0).round() as u8,
                (c.green() * 255.0).round() as u8,
                (c.blue() * 255.0).round() as u8,
            );
            ctx2.cfg.borrow_mut().branding.color = hex;
            ctx2.changed();
        });
    }
    color_row.add_suffix(&color_btn);
    logo_grp.add(&color_row);

    let ascii_grp = adw::PreferencesGroup::builder()
        .title("ASCII-Logo")
        .description(
            "Wird für den Modus „Eigenes ASCII-Art“ genutzt. Tipp: Mit 'figlet' oder 'toilet' erzeugen.",
        )
        .build();
    page.add(&ascii_grp);
    ctx.textview_row(
        &ascii_grp,
        |cfg| cfg.branding.ascii_logo.clone(),
        |cfg, v| cfg.branding.ascii_logo = v,
        140,
        true,
    );

    let prev_grp = adw::PreferencesGroup::builder().title("Vorschau").description("So ungefähr sieht fastfetch aus").build();
    page.add(&prev_grp);
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    box_.add_css_class("ff-preview");
    let prev_pic = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .width_request(160)
        .height_request(160)
        .valign(gtk::Align::Start)
        .build();
    let prev_ascii = gtk::Label::builder().xalign(0.0).yalign(0.0).valign(gtk::Align::Start).build();
    let prev_info = gtk::Label::builder().xalign(0.0).yalign(0.0).valign(gtk::Align::Start).selectable(true).build();
    box_.append(&prev_pic);
    box_.append(&prev_ascii);
    box_.append(&prev_info);
    prev_grp.add(&box_);
    {
        let mut shared = ctx.shared.borrow_mut();
        shared.prev_pic = Some(prev_pic);
        shared.prev_ascii = Some(prev_ascii);
        shared.prev_info = Some(prev_info);
    }

    let wall_grp = adw::PreferencesGroup::builder()
        .title("Hintergrundbild")
        .description(
            "Wird beim ersten Login gesetzt (KDE, GNOME, Xfce, Cinnamon, LXQt) und als Bootmenü-Hintergrund (BIOS) genutzt.",
        )
        .build();
    page.add(&wall_grp);
    ctx.file_row(
        &wall_grp,
        "Hintergrundbild",
        "Am besten 1920×1080 oder größer",
        |cfg| cfg.branding.wallpaper.clone(),
        |cfg, v| cfg.branding.wallpaper = v,
    );

    let term_grp = adw::PreferencesGroup::builder().title("Terminal").build();
    page.add(&term_grp);
    ctx.switch_row(
        &term_grp,
        "fastfetch beim Öffnen eines Terminals anzeigen",
        Some("'neofetch' ist zusätzlich als Alias für fastfetch verfügbar"),
        |cfg| cfg.branding.fastfetch_on_terminal,
        |cfg, v| cfg.branding.fastfetch_on_terminal = v,
    );
    let motd_grp = adw::PreferencesGroup::builder()
        .title("Begrüßungstext (/etc/motd)")
        .description("Wird nach dem Konsolen-Login angezeigt")
        .build();
    page.add(&motd_grp);
    ctx.textview_row(&motd_grp, |cfg| cfg.branding.motd.clone(), |cfg, v| cfg.branding.motd = v, 70, false);

    update_preview(ctx);
    page
}

/// Mirrors ui.py's `update_preview`.
pub fn update_preview(ctx: &Rc<Ctx>) {
    let shared = ctx.shared.borrow();
    let (Some(prev_pic), Some(prev_ascii), Some(prev_info)) = (&shared.prev_pic, &shared.prev_ascii, &shared.prev_info)
    else {
        return;
    };
    let cfg = ctx.cfg.borrow();
    let (b, g) = (&cfg.branding, &cfg.general);
    let color = &b.color;
    let mode = b.logo_mode.as_str();
    let show_img = matches!(mode, "image" | "chafa") && !b.logo.is_empty() && std::path::Path::new(&b.logo).is_file();
    prev_pic.set_visible(show_img);
    if show_img {
        prev_pic.set_filename(Some(&b.logo));
    }
    let ascii_text = if mode == "ascii" { b.ascii_logo.clone() } else if !show_img { ARCH_ASCII.to_string() } else { String::new() };
    prev_ascii.set_visible(!ascii_text.is_empty());
    let ascii_color = if mode == "ascii" { color.as_str() } else { "#1793d1" };
    prev_ascii.set_markup(&format!(
        "<span foreground=\"{}\">{}</span>",
        ascii_color,
        glib::markup_escape_text(&ascii_text)
    ));

    let desktop = C::desktop_info(&cfg).label.to_string();
    let kernel = C::kernels().into_iter().find(|(k, _)| *k == cfg.packages.kernel).map(|(_, l)| l).unwrap_or("linux");
    let host = if g.hostname.is_empty() { "host".to_string() } else { g.hostname.clone() };
    let host_esc = glib::markup_escape_text(&host);
    let mut lines = vec![
        format!(
            "<span foreground=\"{color}\"><b>user</b></span>@<span foreground=\"{color}\"><b>{host_esc}</b></span>"
        ),
        "-".repeat(5 + host.chars().count()),
    ];
    let pkg_count = C::target_packages(&cfg).len();
    for (k, v) in [
        ("OS", format!("{} {} x86_64", g.name, g.version)),
        ("Kernel", format!("{kernel} 6.x")),
        ("Packages", format!("{pkg_count}+ (pacman)")),
        ("Shell", "bash 5.3".to_string()),
        ("DE", desktop),
        ("Terminal", "konsole".to_string()),
        ("CPU", "AMD Ryzen 7 (16)".to_string()),
        ("Memory", "2.1 GiB / 15.5 GiB".to_string()),
    ] {
        lines.push(format!(
            "<span foreground=\"{color}\"><b>{k}</b></span>: {}",
            glib::markup_escape_text(&v)
        ));
    }
    let palette: String = ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6"]
        .iter()
        .map(|c| format!("<span background=\"{c}\">   </span>"))
        .collect();
    lines.push(String::new());
    lines.push(palette);
    prev_info.set_markup(&lines.join("\n"));
}
