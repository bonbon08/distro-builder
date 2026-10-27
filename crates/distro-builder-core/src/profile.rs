//! Erzeugt aus einer Projekt-Konfiguration ein archiso-Profil (basierend auf releng).

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::config::{self as C, Config};
use crate::installer_conf::{InstallerConf, InstallerDefaults, OLLAMA_MODELS_DIR};

pub const RELENG: &str = "/usr/share/archiso/configs/releng";
/// Where the installer binaries are expected to live once installed
/// alongside `distro-builder` (see the Makefile).
pub const INSTALLER_LIB: &str = "/usr/local/lib/distro-installer";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("{0}")]
    Validation(String),
    #[error("archiso-Vorlage nicht gefunden ({0}). Ist 'archiso' installiert?")]
    MissingReleng(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn hex_to_ansi(hexcolor: &str) -> String {
    let h = hexcolor.trim_start_matches('#');
    let parse = || -> Option<(u8, u8, u8)> {
        if h.len() < 6 {
            return None;
        }
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        Some((r, g, b))
    };
    let (r, g, b) = parse().unwrap_or((23, 147, 209));
    format!("38;2;{r};{g};{b}")
}

/// Shared escaping for bash double-quoted strings and `/etc/os-release` values.
pub fn quote_str(s: &str) -> String {
    let re = Regex::new(r#"([\\"$`])"#).unwrap();
    format!("\"{}\"", re.replace_all(s, "\\$1"))
}

pub struct Paths {
    pub base: PathBuf,
    pub profile: PathBuf,
    pub work: PathBuf,
    pub out: PathBuf,
    pub script: PathBuf,
}

pub fn paths(cfg: &Config) -> Paths {
    let dir = shellexpand_home(&cfg.build.project_dir);
    let base = dir.join(&cfg.general.id);
    Paths {
        profile: base.join("profile"),
        work: base.join("work"),
        out: base.join("out"),
        script: base.join("build.sh"),
        base,
    }
}

fn shellexpand_home(p: &str) -> PathBuf {
    if p == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

fn write_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn write_file_mode(path: &Path, content: &str, mode: u32) -> std::io::Result<()> {
    write_file(path, content)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn template(name: &str, cfg: &Config) -> String {
    let text = match name {
        "apply-branding.sh" => include_str!("../templates/apply-branding.sh"),
        "branding.hook" => include_str!("../templates/branding.hook"),
        "live-setup.sh" => include_str!("../templates/live-setup.sh"),
        "live-setup.service" => include_str!("../templates/live-setup.service"),
        "set-wallpaper.sh" => include_str!("../templates/set-wallpaper.sh"),
        other => panic!("unknown template {other}"),
    };
    text.replace("@@ID@@", &cfg.general.id)
        .replace("@@NAME@@", &cfg.general.name)
}

fn symlink(target: &str, link: &Path) -> std::io::Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    if link.exists() || fs::symlink_metadata(link).is_ok() {
        let _ = fs::remove_file(link);
    }
    std::os::unix::fs::symlink(target, link)
}

fn remove_path(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
}

/// Konvertiert ein Bild nach PNG (optional skaliert). Fällt auf Kopieren zurück,
/// wenn das Format nicht dekodiert werden kann.
fn save_image(src: &Path, dest: &Path, max_size: Option<u32>, exact: Option<(u32, u32)>) -> bool {
    let decoded = image::ImageReader::open(src)
        .and_then(|r| r.with_guessed_format())
        .ok()
        .and_then(|r| r.decode().ok());
    let Some(img) = decoded else {
        if exact.is_some() {
            return false;
        }
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        return fs::copy(src, dest).is_ok();
    };
    let resized = if let Some((w, h)) = exact {
        img.resize_exact(w, h, image::imageops::FilterType::Triangle)
    } else if let Some(max) = max_size {
        let (w, h) = (img.width(), img.height());
        if w.max(h) > max {
            img.resize(
                (w as f64 * max as f64 / w.max(h) as f64).max(1.0) as u32,
                (h as f64 * max as f64 / w.max(h) as f64).max(1.0) as u32,
                image::imageops::FilterType::Triangle,
            )
        } else {
            img
        }
    } else {
        img
    };
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    resized
        .save_with_format(dest, image::ImageFormat::Png)
        .is_ok()
}

fn console_keymaps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("de", "de-latin1"),
        ("at", "de-latin1"),
        ("ch", "de_CH-latin1"),
        ("gb", "uk"),
        ("us", "us"),
        ("fr", "fr-latin1"),
        ("es", "es"),
        ("it", "it"),
        ("nl", "nl"),
        ("pl", "pl"),
        ("se", "sv-latin1"),
        ("no", "no-latin1"),
        ("dk", "dk-latin1"),
        ("fi", "fi"),
        ("pt", "pt-latin1"),
        ("ru", "ru"),
        ("tr", "trq"),
        ("cz", "cz"),
        ("hu", "hu"),
        ("be", "be-latin1"),
        ("br", "br-abnt2"),
    ]
}

pub fn console_keymap(layout: &str) -> String {
    if let Some((_, v)) = console_keymaps().into_iter().find(|(k, _)| *k == layout) {
        return v.to_string();
    }
    let short = Regex::new(r"^[a-z]{2,3}$").unwrap();
    if short.is_match(layout) {
        layout.to_string()
    } else {
        "us".to_string()
    }
}

pub fn os_release(cfg: &Config) -> String {
    let (g, b) = (&cfg.general, &cfg.branding);
    let mut lines = vec![
        format!("NAME={}", quote_str(&g.name)),
        format!(
            "PRETTY_NAME={}",
            quote_str(&format!("{} {}", g.name, g.version))
        ),
        format!("ID={}", g.id),
        "ID_LIKE=arch".to_string(),
        "BUILD_ID=rolling".to_string(),
        format!("VERSION_ID={}", quote_str(&g.version)),
        format!("ANSI_COLOR={}", quote_str(&hex_to_ansi(&b.color))),
        format!("LOGO={}", g.id),
    ];
    for (val, field) in [
        (&g.homepage, "HOME_URL"),
        (&g.support_url, "SUPPORT_URL"),
        (&g.bug_url, "BUG_REPORT_URL"),
    ] {
        if !val.is_empty() {
            lines.push(format!("{field}={}", quote_str(val)));
        }
    }
    lines.join("\n") + "\n"
}

pub fn fastfetch_config(cfg: &Config) -> String {
    let (b, gid) = (&cfg.branding, &cfg.general.id);
    let ansi = hex_to_ansi(&b.color);
    let logo = if matches!(b.logo_mode.as_str(), "image" | "chafa") && !b.logo.is_empty() {
        serde_json::json!({"type": "chafa", "source": format!("/usr/share/{gid}/logo.png"), "width": 32, "height": 16})
    } else if b.logo_mode == "ascii" && !b.ascii_logo.trim().is_empty() {
        serde_json::json!({"type": "file", "source": format!("/usr/share/{gid}/logo.txt"), "color": {"1": ansi}})
    } else {
        serde_json::json!({"source": "arch"})
    };
    let conf = serde_json::json!({
        "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
        "logo": logo,
        "display": {"separator": "  ", "color": {"keys": ansi, "title": ansi}},
        "modules": ["title", "separator", "os", "host", "kernel", "uptime", "packages", "shell",
                    "display", "de", "wm", "theme", "icons", "terminal", "cpu", "gpu", "memory",
                    "disk", "locale", "break", "colors"],
    });
    serde_json::to_string_pretty(&conf).unwrap() + "\n"
}

/// Branding-Dateien, die im Live-System UND im installierten System landen.
fn build_overlay(cfg: &Config, root: &Path) -> std::io::Result<Vec<String>> {
    let (g, b) = (&cfg.general, &cfg.branding);
    let gid = &g.id;
    let share = root.join("usr/share").join(gid);
    fs::create_dir_all(&share)?;
    let mut execs = Vec::new();

    write_file(&share.join("os-release"), &os_release(cfg))?;
    write_file(
        &root
            .join("usr/local/bin")
            .join(format!("{gid}-apply-branding")),
        &template("apply-branding.sh", cfg),
    )?;
    execs.push(format!("/usr/local/bin/{gid}-apply-branding"));
    write_file(
        &root
            .join("etc/pacman.d/hooks")
            .join(format!("90-{gid}-branding.hook")),
        &template("branding.hook", cfg),
    )?;

    if !b.logo.is_empty() {
        let logo_src = Path::new(&b.logo);
        save_image(logo_src, &share.join("logo.png"), Some(512), None);
        fs::create_dir_all(root.join("usr/share/pixmaps"))?;
        let _ = fs::copy(
            share.join("logo.png"),
            root.join("usr/share/pixmaps").join(format!("{gid}.png")),
        );
    }
    if !b.ascii_logo.trim().is_empty() {
        write_file(
            &share.join("logo.txt"),
            &(b.ascii_logo.trim_end_matches('\n').to_string() + "\n"),
        )?;
    }
    write_file(
        &root.join("etc/xdg/fastfetch/config.jsonc"),
        &fastfetch_config(cfg),
    )?;
    write_file(
        &root.join("usr/local/bin/neofetch"),
        "#!/bin/sh\nexec fastfetch \"$@\"\n",
    )?;
    execs.push("/usr/local/bin/neofetch".to_string());
    if b.logo_mode == "image" && !b.logo.is_empty() {
        write_file(
            &root.join("usr/local/bin/fastfetch"),
            "#!/bin/sh\ncase \"${TERM:-}:${TERM_PROGRAM:-}:${KONSOLE_VERSION:+konsole}\" in\n  \
             *kitty*|*WezTerm*|*ghostty*|*konsole*) exec /usr/bin/fastfetch --logo-type kitty-direct \
             --logo-width 32 --logo-height 15 \"$@\" ;;\nesac\nexec /usr/bin/fastfetch \"$@\"\n",
        )?;
        execs.push("/usr/local/bin/fastfetch".to_string());
    }

    if !b.wallpaper.is_empty() {
        let ext = Path::new(&b.wallpaper)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| ".png".to_string());
        let _ = fs::copy(&b.wallpaper, share.join(format!("wallpaper{ext}")));
        fs::create_dir_all(root.join("usr/share/backgrounds"))?;
        symlink(
            &format!("/usr/share/{gid}/wallpaper{ext}"),
            &root
                .join("usr/share/backgrounds")
                .join(format!("{gid}{ext}")),
        )?;
        let script = template("set-wallpaper.sh", cfg)
            .replace("@@WALLPAPER@@", &format!("/usr/share/{gid}/wallpaper{ext}"));
        write_file(
            &root
                .join("usr/local/bin")
                .join(format!("{gid}-set-wallpaper")),
            &script,
        )?;
        execs.push(format!("/usr/local/bin/{gid}-set-wallpaper"));
        write_file(
            &root.join("etc/xdg/autostart").join(format!("{gid}-wallpaper.desktop")),
            &format!(
                "[Desktop Entry]\nType=Application\nName={} Hintergrund\nExec=/usr/local/bin/{gid}-set-wallpaper\nNoDisplay=true\nX-GNOME-Autostart-Phase=Applications\n",
                g.name
            ),
        )?;
    }

    if !b.motd.trim().is_empty() {
        write_file(
            &root.join("etc/motd"),
            &(b.motd.trim_end_matches('\n').to_string() + "\n"),
        )?;
    }

    if C::desktop_info(cfg).display_manager == Some("lightdm") {
        write_file(
            &root.join("etc/lightdm/lightdm.conf.d/10-greeter.conf"),
            "[Seat:*]\ngreeter-session=lightdm-gtk-greeter\n",
        )?;
    }
    if cfg.installer.zram {
        write_file(
            &root.join("etc/systemd/zram-generator.conf"),
            "[zram0]\nzram-size = min(ram / 2, 8192)\ncompression-algorithm = zstd\n",
        )?;
    }
    execs.extend(build_ai_containers(cfg, root)?);
    Ok(execs)
}

