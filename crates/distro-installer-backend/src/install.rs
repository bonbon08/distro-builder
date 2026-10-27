//! Copying the installed system onto disk: either unpacking the ISO's own
//! squashfs image (offline) or `pacstrap`ing packages from the internet
//! (online). Mirrors `install_offline`/`install_online`/`pacstrap`/`find_sfs`
//! from the Python `backend.py`.

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use distro_builder_core::installer_conf::{InstallerConf, Selection};

use crate::run::{spawn_merged, Runner};
use crate::{out, progress};

const SFS_CANDIDATES: [&str; 2] =
    ["/run/archiso/copytoram/airootfs.sfs", "/run/archiso/bootmnt/arch/x86_64/airootfs.sfs"];

fn find_sfs() -> Option<PathBuf> {
    for p in SFS_CANDIDATES {
        if Path::new(p).exists() {
            return Some(PathBuf::from(p));
        }
    }
    glob::glob("/run/archiso/bootmnt/*/x86_64/airootfs.sfs").ok()?.flatten().next()
}

pub fn target_packages(conf: &InstallerConf, sel: &Selection) -> Vec<String> {
    let mut pkgs = conf.packages.clone();
    if sel.filesystem == "btrfs" {
        pkgs.push("btrfs-progs".to_string());
    }
    if sel.bootloader == "systemd-boot" {
        pkgs.retain(|p| p != "grub");
    }
    pkgs
}

