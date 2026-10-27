//! Reading the live system for installer choices: config file, locales,
//! timezones, keyboard layouts, and candidate disks. Mirrors the
//! module-level helper functions in the Python `installer.py`.

use std::path::Path;
use std::process::Command;

use distro_builder_core::installer_conf::{InstallerConf, InstallerDefaults, CONF_PATH};
use serde::Deserialize;

pub fn load_conf() -> InstallerConf {
    std::fs::read_to_string(CONF_PATH)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| InstallerConf {
            name: "Linux".to_string(),
            id: "linux".to_string(),
            hostname: "linux".to_string(),
            logo: String::new(),
            welcome: String::new(),
            defaults: InstallerDefaults {
                locale: "en_US.UTF-8".to_string(),
                timezone: "UTC".to_string(),
                keyboard: "us".to_string(),
                filesystem: "ext4".to_string(),
                ..Default::default()
            },
            ..Default::default()
        })
}

fn cmd_lines(cmd: &str, args: &[&str]) -> Vec<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_locales() -> Vec<String> {
    let mut locs = Vec::new();
    if let Ok(text) = std::fs::read_to_string("/usr/share/i18n/SUPPORTED") {
        for line in text.lines() {
            if let Some((name, charset)) = line.trim().split_once(' ') {
                if charset == "UTF-8" {
                    locs.push(name.to_string());
                }
            }
        }
    }
    if locs.is_empty() {
        locs = vec!["en_US.UTF-8".to_string(), "de_DE.UTF-8".to_string()];
    }
    locs
}

pub fn list_timezones() -> Vec<String> {
    let tz = cmd_lines("timedatectl", &["list-timezones"]);
    if tz.is_empty() {
        vec!["UTC".to_string(), "Europe/Berlin".to_string()]
    } else {
        tz
    }
}

pub fn list_layouts() -> Vec<String> {
    let layouts = cmd_lines("localectl", &["list-x11-keymap-layouts"]);
    if layouts.is_empty() {
        vec!["us".to_string(), "de".to_string()]
    } else {
        layouts
    }
}

/// Datenträger, von dem das Live-System gestartet wurde (wird ausgeblendet).
fn boot_disk() -> Option<String> {
    let src = cmd_lines("findmnt", &["-no", "SOURCE", "/run/archiso/bootmnt"]);
    let src = src.first()?;
    let parent = cmd_lines("lsblk", &["-npo", "PKNAME", src]);
    match parent.first() {
        Some(p) if !p.is_empty() => Some(p.clone()),
        _ => Some(src.clone()),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskDevice {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub size: u64,
    pub model: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub tran: Option<String>,
    #[serde(default)]
    pub ro: bool,
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<DiskDevice>,
}

pub fn list_disks() -> Vec<DiskDevice> {
    let output = match Command::new("lsblk")
        .args(["-J", "-b", "-d", "-o", "NAME,PATH,SIZE,MODEL,TYPE,TRAN,RO"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let parsed: LsblkOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let skip = boot_disk();
    parsed
        .blockdevices
        .into_iter()
        .filter(|d| {
            d.kind == "disk"
                && !d.ro
                && skip.as_deref() != Some(d.path.as_str())
                && !d.name.starts_with("loop")
                && !d.name.starts_with("zram")
                && !d.name.starts_with("sr")
                && d.size > 8 * 1024u64.pow(3)
        })
        .collect()
}

pub const DISK_ROLES: &[(&str, &str)] = &[
    ("none", "Nicht verwenden"),
    ("home", "/home (Benutzerdaten)"),
    ("data", "Datenlaufwerk (/data)"),
    ("raid", "btrfs-Spiegel der Systemplatte (RAID 1)"),
];

pub fn has_offline_image() -> bool {
    [
        "/run/archiso/copytoram/airootfs.sfs",
        "/run/archiso/bootmnt/arch/x86_64/airootfs.sfs",
    ]
    .iter()
    .any(|p| Path::new(p).exists())
}

pub fn human(size: u64) -> String {
    let mut size = size as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if size < 1024.0 {
            return format!("{size:.1} {unit}");
        }
        size /= 1024.0;
    }
    format!("{size:.1} PB")
}

pub fn is_uefi() -> bool {
    Path::new("/sys/firmware/efi").is_dir()
}