const NOT_LIVE: &str = "ConditionKernelCommandLine=!archisobasedir";

/// Ollama-Konfiguration, Container-Import beim ersten Start und Compose-Stacks.
fn build_ai_containers(cfg: &Config, root: &Path) -> std::io::Result<Vec<String>> {
    let (g, ai, ct) = (&cfg.general, &cfg.ai, &cfg.containers);
    let gid = &g.id;
    let mut execs = Vec::new();
    if ai.ollama {
        let mut conf = vec![
            "[Service]".to_string(),
            format!("Environment=\"OLLAMA_MODELS={OLLAMA_MODELS_DIR}\""),
        ];
        if ai.expose {
            conf.push("Environment=\"OLLAMA_HOST=0.0.0.0:11434\"".to_string());
        }
        conf.push(format!(
            "ExecStartPre=+/bin/sh -c 'mkdir -p {OLLAMA_MODELS_DIR} && find {OLLAMA_MODELS_DIR} \\( -type d -o -path \"*/manifests/*\" \\) ! -user ollama -exec chown ollama:ollama {{}} +'"
        ));
        write_file(
            &root
                .join("etc/systemd/system/ollama.service.d")
                .join(format!("10-{gid}.conf")),
            &(conf.join("\n") + "\n"),
        )?;
    }

    let engine = ct.engine.as_str();
    if engine == "none" {
        return Ok(execs);
    }
    let tool = if engine == "docker" {
        "/usr/bin/docker"
    } else {
        "/usr/bin/podman"
    };
    let dep = if engine == "docker" {
        "docker.service"
    } else {
        "network-online.target"
    };
    let images = C::container_images(cfg);
    if ct.preload && !images.is_empty() {
        let script = format!(
            "#!/bin/sh\n# Importiert die in die ISO gepackten Container-Images (einmalig)\nDIR=/var/lib/{gid}/container-images\nfor f in \"$DIR\"/*.tar; do\n  [ -e \"$f\" ] || continue\n  echo \"Importiere $f\"\n  {tool} load -i \"$f\" && rm -f \"$f\"\ndone\nrmdir \"$DIR\" 2>/dev/null\nexit 0\n"
        );
        write_file(
            &root
                .join("usr/local/bin")
                .join(format!("{gid}-container-import")),
            &script,
        )?;
        execs.push(format!("/usr/local/bin/{gid}-container-import"));
        write_file(
            &root.join("etc/systemd/system").join(format!("{gid}-container-import.service")),
            &format!(
                "[Unit]\nDescription={} : vorinstallierte Container-Images importieren\nAfter={dep}\nRequires={dep}\nConditionDirectoryNotEmpty=/var/lib/{gid}/container-images\n{NOT_LIVE}\n\n[Service]\nType=oneshot\nExecStart=/usr/local/bin/{gid}-container-import\nTimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n",
                g.name
            ),
        )?;
    }
    if !ct.stacks.is_empty() {
        let up = if engine == "docker" {
            format!("{tool} compose up -d --remove-orphans")
        } else {
            "/usr/bin/podman-compose up -d".to_string()
        };
        let down = if engine == "docker" {
            format!("{tool} compose down")
        } else {
            "/usr/bin/podman-compose down".to_string()
        };
        write_file(
            &root.join("etc/systemd/system").join(format!("{gid}-compose@.service")),
            &format!(
                "[Unit]\nDescription={} Container-Stack %i\nAfter=network-online.target {dep} {gid}-container-import.service\nWants=network-online.target\nRequires={dep}\n{NOT_LIVE}\n\n[Service]\nType=oneshot\nRemainAfterExit=yes\nWorkingDirectory=/etc/{gid}/compose/%i\nExecStart={up}\nExecStop={down}\nTimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n",
                g.name
            ),
        )?;
        for st in &ct.stacks {
            write_file(
                &root
                    .join("etc")
                    .join(gid)
                    .join("compose")
                    .join(&st.name)
                    .join("compose.yaml"),
                &(st.compose.trim_end().to_string() + "\n"),
            )?;
            let url_re = Regex::new(r"https?://[^\s)]+").unwrap();
            let url = if !st.url.is_empty() {
                st.url.clone()
            } else {
                url_re
                    .find(&st.compose)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            };
            if !url.is_empty() {
                let title = if st.title.is_empty() {
                    &st.name
                } else {
                    &st.title
                };
                write_file(
                    &root.join("usr/share/applications").join(format!("{gid}-{}.desktop", st.name)),
                    &format!(
                        "[Desktop Entry]\nType=Application\nName={title}\nComment=Container-App ({})\nExec=xdg-open {url}\nIcon=applications-internet\nCategories=Network;\n",
                        st.name
                    ),
                )?;
            }
        }
    }
    Ok(execs)
}