pub fn install_offline(rt: &mut Runner, conf: &InstallerConf, sel: &Selection) -> Result<()> {
    let sfs = find_sfs().ok_or_else(|| {
        anyhow!("Das System-Abbild der ISO (airootfs.sfs) wurde nicht gefunden. Bitte die Online-Installation wählen.")
    })?;
    progress(0.10, "System wird von der ISO kopiert");
    let target = rt.target.to_str().unwrap().to_string();
    let sfs_str = sfs.to_str().unwrap().to_string();
    let cmd = ["unsquashfs", "-f", "-d", target.as_str(), "-percentage", sfs_str.as_str()];
    out(&format!("$ {}", cmd.join(" ")));
    let (mut child, reader) = spawn_merged(&cmd, None)?;
    let mut br = BufReader::new(reader);
    let mut last = -1i64;
    let mut line = String::new();
    loop {
        line.clear();
        if br.read_line(&mut line)? == 0 {
            break;
        }
        let l = line.trim();
        if let Ok(pct) = l.parse::<i64>() {
            if pct != last {
                last = pct;
                progress(0.10 + 0.50 * pct as f64 / 100.0, &format!("System wird von der ISO kopiert ({pct} %)"));
            }
        } else if !l.is_empty() {
            out(l);
        }
    }
    if !child.wait()?.success() {
        bail!("Kopieren des System-Abbilds ist fehlgeschlagen.");
    }

    progress(0.61, "Live-System-Reste werden entfernt");
    for rel in &conf.live_files {
        let path = rt.target.join(rel.trim_start_matches('/'));
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let mut dirs = conf.live_dirs.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
    for rel in dirs {
        let _ = std::fs::remove_dir(rt.target.join(rel.trim_start_matches('/')));
    }

    progress(0.63, "Paketschlüssel werden eingerichtet");
    let _ = std::fs::remove_dir_all(rt.target.join("etc/pacman.d/gnupg"));
    rt.chroot(&["pacman-key", "--init"], true, None)?;
    rt.chroot(&["pacman-key", "--populate"], true, None)?;
    let _ = rt.chroot(&["gpgconf", "--homedir", "/etc/pacman.d/gnupg", "--kill", "all"], false, None);

    progress(0.66, "Nur-Live-Pakete werden entfernt");
    let installed: std::collections::HashSet<String> =
        rt.chroot_out(&["pacman", "-Qq"]).split_whitespace().map(str::to_string).collect();
    let mut keep = std::collections::HashSet::new();
    for name in target_packages(conf, sel) {
        if installed.contains(&name) {
            keep.insert(name);
        } else {
            for x in rt.chroot_out(&["pacman", "-Qgq", &name]).split_whitespace() {
                if installed.contains(x) {
                    keep.insert(x.to_string());
                }
            }
        }
    }
    let mut installed_sorted: Vec<&str> = installed.iter().map(String::as_str).collect();
    installed_sorted.sort();
    let mut asdeps_args: Vec<&str> = vec!["pacman", "-D", "--asdeps"];
    asdeps_args.extend(installed_sorted.iter());
    let _ = rt.chroot(&asdeps_args, false, None);
    let mut keep_sorted: Vec<&str> = keep.iter().map(String::as_str).collect();
    keep_sorted.sort();
    let mut asexplicit_args: Vec<&str> = vec!["pacman", "-D", "--asexplicit"];
    asexplicit_args.extend(keep_sorted.iter());
    let _ = rt.chroot(&asexplicit_args, false, None);
    for _ in 0..5 {
        let orphans: Vec<String> = rt.chroot_out(&["pacman", "-Qtdq"]).split_whitespace().map(str::to_string).collect();
        if orphans.is_empty() {
            break;
        }
        out(&format!("Entferne {} Pakete, die nur im Live-System gebraucht werden", orphans.len()));
        let mut args: Vec<&str> = vec!["pacman", "-Rns", "--noconfirm"];
        args.extend(orphans.iter().map(String::as_str));
        let _ = rt.chroot(&args, false, None);
    }

    // mkarchiso leert /boot – Kernel aus /usr/lib/modules wiederherstellen
    progress(0.70, "Kernel wird eingerichtet");
    for pkgbase_file in glob::glob(&format!("{}/usr/lib/modules/*/pkgbase", rt.target.display()))?.flatten() {
        let pkgbase = std::fs::read_to_string(&pkgbase_file)?.trim().to_string();
        let src = pkgbase_file.parent().unwrap().join("vmlinuz");
        if src.exists() {
            std::fs::copy(&src, rt.target.join(format!("boot/vmlinuz-{pkgbase}")))?;
            out(&format!("Kernel: vmlinuz-{pkgbase}"));
        }
    }
    Ok(())
}

fn copy_tree_merge(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            let link = std::fs::read_link(entry.path())?;
            let _ = std::fs::remove_file(&target);
            std::os::unix::fs::symlink(link, &target)?;
        } else if ty.is_dir() {
            copy_tree_merge(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn install_online(rt: &Runner, conf: &InstallerConf, sel: &Selection, uefi: bool) -> Result<()> {
    progress(0.10, "Pakete werden heruntergeladen und installiert (das dauert)");
    let _ = rt.run(&["systemctl", "start", "pacman-init.service"], false, None, false);
    let mut pkgs = target_packages(conf, sel);
    if !uefi {
        pkgs.retain(|p| p != "efibootmgr");
    }
    pacstrap(rt, &pkgs)?;
    // Branding und vorab geladene Daten (KI-Modelle, Container) von der ISO übernehmen
    let mut sources = vec![format!("/usr/share/{}/overlay", conf.id)];
    sources.extend(conf.data_paths.clone());
    for src in sources {
        if !Path::new(&src).exists() {
            continue;
        }
        out(&format!("Übernehme {src}"));
        if src.ends_with("/overlay") {
            copy_tree_merge(Path::new(&src), &rt.target)?;
        } else {
            rt.run(&["cp", "-a", "--parents", &src, rt.target.to_str().unwrap()], true, None, false)?;
        }
    }
    for entry in glob::glob(&format!("{}/usr/local/bin/*", rt.target.display()))?.flatten() {
        let _ = std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// pacstrap mit Fortschrittsanzeige zwischen 10 % und 70 %.
fn pacstrap(rt: &Runner, pkgs: &[String]) -> Result<()> {
    let target = rt.target.to_str().unwrap().to_string();
    let mut cmd: Vec<String> = vec!["pacstrap".into(), "-C".into(), "/etc/pacman.conf".into(), target];
    cmd.extend(pkgs.iter().cloned());
    out(&format!("$ {}", cmd.join(" ")));
    let cmd_refs: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let (mut child, reader) = spawn_merged(&cmd_refs, None)?;
    let re = regex::Regex::new(r"\((\s*\d+)/(\d+)\) (installing|upgrading) (\S+)").unwrap();
    let mut br = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        if br.read_line(&mut line)? == 0 {
            break;
        }
        let l = line.trim_end_matches('\n');
        out(l);
        if let Some(m) = re.captures(l) {
            let done: f64 = m[1].trim().parse().unwrap_or(0.0);
            let total: f64 = m[2].parse::<f64>().unwrap_or(1.0).max(1.0);
            progress(0.25 + 0.45 * done / total, &format!("Installiere {} ({}/{})", &m[4], m[1].trim(), &m[2]));
        } else if l.to_lowercase().contains("downloading") {
            progress(0.15, "Pakete werden heruntergeladen");
        }
    }
    if !child.wait()?.success() {
        bail!("pacstrap ist fehlgeschlagen – besteht eine Internetverbindung?");
    }
    Ok(())
}
