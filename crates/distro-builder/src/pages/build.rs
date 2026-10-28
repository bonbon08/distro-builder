use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config as C;
use distro_builder_core::fetch as F;
use distro_builder_core::profile as P;
use gtk::glib;

use crate::ctx::Ctx;

const BUILD_STAGES: &[(&str, f64, &str)] = &[
    ("Installing packages to", 0.04, "Pakete werden installiert"),
    (
        "Creating a list of installed packages",
        0.52,
        "Paketliste wird erstellt",
    ),
    ("Preparing kernel and initrd", 0.56, "Kernel und Initramfs"),
    (
        "Creating SquashFS image",
        0.60,
        "SquashFS-Abbild wird komprimiert (dauert)",
    ),
    ("Creating checksum", 0.86, "Prüfsummen werden erstellt"),
    ("Setting up SYSLINUX", 0.88, "Bootloader wird eingerichtet"),
    (
        "Setting up systemd-boot",
        0.89,
        "Bootloader wird eingerichtet",
    ),
    ("Creating ISO image", 0.92, "ISO-Datei wird geschrieben"),
    ("Done!", 1.0, "Fertig"),
    // ARM-Image (templates/image-build.sh)
    (
        "==> Lade das Arch-Linux-ARM-Basissystem",
        0.02,
        "Basissystem wird geladen",
    ),
    ("==> Erzeuge Image", 0.04, "Image wird angelegt"),
    ("==> Lade Flatpak-Apps", 0.05, "Flatpak-Apps werden geladen"),
    (
        "==> System wird im Image eingerichtet",
        0.06,
        "System wird eingerichtet (Emulation, dauert)",
    ),
    (
        "==> System wird konfiguriert",
        0.80,
        "System wird konfiguriert",
    ),
    (
        "==> Bootloader wird eingerichtet",
        0.86,
        "Bootloader wird eingerichtet",
    ),
    ("==> Komprimiere Image", 0.90, "Image wird komprimiert"),
];

const IMAGE_SUFFIXES: &[&str] = &[".iso", ".img", ".img.zst", ".img.xz"];
const ARM_TOOLS: &[&str] = &[
    "arch-install-scripts",
    "qemu-user-static",
    "qemu-user-static-binfmt",
];