fn compose_services(cfg: &Config) -> Vec<String> {
    let (gid, ct) = (&cfg.general.id, &cfg.containers);
    if ct.engine == "none" {
        return Vec::new();
    }
    let mut svcs = Vec::new();
    if ct.preload && !C::container_images(cfg).is_empty() {
        svcs.push(format!("{gid}-container-import.service"));
    }
    svcs.extend(
        ct.stacks
            .iter()
            .filter(|s| s.autostart)
            .map(|s| format!("{gid}-compose@{}.service", s.name)),
    );
    if ct.engine == "podman" && !ct.stacks.is_empty() {
        svcs.push("podman-restart.service".to_string());
    }
    svcs
}

/// Dateien aus releng, die im installierten System bleiben sollen.
fn releng_keep() -> HashSet<&'static str> {
    [
        "etc/resolv.conf",
        "etc/localtime",
        "etc/passwd",
        "etc/shadow",
        "etc/hostname",
        "etc/locale.conf",
        "etc/systemd/system/dbus-org.freedesktop.resolve1.service",
        "etc/systemd/system/dbus-org.freedesktop.timesync1.service",
        "etc/systemd/system/multi-user.target.wants/systemd-resolved.service",
        "etc/systemd/system/sysinit.target.wants/systemd-timesyncd.service",
    ]
    .into_iter()
    .collect()
}

