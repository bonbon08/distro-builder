//! Disk partitioning, LUKS2 encryption, filesystem creation and mounting.
//! Mirrors `prepare_disks`/`wipe`/`settle`/`luks_format`/`mkfs`/`make_swap`
//! from the Python `backend.py`.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;

use anyhow::{bail, Result};
use distro_builder_core::installer_conf::Selection;

use crate::run::Runner;
use crate::{out, progress};

#[derive(Debug, Default, Clone)]
pub struct DiskInfo {
    /// (LUKS UUID of the underlying partition, mapper name) for every
    /// encrypted root/mirror device.
    pub luks_root: Vec<(String, String)>,
    pub crypttab: Vec<String>,
    pub fs: String,
    pub swap: Option<String>,
    pub root_dev: String,
    pub root_uuid: String,
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn partitions(rt: &Runner, disk: &str) -> Result<Vec<String>> {
    let text = rt.capture(&["lsblk", "-lnpo", "NAME,TYPE", disk], true)?;
    Ok(text
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next()?;
            let ty = it.next()?;
            (ty == "part").then(|| name.to_string())
        })
        .collect())
}

fn uuid(rt: &Runner, dev: &str) -> Result<String> {
    rt.capture(&["blkid", "-s", "UUID", "-o", "value", dev], true)
}

fn wipe(rt: &Runner, disk: &str) -> Result<()> {
    for part in partitions(rt, disk)? {
        let _ = rt.run(&["umount", "-q", &part], false, None, true);
        let _ = rt.run(&["swapoff", &part], false, None, true);
    }
    rt.run(&["wipefs", "-af", disk], true, None, false)?;
    rt.run(&["sgdisk", "--zap-all", disk], true, None, false)?;
    Ok(())
}

fn settle(rt: &Runner, disk: &str, expected: usize) -> Result<Vec<String>> {
    let _ = rt.run(&["partprobe", disk], false, None, false);
    let _ = rt.run(&["udevadm", "settle"], false, None, false);
    let parts = partitions(rt, disk)?;
    if parts.len() < expected {
        bail!("Partitionen auf {disk} wurden nicht erkannt.");
    }
    Ok(parts)
}

/// Creates and opens a LUKS2 container, returning `/dev/mapper/<name>`.
fn luks_format(rt: &mut Runner, part: &str, name: &str, passphrase: &str, keyfile: Option<&str>) -> Result<String> {
    out(&format!("Verschlüssele {part} (LUKS2) …"));
    if let Some(kf) = keyfile {
        rt.run(&["cryptsetup", "luksFormat", "--type", "luks2", "--batch-mode", "--key-file", kf, part], true, None, false)?;
        if !passphrase.is_empty() {
            rt.run(&["cryptsetup", "luksAddKey", "--key-file", kf, "--batch-mode", part, "-"], true, Some(passphrase.as_bytes()), false)?;
        }
        rt.run(&["cryptsetup", "open", "--key-file", kf, part, name], true, None, false)?;
    } else {
        rt.run(&["cryptsetup", "luksFormat", "--type", "luks2", "--batch-mode", "--key-file", "-", part], true, Some(passphrase.as_bytes()), false)?;
        rt.run(&["cryptsetup", "open", "--key-file", "-", part, name], true, Some(passphrase.as_bytes()), false)?;
    }
    rt.opened_luks.push(name.to_string());
    Ok(format!("/dev/mapper/{name}"))
}

fn mkfs(rt: &Runner, fs: &str, devices: &[String], label: &str, raid: bool) -> Result<()> {
    if fs == "btrfs" {
        let mut cmd: Vec<String> = vec!["mkfs.btrfs".into(), "-f".into(), "-L".into(), truncate(label, 16)];
        if raid {
            cmd.extend(["-d".to_string(), "raid1".to_string(), "-m".to_string(), "raid1".to_string()]);
        }
        cmd.extend(devices.iter().cloned());
        let refs: Vec<&str> = cmd.iter().map(String::as_str).collect();
        rt.run(&refs, true, None, false)?;
    } else {
        rt.run(&["mkfs.ext4", "-F", "-L", &truncate(label, 16), &devices[0]], true, None, false)?;
    }
    Ok(())
}

fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    buf
}

