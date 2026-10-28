//! Project configuration: defaults, presets, load/save, and package lists.

use std::path::Path;
use std::process::Command;

use regex::Regex;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------- Config

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub name: String,
    pub id: String,
    pub version: String,
    pub publisher: String,
    pub homepage: String,
    pub support_url: String,
    pub bug_url: String,
    pub hostname: String,
}

impl Default for General {
    fn default() -> Self {
        Self {
            name: "MeinOS".into(),
            id: "meinos".into(),
            version: "1.0".into(),
            publisher: String::new(),
            homepage: String::new(),
            support_url: String::new(),
            bug_url: String::new(),
            hostname: "meinos".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Branding {
    pub logo: String,
    /// image | chafa | ascii | builtin
    pub logo_mode: String,
    pub ascii_logo: String,
    pub color: String,
    pub wallpaper: String,
    pub motd: String,
    pub fastfetch_on_terminal: bool,
}

impl Default for Branding {
    fn default() -> Self {
        Self {
            logo: String::new(),
            logo_mode: "image".into(),
            ascii_logo: "  __  __      _       ___  ____\n |  \\/  | ___(_)_ __ / _ \\/ ___|\n \
                          | |\\/| |/ _ \\ | '_ \\ | | \\___ \\\n | |  | |  __/ | | | | |_| |___) |\n \
                          |_|  |_|\\___|_|_| |_|\\___/|____/"
                .into(),
            color: "#1793d1".into(),
            wallpaper: String::new(),
            motd: "Willkommen bei MeinOS!".into(),
            fastfetch_on_terminal: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomRepo {
    pub name: String,
    pub server: String,
    pub siglevel: String,
    #[serde(default)]
    pub first: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Repos {
    pub core: bool,
    pub extra: bool,
    pub multilib: bool,
    pub core_testing: bool,
    pub extra_testing: bool,
    pub mirror: String,
    pub custom: Vec<CustomRepo>,
}

impl Default for Repos {
    fn default() -> Self {
        Self {
            core: true,
            extra: true,
            multilib: false,
            core_testing: false,
            extra_testing: false,
            mirror: String::new(),
            custom: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Packages {
    pub kernel: String,
    pub desktop: String,
    pub presets: Vec<String>,
    pub custom: String,
    pub rescue_tools: bool,
    pub gpu: String,
}

impl Default for Packages {
    fn default() -> Self {
        Self {
            kernel: "linux".into(),
            desktop: "kde".into(),
            presets: vec![
                "audio".into(),
                "firefox".into(),
                "fonts".into(),
                "utils".into(),
            ],
            custom: String::new(),
            rescue_tools: true,
            gpu: "mesa".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallerCfg {
    pub enabled: bool,
    pub autostart: bool,
    pub locale: String,
    pub timezone: String,
    pub keyboard: String,
    pub filesystem: String,
    pub zram: bool,
    /// offline (von der ISO) | online (pacstrap)
    pub source: String,
    pub allow_online: bool,
    pub encrypt: bool,
    pub bootloader: String,
    pub swap_gb: u32,
    pub welcome: String,
}

impl Default for InstallerCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            autostart: true,
            locale: "de_DE.UTF-8".into(),
            timezone: "Europe/Berlin".into(),
            keyboard: "de".into(),
            filesystem: "ext4".into(),
            zram: true,
            source: "offline".into(),
            allow_online: true,
            encrypt: false,
            bootloader: "grub".into(),
            swap_gb: 0,
            welcome: "Dieser Assistent installiert das System auf deine Festplatte.".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Stack {
    pub name: String,
    #[serde(default)]
    pub title: String,
    pub compose: String,
    #[serde(default = "default_true")]
    pub autostart: bool,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Containers {
    /// none | docker | podman
    pub engine: String,
    pub preload: bool,
    pub images: String,
    pub stacks: Vec<Stack>,
}

impl Default for Containers {
    fn default() -> Self {
        Self {
            engine: "none".into(),
            preload: true,
            images: String::new(),
            stacks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ai {
    pub ollama: bool,
    /// cpu | cuda | rocm | vulkan
    pub accel: String,
    pub models: Vec<String>,
    pub expose: bool,
    pub dev: bool,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            ollama: false,
            accel: "cpu".into(),
            models: Vec::new(),
            expose: false,
            dev: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FlatpakRemote {
    pub name: String,
    /// URL einer .flatpakrepo-Datei
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Flatpak {
    pub enabled: bool,
    pub flathub: bool,
    /// Discover bzw. GNOME Software mit Flatpak-Unterstützung
    pub store: bool,
    /// Apps beim Bauen herunterladen und mitliefern
    pub preload: bool,
    pub remotes: Vec<FlatpakRemote>,
    /// App-IDs, optional mit Quelle: "quelle:org.example.App"
    pub apps: Vec<String>,
}

impl Default for Flatpak {
    fn default() -> Self {
        Self {
            enabled: false,
            flathub: true,
            store: true,
            preload: true,
            remotes: Vec::new(),
            apps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DistroboxBox {
    pub name: String,
    pub image: String,
    /// zusätzliche Pakete im Container
    pub packages: String,
    /// Apps, die ins Menü des Hosts übernommen werden
    pub export: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Distrobox {
    pub enabled: bool,
    pub boxes: Vec<DistroboxBox>,
}

/// Nur für ARM/Raspberry Pi: fertiges System statt ISO mit Installer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageCfg {
    pub username: String,
    pub password: String,
    /// Passwort beim ersten Anmelden ändern
    pub expire_password: bool,
    pub autologin: bool,
    pub size_gb: u32,
}

impl Default for ImageCfg {
    fn default() -> Self {
        Self {
            username: "user".into(),
            password: "user".into(),
            expire_password: true,
            autologin: false,
            size_gb: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    /// x86_64 | aarch64 | rpi
    pub arch: String,
    pub project_dir: String,
    /// zstd (schnell) | xz (klein)
    pub compression: String,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            arch: "x86_64".into(),
            project_dir: default_project_dir(),
            compression: "zstd".into(),
        }
    }
}

fn default_project_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join("DistroBuilder").to_string_lossy().into_owned())
        .unwrap_or_else(|| "~/DistroBuilder".into())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub branding: Branding,
    pub repos: Repos,
    pub packages: Packages,
    pub installer: InstallerCfg,
    pub containers: Containers,
    pub ai: Ai,
    pub flatpak: Flatpak,
    pub distrobox: Distrobox,
    pub image: ImageCfg,
    pub build: Build,
}

pub fn default_config() -> Config {
    Config::default()
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save(cfg: &Config, path: &Path) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(cfg).expect("Config always serializes");
    std::fs::write(path, text)
}

pub fn slugify(name: &str) -> String {
    let re = Regex::new(r"[^a-z0-9]+").unwrap();
    let lowered = name.to_lowercase();
    let s = re.replace_all(&lowered, "-");
    let s = s.trim_matches('-');
    let s: String = s.chars().take(31).collect();
    if s.is_empty() {
        "meinos".into()
    } else {
        s
    }
}

pub fn custom_packages(cfg: &Config) -> Vec<String> {
    split_ws_comma(&cfg.packages.custom)
}

fn split_ws_comma(s: &str) -> Vec<String> {
    let re = Regex::new(r"[\s,]+").unwrap();
    re.split(s.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

// ---------------------------------------------------------------- Static data

#[derive(Debug, Clone)]
pub struct DesktopInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
    pub display_manager: Option<&'static str>,
    pub session: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ArchInfo {
    pub label: &'static str,
    pub desc: &'static str,
    /// Architektur für pacman
    pub pacman: &'static str,
    /// Architektur für Container und OCI-Images
    pub oci: &'static str,
}

pub fn arches() -> Vec<(&'static str, ArchInfo)> {
    vec![
        (
            "x86_64",
            ArchInfo {
                label: "x86_64 (PC, Laptop)",
                desc: "Bootfähige ISO mit Live-System und grafischem Installer",
                pacman: "x86_64",
                oci: "amd64",
            },
        ),
        (
            "aarch64",
            ArchInfo {
                label: "ARM64 (UEFI)",
                desc: "Fertiges Disk-Image für ARM64-Rechner und VMs mit UEFI (Arch Linux ARM)",
                pacman: "aarch64",
                oci: "arm64",
            },
        ),
        (
            "rpi",
            ArchInfo {
                label: "Raspberry Pi 4/5 (ARM64)",
                desc: "SD-Karten-Image für Raspberry Pi 4, 400 und 5 (Arch Linux ARM)",
                pacman: "aarch64",
                oci: "arm64",
            },
        ),
    ]
}

/// Arch Linux ARM: Basissystem pro Plattform
pub fn alarm_tarball(platform: &str) -> &'static str {
    match platform {
        "rpi" => "http://os.archlinuxarm.org/os/ArchLinuxARM-rpi-aarch64-latest.tar.gz",
        _ => "http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz",
    }
}

/// Gewählte Plattform (unbekannte Werte fallen auf x86_64 zurück).
pub fn arch(cfg: &Config) -> &'static str {
    arches()
        .into_iter()
        .map(|(k, _)| k)
        .find(|k| *k == cfg.build.arch)
        .unwrap_or("x86_64")
}

pub fn arch_info(cfg: &Config) -> ArchInfo {
    let key = arch(cfg);
    arches().into_iter().find(|(k, _)| *k == key).unwrap().1
}

/// ARM-Plattformen werden als fertiges Disk-Image gebaut, x86_64 als ISO.
pub fn is_image(cfg: &Config) -> bool {
    arch(cfg) != "x86_64"
}

pub fn pacman_arch(cfg: &Config) -> &'static str {
    arch_info(cfg).pacman
}

pub fn oci_arch(cfg: &Config) -> &'static str {
    arch_info(cfg).oci
}

/// Kernel, die es auf der gewählten Plattform gibt.
pub fn kernels_for(cfg: &Config) -> Vec<(&'static str, &'static str)> {
    match arch(cfg) {
        "aarch64" => vec![("linux-aarch64", "Linux (Arch Linux ARM)")],
        "rpi" => vec![
            ("linux-rpi", "Raspberry Pi (Pi 4/400/5)"),
            ("linux-rpi-16k", "Raspberry Pi 5 (16K-Seiten, schneller)"),
        ],
        _ => kernels(),
    }
}

/// Gewählter Kernel – fällt auf den Standard der Plattform zurück, wenn er dort nicht existiert.
pub fn kernel(cfg: &Config) -> String {
    let options = kernels_for(cfg);
    options
        .iter()
        .map(|(k, _)| *k)
        .find(|k| *k == cfg.packages.kernel)
        .unwrap_or(options[0].0)
        .to_string()
}

pub fn kernels() -> Vec<(&'static str, &'static str)> {
    vec![
        ("linux", "Linux (Standard)"),
        ("linux-lts", "Linux LTS"),
        ("linux-zen", "Linux Zen"),
        ("linux-hardened", "Linux Hardened"),
    ]
}

pub fn desktops() -> Vec<(&'static str, DesktopInfo)> {
    vec![
        (
            "none",
            DesktopInfo {
                label: "Kein Desktop (nur Konsole)",
                packages: vec![],
                display_manager: None,
                session: None,
            },
        ),
        (
            "kde",
            DesktopInfo {
                label: "KDE Plasma",
                packages: vec![
                    "plasma-meta",
                    "konsole",
                    "dolphin",
                    "kate",
                    "ark",
                    "spectacle",
                    "sddm",
                ],
                display_manager: Some("sddm"),
                session: Some("plasma"),
            },
        ),
        (
            "gnome",
            DesktopInfo {
                label: "GNOME",
                packages: vec!["gnome", "gnome-tweaks", "gdm"],
                display_manager: Some("gdm"),
                session: None,
            },
        ),
        (
            "xfce",
            DesktopInfo {
                label: "Xfce",
                packages: vec![
                    "xorg-server",
                    "xfce4",
                    "xfce4-goodies",
                    "lightdm",
                    "lightdm-gtk-greeter",
                    "network-manager-applet",
                    "gvfs",
                ],
                display_manager: Some("lightdm"),
                session: Some("xfce"),
            },
        ),
        (
            "cinnamon",
            DesktopInfo {
                label: "Cinnamon",
                packages: vec![
                    "xorg-server",
                    "cinnamon",
                    "gnome-terminal",
                    "nemo",
                    "lightdm",
                    "lightdm-gtk-greeter",
                    "gvfs",
                ],
                display_manager: Some("lightdm"),
                session: Some("cinnamon"),
            },
        ),
        (
            "lxqt",
            DesktopInfo {
                label: "LXQt",
                packages: vec![
                    "xorg-server",
                    "lxqt",
                    "breeze-icons",
                    "sddm",
                    "network-manager-applet",
                ],
                display_manager: Some("sddm"),
                session: Some("lxqt"),
            },
        ),
    ]
}

pub fn desktop_info(cfg: &Config) -> DesktopInfo {
    desktops()
        .into_iter()
        .find(|(k, _)| *k == cfg.packages.desktop)
        .map(|(_, v)| v)
        .unwrap_or_else(|| desktops().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct PresetInfo {
    pub label: &'static str,
    pub desc: &'static str,
    pub packages: Vec<&'static str>,
    pub services: Vec<&'static str>,
}

pub fn presets() -> Vec<(&'static str, PresetInfo)> {
    vec![
        (
            "audio",
            PresetInfo {
                label: "Audio (PipeWire)",
                desc: "pipewire, pulse, alsa, wireplumber",
                packages: vec!["pipewire", "pipewire-pulse", "pipewire-alsa", "wireplumber"],
                services: vec![],
            },
        ),
        (
            "bluetooth",
            PresetInfo {
                label: "Bluetooth",
                desc: "bluez + Dienst",
                packages: vec!["bluez", "bluez-utils"],
                services: vec!["bluetooth.service"],
            },
        ),
        (
            "printing",
            PresetInfo {
                label: "Drucken",
                desc: "CUPS + Treiber",
                packages: vec!["cups", "cups-pdf", "gutenprint"],
                services: vec!["cups.service"],
            },
        ),
        (
            "firefox",
            PresetInfo {
                label: "Firefox",
                desc: "Webbrowser",
                packages: vec!["firefox"],
                services: vec![],
            },
        ),
        (
            "chromium",
            PresetInfo {
                label: "Chromium",
                desc: "Webbrowser",
                packages: vec!["chromium"],
                services: vec![],
            },
        ),
        (
            "office",
            PresetInfo {
                label: "LibreOffice",
                desc: "Office-Paket",
                packages: vec!["libreoffice-fresh"],
                services: vec![],
            },
        ),
        (
            "media",
            PresetInfo {
                label: "Multimedia",
                desc: "VLC, mpv, GIMP",
                packages: vec!["vlc", "mpv", "gimp"],
                services: vec![],
            },
        ),
        (
            "fonts",
            PresetInfo {
                label: "Schriften",
                desc: "Noto + Emoji",
                packages: vec!["noto-fonts", "noto-fonts-emoji", "ttf-dejavu"],
                services: vec![],
            },
        ),
        (
            "flatpak",
            PresetInfo {
                label: "Flatpak",
                desc: "Flatpak-Unterstützung",
                packages: vec!["flatpak"],
                services: vec![],
            },
        ),
        (
            "dev",
            PresetInfo {
                label: "Entwicklung",
                desc: "base-devel, git",
                packages: vec!["base-devel", "git"],
                services: vec![],
            },
        ),
        (
            "utils",
            PresetInfo {
                label: "Werkzeuge",
                desc: "htop, wget, unzip, man",
                packages: vec!["htop", "wget", "unzip", "zip", "man-db", "man-pages"],
                services: vec![],
            },
        ),
        (
            "gaming",
            PresetInfo {
                label: "Gaming",
                desc: "Steam (benötigt multilib)",
                packages: vec!["steam"],
                services: vec![],
            },
        ),
    ]
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
}

pub fn gpus() -> Vec<(&'static str, GpuInfo)> {
    vec![
        (
            "mesa",
            GpuInfo {
                label: "Standard (Mesa, freie Treiber)",
                packages: vec!["mesa", "vulkan-icd-loader"],
            },
        ),
        (
            "amd",
            GpuInfo {
                label: "AMD Radeon (Mesa + Vulkan)",
                packages: vec!["mesa", "vulkan-radeon", "vulkan-icd-loader"],
            },
        ),
        (
            "intel",
            GpuInfo {
                label: "Intel (Mesa + Vulkan + VA-API)",
                packages: vec![
                    "mesa",
                    "vulkan-intel",
                    "intel-media-driver",
                    "vulkan-icd-loader",
                ],
            },
        ),
        (
            "nvidia",
            GpuInfo {
                label: "NVIDIA (proprietär, nvidia-open)",
                packages: vec![
                    "nvidia-open-dkms",
                    "nvidia-utils",
                    "nvidia-settings",
                    "vulkan-icd-loader",
                ],
            },
        ),
    ]
}

/// Grafiktreiber, die es auf der gewählten Plattform gibt (ARM: nur Mesa).
pub fn gpus_for(cfg: &Config) -> Vec<(&'static str, GpuInfo)> {
    if is_image(cfg) {
        gpus().into_iter().filter(|(k, _)| *k == "mesa").collect()
    } else {
        gpus()
    }
}

fn gpu_info(key: &str) -> GpuInfo {
    gpus()
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| gpus().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct EngineInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
    pub services: Vec<&'static str>,
}

pub fn container_engines() -> Vec<(&'static str, EngineInfo)> {
    vec![
        (
            "none",
            EngineInfo {
                label: "Keine",
                packages: vec![],
                services: vec![],
            },
        ),
        (
            "docker",
            EngineInfo {
                label: "Docker",
                packages: vec!["docker", "docker-compose", "docker-buildx", "lazydocker"],
                services: vec!["docker.service"],
            },
        ),
        (
            "podman",
            EngineInfo {
                label: "Podman (ohne Daemon, rootless)",
                packages: vec!["podman", "podman-compose", "podman-docker", "distrobox"],
                services: vec!["podman.socket"],
            },
        ),
    ]
}

fn engine_info(key: &str) -> EngineInfo {
    container_engines()
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| container_engines().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct AccelInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
}

pub fn ollama_accel() -> Vec<(&'static str, AccelInfo)> {
    vec![
        (
            "cpu",
            AccelInfo {
                label: "Nur CPU",
                packages: vec!["ollama"],
            },
        ),
        (
            "cuda",
            AccelInfo {
                label: "NVIDIA (CUDA)",
                packages: vec!["ollama", "ollama-cuda"],
            },
        ),
        (
            "rocm",
            AccelInfo {
                label: "AMD (ROCm)",
                packages: vec!["ollama", "ollama-rocm"],
            },
        ),
        (
            "vulkan",
            AccelInfo {
                label: "Vulkan (AMD/Intel/NVIDIA)",
                packages: vec!["ollama", "ollama-vulkan", "vulkan-icd-loader"],
            },
        ),
    ]
}

fn accel_info(key: &str) -> AccelInfo {
    ollama_accel()
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| ollama_accel().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct ModelPreset {
    pub name: &'static str,
    pub desc: &'static str,
    pub size_gb: f64,
}

pub fn model_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset {
            name: "llama3.2:3b",
            desc: "Meta Llama 3.2 – 3B, schnell",
            size_gb: 2.0,
        },
        ModelPreset {
            name: "llama3.1:8b",
            desc: "Meta Llama 3.1 – 8B, Allrounder",
            size_gb: 4.9,
        },
        ModelPreset {
            name: "qwen3:8b",
            desc: "Qwen 3 – 8B, stark mehrsprachig",
            size_gb: 5.2,
        },
        ModelPreset {
            name: "gemma3:4b",
            desc: "Google Gemma 3 – 4B",
            size_gb: 3.3,
        },
        ModelPreset {
            name: "mistral:7b",
            desc: "Mistral 7B",
            size_gb: 4.1,
        },
        ModelPreset {
            name: "deepseek-r1:8b",
            desc: "DeepSeek R1 – 8B, Reasoning",
            size_gb: 5.2,
        },
        ModelPreset {
            name: "phi4-mini",
            desc: "Microsoft Phi-4 mini – 3.8B",
            size_gb: 2.5,
        },
        ModelPreset {
            name: "nomic-embed-text",
            desc: "Embeddings für RAG/Suche",
            size_gb: 0.3,
        },
    ]
}

#[derive(Debug, Clone)]
pub struct StackPreset {
    pub title: &'static str,
    pub desc: &'static str,
    pub compose: &'static str,
}

pub fn stack_presets() -> Vec<(&'static str, StackPreset)> {
    vec![
        (
            "open-webui",
            StackPreset {
                title: "Open WebUI",
                desc: "ChatGPT-ähnliche Weboberfläche für Ollama – http://localhost:3000",
                compose: "services:\n  open-webui:\n    image: ghcr.io/open-webui/open-webui:main\n    network_mode: host\n    environment:\n      - PORT=3000\n      - OLLAMA_BASE_URL=http://127.0.0.1:11434\n    volumes:\n      - open-webui:/app/backend/data\n    restart: unless-stopped\nvolumes:\n  open-webui: {}\n",
            },
        ),
        (
            "portainer",
            StackPreset {
                title: "Portainer",
                desc: "Container-Verwaltung im Browser – https://localhost:9443",
                compose: "services:\n  portainer:\n    image: docker.io/portainer/portainer-ce:latest\n    ports:\n      - \"9443:9443\"\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock\n      - portainer_data:/data\n    restart: unless-stopped\nvolumes:\n  portainer_data: {}\n",
            },
        ),
        (
            "searxng",
            StackPreset {
                title: "SearXNG",
                desc: "Private Metasuchmaschine – http://localhost:8888",
                compose: "services:\n  searxng:\n    image: docker.io/searxng/searxng:latest\n    ports:\n      - \"8888:8080\"\n    restart: unless-stopped\n",
            },
        ),
    ]
}

#[derive(Debug, Clone)]
pub struct FlatpakPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub x86_only: bool,
}

pub fn flatpak_presets() -> Vec<FlatpakPreset> {
    [
        (
            "com.github.tchx84.Flatseal",
            "Flatseal – Flatpak-Berechtigungen verwalten",
            false,
        ),
        ("org.mozilla.firefox", "Firefox", false),
        ("org.libreoffice.LibreOffice", "LibreOffice", false),
        ("org.gimp.GIMP", "GIMP", false),
        ("org.videolan.VLC", "VLC", false),
        ("org.kde.kdenlive", "Kdenlive – Videoschnitt", false),
        ("com.obsproject.Studio", "OBS Studio", false),
        ("org.signal.Signal", "Signal", false),
        ("com.spotify.Client", "Spotify", true),
        ("com.discordapp.Discord", "Discord", true),
        ("com.valvesoftware.Steam", "Steam", true),
        ("com.visualstudio.code", "Visual Studio Code", false),
    ]
    .into_iter()
    .map(|(id, label, x86_only)| FlatpakPreset {
        id,
        label,
        x86_only,
    })
    .collect()
}

pub const FLATHUB_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// key: (Label, Image)
pub fn distrobox_presets() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("ubuntu", "Ubuntu 24.04", "docker.io/library/ubuntu:24.04"),
        ("debian", "Debian 12", "docker.io/library/debian:12"),
        (
            "fedora",
            "Fedora 42",
            "registry.fedoraproject.org/fedora-toolbox:42",
        ),
        (
            "opensuse",
            "openSUSE Tumbleweed",
            "registry.opensuse.org/opensuse/distrobox:latest",
        ),
        ("alpine", "Alpine Linux", "docker.io/library/alpine:latest"),
        (
            "arch",
            "Arch Linux (nur x86_64)",
            "docker.io/library/archlinux:latest",
        ),
    ]
}

pub fn siglevels() -> Vec<&'static str> {
    vec!["Optional TrustAll", "Optional", "Required", "Never"]
}

/// Minimale Live-Pakete, wenn die Arch-Rettungswerkzeuge abgewählt sind
pub fn minimal_live() -> Vec<&'static str> {
    vec![
        "base",
        "linux",
        "linux-firmware",
        "mkinitcpio",
        "mkinitcpio-archiso",
        "syslinux",
        "memtest86+",
        "memtest86+-efi",
        "edk2-shell",
        "efibootmgr",
        "grub",
        "amd-ucode",
        "intel-ucode",
        "arch-install-scripts",
        "gptfdisk",
        "parted",
        "dosfstools",
        "e2fsprogs",
        "btrfs-progs",
        "sudo",
        "nano",
        "vim",
        "zsh",
        "grml-zsh-config",
        "iwd",
        "openssh",
        "reflector",
        "rsync",
        "less",
        "usbutils",
        "sof-firmware",
    ]
}

/// Dienst-Links aus releng, die im Minimalmodus ins Leere zeigen würden
pub fn minimal_drop_units() -> Vec<&'static str> {
    vec![
        "cloud-init.target.wants",
        "multi-user.target.wants/hv_fcopy_daemon.service",
        "multi-user.target.wants/hv_kvp_daemon.service",
        "multi-user.target.wants/hv_vss_daemon.service",
        "multi-user.target.wants/vboxservice.service",
        "multi-user.target.wants/vmtoolsd.service",
        "multi-user.target.wants/vmware-vmblock-fuse.service",
        "multi-user.target.wants/livecd-talk.service",
        "multi-user.target.wants/ModemManager.service",
        "dbus-org.freedesktop.ModemManager1.service",
        "sockets.target.wants/pcscd.socket",
        "sound.target.wants/livecd-alsa-unmuter.service",
    ]
}

pub fn installer_deps() -> Vec<&'static str> {
    vec![
        "arch-install-scripts",
        "gptfdisk",
        "parted",
        "dosfstools",
        "e2fsprogs",
        "btrfs-progs",
        "rsync",
        "sudo",
        "squashfs-tools",
        "cryptsetup",
    ]
}

fn image_re() -> Regex {
    Regex::new(r"^[a-z0-9][a-z0-9._/:@-]*$").unwrap()
}
fn model_re() -> Regex {
    Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._/:-]*$").unwrap()
}
pub fn stack_re() -> Regex {
    Regex::new(r"^[a-z0-9][a-z0-9-]{0,40}$").unwrap()
}
fn id_re() -> Regex {
    Regex::new(r"^[a-z0-9][a-z0-9-]{0,30}$").unwrap()
}
fn pkg_re() -> Regex {
    Regex::new(r"^[a-zA-Z0-9@._+-]+$").unwrap()
}
pub fn user_re() -> Regex {
    Regex::new(r"^[a-z_][a-z0-9_-]{0,31}$").unwrap()
}
pub fn flatpak_app_re() -> Regex {
    Regex::new(r"^[A-Za-z_][\w-]*(\.[A-Za-z0-9_-]+)+$").unwrap()
}
pub fn remote_re() -> Regex {
    Regex::new(r"^[A-Za-z0-9_-]+$").unwrap()
}
pub fn box_re() -> Regex {
    Regex::new(r"^[a-z0-9][a-z0-9_.-]{0,40}$").unwrap()
}
pub fn image_name_valid(name: &str) -> bool {
    image_re().is_match(name)
}
pub fn model_name_valid(name: &str) -> bool {
    model_re().is_match(name)
}

pub fn needs_chafa(cfg: &Config) -> bool {
    matches!(cfg.branding.logo_mode.as_str(), "image" | "chafa") && !cfg.branding.logo.is_empty()
}

/// [(Quelle, App-ID)] – "quelle:app.id" oder nur "app.id" (Flathub).
pub fn flatpak_apps(cfg: &Config) -> Vec<(String, String)> {
    if !cfg.flatpak.enabled {
        return Vec::new();
    }
    cfg.flatpak
        .apps
        .iter()
        .map(|entry| match entry.rsplit_once(':') {
            Some((remote, app)) if !remote.is_empty() => (remote.to_string(), app.to_string()),
            Some((_, app)) => ("flathub".to_string(), app.to_string()),
            None => ("flathub".to_string(), entry.clone()),
        })
        .collect()
}

/// [(Name, URL der .flatpakrepo-Datei)]
pub fn flatpak_remotes(cfg: &Config) -> Vec<(String, String)> {
    let f = &cfg.flatpak;
    if !f.enabled {
        return Vec::new();
    }
    let mut remotes = Vec::new();
    if f.flathub {
        remotes.push(("flathub".to_string(), FLATHUB_URL.to_string()));
    }
    remotes.extend(f.remotes.iter().map(|r| (r.name.clone(), r.url.clone())));
    remotes
}

pub fn distrobox_active(cfg: &Config) -> bool {
    cfg.distrobox.enabled
}

/// App-Store passend zum Desktop, der Flatpaks anzeigt und aktualisiert.
pub fn flatpak_store(cfg: &Config) -> Vec<&'static str> {
    match cfg.packages.desktop.as_str() {
        "none" => vec![],
        "kde" | "lxqt" => vec!["discover"],
        _ => vec!["gnome-software"],
    }
}

/// Paket- bzw. App-Liste, die in einem Wort/Komma-Feld steht.
pub fn split_list(s: &str) -> Vec<String> {
    split_ws_comma(s)
}

fn dedup(items: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for i in items {
        if seen.insert(i.clone()) {
            out.push(i);
        }
    }
    out
}

fn to_strings(v: Vec<&str>) -> Vec<String> {
    v.into_iter().map(|s| s.to_string()).collect()
}

/// Pakete, die der Installer auf dem Zielsystem installiert.
pub fn target_packages(cfg: &Config) -> Vec<String> {
    let p = &cfg.packages;
    let kern = kernel(cfg);
    let mut pkgs: Vec<String> = vec!["base".into(), kern.clone(), "linux-firmware".into()];
    pkgs.extend(to_strings(match arch(cfg) {
        "x86_64" => vec!["amd-ucode", "intel-ucode", "grub", "efibootmgr"],
        "aarch64" => vec!["archlinuxarm-keyring", "grub", "efibootmgr"],
        _ => vec![
            "archlinuxarm-keyring",
            "raspberrypi-bootloader",
            "firmware-raspberrypi",
        ],
    }));
    pkgs.extend(to_strings(vec![
        "dosfstools",
        "networkmanager",
        "sudo",
        "nano",
        "vim",
        "bash-completion",
        "fastfetch",
    ]));
    if needs_chafa(cfg) {
        pkgs.push("chafa".into());
    }
    if cfg.installer.zram {
        pkgs.push("zram-generator".into());
    }
    pkgs.extend(desktop_info(cfg).packages.iter().map(|s| s.to_string()));
    let gpu = if p.gpu.is_empty() || !gpus_for(cfg).iter().any(|(k, _)| *k == p.gpu) {
        "mesa"
    } else {
        p.gpu.as_str()
    };
    pkgs.extend(gpu_info(gpu).packages.iter().map(|s| s.to_string()));
    if gpu == "nvidia" {
        pkgs.push(format!("{kern}-headers"));
    }
    let eng = &cfg.containers.engine;
    pkgs.extend(engine_info(eng).packages.iter().map(|s| s.to_string()));
    if eng != "none" && gpu == "nvidia" {
        pkgs.push("nvidia-container-toolkit".into());
    }
    if distrobox_active(cfg) {
        pkgs.push("distrobox".into());
        if eng == "none" {
            pkgs.push("podman".into());
        }
    }
    if cfg.flatpak.enabled {
        pkgs.push("flatpak".into());
        if cfg.flatpak.store {
            pkgs.extend(to_strings(flatpak_store(cfg)));
        }
    }
    let ai = &cfg.ai;
    if ai.ollama {
        pkgs.extend(accel_info(&ai.accel).packages.iter().map(|s| s.to_string()));
    }
    if ai.dev {
        let torch = match ai.accel.as_str() {
            "cuda" => "python-pytorch-cuda",
            "rocm" => "python-pytorch-rocm",
            _ => "python-pytorch",
        };
        pkgs.extend(
            [
                torch,
                "python-numpy",
                "python-pandas",
                "python-scikit-learn",
                "jupyterlab",
                "python-huggingface-hub",
                "python-pip",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
    }
    let all_presets = presets();
    for key in &p.presets {
        if let Some((_, info)) = all_presets.iter().find(|(k, _)| k == key) {
            pkgs.extend(info.packages.iter().map(|s| s.to_string()));
        }
    }
    pkgs.extend(custom_packages(cfg));
    dedup(pkgs)
}

pub fn target_services(cfg: &Config) -> Vec<String> {
    let mut services: Vec<String> = to_strings(vec![
        "NetworkManager.service",
        "systemd-timesyncd.service",
        "fstrim.timer",
    ]);
    if let Some(dm) = desktop_info(cfg).display_manager {
        services.push(format!("{dm}.service"));
    }
    let all_presets = presets();
    for key in &cfg.packages.presets {
        if let Some((_, info)) = all_presets.iter().find(|(k, _)| k == key) {
            services.extend(info.services.iter().map(|s| s.to_string()));
        }
    }
    services.extend(
        engine_info(&cfg.containers.engine)
            .services
            .iter()
            .map(|s| s.to_string()),
    );
    if distrobox_active(cfg) && cfg.containers.engine == "none" {
        services.push("podman.socket".into());
    }
    if cfg.ai.ollama {
        services.push("ollama.service".into());
    }
    dedup(services)
}

/// Pakete für das fertige ARM-Image (zusätzlich zum Arch-Linux-ARM-Basissystem).
pub fn image_packages(cfg: &Config) -> Vec<String> {
    let mut pkgs = target_packages(cfg);
    pkgs.extend(to_strings(vec!["e2fsprogs", "util-linux"]));
    dedup(pkgs)
}

/// Alle Images, die vorab in die ISO geladen werden sollen (Liste + aus Compose-Stacks).
pub fn container_images(cfg: &Config) -> Vec<String> {
    let c = &cfg.containers;
    if c.engine == "none" {
        return Vec::new();
    }
    let mut images = split_ws_comma(&c.images);
    let image_line = Regex::new(r#"(?m)^\s*image:\s*["']?([^\s"']+)"#).unwrap();
    for st in &c.stacks {
        for cap in image_line.captures_iter(&st.compose) {
            images.push(cap[1].to_string());
        }
    }
    dedup(images)
}

pub fn kernel_params(cfg: &Config) -> Vec<String> {
    let mut params = Vec::new();
    if cfg.packages.gpu == "nvidia" {
        params.push("nvidia_drm.modeset=1".to_string());
    }
    if cfg.installer.zram {
        params.push("zswap.enabled=0".to_string());
    }
    params
}

pub fn live_packages(cfg: &Config, releng_packages: &[String]) -> Vec<String> {
    let mut base: Vec<String> = if cfg.packages.rescue_tools {
        releng_packages.to_vec()
    } else {
        to_strings(minimal_live())
    };
    base.extend(target_packages(cfg));
    if cfg.packages.gpu == "nvidia" {
        base.push("linux-headers".into());
    }
    if installer_active(cfg) {
        base.extend(to_strings(installer_deps()));
    }
    dedup(base)
}

pub fn installer_active(cfg: &Config) -> bool {
    !is_image(cfg) && cfg.installer.enabled && cfg.packages.desktop != "none"
}

/// Gibt (Fehler, Warnungen) zurück.
pub fn validate(cfg: &Config) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let (g, b) = (&cfg.general, &cfg.branding);
    if g.name.trim().is_empty() {
        errors.push("Der Distro-Name darf nicht leer sein.".to_string());
    }
    if !id_re().is_match(&g.id) {
        errors.push(
            "Die ID darf nur Kleinbuchstaben, Ziffern und '-' enthalten (max. 31 Zeichen)."
                .to_string(),
        );
    }
    let hostname_re = Regex::new(r"^[a-zA-Z0-9-]{1,63}$").unwrap();
    if !hostname_re.is_match(&g.hostname) {
        errors.push("Ungültiger Hostname.".to_string());
    }
    for (path, label) in [(&b.logo, "Logo"), (&b.wallpaper, "Hintergrundbild")] {
        if !path.is_empty() && !Path::new(path).is_file() {
            errors.push(format!("{label}-Datei nicht gefunden: {path}"));
        }
    }
    if matches!(b.logo_mode.as_str(), "image" | "chafa") && b.logo.is_empty() {
        warnings.push("Kein Logo-Bild gewählt – fastfetch zeigt das Arch-Logo.".to_string());
    }
    for pkg in custom_packages(cfg) {
        if !pkg_re().is_match(&pkg) {
            errors.push(format!("Ungültiger Paketname: {pkg}"));
        }
    }
    let repo_name_re = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    for repo in &cfg.repos.custom {
        if !repo_name_re.is_match(&repo.name) || repo.server.is_empty() {
            errors.push(format!(
                "Repository '{}' braucht einen gültigen Namen und eine Server-URL.",
                repo.name
            ));
        }
    }
    if cfg.packages.presets.iter().any(|p| p == "gaming") && !cfg.repos.multilib {
        errors.push("Steam benötigt das multilib-Repository.".to_string());
    }
    for img in container_images(cfg) {
        if !image_re().is_match(&img) {
            errors.push(format!("Ungültiger Image-Name: {img}"));
        }
    }
    let names: Vec<&str> = cfg
        .containers
        .stacks
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    for n in &names {
        if !stack_re().is_match(n) {
            errors.push(format!("Ungültiger Stack-Name '{n}' (a-z, 0-9, -)"));
        }
    }
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    if unique.len() != names.len() {
        errors.push("Stack-Namen müssen eindeutig sein.".to_string());
    }
    if !cfg.containers.stacks.is_empty() && cfg.containers.engine == "none" {
        errors.push("Compose-Stacks benötigen Docker oder Podman.".to_string());
    }
    for m in &cfg.ai.models {
        if !model_re().is_match(m) {
            errors.push(format!("Ungültiger Modellname: {m}"));
        }
    }
    if !cfg.ai.models.is_empty() && !cfg.ai.ollama {
        warnings.push("KI-Modelle sind ausgewählt, aber Ollama ist deaktiviert.".to_string());
    }
    if cfg.ai.accel == "cuda" && cfg.packages.gpu != "nvidia" {
        warnings.push(
            "CUDA-Beschleunigung ohne NVIDIA-Treiber – bitte unter Pakete NVIDIA wählen."
                .to_string(),
        );
    }
    if matches!(cfg.ai.accel.as_str(), "cuda" | "rocm") && (cfg.ai.ollama || cfg.ai.dev) {
        warnings
            .push("CUDA/ROCm sind mehrere GB groß – die ISO wird entsprechend größer.".to_string());
    }
    if !is_image(cfg) && cfg.installer.enabled && cfg.packages.desktop == "none" {
        warnings.push(
            "Der grafische Installer benötigt einen Desktop – ohne Desktop wird archinstall genutzt.".to_string(),
        );
    }
    if cfg.repos.core_testing || cfg.repos.extra_testing {
        warnings.push("Testing-Repositories können instabil sein.".to_string());
    }
    validate_arch(cfg, &mut errors, &mut warnings);
    validate_flatpak(cfg, &mut errors, &mut warnings);
    validate_distrobox(cfg, &mut errors, &mut warnings);
    (errors, warnings)
}

fn validate_arch(cfg: &Config, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    let plat = cfg.build.arch.as_str();
    if !arches().iter().any(|(k, _)| *k == plat) {
        errors.push(format!("Unbekannte Architektur: {plat}"));
        return;
    }
    if !is_image(cfg) {
        return;
    }
    let label = arch_info(cfg).label;
    let gpu = if cfg.packages.gpu.is_empty() {
        "mesa"
    } else {
        cfg.packages.gpu.as_str()
    };
    if gpu != "mesa" {
        errors.push(format!(
            "Der Grafiktreiber '{gpu}' ist für {label} nicht verfügbar – bitte Mesa wählen."
        ));
    }
    if matches!(cfg.ai.accel.as_str(), "cuda" | "rocm") && (cfg.ai.ollama || cfg.ai.dev) {
        errors.push(format!(
            "CUDA/ROCm gibt es für {label} nicht – bitte CPU oder Vulkan wählen."
        ));
    }
    if cfg.repos.multilib {
        errors.push("Das multilib-Repository gibt es nur für x86_64.".to_string());
    }
    if cfg.packages.presets.iter().any(|p| p == "gaming") {
        errors.push(format!("Steam ist für {label} nicht verfügbar."));
    }
    if cfg.repos.core_testing || cfg.repos.extra_testing {
        warnings.push(
            "Arch Linux ARM hat keine Testing-Repositories – die Einstellung wird ignoriert."
                .to_string(),
        );
    }
    let kern = kernel(cfg);
    if cfg.packages.kernel != kern {
        warnings.push(format!(
            "Kernel '{}' gibt es für {label} nicht – es wird {kern} verwendet.",
            cfg.packages.kernel
        ));
    }
    let img = &cfg.image;
    if !user_re().is_match(&img.username) || img.username == "root" {
        errors.push("Ungültiger Benutzername für das Image (a-z, 0-9, _ und -).".to_string());
    }
    if img.password.is_empty() {
        errors.push("Das Image braucht ein Startpasswort für den Benutzer.".to_string());
    }
    if img.autologin && img.expire_password && cfg.packages.desktop != "none" {
        warnings.push(
            "Mit automatischer Anmeldung muss das Startpasswort nicht geändert werden – bitte selbst ändern."
                .to_string(),
        );
    }
    if img.size_gb < 3 {
        errors.push("Das Image muss mindestens 3 GB groß sein.".to_string());
    }
    let known: std::collections::HashMap<&str, f64> = model_presets()
        .into_iter()
        .map(|m| (m.name, m.size_gb))
        .collect();
    let mut need = 3.0;
    if cfg.ai.ollama {
        need += cfg
            .ai
            .models
            .iter()
            .map(|m| known.get(m.as_str()).copied().unwrap_or(0.0))
            .sum::<f64>();
    }
    if cfg.packages.desktop != "none" {
        need += 3.0;
    }
    if need > img.size_gb as f64 {
        warnings.push(format!(
            "Das Image ist mit {} GB vermutlich zu klein (geschätzt mind. {need:.0} GB).",
            img.size_gb
        ));
    }
    warnings.push(format!(
        "{label}: Es entsteht ein fertig installiertes Image (Arch Linux ARM). Nicht jedes Paket aus Arch Linux gibt es auch für ARM."
    ));
}

fn validate_flatpak(cfg: &Config, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    let f = &cfg.flatpak;
    if !f.enabled {
        if !f.apps.is_empty() {
            warnings
                .push("Flatpak-Apps sind ausgewählt, aber Flatpak ist deaktiviert.".to_string());
        }
        return;
    }
    let remotes = flatpak_remotes(cfg);
    let url_re = Regex::new(r"^https?://\S+$").unwrap();
    for (name, url) in &remotes {
        if !remote_re().is_match(name) || !url_re.is_match(url) {
            errors.push(format!(
                "Flatpak-Quelle '{name}' braucht einen gültigen Namen und eine URL."
            ));
        }
    }
    let names: Vec<&str> = remotes.iter().map(|(n, _)| n.as_str()).collect();
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    if unique.len() != names.len() {
        errors.push("Flatpak-Quellen müssen eindeutige Namen haben.".to_string());
    }
    let x86_only: std::collections::HashSet<&str> = flatpak_presets()
        .into_iter()
        .filter(|p| p.x86_only)
        .map(|p| p.id)
        .collect();
    for (remote, app) in flatpak_apps(cfg) {
        if !flatpak_app_re().is_match(&app) {
            errors.push(format!("Ungültige Flatpak-App-ID: {app}"));
        } else if !names.contains(&remote.as_str()) {
            errors.push(format!(
                "Flatpak-App {app}: Quelle '{remote}' ist nicht eingerichtet."
            ));
        } else if is_image(cfg) && x86_only.contains(app.as_str()) {
            errors.push(format!("Die Flatpak-App {app} gibt es nur für x86_64."));
        }
    }
    if f.store && cfg.packages.desktop == "none" {
        warnings.push("Ein App-Store für Flatpaks braucht einen Desktop.".to_string());
    }
}

fn validate_distrobox(cfg: &Config, errors: &mut Vec<String>, warnings: &mut Vec<String>) {
    if !distrobox_active(cfg) {
        return;
    }
    let boxes = &cfg.distrobox.boxes;
    for b in boxes {
        if !box_re().is_match(&b.name) {
            errors.push(format!(
                "Ungültiger Distrobox-Name '{}' (a-z, 0-9, _ . -)",
                b.name
            ));
        }
        if !image_re().is_match(&b.image) {
            errors.push(format!(
                "Ungültiges Image für Distrobox '{}': {}",
                b.name, b.image
            ));
        }
        for pkg in split_ws_comma(&b.packages) {
            if !pkg_re().is_match(&pkg) {
                errors.push(format!(
                    "Distrobox '{}': ungültiger Paketname {pkg}",
                    b.name
                ));
            }
        }
        for app in split_ws_comma(&b.export) {
            if !pkg_re().is_match(&app) {
                errors.push(format!("Distrobox '{}': ungültiger App-Name {app}", b.name));
            }
        }
        if is_image(cfg) && b.image.contains("archlinux") {
            errors.push(format!(
                "Distrobox '{}': das Arch-Linux-Image gibt es nur für x86_64.",
                b.name
            ));
        }
    }
    let names: Vec<&str> = boxes.iter().map(|b| b.name.as_str()).collect();
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    if unique.len() != names.len() {
        errors.push("Distrobox-Namen müssen eindeutig sein.".to_string());
    }
    if boxes.is_empty() {
        warnings.push("Distrobox ist aktiv, aber es sind keine Container angelegt.".to_string());
    }
}

/// Prüft Pakete gegen die lokale pacman-Datenbank. Gibt unbekannte zurück.
pub fn check_packages(names: &[String]) -> Vec<String> {
    if names.is_empty() {
        return Vec::new();
    }
    let mut cmd = Command::new("pacman");
    cmd.args(["-Sp", "--print-format", "%n"]).args(names);
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    let re_de = Regex::new(r"target not found: (\S+)").unwrap();
    let re_en = Regex::new(r"Ziel nicht gefunden: (\S+)").unwrap();
    let mut missing: Vec<String> = re_de
        .captures_iter(&stderr)
        .map(|c| c[1].to_string())
        .collect();
    missing.extend(re_en.captures_iter(&stderr).map(|c| c[1].to_string()));
    dedup(missing)
}

#[derive(Debug, Clone)]
pub struct PackageSearchResult {
    pub repo: String,
    pub name: String,
    pub version: String,
    pub desc: String,
    pub installed: bool,
}

pub fn search_packages(query: &str, limit: usize) -> Vec<PackageSearchResult> {
    let output = match Command::new("pacman").args(["-Ss", query]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let head_re = Regex::new(r"^(\S+)/(\S+) (\S+)").unwrap();
    let mut results = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let head = lines[i];
        let desc = lines[i + 1].trim();
        if let Some(m) = head_re.captures(head) {
            results.push(PackageSearchResult {
                repo: m[1].to_string(),
                name: m[2].to_string(),
                version: m[3].to_string(),
                desc: desc.to_string(),
                installed: head.contains('['),
            });
        }
        if results.len() >= limit {
            break;
        }
        i += 2;
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_valid() {
        let (errors, _) = validate(&default_config());
        assert!(errors.is_empty());
    }

    #[test]
    fn invalid_id_and_packages() {
        let mut cfg = default_config();
        cfg.general.id = "Mein OS".into();
        cfg.packages.custom = "gut b@d$name".into();
        let (errors, _) = validate(&cfg);
        assert!(errors.iter().any(|e| e.contains("ID")));
        assert!(errors.iter().any(|e| e.contains("b@d$name")));
    }

    #[test]
    fn steam_needs_multilib() {
        let mut cfg = default_config();
        cfg.packages.presets.push("gaming".into());
        let (errors, _) = validate(&cfg);
        assert!(errors.iter().any(|e| e.contains("multilib")));
    }

    #[test]
    fn target_packages_and_services() {
        let mut cfg = default_config();
        cfg.packages.kernel = "linux-zen".into();
        cfg.packages.gpu = "nvidia".into();
        cfg.containers.engine = "docker".into();
        cfg.ai.ollama = true;
        cfg.ai.accel = "cuda".into();
        let pkgs = target_packages(&cfg);
        for p in [
            "linux-zen",
            "linux-zen-headers",
            "nvidia-open-dkms",
            "docker",
            "nvidia-container-toolkit",
            "ollama",
            "ollama-cuda",
        ] {
            assert!(pkgs.contains(&p.to_string()), "missing {p}");
        }
        let unique: std::collections::HashSet<_> = pkgs.iter().collect();
        assert_eq!(pkgs.len(), unique.len());
        let svcs = target_services(&cfg);
        assert!(svcs.contains(&"docker.service".to_string()));
        assert!(svcs.contains(&"ollama.service".to_string()));
        assert!(svcs.contains(&"sddm.service".to_string()));
    }

    #[test]
    fn live_contains_target() {
        let mut cfg = default_config();
        cfg.packages.kernel = "linux-lts".into();
        let live = live_packages(&cfg, &["base".into(), "linux".into()]);
        let live_set: std::collections::HashSet<_> = live.iter().collect();
        for p in target_packages(&cfg) {
            assert!(live_set.contains(&p), "missing {p} in live set");
        }
        assert!(live.contains(&"squashfs-tools".to_string()));
    }

    #[test]
    fn container_images_from_stacks() {
        let mut cfg = default_config();
        cfg.containers.engine = "docker".into();
        cfg.containers.images = "nginx:alpine\npostgres:17".into();
        let preset = stack_presets()
            .into_iter()
            .find(|(k, _)| *k == "open-webui")
            .unwrap()
            .1;
        cfg.containers.stacks = vec![Stack {
            name: "open-webui".into(),
            title: preset.title.into(),
            compose: preset.compose.into(),
            autostart: true,
            url: String::new(),
        }];
        assert_eq!(
            container_images(&cfg),
            vec![
                "nginx:alpine".to_string(),
                "postgres:17".to_string(),
                "ghcr.io/open-webui/open-webui:main".to_string()
            ]
        );
    }

    #[test]
    fn arm_packages() {
        let mut cfg = default_config();
        cfg.build.arch = "rpi".into();
        let pkgs = target_packages(&cfg);
        assert!(pkgs.contains(&"linux-rpi".to_string()));
        assert!(pkgs.contains(&"raspberrypi-bootloader".to_string()));
        for p in ["linux", "intel-ucode", "amd-ucode", "grub"] {
            assert!(!pkgs.contains(&p.to_string()), "{p} on rpi");
        }
        cfg.build.arch = "aarch64".into();
        let pkgs = target_packages(&cfg);
        assert!(pkgs.contains(&"linux-aarch64".to_string()));
        assert!(pkgs.contains(&"grub".to_string()));
        assert_eq!(oci_arch(&cfg), "arm64");
        assert!(!installer_active(&cfg));
    }

    #[test]
    fn kernel_choice() {
        let mut cfg = default_config();
        cfg.build.arch = "rpi".into();
        cfg.packages.kernel = "linux-rpi-16k".into();
        assert_eq!(kernel(&cfg), "linux-rpi-16k");
        cfg.build.arch = "x86_64".into();
        assert_eq!(kernel(&cfg), "linux");
    }

    #[test]
    fn arm_validation() {
        let mut cfg = default_config();
        cfg.build.arch = "aarch64".into();
        cfg.packages.kernel = "linux-aarch64".into();
        assert!(validate(&cfg).0.is_empty(), "{:?}", validate(&cfg).0);
        cfg.packages.gpu = "nvidia".into();
        cfg.repos.multilib = true;
        cfg.ai.ollama = true;
        cfg.ai.accel = "cuda".into();
        cfg.image.password.clear();
        let errors = validate(&cfg).0;
        for word in ["Grafiktreiber", "multilib", "CUDA", "Startpasswort"] {
            assert!(errors.iter().any(|e| e.contains(word)), "{word}");
        }
        cfg.build.arch = "sparc".into();
        assert!(validate(&cfg).0.iter().any(|e| e.contains("Architektur")));
    }

    #[test]
    fn flatpak() {
        let mut cfg = default_config();
        cfg.flatpak.enabled = true;
        cfg.flatpak.apps = vec!["org.mozilla.firefox".into(), "kde:org.kde.kate".into()];
        assert_eq!(
            flatpak_apps(&cfg),
            vec![
                ("flathub".to_string(), "org.mozilla.firefox".to_string()),
                ("kde".to_string(), "org.kde.kate".to_string())
            ]
        );
        assert!(validate(&cfg).0.iter().any(|e| e.contains("kde")));
        cfg.flatpak.remotes = vec![FlatpakRemote {
            name: "kde".into(),
            url: "https://distribute.kde.org/kdeapps.flatpakrepo".into(),
        }];
        assert!(validate(&cfg).0.is_empty(), "{:?}", validate(&cfg).0);
        let pkgs = target_packages(&cfg);
        assert!(pkgs.contains(&"flatpak".to_string()));
        assert!(pkgs.contains(&"discover".to_string()));
        cfg.flatpak.apps.push("kein app".into());
        assert!(validate(&cfg).0.iter().any(|e| e.contains("kein app")));
    }

    #[test]
    fn flatpak_x86_only_on_arm() {
        let mut cfg = default_config();
        cfg.build.arch = "rpi".into();
        cfg.packages.kernel = "linux-rpi".into();
        cfg.flatpak.enabled = true;
        cfg.flatpak.apps = vec!["com.valvesoftware.Steam".into()];
        assert!(validate(&cfg).0.iter().any(|e| e.contains("x86_64")));
    }

    #[test]
    fn distrobox() {
        let mut cfg = default_config();
        cfg.distrobox.enabled = true;
        cfg.distrobox.boxes = vec![DistroboxBox {
            name: "ubuntu".into(),
            image: "docker.io/library/ubuntu:24.04".into(),
            packages: "git".into(),
            export: String::new(),
        }];
        assert!(validate(&cfg).0.is_empty(), "{:?}", validate(&cfg).0);
        let pkgs = target_packages(&cfg);
        assert!(pkgs.contains(&"distrobox".to_string()));
        assert!(pkgs.contains(&"podman".to_string()));
        assert!(target_services(&cfg).contains(&"podman.socket".to_string()));
        let dup = cfg.distrobox.boxes[0].clone();
        cfg.distrobox.boxes.push(dup);
        assert!(validate(&cfg).0.iter().any(|e| e.contains("eindeutig")));
    }

    #[test]
    fn python_project_files_still_load() {
        // Projekte aus der Python-Version (gleiche JSON-Schlüssel) laden weiterhin
        let json = r#"{"general": {"name": "Alt", "id": "alt"}, "build": {"arch": "rpi"},
            "flatpak": {"enabled": true, "apps": ["org.gimp.GIMP"]},
            "image": {"username": "pi", "size_gb": 16}}"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.build.arch, "rpi");
        assert_eq!(cfg.image.username, "pi");
        assert_eq!(cfg.image.password, "user");
        assert!(cfg.flatpak.flathub);
        assert_eq!(cfg.packages.desktop, "kde");
    }

    #[test]
    fn save_load_roundtrip() {
        let mut cfg = default_config();
        cfg.general.name = "Rundreise".into();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.json");
        save(&cfg, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }
}