/// Pfade, die der Installer nach dem Offline-Kopieren entfernt (Live-Konfiguration).
fn live_only_paths(
    releng_air: &Path,
    cfg: &Config,
    overlay_files: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let gid = &cfg.general.id;
    let keep = releng_keep();
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    walk_releng(
        releng_air,
        releng_air,
        &keep,
        overlay_files,
        &mut files,
        &mut dirs,
    );
    files.extend([
        "/etc/systemd/system/live-setup.service".to_string(),
        "/etc/systemd/system/multi-user.target.wants/live-setup.service".to_string(),
        "/usr/local/bin/live-setup".to_string(),
        "/etc/sddm.conf.d/autologin.conf".to_string(),
        "/etc/gdm/custom.conf".to_string(),
        "/etc/lightdm/lightdm.conf.d/50-autologin.conf".to_string(),
        "/etc/systemd/system/display-manager.service".to_string(),
        INSTALLER_LIB.to_string(),
        "/etc/distro-installer.json".to_string(),
        "/usr/local/bin/distro-installer".to_string(),
        "/usr/share/applications/distro-installer.desktop".to_string(),
        "/etc/xdg/autostart/distro-installer.desktop".to_string(),
        format!("/usr/share/{gid}/overlay"),
        "/root/.zshrc.local".to_string(),
        "/etc/sudoers.d/00-live".to_string(),
    ]);
    let mut files: Vec<String> = files
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    files.sort();
    let mut dirs: Vec<String> = dirs
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    dirs.sort();
    (files, dirs)
}

fn walk_releng(
    base: &Path,
    dir: &Path,
    keep: &HashSet<&'static str>,
    overlay_files: &HashSet<String>,
    files: &mut Vec<String>,
    dirs: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let rel_dir = dir.strip_prefix(base).unwrap_or(Path::new(""));
    if !rel_dir.as_os_str().is_empty() {
        dirs.push(format!("/{}", rel_dir.to_string_lossy()));
    }
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if meta.file_type().is_symlink() {
            if !keep.contains(rel.as_str()) && !overlay_files.contains(&rel) {
                files.push(format!("/{rel}"));
            }
        } else if meta.is_dir() {
            walk_releng(base, &path, keep, overlay_files, files, dirs);
        } else if !keep.contains(rel.as_str()) && !overlay_files.contains(&rel) {
            files.push(format!("/{rel}"));
        }
    }
}