fn is_build_output(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    IMAGE_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// arch-chroot und QEMU-Emulation für aarch64 (oder ein ARM-Rechner).
fn arm_tools_ready() -> bool {
    let emulated = std::env::consts::ARCH == "aarch64"
        || Path::new("/proc/sys/fs/binfmt_misc/qemu-aarch64").exists()
        || Path::new("/usr/lib/binfmt.d/qemu-aarch64-static.conf").exists();
    which("arch-chroot") && emulated
}

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let req = adw::PreferencesGroup::builder()
        .title("Voraussetzungen")
        .build();
    page.add(&req);
    let archiso_row = adw::ActionRow::builder().title("archiso").build();
    let archiso_btn = gtk::Button::builder()
        .label("Installieren")
        .valign(gtk::Align::Center)
        .build();
    {
        let ctx2 = ctx.clone();
        archiso_btn.connect_clicked(move |_| install_pkg(&ctx2, &["archiso"]));
    }
    archiso_row.add_suffix(&archiso_btn);
    req.add(&archiso_row);
    let space_row = adw::ActionRow::builder()
        .title("Freier Speicher")
        .css_classes(["property"])
        .build();
    req.add(&space_row);
    let skopeo_row = adw::ActionRow::builder()
        .title("Container-Images vorladen")
        .build();
    let skopeo_btn = gtk::Button::builder()
        .label("skopeo installieren")
        .valign(gtk::Align::Center)
        .build();
    {
        let ctx2 = ctx.clone();
        skopeo_btn.connect_clicked(move |_| install_pkg(&ctx2, &["skopeo"]));
    }
    skopeo_row.add_suffix(&skopeo_btn);
    req.add(&skopeo_row);
    let arm_row = adw::ActionRow::builder()
        .title("ARM-Image bauen")
        .subtitle("arch-install-scripts, QEMU-Emulation (qemu-user-static-binfmt)")
        .build();
    let arm_btn = gtk::Button::builder()
        .label("Installieren")
        .valign(gtk::Align::Center)
        .build();
    {
        let ctx2 = ctx.clone();
        arm_btn.connect_clicked(move |_| install_pkg(&ctx2, ARM_TOOLS));
    }
    arm_row.add_suffix(&arm_btn);
    req.add(&arm_row);
    let flatpak_req_row = adw::ActionRow::builder()
        .title("Flatpak-Apps mitliefern")
        .build();
    let flatpak_btn = gtk::Button::builder()
        .label("flatpak installieren")
        .valign(gtk::Align::Center)
        .build();
    {
        let ctx2 = ctx.clone();
        flatpak_btn.connect_clicked(move |_| install_pkg(&ctx2, &["flatpak"]));
    }
    flatpak_req_row.add_suffix(&flatpak_btn);
    req.add(&flatpak_req_row);
    let is_image = C::is_image(&ctx.cfg.borrow());

    let out = adw::PreferencesGroup::builder().title("Ausgabe").build();
    page.add(&out);
    let dir_row = adw::ActionRow::builder().title("Projektordner").build();
    let pick = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    {
        let ctx2 = ctx.clone();
        pick.connect_clicked(move |_| pick_dir(&ctx2));
    }
    dir_row.add_suffix(&pick);
    out.add(&dir_row);
    let compressions = vec![
        ("zstd".to_string(), "zstd – schneller Build".to_string()),
        ("xz".to_string(), "xz – kleinere Datei".to_string()),
    ];
    ctx.combo_row(
        &out,
        "Kompression",
        &compressions,
        |cfg| cfg.build.compression.clone(),
        |cfg, v| cfg.build.compression = v,
        None,
        false,
    );

    let act = adw::PreferencesGroup::new();
    page.add(&act);
    let btns = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    btns.set_halign(gtk::Align::Center);
    btns.set_margin_top(6);
    btns.set_margin_bottom(6);
    let profile_btn = gtk::Button::builder()
        .label("Nur Profil erzeugen")
        .css_classes(["pill"])
        .build();
    let build_btn = gtk::Button::builder()
        .label(if is_image { "Image bauen" } else { "ISO bauen" })
        .css_classes(["pill", "suggested-action"])
        .build();
    let cancel_btn = gtk::Button::builder()
        .label("Abbrechen")
        .css_classes(["pill", "destructive-action"])
        .visible(false)
        .build();
    {
        let ctx2 = ctx.clone();
        profile_btn.connect_clicked(move |_| on_profile_only(&ctx2));
    }
    {
        let ctx2 = ctx.clone();
        build_btn.connect_clicked(move |_| on_build(&ctx2));
    }
    {
        let ctx2 = ctx.clone();
        cancel_btn.connect_clicked(move |_| on_cancel(&ctx2));
    }
    for b in [&profile_btn, &build_btn, &cancel_btn] {
        btns.append(b);
    }
    act.add(&btns);

    let prog = adw::PreferencesGroup::builder()
        .title("Fortschritt")
        .build();
    page.add(&prog);
    let build_status = gtk::Label::builder()
        .label("Bereit")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .margin_bottom(6)
        .build();
    let build_bar = gtk::ProgressBar::builder().margin_bottom(10).build();
    prog.add(&build_status);
    prog.add(&build_bar);
    let log_view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::Char)
        .css_classes(["log-view"])
        .build();
    let log_scroll = gtk::ScrolledWindow::builder()
        .child(&log_view)
        .min_content_height(280)
        .build();
    prog.add(&gtk::Frame::builder().child(&log_scroll).build());

    let result_grp = adw::PreferencesGroup::builder()
        .title("Ergebnis")
        .visible(false)
        .build();
    page.add(&result_grp);
    let iso_row = adw::ActionRow::builder()
        .title(if is_image { "Image-Datei" } else { "ISO-Datei" })
        .subtitle_selectable(true)
        .build();
    let open_btn = gtk::Button::builder()
        .label("Ordner öffnen")
        .valign(gtk::Align::Center)
        .build();
    {
        let ctx2 = ctx.clone();
        open_btn.connect_clicked(move |_| {
            if let Some(iso) = ctx2.last_iso.borrow().clone() {
                if let Some(parent) = iso.parent() {
                    let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
                }
            }
        });
    }
    let qemu_btn = gtk::Button::builder()
        .label("In VM testen")
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();
    {
        let ctx2 = ctx.clone();
        qemu_btn.connect_clicked(move |_| run_vm(&ctx2));
    }
    iso_row.add_suffix(&open_btn);
    iso_row.add_suffix(&qemu_btn);
    result_grp.add(&iso_row);

    {
        let mut shared = ctx.shared.borrow_mut();
        shared.archiso_row = Some(archiso_row);
        shared.archiso_btn = Some(archiso_btn);
        shared.space_row = Some(space_row);
        shared.skopeo_row = Some(skopeo_row);
        shared.skopeo_btn = Some(skopeo_btn);
        shared.arm_row = Some(arm_row);
        shared.arm_btn = Some(arm_btn);
        shared.flatpak_req_row = Some(flatpak_req_row);
        shared.flatpak_btn = Some(flatpak_btn);
        shared.dir_row = Some(dir_row);
        shared.profile_btn = Some(profile_btn);
        shared.build_btn = Some(build_btn);
        shared.cancel_btn = Some(cancel_btn);
        shared.build_status = Some(build_status);
        shared.build_bar = Some(build_bar);
        shared.log_view = Some(log_view);
        shared.log_scroll = Some(log_scroll);
        shared.result_grp = Some(result_grp);
        shared.iso_row = Some(iso_row);
        shared.qemu_btn = Some(qemu_btn);
    }
    refresh_build_info(ctx);
    page
}

