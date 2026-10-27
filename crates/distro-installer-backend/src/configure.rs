//! System configuration after the files are in place: fstab, locale,
//! keyboard, timezone, hostname, user account, services. Mirrors
//! `configure`/`autologin`/`console_keymap` from the Python `backend.py`.

use std::path::Path;

use anyhow::Result;
use distro_builder_core::installer_conf::{InstallerConf, Selection};

use crate::disks::{make_swap, DiskInfo};
use crate::progress;
use crate::run::Runner;

const CONSOLE_KEYMAPS: &[(&str, &str)] = &[
    ("de", "de-latin1"),
    ("at", "de-latin1"),
    ("ch", "de_CH-latin1"),
    ("gb", "uk"),
    ("fr", "fr-latin1"),
    ("se", "sv-latin1"),
    ("no", "no-latin1"),
    ("dk", "dk-latin1"),
    ("pt", "pt-latin1"),
    ("tr", "trq"),
    ("be", "be-latin1"),
    ("br", "br-abnt2"),
];

fn console_keymap(layout: &str) -> String {
    let km = CONSOLE_KEYMAPS
        .iter()
        .find(|(k, _)| *k == layout)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| layout.to_string());
    let pattern = format!("/usr/share/kbd/keymaps/**/{km}.map.gz");
    let found = glob::glob(&pattern)
        .map(|mut g| g.next().is_some())
        .unwrap_or(false);
    if found {
        km
    } else {
        "us".to_string()
    }
}