pub fn pacman_conf(cfg: &Config) -> String {
    let r = &cfg.repos;
    let server = if !r.mirror.trim().is_empty() {
        format!("Server = {}", r.mirror.trim())
    } else {
        "Include = /etc/pacman.d/mirrorlist".to_string()
    };
    let mut out = vec![
        "# Erzeugt von Distro Builder".to_string(),
        "[options]".to_string(),
        "HoldPkg     = pacman glibc".to_string(),
        "Architecture = auto".to_string(),
        "Color".to_string(),
        "ILoveCandy".to_string(),
        "ParallelDownloads = 5".to_string(),
        "SigLevel    = Required DatabaseOptional".to_string(),
        "LocalFileSigLevel = Optional".to_string(),
        String::new(),
    ];
    for repo in r.custom.iter().filter(|r| r.first) {
        out.push(format!("[{}]", repo.name));
        out.push(format!(
            "SigLevel = {}",
            if repo.siglevel.is_empty() {
                "Optional TrustAll"
            } else {
                &repo.siglevel
            }
        ));
        out.push(format!("Server = {}", repo.server));
        out.push(String::new());
    }
    for (enabled, name) in [
        (r.core_testing, "core-testing"),
        (r.core, "core"),
        (r.extra_testing, "extra-testing"),
        (r.extra, "extra"),
        (r.multilib, "multilib"),
    ] {
        if enabled || name == "core" {
            out.push(format!("[{name}]"));
            out.push(server.clone());
            out.push(String::new());
        }
    }
    for repo in r.custom.iter().filter(|r| !r.first) {
        out.push(format!("[{}]", repo.name));
        out.push(format!(
            "SigLevel = {}",
            if repo.siglevel.is_empty() {
                "Optional TrustAll"
            } else {
                &repo.siglevel
            }
        ));
        out.push(format!("Server = {}", repo.server));
        out.push(String::new());
    }
    out.join("\n")
}

pub fn installer_json(cfg: &Config, live_files: &[String], live_dirs: &[String]) -> String {
    let (g, i) = (&cfg.general, &cfg.installer);
    let mut data_paths = Vec::new();
    if cfg.ai.ollama && !cfg.ai.models.is_empty() {
        data_paths.push(OLLAMA_MODELS_DIR.to_string());
    }
    if cfg.containers.preload && !C::container_images(cfg).is_empty() {
        data_paths.push(format!("/var/lib/{}/container-images", g.id));
    }
    let conf = InstallerConf {
        name: g.name.clone(),
        id: g.id.clone(),
        hostname: g.hostname.clone(),
        logo: if !cfg.branding.logo.is_empty() {
            format!("/usr/share/{}/logo.png", g.id)
        } else {
            String::new()
        },
        welcome: i.welcome.clone(),
        packages: C::target_packages(cfg),
        services: {
            let mut s = C::target_services(cfg);
            s.extend(compose_services(cfg));
            s
        },
        kernel_params: C::kernel_params(cfg),
        zram: i.zram,
        allow_online: i.allow_online,
        data_paths,
        live_files: live_files.to_vec(),
        live_dirs: live_dirs.to_vec(),
        desktop: cfg.packages.desktop.clone(),
        fastfetch_on_terminal: cfg.branding.fastfetch_on_terminal,
        defaults: InstallerDefaults {
            locale: i.locale.clone(),
            timezone: i.timezone.clone(),
            keyboard: i.keyboard.clone(),
            filesystem: i.filesystem.clone(),
            source: i.source.clone(),
            encrypt: i.encrypt,
            bootloader: i.bootloader.clone(),
            swap_gb: i.swap_gb,
        },
    };
    serde_json::to_string_pretty(&conf).unwrap()
}

fn rebrand_boot(profile: &Path, name: &str) -> std::io::Result<()> {
    for sub in ["efiboot", "grub", "syslinux"] {
        let dir = profile.join(sub);
        if !dir.exists() {
            continue;
        }
        for entry in walkdir(&dir) {
            if !matches!(
                entry.extension().and_then(|e| e.to_str()),
                Some("cfg") | Some("conf")
            ) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&entry) else {
                continue;
            };
            let mut new = text.replace("Arch Linux install medium", &format!("{name} Live"));
            new = new
                .replace("install Arch Linux", &format!("install {name}"))
                .replace("Arch Linux", name);
            if new != text {
                fs::write(&entry, new)?;
            }
        }
    }
    Ok(())
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push(path);
        }
    }
    out
}