pub fn refresh_build_info(ctx: &Rc<Ctx>) {
    let shared = ctx.shared.borrow();
    let Some(archiso_row) = &shared.archiso_row else {
        return;
    };
    let cfg = ctx.cfg.borrow();
    let need_images = !C::container_images(&cfg).is_empty() && cfg.containers.preload;
    if let Some(row) = &shared.skopeo_row {
        row.set_visible(need_images);
        let tool = F::container_tool();
        row.set_subtitle(&match tool {
            Some(t) => format!("\u{2714} über {t}"),
            None => "\u{2718} skopeo, docker oder podman wird benötigt".to_string(),
        });
    }
    if let Some(btn) = &shared.skopeo_btn {
        btn.set_visible(F::container_tool().is_none());
    }
    let is_image = C::is_image(&cfg);
    archiso_row.set_visible(!is_image);
    let arm_ok = arm_tools_ready();
    if let Some(row) = &shared.arm_row {
        row.set_visible(is_image);
        row.set_subtitle(if arm_ok {
            "\u{2714} bereit"
        } else {
            "\u{2718} arch-install-scripts und QEMU-Emulation (qemu-user-static-binfmt) fehlen"
        });
    }
    if let Some(btn) = &shared.arm_btn {
        btn.set_visible(!arm_ok);
    }
    let has_flatpak = which("flatpak");
    if let Some(row) = &shared.flatpak_req_row {
        row.set_visible(P::flatpak_preload(&cfg));
        row.set_subtitle(if has_flatpak {
            "\u{2714} flatpak installiert"
        } else {
            "\u{2718} flatpak fehlt – sonst werden die Apps erst beim ersten Start geladen"
        });
    }
    if let Some(btn) = &shared.flatpak_btn {
        btn.set_visible(!has_flatpak);
    }
    let has_archiso = which("mkarchiso") && Path::new(P::RELENG).is_dir();
    archiso_row.set_subtitle(if has_archiso {
        "\u{2714} installiert"
    } else {
        "\u{2718} nicht installiert – wird zum Bauen benötigt"
    });
    if let Some(btn) = &shared.archiso_btn {
        btn.set_visible(!has_archiso);
    }
    let pth = P::paths(&cfg);
    if let Some(row) = &shared.dir_row {
        row.set_subtitle(&glib::markup_escape_text(&pth.base.to_string_lossy()));
    }
    let mut probe = expand_home(&cfg.build.project_dir);
    while !probe.as_os_str().is_empty() && !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p.to_path_buf(),
            None => break,
        }
    }
    if let Some(row) = &shared.space_row {
        match free_space_gb(&probe) {
            Some(free) => row.set_subtitle(&format!(
                "{free:.1} GB frei{}",
                if free < 20.0 {
                    "  \u{26a0} empfohlen: mind. 20 GB"
                } else {
                    ""
                }
            )),
            None => row.set_subtitle("unbekannt"),
        }
    }
    if pth.out.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&pth.out) {
            let mut isos: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_build_output(p))
                .collect();
            isos.sort();
            if let Some(last) = isos.pop() {
                drop(shared);
                show_result(ctx, &last);
            }
        }
    }
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file()))
        .unwrap_or(false)
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs_home() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Free space on the filesystem containing `path`, in GB. Shells out to `df`
/// rather than binding `statvfs` directly, to avoid hand-rolling a
/// platform-specific struct layout.
fn free_space_gb(path: &Path) -> Option<f64> {
    let probe = if path.as_os_str().is_empty() {
        Path::new("/")
    } else {
        path
    };
    let out = std::process::Command::new("df")
        .args(["-k", "--output=avail"])
        .arg(probe)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let kb: f64 = text.lines().nth(1)?.trim().parse().ok()?;
    Some(kb / 1024f64.powi(2))
}

