//! The installer wizard window: welcome → region → disk → options → user →
//! summary → progress → done. Faithful port of `installer.py`'s `Installer`
//! class.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::installer_conf::{ExtraDisk, InstallerConf, Selection};
use distro_builder_core::profile::INSTALLER_LIB;
use gtk::glib;

use crate::sysinfo::{self, DISK_ROLES};

const PAGES: &[&str] = &[
    "welcome", "region", "disk", "options", "user", "summary", "progress", "done",
];

fn make_combo(title: &str, items: &[String], selected: &str) -> adw::ComboRow {
    let strs: Vec<&str> = items.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&strs);
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&model)
        .enable_search(true)
        .build();
    let expr = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    );
    row.set_expression(Some(expr));
    if let Some(idx) = items.iter().position(|i| i == selected) {
        row.set_selected(idx as u32);
    }
    row
}

fn combo_value(row: &adw::ComboRow) -> String {
    row.selected_item()
        .and_then(|i| i.downcast::<gtk::StringObject>().ok())
        .map(|s| s.string().to_string())
        .unwrap_or_default()
}

/// Mutable wizard state plus every widget a later page needs to read from.
pub struct Wizard {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    stack: gtk::Stack,
    back_btn: gtk::Button,
    next_btn: gtk::Button,
    index: Cell<usize>,
    conf: InstallerConf,

    locale_row: adw::ComboRow,
    tz_row: adw::ComboRow,
    kb_row: adw::ComboRow,

    selected_disk: RefCell<Option<String>>,
    disk_checks: RefCell<Vec<(String, gtk::CheckButton)>>,
    extra_rows: RefCell<HashMap<String, adw::ComboRow>>,
    disk_hint_row: adw::ActionRow,

    fs_row: adw::ComboRow,
    swap_row: adw::SpinRow,
    encrypt_row: adw::SwitchRow,
    luks_same_row: adw::SwitchRow,
    luks1: adw::PasswordEntryRow,
    luks2: adw::PasswordEntryRow,
    loader_row: adw::ComboRow,
    loader_keys: Vec<String>,
    source_row: adw::ComboRow,
    source_keys: Vec<String>,

    fullname: adw::EntryRow,
    username: adw::EntryRow,
    hostname: adw::EntryRow,
    pw1: adw::PasswordEntryRow,
    pw2: adw::PasswordEntryRow,
    root_same: adw::SwitchRow,
    autologin: adw::SwitchRow,
    auto_user: RefCell<String>,

    summary_group: adw::PreferencesGroup,
    summary_rows: RefCell<Vec<adw::ActionRow>>,

    status_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    log_view: gtk::TextView,
    done_page: adw::StatusPage,
}