fn edit_profiledef(
    cfg: &Config,
    profile: &Path,
    perms: &[(String, String)],
) -> std::io::Result<()> {
    let g = &cfg.general;
    let p = profile.join("profiledef.sh");
    let mut text = fs::read_to_string(&p)?;
    let label_re = Regex::new(r"[^A-Z0-9_]").unwrap();
    let mut label: String = label_re.replace_all(&g.id.to_uppercase(), "_").into_owned();
    label.truncate(22);
    let repl: Vec<(&str, String)> = vec![
        ("iso_name", quote_str(&g.id)),
        (
            "iso_label",
            format!("\"{label}_$(date --date=\"@${{SOURCE_DATE_EPOCH:-$(date +%s)}}\" +%Y%m)\""),
        ),
        (
            "iso_publisher",
            quote_str(if g.publisher.is_empty() {
                &g.name
            } else {
                &g.publisher
            }),
        ),
        (
            "iso_application",
            quote_str(&format!("{} Live/Installations-Medium", g.name)),
        ),
        (
            "iso_version",
            format!(
                "{}\"$(date --date=\"@${{SOURCE_DATE_EPOCH:-$(date +%s)}}\" +%Y.%m.%d)\"",
                quote_str(&format!("{}-", g.version))
            ),
        ),
    ];
    // Use a closure replacer (not a `$`-expanding string replacer) since the
    // replacement values themselves contain literal `$` (date substitutions).
    for (key, val) in &repl {
        let line_re = Regex::new(&format!(r"(?m)^{key}=.*$")).unwrap();
        let replacement = format!("{key}={val}");
        text = line_re
            .replace(&text, |_: &regex::Captures| replacement.clone())
            .into_owned();
    }
    if cfg.build.compression == "zstd" {
        let re = Regex::new(r"(?m)^airootfs_image_tool_options=.*$").unwrap();
        text = re
            .replace(&text, |_: &regex::Captures| {
                "airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '15' '-b' '1M')"
                    .to_string()
            })
            .into_owned();
    }
    text.push_str("\n# Distro Builder\nfile_permissions+=(\n");
    for (path, mode) in perms {
        text.push_str(&format!("  [\"{path}\"]=\"{mode}\"\n"));
    }
    text.push(')');
    text.push('\n');
    fs::write(&p, text)
}

pub fn build_script(cfg: &Config) -> String {
    let pth = paths(cfg);
    let uid = unsafe { libc_getuid() };
    let gid = unsafe { libc_getgid() };
    format!(
        "#!/bin/bash\n# Erzeugt von Distro Builder – kann auch manuell mit 'sudo bash build.sh' ausgeführt werden.\nset -euo pipefail\nPROFILE={}\nWORK={}\nOUT={}\nif ! command -v mkarchiso >/dev/null; then\n  echo \"FEHLER: mkarchiso nicht gefunden. Bitte 'pacman -S archiso' installieren.\" >&2\n  exit 1\nfi\ncleanup() {{ rm -rf -- \"$WORK\"; }}\ntrap cleanup EXIT\nrm -rf -- \"$WORK\"\nmkdir -p -- \"$OUT\"\nmkarchiso -v -r -w \"$WORK\" -o \"$OUT\" \"$PROFILE\"\nchown -R {uid}:{gid} -- \"$OUT\"\necho \"##DONE##\"\n",
        quote_str(&pth.profile.to_string_lossy()),
        quote_str(&pth.work.to_string_lossy()),
        quote_str(&pth.out.to_string_lossy()),
    )
}

/// Minimal libc shims so the core crate doesn't need the `libc` crate just for
/// two syscalls already exposed indirectly via `std`.
mod ids {
    extern "C" {
        pub fn getuid() -> u32;
        pub fn getgid() -> u32;
    }
}
unsafe fn libc_getuid() -> u32 {
    ids::getuid()
}
unsafe fn libc_getgid() -> u32 {
    ids::getgid()
}

/// Paths to the two compiled installer binaries that get copied into the
/// generated profile. Resolve with [`locate_installer_binaries`].
pub struct InstallerBinaries {
    pub gui: PathBuf,
    pub backend: PathBuf,
}

/// Looks for `distro-installer` and `distro-installer-backend` next to the
/// running executable first (development builds, same `target/` dir), then
/// at the fixed installed location used by the Makefile.
pub fn locate_installer_binaries() -> Option<InstallerBinaries> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
            // `cargo test` binaries live in `target/<profile>/deps/`, one level
            // below where `cargo build`'s own binaries land - check that too so
            // integration tests find the sibling installer binaries.
            if let Some(parent) = dir.parent() {
                candidates.push(parent.to_path_buf());
            }
        }
    }
    candidates.push(PathBuf::from("/usr/lib/distro-builder/bin"));
    candidates.push(PathBuf::from("/usr/local/lib/distro-builder/bin"));
    for dir in candidates {
        let gui = dir.join("distro-installer");
        let backend = dir.join("distro-installer-backend");
        if gui.is_file() && backend.is_file() {
            return Some(InstallerBinaries { gui, backend });
        }
    }
    None
}