fn pick_dir(ctx: &Rc<Ctx>) {
    let dlg = gtk::FileDialog::builder()
        .title("Projektordner wählen")
        .build();
    let ctx2 = ctx.clone();
    dlg.select_folder(Some(&ctx.window), None::<&gio::Cancellable>, move |res| {
        let Ok(file) = res else { return };
        let Some(path) = file.path() else { return };
        ctx2.cfg.borrow_mut().build.project_dir = path.to_string_lossy().into_owned();
        refresh_build_info(&ctx2);
    });
}

fn install_pkg(ctx: &Rc<Ctx>, pkgs: &'static [&'static str]) {
    let names = pkgs.join(" ");
    ctx.log(&format!("Installiere {names} …"));
    let ctx2 = ctx.clone();
    let mut cmd: Vec<String> = ["pkexec", "pacman", "-S", "--needed", "--noconfirm"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    cmd.extend(pkgs.iter().map(|s| s.to_string()));
    ctx.run_streaming(
        cmd,
        |ctx, line| ctx.log(line),
        move |_ctx, rc| {
            let msg = if rc == 0 {
                format!("{names} installiert")
            } else {
                "Installation fehlgeschlagen".to_string()
            };
            ctx2.toast(&msg);
            refresh_build_info(&ctx2);
        },
    );
}

/// Mirrors ui.py's `prepare()`: validate, then generate the archiso profile.
fn prepare(ctx: &Rc<Ctx>) -> Option<P::Paths> {
    let cfg = ctx.cfg.borrow().clone();
    let (errors, warnings) = C::validate(&cfg);
    if !errors.is_empty() {
        let body = errors
            .iter()
            .map(|e| format!("\u{2022} {e}"))
            .collect::<Vec<_>>()
            .join("\n");
        ctx.error("Bitte korrigieren", &body);
        return None;
    }
    for w in &warnings {
        ctx.log(&format!("Hinweis: {w}"));
    }
    let bins = P::locate_installer_binaries();
    let ctx2 = ctx.clone();
    match P::generate(&cfg, |l| ctx2.log(l), Path::new(P::RELENG), bins.as_ref()) {
        Ok(pth) => Some(pth),
        Err(e) => {
            ctx.error("Profil konnte nicht erzeugt werden", &e.to_string());
            None
        }
    }
}

/// Mirrors ui.py's `fetch_assets()`: download AI models / container images in
/// the background, then call `then`.
fn fetch_assets(ctx: &Rc<Ctx>, pth: P::Paths, then: impl Fn(&Rc<Ctx>) + 'static) {
    let cfg = ctx.cfg.borrow().clone();
    let needs = (cfg.ai.ollama && !cfg.ai.models.is_empty())
        || (cfg.containers.preload && !C::container_images(&cfg).is_empty());
    if !needs {
        then(ctx);
        return;
    }
    ctx.set_busy(true);
    if let Some(status) = &ctx.shared.borrow().build_status {
        status.set_label("Lade KI-Modelle und Container-Images …");
    }
    let (tx, rx) = async_channel::unbounded::<AssetMsg>();
    std::thread::spawn(move || {
        let tx_log = tx.clone();
        let tx_prog = tx.clone();
        let mut log = move |t: &str| {
            let _ = tx_log.send_blocking(AssetMsg::Log(t.to_string()));
        };
        let mut progress = move |frac: f64, t: &str| {
            let _ = tx_prog.send_blocking(AssetMsg::Progress(frac, t.to_string()));
        };
        let result = F::prepare_assets(&cfg, &pth, &mut log, Some(&mut progress));
        let _ = tx.send_blocking(AssetMsg::Done(result.err().map(|e| e.to_string())));
    });
    let ctx2 = ctx.clone();
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                AssetMsg::Log(t) => ctx2.log(&t),
                AssetMsg::Progress(frac, t) => {
                    let shared = ctx2.shared.borrow();
                    if let Some(bar) = &shared.build_bar {
                        bar.set_fraction(frac);
                    }
                    if let Some(status) = &shared.build_status {
                        status.set_label(&t);
                    }
                }
                AssetMsg::Done(err) => {
                    ctx2.set_busy(false);
                    if let Some(e) = err {
                        if let Some(status) = &ctx2.shared.borrow().build_status {
                            status.set_label("Download fehlgeschlagen");
                        }
                        ctx2.error("Download fehlgeschlagen", &e);
                    } else {
                        then(&ctx2);
                    }
                    break;
                }
            }
        }
    });
}