/// Partitions every involved disk and mounts everything under `rt.target`.
/// Returns info needed later by `configure`/`bootloader`.
pub fn prepare_disks(rt: &mut Runner, sel: &Selection, gid: &str, uefi: bool) -> Result<DiskInfo> {
    let fs = sel.filesystem.as_str();
    let encrypt = sel.encrypt;
    let pw = sel.luks_password.as_str();
    let disk = sel.disk.as_str();
    let extras = &sel.extra_disks;
    let mut info = DiskInfo { fs: fs.to_string(), ..Default::default() };

    // Systemplatte
    wipe(rt, disk)?;
    let root_type = if encrypt { "8309" } else { "8304" };
    let (boot_part, root_part);
    if uefi {
        rt.run(
            &["sgdisk", "-n1:0:+1G", "-t1:ef00", "-c1:EFI", "-n2:0:0", &format!("-t2:{root_type}"), &format!("-c2:{gid}"), disk],
            true, None, false,
        )?;
        let parts = settle(rt, disk, 2)?;
        boot_part = parts[0].clone();
        root_part = parts[1].clone();
    } else {
        rt.run(
            &["sgdisk", "-n1:0:+1M", "-t1:ef02", "-c1:BIOS", "-n2:0:+1G", "-t2:8300", "-c2:boot", "-n3:0:0",
              &format!("-t3:{root_type}"), &format!("-c3:{gid}"), disk],
            true, None, false,
        )?;
        let parts = settle(rt, disk, 3)?;
        boot_part = parts[1].clone();
        root_part = parts[2].clone();
    }

    // RAID1-Spiegel (nur btrfs)
    let mut raid_parts = Vec::new();
    for (i, ex) in extras.iter().filter(|e| e.role == "raid").enumerate() {
        wipe(rt, &ex.path)?;
        rt.run(&["sgdisk", "-n1:0:0", &format!("-t1:{root_type}"), &format!("-c1:{gid}-mirror{i}"), &ex.path], true, None, false)?;
        raid_parts.push(settle(rt, &ex.path, 1)?[0].clone());
    }

    progress(0.05, "Dateisysteme werden erstellt");
    let mut all_root_parts = vec![root_part];
    all_root_parts.extend(raid_parts);
    let mut root_devs = Vec::new();
    for (i, part) in all_root_parts.iter().enumerate() {
        if encrypt {
            let name = if i == 0 { "cryptroot".to_string() } else { format!("cryptroot{}", i + 1) };
            root_devs.push(luks_format(rt, part, &name, pw, None)?);
            info.luks_root.push((uuid(rt, part)?, name));
        } else {
            root_devs.push(part.clone());
        }
    }
    mkfs(rt, fs, &root_devs, gid, root_devs.len() > 1)?;
    info.root_dev = root_devs[0].clone();
    info.root_uuid = uuid(rt, &root_devs[0])?;

    let separate_home = extras.iter().any(|e| e.role == "home");
    std::fs::create_dir_all(&rt.target)?;
    let target_str = rt.target.to_str().unwrap().to_string();
    if fs == "btrfs" {
        if root_devs.len() > 1 {
            let _ = rt.run(&["btrfs", "device", "scan"], false, None, false);
        }
        rt.run(&["mount", &root_devs[0], &target_str], true, None, false)?;
        let mut subvols = vec!["@".to_string(), "@log".to_string(), "@pkg".to_string(), "@snapshots".to_string()];
        if !separate_home {
            subvols.push("@home".to_string());
        }
        if sel.swap_gb > 0 && root_devs.len() == 1 {
            subvols.push("@swap".to_string());
        }
        for sub in &subvols {
            rt.run(&["btrfs", "subvolume", "create", &format!("{target_str}/{sub}")], true, None, false)?;
        }
        rt.run(&["umount", &target_str], true, None, false)?;
        let opts = "compress=zstd,noatime";
        rt.run(&["mount", "-o", &format!("subvol=@,{opts}"), &root_devs[0], &target_str], true, None, false)?;
        let mut mounts = vec![("@log", "var/log"), ("@pkg", "var/cache/pacman/pkg"), ("@snapshots", ".snapshots")];
        if !separate_home {
            mounts.push(("@home", "home"));
        }
        if subvols.iter().any(|s| s == "@swap") {
            mounts.push(("@swap", "swap"));
        }
        for (sub, mnt) in mounts {
            let full = rt.target.join(mnt);
            std::fs::create_dir_all(&full)?;
            let o = if sub == "@swap" { "noatime".to_string() } else { opts.to_string() };
            rt.run(&["mount", "-o", &format!("subvol={sub},{o}"), &root_devs[0], full.to_str().unwrap()], true, None, false)?;
        }
    } else {
        rt.run(&["mount", &root_devs[0], &target_str], true, None, false)?;
    }

    // Bootpartition
    let boot_dir = rt.target.join("boot");
    std::fs::create_dir_all(&boot_dir)?;
    if uefi {
        rt.run(&["mkfs.fat", "-F32", "-n", "EFI", &boot_part], true, None, false)?;
        rt.run(&["mount", "-o", "fmask=0077,dmask=0077", &boot_part, boot_dir.to_str().unwrap()], true, None, false)?;
    } else {
        rt.run(&["mkfs.ext4", "-F", "-L", "boot", &boot_part], true, None, false)?;
        rt.run(&["mount", &boot_part, boot_dir.to_str().unwrap()], true, None, false)?;
    }

    // /home und Datenlaufwerke
    let mut data_n = 0u32;
    for ex in extras.iter().filter(|e| e.role == "home" || e.role == "data") {
        wipe(rt, &ex.path)?;
        let ptype = if encrypt { "8309" } else if ex.role == "home" { "8302" } else { "8300" };
        rt.run(&["sgdisk", "-n1:0:0", &format!("-t1:{ptype}"), &ex.path], true, None, false)?;
        let part = settle(rt, &ex.path, 1)?[0].clone();
        let (mnt, name, label) = if ex.role == "home" {
            ("/home".to_string(), "crypthome".to_string(), "home".to_string())
        } else {
            data_n += 1;
            let suffix = if data_n == 1 { String::new() } else { data_n.to_string() };
            (format!("/data{suffix}"), format!("cryptdata{suffix}"), format!("data{suffix}"))
        };
        let mut dev = part.clone();
        if encrypt {
            let keydir = rt.target.join("etc/cryptsetup-keys.d");
            std::fs::create_dir_all(&keydir)?;
            std::fs::set_permissions(&keydir, std::fs::Permissions::from_mode(0o700))?;
            let keyfile = keydir.join(format!("{name}.key"));
            std::fs::write(&keyfile, random_bytes(512))?;
            std::fs::set_permissions(&keyfile, std::fs::Permissions::from_mode(0o400))?;
            dev = luks_format(rt, &part, &name, pw, Some(keyfile.to_str().unwrap()))?;
            info.crypttab.push(format!("{name} UUID={} /etc/cryptsetup-keys.d/{name}.key luks,discard", uuid(rt, &part)?));
        }
        let mfs = if fs == "btrfs" { "btrfs" } else { "ext4" };
        mkfs(rt, mfs, &[dev.clone()], &label, false)?;
        let full = rt.target.join(mnt.trim_start_matches('/'));
        std::fs::create_dir_all(&full)?;
        let opt = if fs == "btrfs" { "compress=zstd,noatime" } else { "defaults" };
        rt.run(&["mount", "-o", opt, &dev, full.to_str().unwrap()], true, None, false)?;
    }
    Ok(info)
}

pub fn make_swap(rt: &Runner, sel: &Selection, info: &mut DiskInfo) -> Result<()> {
    let gb = sel.swap_gb;
    if gb == 0 {
        return Ok(());
    }
    let path;
    if info.fs == "btrfs" {
        let swap_dir = rt.target.join("swap");
        if info.luks_root.len() > 1 || !swap_dir.is_dir() {
            out("Hinweis: Swapfile auf btrfs-RAID wird nicht unterstützt – übersprungen.");
            return Ok(());
        }
        path = "/swap/swapfile".to_string();
        rt.run(
            &["btrfs", "filesystem", "mkswapfile", "--size", &format!("{gb}g"), "--uuid", "clear",
              rt.target.join("swap/swapfile").to_str().unwrap()],
            true, None, false,
        )?;
    } else {
        path = "/swapfile".to_string();
        rt.run(
            &["mkswap", "-U", "clear", "--size", &format!("{gb}G"), "--file", rt.target.join("swapfile").to_str().unwrap()],
            true, None, false,
        )?;
    }
    rt.append("/etc/fstab", &format!("\n# Swapfile\n{path} none swap defaults 0 0\n"))?;
    info.swap = Some(path);
    Ok(())
}