pub fn generate(
    cfg: &Config,
    mut log: impl FnMut(&str),
    releng: &Path,
    installer_binaries: Option<&InstallerBinaries>,
) -> Result<Paths, ProfileError> {
    let (errors, _warnings) = C::validate(cfg);
    if !errors.is_empty() {
        return Err(ProfileError::Validation(errors.join("\n")));
    }
    if !releng.is_dir() {
        return Err(ProfileError::MissingReleng(
            releng.to_string_lossy().into_owned(),
        ));
    }

    let g = &cfg.general;
    let gid = &g.id;
    let pth = paths(cfg);
    fs::create_dir_all(&pth.base)?;
    let profile = &pth.profile;
    log(&format!("Erzeuge Profil in {}", profile.display()));
    remove_path(profile);
    copy_dir_all(releng, profile)?;
    let air = profile.join("airootfs");
    let systemd = air.join("etc/systemd/system");
    let mut perms: Vec<(String, String)> = Vec::new();

    // Pakete
    let releng_pkgs_text = fs::read_to_string(releng.join("packages.x86_64"))?;
    let releng_pkgs: Vec<String> = releng_pkgs_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();
    let pkgs = C::live_packages(cfg, &releng_pkgs);
    write_file(&profile.join("packages.x86_64"), &(pkgs.join("\n") + "\n"))?;
    log(&format!("{} Pakete für das Live-System", pkgs.len()));
    if !cfg.packages.rescue_tools {
        for unit in C::minimal_drop_units() {
            remove_path(&systemd.join(unit));
        }
    }

    // Repositories
    let pconf = pacman_conf(cfg);
    write_file(&profile.join("pacman.conf"), &pconf)?;
    write_file(&air.join("etc/pacman.conf"), &pconf)?;

    // Branding
    write_file(&air.join("etc/hostname"), &(g.hostname.clone() + "\n"))?;
    let kb = &cfg.installer.keyboard;
    write_file(
        &air.join("etc/vconsole.conf"),
        &format!("KEYMAP={}\n", console_keymap(kb)),
    )?;
    write_file(
        &air.join("etc/X11/xorg.conf.d/00-keyboard.conf"),
        &format!(
            "Section \"InputClass\"\n    Identifier \"system-keyboard\"\n    MatchIsKeyboard \"on\"\n    Option \"XkbLayout\" \"{kb}\"\nEndSection\n"
        ),
    )?;
    let execs = build_overlay(cfg, &air)?;
    let staging = air.join("usr/share").join(gid).join("overlay");
    build_overlay(cfg, &staging)?;
    let mut overlay_files = HashSet::new();
    for path in walkdir(&staging) {
        if let Ok(rel) = path.strip_prefix(&staging) {
            overlay_files.insert(rel.to_string_lossy().into_owned());
        }
    }
    for e in &execs {
        perms.push((e.clone(), "0:0:755".to_string()));
    }
    rebrand_boot(profile, &g.name)?;
    if !cfg.branding.wallpaper.is_empty()
        && save_image(
            Path::new(&cfg.branding.wallpaper),
            &profile.join("syslinux/splash.png"),
            None,
            Some((640, 480)),
        )
    {
        log("Bootmenü-Hintergrund ersetzt");
    }

    // Desktop-Live-Umgebung
    let desktop = C::desktop_info(cfg);
    if let Some(dm) = desktop.display_manager {
        log(&format!("Live-Desktop: {} ({dm})", desktop.label));
        for unit in [
            "multi-user.target.wants/systemd-networkd.service",
            "multi-user.target.wants/iwd.service",
            "sockets.target.wants/systemd-networkd.socket",
            "dbus-org.freedesktop.network1.service",
            "network-online.target.wants/systemd-networkd-wait-online.service",
        ] {
            remove_path(&systemd.join(unit));
        }
        symlink(
            "/usr/lib/systemd/system/NetworkManager.service",
            &systemd.join("multi-user.target.wants/NetworkManager.service"),
        )?;
        symlink(
            &format!("/usr/lib/systemd/system/{dm}.service"),
            &systemd.join("display-manager.service"),
        )?;
        let session = desktop.session.unwrap_or("");
        match dm {
            "sddm" => write_file(
                &air.join("etc/sddm.conf.d/autologin.conf"),
                &format!("[Autologin]\nUser=live\nSession={session}\n"),
            )?,
            "gdm" => write_file(
                &air.join("etc/gdm/custom.conf"),
                "[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=live\n",
            )?,
            "lightdm" => write_file(
                &air.join("etc/lightdm/lightdm.conf.d/50-autologin.conf"),
                &format!("[Seat:*]\nautologin-user=live\nautologin-session={session}\n"),
            )?,
            _ => {}
        }

        let script = template("live-setup.sh", cfg).replace(
            "@@FASTFETCH@@",
            if cfg.branding.fastfetch_on_terminal {
                "1"
            } else {
                "0"
            },
        );
        write_file(&air.join("usr/local/bin/live-setup"), &script)?;
        perms.push((
            "/usr/local/bin/live-setup".to_string(),
            "0:0:755".to_string(),
        ));
        write_file(
            &systemd.join("live-setup.service"),
            &template("live-setup.service", cfg),
        )?;
        symlink(
            "/etc/systemd/system/live-setup.service",
            &systemd.join("multi-user.target.wants/live-setup.service"),
        )?;
    }

    // Installer
    if C::installer_active(cfg) {
        log("Grafischer Installer wird eingebunden");
        match installer_binaries {
            Some(bins) => {
                let lib = PathBuf::from(format!("{}{INSTALLER_LIB}", air.display()));
                fs::create_dir_all(&lib)?;
                fs::copy(&bins.backend, lib.join("distro-installer-backend"))?;
                fs::set_permissions(
                    lib.join("distro-installer-backend"),
                    fs::Permissions::from_mode(0o755),
                )?;
                perms.push((
                    format!("{INSTALLER_LIB}/distro-installer-backend"),
                    "0:0:755".to_string(),
                ));
                fs::create_dir_all(air.join("usr/local/bin"))?;
                fs::copy(&bins.gui, air.join("usr/local/bin/distro-installer"))?;
                fs::set_permissions(
                    air.join("usr/local/bin/distro-installer"),
                    fs::Permissions::from_mode(0o755),
                )?;
                perms.push((
                    "/usr/local/bin/distro-installer".to_string(),
                    "0:0:755".to_string(),
                ));

                let (live_files, live_dirs) =
                    live_only_paths(&releng.join("airootfs"), cfg, &overlay_files);
                write_file(
                    &air.join("etc/distro-installer.json"),
                    &installer_json(cfg, &live_files, &live_dirs),
                )?;
                let icon = if !cfg.branding.logo.is_empty() {
                    gid.clone()
                } else {
                    "system-software-install".to_string()
                };
                let desktop_entry = format!(
                    "[Desktop Entry]\nType=Application\nName={} installieren\nComment={} auf der Festplatte installieren\nExec=distro-installer\nIcon={icon}\nTerminal=false\nCategories=System;\n",
                    g.name, g.name
                );
                write_file(
                    &air.join("usr/share/applications/distro-installer.desktop"),
                    &desktop_entry,
                )?;
                if cfg.installer.autostart {
                    write_file(
                        &air.join("etc/xdg/autostart/distro-installer.desktop"),
                        &desktop_entry,
                    )?;
                }
            }
            None => {
                return Err(ProfileError::Validation(
                    "Die distro-installer-Binärdateien wurden nicht gefunden – Installer kann nicht eingebunden werden.".to_string(),
                ));
            }
        }
    } else if cfg.branding.fastfetch_on_terminal {
        write_file(&air.join("root/.zshrc.local"), "fastfetch\n")?;
    }

    // KI im Live-System direkt nutzbar
    if cfg.ai.ollama {
        symlink(
            "/usr/lib/systemd/system/ollama.service",
            &systemd.join("multi-user.target.wants/ollama.service"),
        )?;
        if !cfg.ai.models.is_empty() {
            log(&format!("KI-Modelle: {}", cfg.ai.models.join(", ")));
        }
    }
    if cfg.containers.engine != "none" {
        let n_images = if cfg.containers.preload {
            C::container_images(cfg).len()
        } else {
            0
        };
        log(&format!(
            "Container: {}, {} Stack(s), {n_images} vorab geladene(s) Image(s)",
            cfg.containers.engine,
            cfg.containers.stacks.len()
        ));
    }

    edit_profiledef(cfg, profile, &perms)?;
    write_file_mode(&pth.script, &build_script(cfg), 0o755)?;
    write_file(
        &pth.base.join("project.json"),
        &serde_json::to_string_pretty(cfg).unwrap(),
    )?;
    log("Profil fertig");
    Ok(pth)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_symlink() {
            let target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(target, dest_path)?;
        } else if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn releng_dir() -> PathBuf {
        env::var("DISTRO_BUILDER_RELENG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(RELENG))
    }

    fn make_cfg(tmp: &Path) -> Config {
        let mut cfg = C::default_config();
        cfg.general.name = "Test OS".into();
        cfg.general.id = "testos".into();
        cfg.general.hostname = "testos".into();
        cfg.build.project_dir = tmp.to_string_lossy().into_owned();
        cfg
    }

    #[test]
    fn repo_order() {
        let mut cfg = C::default_config();
        cfg.repos.multilib = true;
        cfg.repos.custom = vec![
            crate::config::CustomRepo {
                name: "vorne".into(),
                server: "https://a/$arch".into(),
                siglevel: "Optional TrustAll".into(),
                first: true,
            },
            crate::config::CustomRepo {
                name: "hinten".into(),
                server: "https://b/$arch".into(),
                siglevel: "Never".into(),
                first: false,
            },
        ];
        let text = pacman_conf(&cfg);
        let order: Vec<usize> = ["vorne", "core", "extra", "multilib", "hinten"]
            .iter()
            .map(|r| text.find(&format!("[{r}]")).unwrap())
            .collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted);
        assert!(!text.contains("[core-testing]"));
    }

    #[test]
    #[ignore = "requires archiso releng template; run with --include-ignored on Arch Linux"]
    fn all_desktops() {
        if !releng_dir().is_dir() {
            eprintln!("skipping: no releng at {}", releng_dir().display());
            return;
        }
        for (key, _) in C::desktops() {
            let tmp = tempfile::tempdir().unwrap();
            let mut cfg = make_cfg(tmp.path());
            cfg.packages.desktop = key.to_string();
            let bins = locate_installer_binaries();
            let pth = generate(&cfg, |_| {}, &releng_dir(), bins.as_ref()).unwrap();
            let air = pth.profile.join("airootfs");
            assert!(air.join("usr/share/testos/os-release").is_file());
            let has_installer = key != "none";
            assert_eq!(
                air.join("etc/distro-installer.json").is_file(),
                has_installer
            );
        }
    }
}