enum AssetMsg {
    Log(String),
    Progress(f64, String),
    Done(Option<String>),
}

fn on_profile_only(ctx: &Rc<Ctx>) {
    let Some(pth) = prepare(ctx) else { return };
    let script = pth.script.clone();
    fetch_assets(ctx, pth, move |ctx| {
        if let Some(status) = &ctx.shared.borrow().build_status {
            status.set_label("Profil erzeugt");
        }
        ctx.toast("Profil erzeugt");
        ctx.log(&format!(
            "Zum manuellen Bauen: sudo bash {}",
            script.display()
        ));
    });
}

fn on_build(ctx: &Rc<Ctx>) {
    if C::is_image(&ctx.cfg.borrow()) {
        if !arm_tools_ready() {
            ctx.error(
                "Werkzeuge fehlen",
                "Für ARM-Images werden arch-install-scripts und die QEMU-Emulation benötigt (Button „Installieren“ oben).",
            );
            return;
        }
    } else if !which("mkarchiso") {
        ctx.error(
            "archiso fehlt",
            "Bitte zuerst archiso installieren (Button oben).",
        );
        return;
    }
    if let Some(view) = &ctx.shared.borrow().log_view {
        view.buffer().set_text("");
    }
    let Some(pth) = prepare(ctx) else { return };
    if let Some(grp) = &ctx.shared.borrow().result_grp {
        grp.set_visible(false);
    }
    if let Some(bar) = &ctx.shared.borrow().build_bar {
        bar.set_fraction(0.0);
    }
    fetch_assets(ctx, pth, move |ctx| {
        start_mkarchiso(ctx, &prepare_paths_for_start(ctx))
    });
}

/// `on_build` re-derives the project paths right before starting mkarchiso
/// (cheap, and keeps `fetch_assets`'s callback signature simple).
fn prepare_paths_for_start(ctx: &Rc<Ctx>) -> P::Paths {
    P::paths(&ctx.cfg.borrow())
}

fn start_mkarchiso(ctx: &Rc<Ctx>, pth: &P::Paths) {
    {
        let shared = ctx.shared.borrow();
        if let Some(bar) = &shared.build_bar {
            bar.set_fraction(0.0);
        }
        if let Some(status) = &shared.build_status {
            status.set_label("Warte auf Passwort-Bestätigung …");
        }
    }
    ctx.shared.borrow_mut().stage_frac = 0.0;
    ctx.shared.borrow_mut().build_out_dir = Some(pth.out.clone());
    let started: std::collections::HashSet<std::ffi::OsString> = if pth.out.is_dir() {
        std::fs::read_dir(&pth.out)
            .map(|it| it.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let pulse_ctx = ctx.clone();
    let pulse = glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let shared = pulse_ctx.shared.borrow();
        if shared.stage_frac <= 0.0 {
            if let Some(bar) = &shared.build_bar {
                bar.pulse();
            }
        }
        glib::ControlFlow::Continue
    });
    ctx.shared.borrow_mut().pulse_source = Some(pulse);

    let script = pth.script.clone();
    ctx.run_streaming(
        vec![
            "pkexec".into(),
            "bash".into(),
            script.to_string_lossy().into_owned(),
        ],
        build_line,
        move |ctx, rc| build_done(ctx, rc, &started),
    );
}

fn build_line(ctx: &Rc<Ctx>, line: &str) {
    ctx.log(line);
    for (key, frac, label) in BUILD_STAGES {
        if line.contains(key) {
            ctx.shared.borrow_mut().stage_frac = *frac;
            let shared = ctx.shared.borrow();
            if let Some(bar) = &shared.build_bar {
                bar.set_fraction(*frac);
            }
            if let Some(status) = &shared.build_status {
                status.set_label(label);
            }
            return;
        }
    }
    let re = regex::Regex::new(r"\(\s*(\d+)/(\d+)\) installing (\S+)").unwrap();
    if let Some(m) = re.captures(line) {
        let done: f64 = m[1].parse().unwrap_or(0.0);
        let total: f64 = m[2].parse::<f64>().unwrap_or(1.0).max(1.0);
        let frac = 0.06 + 0.44 * done / total;
        ctx.shared.borrow_mut().stage_frac = frac;
        let shared = ctx.shared.borrow();
        if let Some(bar) = &shared.build_bar {
            bar.set_fraction(frac);
        }
        if let Some(status) = &shared.build_status {
            status.set_label(&format!(
                "Installiere Pakete: {} ({}/{})",
                &m[3], &m[1], &m[2]
            ));
        }
    } else if line.contains("downloading") && ctx.shared.borrow().stage_frac < 0.06 {
        if let Some(status) = &ctx.shared.borrow().build_status {
            status.set_label("Pakete werden heruntergeladen …");
        }
    }
}

