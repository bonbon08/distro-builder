//! Shared JSON schema written by `distro-builder` into the ISO at
//! `/etc/distro-installer.json`, and read by the `distro-installer` /
//! `distro-installer-backend` binaries that run inside the live system.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallerDefaults {
    pub locale: String,
    pub timezone: String,
    pub keyboard: String,
    pub filesystem: String,
    pub source: String,
    pub encrypt: bool,
    pub bootloader: String,
    pub swap_gb: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallerConf {
    pub name: String,
    pub id: String,
    pub hostname: String,
    pub logo: String,
    pub welcome: String,
    pub packages: Vec<String>,
    pub services: Vec<String>,
    pub kernel_params: Vec<String>,
    pub zram: bool,
    pub allow_online: bool,
    pub data_paths: Vec<String>,
    pub live_files: Vec<String>,
    pub live_dirs: Vec<String>,
    pub desktop: String,
    pub fastfetch_on_terminal: bool,
    pub defaults: InstallerDefaults,
}

/// An extra (non-system) disk the user assigned a role to in the installer
/// wizard: `role` is one of "home", "data", "raid".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraDisk {
    pub path: String,
    pub role: String,
}

/// The installer wizard's final answers, written to a temp JSON file by
/// `distro-installer` and read by `distro-installer-backend` (first CLI arg).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub disk: String,
    pub extra_disks: Vec<ExtraDisk>,
    pub filesystem: String,
    pub swap_gb: u32,
    pub encrypt: bool,
    pub luks_password: String,
    pub bootloader: String,
    pub source: String,
    pub locale: String,
    pub timezone: String,
    pub keyboard: String,
    pub fullname: String,
    pub username: String,
    pub hostname: String,
    pub password: String,
    pub root_same: bool,
    pub autologin: bool,
}

pub const CONF_PATH: &str = "/etc/distro-installer.json";
pub const OLLAMA_MODELS_DIR: &str = "/var/lib/ollama/models";

/// Where the backend copies the installed system to while it is being
/// assembled under the live environment.
pub const INSTALL_TARGET: &str = "/mnt/distro-installer-target";