pub fn configure(
    rt: &mut Runner,
    conf: &InstallerConf,
    sel: &Selection,
    info: &mut DiskInfo,
    _uefi: bool,
) -> Result<()> {
    let gid = &conf.id;
    progress(0.74, "System wird konfiguriert");
    let fstab = rt.capture(&["genfstab", "-U", rt.target.to_str().unwrap()], true)?;
    rt.write(
        "/etc/fstab",
        &format!("# Erzeugt vom {} Installer\n{fstab}\n", conf.name),
        None,
    )?;
    make_swap(rt, sel, info)?;
    if !info.crypttab.is_empty() {
        rt.write(
            "/etc/crypttab",
            &format!(
                "# <name> <device> <keyfile> <options>\n{}\n",
                info.crypttab.join("\n")
            ),
            Some(0o600),
        )?;
    }
    std::fs::copy("/etc/pacman.conf", rt.target.join("etc/pacman.conf"))?;
    if Path::new("/etc/pacman.d/mirrorlist").exists() {
        let _ = std::fs::copy(
            "/etc/pacman.d/mirrorlist",
            rt.target.join("etc/pacman.d/mirrorlist"),
        );
    }
    std::fs::write(rt.target.join("etc/machine-id"), "")?;
    let _ = rt.chroot(&["systemd-machine-id-setup"], false, None);

    let (locale, tz, kb) = (
        sel.locale.as_str(),
        sel.timezone.as_str(),
        sel.keyboard.as_str(),
    );
    let gen_path = rt.target.join("etc/locale.gen");
    let mut gen = std::fs::read_to_string(&gen_path).unwrap_or_default();
    let mut locales_needed = vec![locale.to_string()];
    if locale != "en_US.UTF-8" {
        locales_needed.push("en_US.UTF-8".to_string());
    }
    for loc in locales_needed {
        let pat = regex::Regex::new(&format!(r"(?m)^#\s*({} UTF-8)", regex::escape(&loc))).unwrap();
        if pat.is_match(&gen) {
            gen = pat.replace(&gen, "$1").into_owned();
        } else {
            gen.push_str(&format!("\n{loc} UTF-8\n"));
        }
    }
    std::fs::write(&gen_path, gen)?;
    rt.chroot(&["locale-gen"], true, None)?;
    rt.write("/etc/locale.conf", &format!("LANG={locale}\n"), None)?;
    rt.write(
        "/etc/vconsole.conf",
        &format!("KEYMAP={}\n", console_keymap(kb)),
        None,
    )?;
    rt.write(
        "/etc/X11/xorg.conf.d/00-keyboard.conf",
        &format!(
            "Section \"InputClass\"\n    Identifier \"system-keyboard\"\n    MatchIsKeyboard \"on\"\n    Option \"XkbLayout\" \"{kb}\"\nEndSection\n"
        ),
        None,
    )?;
    rt.chroot(
        &[
            "ln",
            "-sf",
            &format!("/usr/share/zoneinfo/{tz}"),
            "/etc/localtime",
        ],
        true,
        None,
    )?;
    let _ = rt.chroot(&["hwclock", "--systohc"], false, None);
    let host = sel.hostname.as_str();
    rt.write("/etc/hostname", &format!("{host}\n"), None)?;
    rt.write(
        "/etc/hosts",
        &format!("127.0.0.1 localhost\n::1 localhost\n127.0.1.1 {host}.localdomain {host}\n"),
        None,
    )?;

    // Benutzer
    progress(0.80, "Benutzer wird angelegt");
    let bashrc = rt.target.join("etc/skel/.bashrc");
    if conf.fastfetch_on_terminal && bashrc.exists() {
        let skel = std::fs::read_to_string(&bashrc).unwrap_or_default();
        if !skel.contains("fastfetch") {
            rt.append(
                "/etc/skel/.bashrc",
                "\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n",
            )?;
        }
    }
    let user = sel.username.as_str();
    let candidate_groups = [
        "wheel", "audio", "video", "storage", "network", "lp", "docker", "render", "ollama",
    ];
    let mut groups = Vec::new();
    for g in candidate_groups {
        let ok = std::process::Command::new("chroot")
            .arg(&rt.target)
            .arg("getent")
            .arg("group")
            .arg(g)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            groups.push(g);
        }
    }
    rt.chroot(
        &[
            "useradd",
            "-m",
            "-G",
            &groups.join(","),
            "-s",
            "/bin/bash",
            "-c",
            &sel.fullname,
            user,
        ],
        true,
        None,
    )?;
    rt.chroot(
        &["chpasswd"],
        true,
        Some(format!("{user}:{}\n", sel.password).as_bytes()),
    )?;
    if rt.target.join("usr/bin/bash").exists() {
        let _ = rt.chroot(&["usermod", "-s", "/bin/bash", "root"], false, None);
    }
    if sel.root_same {
        rt.chroot(
            &["chpasswd"],
            true,
            Some(format!("root:{}\n", sel.password).as_bytes()),
        )?;
    } else {
        rt.chroot(&["passwd", "-l", "root"], true, None)?;
    }
    rt.write(
        "/etc/sudoers.d/10-wheel",
        "%wheel ALL=(ALL:ALL) ALL\n",
        Some(0o440),
    )?;
    if sel.autologin {
        autologin(rt, conf, user)?;
    }

    // Dienste & Branding
    progress(0.84, "Dienste werden aktiviert");
    let mut services = conf.services.clone();
    if info.fs == "btrfs" {
        services.push("btrfs-scrub@-.timer".to_string());
    }
    for svc in &services {
        let _ = rt.chroot(&["systemctl", "enable", svc], false, None);
    }
    let brand_script_rel = format!("/usr/local/bin/{gid}-apply-branding");
    if rt
        .target
        .join(brand_script_rel.trim_start_matches('/'))
        .exists()
    {
        let _ = rt.chroot(&[brand_script_rel.as_str()], false, None);
    }
    Ok(())
}

fn autologin(rt: &Runner, conf: &InstallerConf, user: &str) -> Result<()> {
    match conf.desktop.as_str() {
        "kde" | "lxqt" => {
            let session = if conf.desktop == "kde" {
                "plasma"
            } else {
                "lxqt"
            };
            rt.write(
                "/etc/sddm.conf.d/autologin.conf",
                &format!("[Autologin]\nUser={user}\nSession={session}\n"),
                None,
            )?;
        }
        "gnome" => {
            rt.write(
                "/etc/gdm/custom.conf",
                &format!("[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin={user}\n"),
                None,
            )?;
        }
        "xfce" | "cinnamon" => {
            let _ = rt.chroot(&["groupadd", "-rf", "autologin"], false, None);
            let _ = rt.chroot(&["gpasswd", "-a", user, "autologin"], false, None);
            rt.write(
                "/etc/lightdm/lightdm.conf.d/50-autologin.conf",
                &format!(
                    "[Seat:*]\nautologin-user={user}\nautologin-session={}\n",
                    conf.desktop
                ),
                None,
            )?;
        }
        _ => {}
    }
    Ok(())
}