pub fn build(app: &adw::Application) {
    let conf = sysinfo::load_conf();
    let d = conf.defaults.clone();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(format!("{} installieren", conf.name))
        .default_width(760)
        .default_height(640)
        .build();

    let header = adw::HeaderBar::new();
    let back_btn = gtk::Button::with_label("Zurück");
    let next_btn = gtk::Button::builder()
        .label("Weiter")
        .css_classes(["suggested-action"])
        .build();
    header.pack_start(&back_btn);
    header.pack_end(&next_btn);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .build();
    view.set_content(Some(&stack));
    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&view));
    window.set_content(Some(&toast_overlay));

    // ---------- welcome ----------
    let welcome = adw::StatusPage::builder()
        .title(format!("Willkommen bei {}", conf.name))
        .description(if conf.welcome.is_empty() {
            "Dieser Assistent installiert das System.".to_string()
        } else {
            conf.welcome.clone()
        })
        .build();
    if !conf.logo.is_empty() && std::path::Path::new(&conf.logo).exists() {
        if let Ok(tex) = gdk::Texture::from_filename(&conf.logo) {
            welcome.set_paintable(Some(&tex));
        } else {
            welcome.set_icon_name(Some("system-software-install-symbolic"));
        }
    } else {
        welcome.set_icon_name(Some("system-software-install-symbolic"));
    }
    let net_label = gtk::Label::builder().css_classes(["dim-label"]).build();
    welcome.set_child(Some(&net_label));
    {
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let ok = Command::new("ping")
                .args(["-c1", "-W2", "archlinux.org"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            let _ = tx.send_blocking(ok);
        });
        glib::spawn_future_local(glib::clone!(
            #[strong]
            net_label,
            async move {
                if let Ok(ok) = rx.recv().await {
                    net_label.set_label(if ok {
                        "\u{2714} Internetverbindung vorhanden"
                    } else {
                        "Keine Internetverbindung – die Offline-Installation von der ISO funktioniert trotzdem."
                    });
                }
            }
        ));
    }
    stack.add_named(&welcome, Some("welcome"));

    // ---------- region ----------
    let region_page = adw::PreferencesPage::builder().title("Region").build();
    let region_grp = adw::PreferencesGroup::builder()
        .title("Sprache und Region")
        .description("Diese Einstellungen gelten für das installierte System.")
        .build();
    let locale_row = make_combo("Sprache", &sysinfo::list_locales(), &d.locale);
    let tz_row = make_combo("Zeitzone", &sysinfo::list_timezones(), &d.timezone);
    let kb_row = make_combo("Tastaturlayout", &sysinfo::list_layouts(), &d.keyboard);
    region_grp.add(&locale_row);
    region_grp.add(&tz_row);
    region_grp.add(&kb_row);
    region_page.add(&region_grp);
    let test_grp = adw::PreferencesGroup::builder()
        .title("Tastatur testen")
        .build();
    test_grp.add(&adw::EntryRow::builder().title("Hier tippen …").build());
    region_page.add(&test_grp);
    kb_row.connect_selected_notify(|row| {
        let layout = combo_value(row);
        if !layout.is_empty() {
            let _ = Command::new("setxkbmap")
                .arg(&layout)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    });
    stack.add_named(&region_page, Some("region"));

    // ---------- disk ----------
    let disk_page = adw::PreferencesPage::builder().title("Festplatten").build();
    let disk_grp = adw::PreferencesGroup::builder()
        .title("Systemfestplatte")
        .description("\u{26A0} Die gewählte Festplatte wird vollständig gelöscht!")
        .build();
    let disks = sysinfo::list_disks();
    let mut disk_checks = Vec::new();
    let mut first_check: Option<gtk::CheckButton> = None;
    for dev in &disks {
        let check = gtk::CheckButton::new();
        if let Some(first) = &first_check {
            check.set_group(Some(first));
        } else {
            first_check = Some(check.clone());
        }
        let row = adw::ActionRow::builder()
            .title(format!(
                "{} ({})",
                dev.model
                    .clone()
                    .unwrap_or_else(|| "Datenträger".to_string()),
                sysinfo::human(dev.size)
            ))
            .subtitle(format!(
                "{}  ·  {}",
                dev.path,
                dev.tran.clone().unwrap_or_default().to_uppercase()
            ))
            .build();
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        disk_grp.add(&row);
        disk_checks.push((dev.path.clone(), check));
    }
    if disks.is_empty() {
        disk_grp.add(
            &adw::ActionRow::builder()
                .title("Keine geeignete Festplatte gefunden (mind. 8 GB).")
                .build(),
        );
    }
    disk_page.add(&disk_grp);

    let extra_grp = adw::PreferencesGroup::builder()
        .title("Weitere Festplatten")
        .description(
            "Optional: zusätzliche Platten als /home, als Datenlaufwerk (/data) oder als btrfs-Spiegel (RAID 1) der \
             Systemplatte nutzen. Genutzte Platten werden gelöscht.",
        )
        .build();
    let role_labels: Vec<String> = DISK_ROLES.iter().map(|(_, l)| l.to_string()).collect();
    let mut extra_rows = HashMap::new();
    for dev in &disks {
        let model =
            gtk::StringList::new(&role_labels.iter().map(String::as_str).collect::<Vec<_>>());
        let row = adw::ComboRow::builder()
            .title(format!(
                "{} ({})",
                dev.model
                    .clone()
                    .unwrap_or_else(|| "Datenträger".to_string()),
                sysinfo::human(dev.size)
            ))
            .subtitle(dev.path.clone())
            .model(&model)
            .build();
        extra_grp.add(&row);
        extra_rows.insert(dev.path.clone(), row);
    }
    extra_grp.set_visible(disks.len() >= 2);
    disk_page.add(&extra_grp);
    let hint_grp = adw::PreferencesGroup::new();
    let disk_hint_row = adw::ActionRow::builder().css_classes(["property"]).build();
    hint_grp.add(&disk_hint_row);
    disk_page.add(&hint_grp);
    stack.add_named(&disk_page, Some("disk"));

    // ---------- options ----------
    let uefi = sysinfo::is_uefi();
    let options_page = adw::PreferencesPage::builder().title("Optionen").build();
    let fs_grp = adw::PreferencesGroup::builder()
        .title("Dateisystem und Speicher")
        .build();
    let fs_row = make_combo(
        "Dateisystem",
        &["ext4".to_string(), "btrfs".to_string()],
        &d.filesystem,
    );
    fs_row.set_subtitle("btrfs: Subvolumes, Kompression, Snapshots, RAID");
    let swap_row = adw::SpinRow::with_range(0.0, 128.0, 1.0);
    swap_row.set_title("Swap-Datei (GB)");
    swap_row.set_subtitle(if conf.zram {
        "0 = keine Swap-Datei (zram ist aktiv)"
    } else {
        "0 = keine Swap-Datei"
    });
    swap_row.set_value(d.swap_gb as f64);
    fs_grp.add(&fs_row);
    fs_grp.add(&swap_row);
    options_page.add(&fs_grp);

    let enc_grp = adw::PreferencesGroup::builder()
        .title("Verschlüsselung")
        .description(
            "Vollständige Festplattenverschlüsselung mit LUKS2. Beim Start wird einmal das Passwort abgefragt – auch \
             für weitere Platten.",
        )
        .build();
    let encrypt_row = adw::SwitchRow::builder()
        .title("Festplatten verschlüsseln")
        .active(d.encrypt)
        .build();
    let luks_same_row = adw::SwitchRow::builder()
        .title("Benutzerpasswort verwenden")
        .active(true)
        .build();
    let luks1 = adw::PasswordEntryRow::builder()
        .title("Verschlüsselungspasswort")
        .build();
    let luks2 = adw::PasswordEntryRow::builder()
        .title("Passwort wiederholen")
        .build();
    enc_grp.add(&encrypt_row);
    enc_grp.add(&luks_same_row);
    enc_grp.add(&luks1);
    enc_grp.add(&luks2);
    options_page.add(&enc_grp);
    {
        let sync = {
            let encrypt_row = encrypt_row.clone();
            let luks_same_row = luks_same_row.clone();
            let luks1 = luks1.clone();
            let luks2 = luks2.clone();
            move || {
                let on = encrypt_row.is_active();
                luks_same_row.set_visible(on);
                luks1.set_visible(on && !luks_same_row.is_active());
                luks2.set_visible(on && !luks_same_row.is_active());
            }
        };
        sync();
        encrypt_row.connect_active_notify(glib::clone!(
            #[strong]
            sync,
            move |_| sync()
        ));
        luks_same_row.connect_active_notify(glib::clone!(
            #[strong]
            sync,
            move |_| sync()
        ));
    }

    let boot_grp = adw::PreferencesGroup::builder().title("System").build();
    let mut loaders = vec![
        ("grub".to_string(), "GRUB".to_string()),
        (
            "systemd-boot".to_string(),
            "systemd-boot (schlank, nur UEFI)".to_string(),
        ),
    ];
    if !uefi {
        loaders.truncate(1);
    }
    let loader_keys: Vec<String> = loaders.iter().map(|(k, _)| k.clone()).collect();
    let loader_labels: Vec<String> = loaders.iter().map(|(_, l)| l.clone()).collect();
    let loader_row = adw::ComboRow::builder()
        .title("Bootloader")
        .model(&gtk::StringList::new(
            &loader_labels.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .build();
    if let Some(idx) = loader_keys.iter().position(|k| k == &d.bootloader) {
        loader_row.set_selected(idx as u32);
    }
    boot_grp.add(&loader_row);
    boot_grp.add(
        &adw::ActionRow::builder()
            .title("Startmodus")
            .subtitle(if uefi { "UEFI" } else { "BIOS (Legacy)" })
            .css_classes(["property"])
            .build(),
    );
    let mut sources = Vec::new();
    if sysinfo::has_offline_image() {
        sources.push((
            "offline".to_string(),
            "Von der ISO (offline, schnell)".to_string(),
        ));
    }
    if conf.allow_online || sources.is_empty() {
        sources.push((
            "online".to_string(),
            "Aus dem Internet (neueste Pakete)".to_string(),
        ));
    }
    let source_keys: Vec<String> = sources.iter().map(|(k, _)| k.clone()).collect();
    let source_labels: Vec<String> = sources.iter().map(|(_, l)| l.clone()).collect();
    let source_row = adw::ComboRow::builder()
        .title("Installationsquelle")
        .model(&gtk::StringList::new(
            &source_labels.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .build();
    if let Some(idx) = source_keys.iter().position(|k| k == &d.source) {
        source_row.set_selected(idx as u32);
    }
    boot_grp.add(&source_row);
    options_page.add(&boot_grp);
    stack.add_named(&options_page, Some("options"));

    // ---------- user ----------
    let user_page = adw::PreferencesPage::builder().title("Benutzer").build();
    let user_grp = adw::PreferencesGroup::builder()
        .title("Dein Benutzerkonto")
        .build();
    let fullname = adw::EntryRow::builder().title("Vollständiger Name").build();
    let username = adw::EntryRow::builder().title("Benutzername").build();
    let hostname = adw::EntryRow::builder()
        .title("Computername")
        .text(if conf.hostname.is_empty() {
            "computer".to_string()
        } else {
            conf.hostname.clone()
        })
        .build();
    let pw1 = adw::PasswordEntryRow::builder().title("Passwort").build();
    let pw2 = adw::PasswordEntryRow::builder()
        .title("Passwort wiederholen")
        .build();
    for w in [&fullname, &username, &hostname] {
        user_grp.add(w);
    }
    user_grp.add(&pw1);
    user_grp.add(&pw2);
    user_page.add(&user_grp);
    let opt_grp = adw::PreferencesGroup::builder().title("Optionen").build();
    let root_same = adw::SwitchRow::builder()
        .title("Gleiches Passwort für root")
        .subtitle("Sonst wird das root-Konto gesperrt (sudo bleibt nutzbar)")
        .build();
    let autologin = adw::SwitchRow::builder()
        .title("Automatisch anmelden")
        .build();
    opt_grp.add(&root_same);
    if !matches!(conf.desktop.as_str(), "" | "none") {
        opt_grp.add(&autologin);
    }
    user_page.add(&opt_grp);
    stack.add_named(&user_page, Some("user"));

    // ---------- summary ----------
    let summary_page = adw::PreferencesPage::builder()
        .title("Zusammenfassung")
        .build();
    let summary_group = adw::PreferencesGroup::builder()
        .title("Bereit zur Installation")
        .description("Bitte prüfe die Einstellungen.")
        .build();
    summary_page.add(&summary_group);
    stack.add_named(&summary_page, Some("summary"));

    // ---------- progress ----------
    let progress_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    progress_box.append(
        &gtk::Label::builder()
            .label(format!("{} wird installiert …", conf.name))
            .css_classes(["title-2"])
            .build(),
    );
    let status_label = gtk::Label::builder()
        .label("Starte …")
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    let progress_bar = gtk::ProgressBar::builder().show_text(true).build();
    let log_view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&log_view)
        .build();
    let expander = gtk::Expander::builder()
        .label("Details anzeigen")
        .child(&scroller)
        .vexpand(true)
        .build();
    for w in [
        &status_label.clone().upcast::<gtk::Widget>(),
        &progress_bar.clone().upcast(),
        &expander.clone().upcast(),
    ] {
        progress_box.append(w);
    }
    stack.add_named(&progress_box, Some("progress"));

    // ---------- done ----------
    let done_page = adw::StatusPage::builder()
        .icon_name("emblem-ok-symbolic")
        .title("Installation abgeschlossen")
        .description(format!(
            "{} wurde installiert. Entferne das Installationsmedium und starte neu.",
            conf.name
        ))
        .build();
    let reboot_btn = gtk::Button::builder()
        .label("Jetzt neu starten")
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .build();
    reboot_btn.connect_clicked(|_| {
        let _ = Command::new("systemctl").arg("reboot").spawn();
    });
    done_page.set_child(Some(&reboot_btn));
    stack.add_named(&done_page, Some("done"));

    let wizard = Rc::new(Wizard {
        window: window.clone(),
        toast_overlay,
        stack,
        back_btn: back_btn.clone(),
        next_btn: next_btn.clone(),
        index: Cell::new(0),
        conf,
        locale_row,
        tz_row,
        kb_row,
        selected_disk: RefCell::new(None),
        disk_checks: RefCell::new(disk_checks),
        extra_rows: RefCell::new(extra_rows),
        disk_hint_row,
        fs_row,
        swap_row,
        encrypt_row,
        luks_same_row,
        luks1,
        luks2,
        loader_row,
        loader_keys,
        source_row,
        source_keys,
        fullname: fullname.clone(),
        username: username.clone(),
        hostname,
        pw1,
        pw2,
        root_same,
        autologin,
        auto_user: RefCell::new(String::new()),
        summary_group,
        summary_rows: RefCell::new(Vec::new()),
        status_label,
        progress_bar,
        log_view,
        done_page,
    });

    // Disk selection wiring (needs `wizard` to update the hint + disable the matching extra-role row).
    for (path, check) in wizard.disk_checks.borrow().iter() {
        let wizard = wizard.clone();
        let path = path.clone();
        check.connect_toggled(move |c| {
            if !c.is_active() {
                return;
            }
            *wizard.selected_disk.borrow_mut() = Some(path.clone());
            for (p, row) in wizard.extra_rows.borrow().iter() {
                row.set_sensitive(p != &path);
                if p == &path {
                    row.set_selected(0);
                }
            }
            wizard.update_disk_hint();
        });
    }
    for row in wizard.extra_rows.borrow().values() {
        let wizard = wizard.clone();
        row.connect_selected_notify(move |_| wizard.update_disk_hint());
    }
    if let Some((path, check)) = wizard.disk_checks.borrow().first() {
        check.set_active(true);
        *wizard.selected_disk.borrow_mut() = Some(path.clone());
    }
    wizard.update_disk_hint();

    fullname.connect_changed(glib::clone!(
        #[strong]
        wizard,
        move |row| wizard.on_fullname_changed(row)
    ));
    back_btn.connect_clicked(glib::clone!(
        #[strong]
        wizard,
        move |_| wizard.go(-1)
    ));
    next_btn.connect_clicked(glib::clone!(
        #[strong]
        wizard,
        move |_| wizard.go(1)
    ));

    wizard.update_nav();
    window.present();
}

impl Wizard {
    fn toast(&self, text: &str) {
        self.toast_overlay
            .add_toast(adw::Toast::builder().title(text).timeout(4).build());
    }

    fn on_fullname_changed(&self, row: &adw::EntryRow) {
        let first = row
            .text()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        let suggestion: String = first
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        let current = self.username.text().to_string();
        if current.is_empty() || current == *self.auto_user.borrow() {
            self.username.set_text(&suggestion);
        }
        *self.auto_user.borrow_mut() = suggestion;
    }

    fn extra_disks(&self) -> Vec<ExtraDisk> {
        let selected = self.selected_disk.borrow().clone();
        self.extra_rows
            .borrow()
            .iter()
            .filter_map(|(path, row)| {
                let role = DISK_ROLES[row.selected() as usize].0;
                if Some(path.clone()) != selected && role != "none" {
                    Some(ExtraDisk {
                        path: path.clone(),
                        role: role.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn update_disk_hint(&self) {
        let disk = self.selected_disk.borrow().clone();
        let extras = self.extra_disks();
        let n = extras.len() + usize::from(disk.is_some());
        self.disk_hint_row.set_title("Werden gelöscht");
        let mut parts = vec![disk.unwrap_or_else(|| "\u{2013}".to_string())];
        parts.extend(extras.iter().map(|e| e.path.clone()));
        self.disk_hint_row.set_subtitle(&format!(
            "{}  ({n} Festplatte{})",
            parts.join(", "),
            if n != 1 { "n" } else { "" }
        ));
    }

    fn selection(&self) -> Selection {
        let encrypt = self.encrypt_row.is_active();
        Selection {
            disk: self.selected_disk.borrow().clone().unwrap_or_default(),
            extra_disks: self.extra_disks(),
            filesystem: combo_value(&self.fs_row),
            swap_gb: self.swap_row.value() as u32,
            encrypt,
            luks_password: if encrypt {
                if self.luks_same_row.is_active() {
                    self.pw1.text().to_string()
                } else {
                    self.luks1.text().to_string()
                }
            } else {
                String::new()
            },
            bootloader: self
                .loader_keys
                .get(self.loader_row.selected() as usize)
                .cloned()
                .unwrap_or_else(|| "grub".to_string()),
            source: self
                .source_keys
                .get(self.source_row.selected() as usize)
                .cloned()
                .unwrap_or_else(|| "offline".to_string()),
            locale: combo_value(&self.locale_row),
            timezone: combo_value(&self.tz_row),
            keyboard: combo_value(&self.kb_row),
            fullname: self.fullname.text().trim().to_string(),
            username: self.username.text().trim().to_string(),
            hostname: self.hostname.text().trim().to_string(),
            password: self.pw1.text().to_string(),
            root_same: self.root_same.is_active(),
            autologin: self.autologin.is_active(),
        }
    }

    fn validate(&self, name: &str) -> Option<String> {
        let s = self.selection();
        match name {
            "disk" => {
                if s.disk.is_empty() {
                    return Some("Bitte eine Festplatte wählen.".to_string());
                }
                let home_count = s.extra_disks.iter().filter(|e| e.role == "home").count();
                if home_count > 1 {
                    return Some("Nur eine Festplatte kann /home sein.".to_string());
                }
            }
            "options" => {
                let roles: Vec<&str> = s.extra_disks.iter().map(|e| e.role.as_str()).collect();
                if roles.contains(&"raid") && s.filesystem != "btrfs" {
                    return Some("Der RAID-1-Spiegel benötigt btrfs als Dateisystem.".to_string());
                }
                if roles.contains(&"raid") && s.swap_gb > 0 {
                    return Some("Eine Swap-Datei ist auf btrfs-RAID nicht möglich – bitte 0 wählen (zram nutzen).".to_string());
                }
                if s.encrypt && !self.luks_same_row.is_active() {
                    if self.luks1.text().len() < 8 {
                        return Some(
                            "Das Verschlüsselungspasswort muss mindestens 8 Zeichen haben."
                                .to_string(),
                        );
                    }
                    if self.luks1.text() != self.luks2.text() {
                        return Some(
                            "Die Verschlüsselungspasswörter stimmen nicht überein.".to_string(),
                        );
                    }
                }
            }
            "user" => {
                let username_re = regex::Regex::new(r"^[a-z_][a-z0-9_-]{0,31}$").unwrap();
                if !username_re.is_match(&s.username) {
                    return Some(
                        "Benutzername: nur Kleinbuchstaben, Ziffern, '-' und '_'.".to_string(),
                    );
                }
                if s.username == "root" || s.username == "live" {
                    return Some("Dieser Benutzername ist reserviert.".to_string());
                }
                let hostname_re = regex::Regex::new(r"^[a-zA-Z0-9-]{1,63}$").unwrap();
                if !hostname_re.is_match(&s.hostname) {
                    return Some("Ungültiger Computername.".to_string());
                }
                if s.password.is_empty() {
                    return Some("Bitte ein Passwort eingeben.".to_string());
                }
                if s.password != self.pw2.text() {
                    return Some("Die Passwörter stimmen nicht überein.".to_string());
                }
                if s.encrypt && self.luks_same_row.is_active() && s.password.len() < 8 {
                    return Some(
                        "Für die Verschlüsselung muss das Passwort mindestens 8 Zeichen haben."
                            .to_string(),
                    );
                }
            }
            _ => {}
        }
        None
    }

    fn go(self: &Rc<Self>, delta: i32) {
        let name = PAGES[self.index.get()];
        if delta > 0 {
            if let Some(err) = self.validate(name) {
                self.toast(&err);
                return;
            }
            if name == "summary" {
                self.confirm();
                return;
            }
        }
        let new_index = (self.index.get() as i32 + delta).clamp(0, PAGES.len() as i32 - 1) as usize;
        self.index.set(new_index);
        if PAGES[new_index] == "summary" {
            self.fill_summary();
        }
        self.update_nav();
    }

    fn update_nav(&self) {
        let name = PAGES[self.index.get()];
        self.stack.set_visible_child_name(name);
        let busy = matches!(name, "progress" | "done");
        self.back_btn.set_visible(self.index.get() > 0 && !busy);
        self.next_btn.set_visible(!busy);
        self.next_btn.set_label(if name == "summary" {
            "Installieren"
        } else {
            "Weiter"
        });
        self.next_btn.set_css_classes(if name == "summary" {
            &["destructive-action"]
        } else {
            &["suggested-action"]
        });
    }

    fn fill_summary(&self) {
        for r in self.summary_rows.borrow_mut().drain(..) {
            self.summary_group.remove(&r);
        }
        let s = self.selection();
        let role_names: HashMap<&str, &str> = DISK_ROLES.iter().cloned().collect();
        let extras = if s.extra_disks.is_empty() {
            "keine".to_string()
        } else {
            s.extra_disks
                .iter()
                .map(|e| {
                    format!(
                        "{} \u{2192} {}",
                        e.path,
                        role_names.get(e.role.as_str()).unwrap_or(&e.role.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let loader = if s.bootloader == "systemd-boot" {
            "systemd-boot"
        } else {
            "GRUB"
        };
        let rows: Vec<(&str, String)> = vec![
            ("Systemfestplatte (wird gelöscht!)", s.disk.clone()),
            ("Weitere Festplatten (werden gelöscht!)", extras),
            (
                "Dateisystem",
                if s.swap_gb > 0 {
                    format!("{}, Swap {} GB", s.filesystem, s.swap_gb)
                } else {
                    s.filesystem.clone()
                },
            ),
            (
                "Verschlüsselung",
                if s.encrypt {
                    "LUKS2".to_string()
                } else {
                    "aus".to_string()
                },
            ),
            ("Bootloader", loader.to_string()),
            (
                "Quelle",
                if s.source == "offline" {
                    "ISO (offline)".to_string()
                } else {
                    "Internet".to_string()
                },
            ),
            ("Sprache", s.locale.clone()),
            ("Zeitzone", s.timezone.clone()),
            ("Tastatur", s.keyboard.clone()),
            ("Benutzer", format!("{} ({})", s.fullname, s.username)),
            ("Computername", s.hostname.clone()),
            (
                "Pakete",
                format!("{} Pakete/Gruppen", self.conf.packages.len()),
            ),
        ];
        let mut stored = self.summary_rows.borrow_mut();
        for (title, value) in rows {
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(if value.is_empty() {
                    "\u{2013}".to_string()
                } else {
                    value
                })
                .css_classes(["property"])
                .build();
            self.summary_group.add(&row);
            stored.push(row);
        }
    }

    fn confirm(self: &Rc<Self>) {
        let s = self.selection();
        let mut disks = vec![s.disk.clone()];
        disks.extend(s.extra_disks.iter().map(|e| e.path.clone()));
        let dlg = adw::AlertDialog::builder()
            .heading("Festplatten löschen?")
            .body(format!(
                "Alle Daten auf {} gehen unwiderruflich verloren.",
                disks.join(", ")
            ))
            .build();
        dlg.add_response("cancel", "Abbrechen");
        dlg.add_response("install", "Löschen und installieren");
        dlg.set_response_appearance("install", adw::ResponseAppearance::Destructive);
        let wizard = self.clone();
        dlg.connect_response(None, move |_, response| {
            if response == "install" {
                wizard.start();
            }
        });
        dlg.present(Some(&self.window));
    }

    fn start(self: &Rc<Self>) {
        self.index
            .set(PAGES.iter().position(|p| *p == "progress").unwrap());
        self.update_nav();
        let selection = self.selection();
        let backend = format!("{INSTALLER_LIB}/distro-installer-backend");
        let wizard = self.clone();
        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::unbounded::<WorkerMsg>();
            let sel = selection;
            std::thread::spawn(move || run_backend(&backend, &sel, tx));
            while let Ok(msg) = rx.recv().await {
                match msg {
                    WorkerMsg::Progress(frac, text) => wizard.set_progress(frac, &text),
                    WorkerMsg::Log(line) => wizard.append_log(&line),
                    WorkerMsg::Done(error) => {
                        wizard.finished(error);
                        break;
                    }
                }
            }
        });
    }

    fn set_progress(&self, frac: f64, text: &str) {
        self.progress_bar.set_fraction(frac);
        self.status_label.set_label(text);
    }

    fn append_log(&self, line: &str) {
        let buf = self.log_view.buffer();
        let mut end = buf.end_iter();
        buf.insert(&mut end, &format!("{line}\n"));
        self.log_view
            .scroll_to_mark(&buf.get_insert(), 0.0, false, 0.0, 0.0);
    }

    fn finished(&self, error: Option<String>) {
        if let Some(err) = error {
            self.done_page.set_icon_name(Some("dialog-error-symbolic"));
            self.done_page.set_title("Installation fehlgeschlagen");
            self.done_page.set_description(Some(&format!(
                "{err}\n\nDetails stehen im Protokoll. Du kannst den Installer neu starten."
            )));
            let btn = gtk::Button::builder()
                .label("Protokoll anzeigen")
                .halign(gtk::Align::Center)
                .css_classes(["pill"])
                .build();
            let stack = self.stack.clone();
            btn.connect_clicked(move |_| stack.set_visible_child_name("progress"));
            self.done_page.set_child(Some(&btn));
        }
        self.index
            .set(PAGES.iter().position(|p| *p == "done").unwrap());
        self.update_nav();
    }
}

enum WorkerMsg {
    Progress(f64, String),
    Log(String),
    Done(Option<String>),
}

/// Spawns the root backend (directly if already root, else via `sudo -n`,
/// matching the Python installer exactly) with the selection written to a
/// temp JSON file, and streams its stdout protocol back through `tx`.
fn run_backend(backend: &str, sel: &Selection, tx: async_channel::Sender<WorkerMsg>) {
    let tmp = match tempfile::Builder::new()
        .prefix("installer-")
        .suffix(".json")
        .tempfile()
    {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send_blocking(WorkerMsg::Done(Some(e.to_string())));
            return;
        }
    };
    if let Err(e) = serde_json::to_writer(&tmp, sel) {
        let _ = tx.send_blocking(WorkerMsg::Done(Some(e.to_string())));
        return;
    }
    let path = tmp.path().to_path_buf();
    let is_root = unsafe { libc_geteuid() } == 0;
    let mut cmd = if is_root {
        Command::new(backend)
    } else {
        Command::new("sudo")
    };
    if !is_root {
        cmd.args(["-n", backend]);
    }
    cmd.arg(&path);
    let child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut error: Option<String> = None;
    match child {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(rest) = line.strip_prefix("##PROGRESS ") {
                        if let Some((frac, text)) = rest.split_once(' ') {
                            if let Ok(frac) = frac.parse::<f64>() {
                                let _ =
                                    tx.send_blocking(WorkerMsg::Progress(frac, text.to_string()));
                            }
                        }
                    } else if let Some(msg) = line.strip_prefix("##ERROR## ") {
                        error = Some(msg.to_string());
                    } else if line != "##DONE##" {
                        let _ = tx.send_blocking(WorkerMsg::Log(line));
                    }
                }
            }
            match child.wait() {
                Ok(status) if !status.success() && error.is_none() => {
                    error = Some(
                        "Das Installationsprogramm wurde mit einem Fehler beendet.".to_string(),
                    );
                }
                Err(e) => error = Some(e.to_string()),
                _ => {}
            }
        }
        Err(e) => error = Some(e.to_string()),
    }
    let _ = std::fs::remove_file(&path);
    let _ = tx.send_blocking(WorkerMsg::Done(error));
}

unsafe fn libc_geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}