fn build_done(ctx: &Rc<Ctx>, rc: i32, started: &std::collections::HashSet<std::ffi::OsString>) {
    if let Some(id) = ctx.shared.borrow_mut().pulse_source.take() {
        id.remove();
    }
    let out = ctx.shared.borrow().build_out_dir.clone();
    let new_iso: Option<PathBuf> = out.as_ref().filter(|o| o.is_dir()).and_then(|out| {
        let mut isos: Vec<PathBuf> = std::fs::read_dir(out)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_build_output(p) && !started.contains(p.file_name().unwrap()))
            .collect();
        isos.sort();
        isos.pop()
    });
    let shared_status_label = |ctx: &Rc<Ctx>, s: &str| {
        if let Some(status) = &ctx.shared.borrow().build_status {
            status.set_label(s);
        }
    };
    let kind = if C::is_image(&ctx.cfg.borrow()) {
        "Image"
    } else {
        "ISO"
    };
    if rc == 0 {
        if let Some(iso) = new_iso {
            if let Some(bar) = &ctx.shared.borrow().build_bar {
                bar.set_fraction(1.0);
            }
            shared_status_label(ctx, &format!("{kind} erfolgreich gebaut \u{1f389}"));
            show_result(ctx, &iso);
            ctx.toast(&format!("{kind} fertig!"));
            return;
        }
    }
    if rc == 126 || rc == 127 {
        shared_status_label(ctx, "Abgebrochen (keine Berechtigung)");
    } else {
        if let Some(bar) = &ctx.shared.borrow().build_bar {
            bar.set_fraction(0.0);
        }
        shared_status_label(
            ctx,
            &format!("Build fehlgeschlagen (Code {rc}) – siehe Protokoll"),
        );
        ctx.error(
            "Build fehlgeschlagen",
            "Die Ursache steht im Protokoll. Häufig: unbekannter Paketname, nicht erreichbares Repository oder zu wenig Speicher.",
        );
    }
}

fn show_result(ctx: &Rc<Ctx>, iso: &Path) {
    *ctx.last_iso.borrow_mut() = Some(iso.to_path_buf());
    let size = std::fs::metadata(iso)
        .map(|m| m.len() as f64 / 1024f64.powi(3))
        .unwrap_or(0.0);
    let shared = ctx.shared.borrow();
    if let Some(row) = &shared.iso_row {
        row.set_subtitle(&glib::markup_escape_text(&format!(
            "{}  ({size:.2} GB)",
            iso.display()
        )));
    }
    if let Some(btn) = &shared.qemu_btn {
        let is_iso = iso.extension().map(|e| e == "iso").unwrap_or(false);
        btn.set_visible(is_iso && which("run_archiso") && which("qemu-system-x86_64"));
    }
    if let Some(grp) = &shared.result_grp {
        grp.set_visible(true);
    }
}

fn on_cancel(ctx: &Rc<Ctx>) {
    ctx.log("Abbruch angefordert …");
    let pattern = if C::is_image(&ctx.cfg.borrow()) {
        P::paths(&ctx.cfg.borrow())
            .script
            .to_string_lossy()
            .into_owned()
    } else {
        "mkarchiso".to_string()
    };
    let _ = std::process::Command::new("pkexec")
        .args(["pkill", "-TERM", "-f", &pattern])
        .spawn();
}

fn run_vm(ctx: &Rc<Ctx>) {
    let Some(iso) = ctx.last_iso.borrow().clone() else {
        return;
    };
    let uefi = Path::new("/usr/share/edk2/x64/OVMF_CODE.4m.fd").exists();
    let mut cmd = vec![
        "run_archiso".to_string(),
        "-i".to_string(),
        iso.to_string_lossy().into_owned(),
    ];
    if uefi {
        cmd.push("-u".to_string());
    }
    ctx.log(&format!("Starte VM: {}", cmd.join(" ")));
    let _ = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
