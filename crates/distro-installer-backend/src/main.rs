//! Root-privileged install backend. Reads `/etc/distro-installer.json` plus
//! a `Selection` JSON file (path given as argv[1]), partitions/formats the
//! chosen disk(s), copies the system onto them (offline from the ISO's own
//! squashfs, or online via pacstrap), configures it, and installs a
//! bootloader. Mirrors the Python `backend.py` wire protocol exactly:
//! stdout lines `##PROGRESS <0..1> <text>`, `##ERROR## <msg>`, `##DONE##`
//! are parsed by `distro-installer`'s progress page.

mod bootloader;
mod configure;
mod disks;
mod install;
mod run;

use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use distro_builder_core::installer_conf::{InstallerConf, Selection, CONF_PATH, INSTALL_TARGET};

use run::Runner;

pub fn out(msg: &str) {
    println!("{msg}");
    let _ = std::io::stdout().flush();
}

pub fn progress(frac: f64, text: &str) {
    out(&format!("##PROGRESS {frac:.3} {text}"));
}

fn geteuid() -> u32 {
    extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

fn load_inputs() -> Result<(InstallerConf, Selection)> {
    let conf_text = std::fs::read_to_string(CONF_PATH).with_context(|| format!("Konfiguration {CONF_PATH} nicht lesbar"))?;
    let conf: InstallerConf = serde_json::from_str(&conf_text).context("Ungültige Konfiguration")?;
    let sel_path = std::env::args().nth(1).ok_or_else(|| anyhow!("Kein Auswahl-Pfad übergeben"))?;
    let sel_text = std::fs::read_to_string(&sel_path).with_context(|| format!("Auswahl-Datei {sel_path} nicht lesbar"))?;
    let sel: Selection = serde_json::from_str(&sel_text).context("Ungültige Auswahl")?;
    Ok((conf, sel))
}

fn real_main(rt: &mut Runner) -> Result<()> {
    let (conf, mut sel) = load_inputs()?;
    let uefi = std::path::Path::new("/sys/firmware/efi").is_dir();
    if sel.bootloader == "systemd-boot" && !uefi {
        sel.bootloader = "grub".to_string();
    }
    if sel.source.is_empty() {
        sel.source = "offline".to_string();
    }

    let mode = if sel.source == "offline" { "von der ISO (offline)" } else { "aus dem Internet" };
    out(&format!(
        "Installiere {} {mode} auf {} ({}, {}, {})",
        conf.name,
        sel.disk,
        sel.filesystem,
        if uefi { "UEFI" } else { "BIOS" },
        if sel.encrypt { "verschlüsselt" } else { "unverschlüsselt" }
    ));
    for ex in &sel.extra_disks {
        out(&format!("  zusätzlich: {} als {}", ex.path, ex.role));
    }

    progress(0.01, "Festplatten werden vorbereitet");
    rt.cleanup();
    let mut info = disks::prepare_disks(rt, &sel, &conf.id, uefi)?;
    if sel.source == "offline" {
        install::install_offline(rt, &conf, &sel)?;
    } else {
        install::install_online(rt, &conf, &sel, uefi)?;
    }
    configure::configure(rt, &conf, &sel, &mut info, uefi)?;
    bootloader::bootloader(rt, &conf, &sel, &info, uefi)?;

    progress(0.98, "Abschluss");
    rt.run(&["sync"], true, None, false)?;
    rt.cleanup();
    progress(1.0, "Fertig");
    out("##DONE##");
    Ok(())
}

fn main() {
    if geteuid() != 0 {
        out("##ERROR## Das Backend muss als root laufen.");
        std::process::exit(1);
    }
    let mut rt = Runner::new(PathBuf::from(INSTALL_TARGET));
    if let Err(e) = real_main(&mut rt) {
        out(&format!("##ERROR## {e}"));
        rt.cleanup();
        std::process::exit(1);
    }
}
