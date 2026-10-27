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
            presets: vec!["audio".into(), "firefox".into(), "fonts".into(), "utils".into()],
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
        Self { engine: "none".into(), preload: true, images: String::new(), stacks: Vec::new() }
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
        Self { ollama: false, accel: "cpu".into(), models: Vec::new(), expose: false, dev: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    pub project_dir: String,
    /// zstd (schnell) | xz (klein)
    pub compression: String,
}

impl Default for Build {
    fn default() -> Self {
        Self {
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
    re.split(s.trim()).filter(|p| !p.is_empty()).map(|p| p.to_string()).collect()
}

// ---------------------------------------------------------------- Static data

#[derive(Debug, Clone)]
pub struct DesktopInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
    pub display_manager: Option<&'static str>,
    pub session: Option<&'static str>,
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
        ("none", DesktopInfo { label: "Kein Desktop (nur Konsole)", packages: vec![], display_manager: None, session: None }),
        (
            "kde",
            DesktopInfo {
                label: "KDE Plasma",
                packages: vec!["plasma-meta", "konsole", "dolphin", "kate", "ark", "spectacle", "sddm"],
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
                    "xorg-server", "xfce4", "xfce4-goodies", "lightdm", "lightdm-gtk-greeter",
                    "network-manager-applet", "gvfs",
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
                    "xorg-server", "cinnamon", "gnome-terminal", "nemo", "lightdm", "lightdm-gtk-greeter", "gvfs",
                ],
                display_manager: Some("lightdm"),
                session: Some("cinnamon"),
            },
        ),
        (
            "lxqt",
            DesktopInfo {
                label: "LXQt",
                packages: vec!["xorg-server", "lxqt", "breeze-icons", "sddm", "network-manager-applet"],
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
        ("audio", PresetInfo { label: "Audio (PipeWire)", desc: "pipewire, pulse, alsa, wireplumber", packages: vec!["pipewire", "pipewire-pulse", "pipewire-alsa", "wireplumber"], services: vec![] }),
        ("bluetooth", PresetInfo { label: "Bluetooth", desc: "bluez + Dienst", packages: vec!["bluez", "bluez-utils"], services: vec!["bluetooth.service"] }),
        ("printing", PresetInfo { label: "Drucken", desc: "CUPS + Treiber", packages: vec!["cups", "cups-pdf", "gutenprint"], services: vec!["cups.service"] }),
        ("firefox", PresetInfo { label: "Firefox", desc: "Webbrowser", packages: vec!["firefox"], services: vec![] }),
        ("chromium", PresetInfo { label: "Chromium", desc: "Webbrowser", packages: vec!["chromium"], services: vec![] }),
        ("office", PresetInfo { label: "LibreOffice", desc: "Office-Paket", packages: vec!["libreoffice-fresh"], services: vec![] }),
        ("media", PresetInfo { label: "Multimedia", desc: "VLC, mpv, GIMP", packages: vec!["vlc", "mpv", "gimp"], services: vec![] }),
        ("fonts", PresetInfo { label: "Schriften", desc: "Noto + Emoji", packages: vec!["noto-fonts", "noto-fonts-emoji", "ttf-dejavu"], services: vec![] }),
        ("flatpak", PresetInfo { label: "Flatpak", desc: "Flatpak-Unterstützung", packages: vec!["flatpak"], services: vec![] }),
        ("dev", PresetInfo { label: "Entwicklung", desc: "base-devel, git", packages: vec!["base-devel", "git"], services: vec![] }),
        ("utils", PresetInfo { label: "Werkzeuge", desc: "htop, wget, unzip, man", packages: vec!["htop", "wget", "unzip", "zip", "man-db", "man-pages"], services: vec![] }),
        ("gaming", PresetInfo { label: "Gaming", desc: "Steam (benötigt multilib)", packages: vec!["steam"], services: vec![] }),
    ]
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
}

pub fn gpus() -> Vec<(&'static str, GpuInfo)> {
    vec![
        ("mesa", GpuInfo { label: "Standard (Mesa, freie Treiber)", packages: vec!["mesa", "vulkan-icd-loader"] }),
        ("amd", GpuInfo { label: "AMD Radeon (Mesa + Vulkan)", packages: vec!["mesa", "vulkan-radeon", "vulkan-icd-loader"] }),
        ("intel", GpuInfo { label: "Intel (Mesa + Vulkan + VA-API)", packages: vec!["mesa", "vulkan-intel", "intel-media-driver", "vulkan-icd-loader"] }),
        ("nvidia", GpuInfo { label: "NVIDIA (proprietär, nvidia-open)", packages: vec!["nvidia-open-dkms", "nvidia-utils", "nvidia-settings", "vulkan-icd-loader"] }),
    ]
}

fn gpu_info(key: &str) -> GpuInfo {
    gpus().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_else(|| gpus().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct EngineInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
    pub services: Vec<&'static str>,
}

pub fn container_engines() -> Vec<(&'static str, EngineInfo)> {
    vec![
        ("none", EngineInfo { label: "Keine", packages: vec![], services: vec![] }),
        ("docker", EngineInfo { label: "Docker", packages: vec!["docker", "docker-compose", "docker-buildx", "lazydocker"], services: vec!["docker.service"] }),
        ("podman", EngineInfo { label: "Podman (ohne Daemon, rootless)", packages: vec!["podman", "podman-compose", "podman-docker", "distrobox"], services: vec!["podman.socket"] }),
    ]
}

fn engine_info(key: &str) -> EngineInfo {
    container_engines().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_else(|| container_engines().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct AccelInfo {
    pub label: &'static str,
    pub packages: Vec<&'static str>,
}

pub fn ollama_accel() -> Vec<(&'static str, AccelInfo)> {
    vec![
        ("cpu", AccelInfo { label: "Nur CPU", packages: vec!["ollama"] }),
        ("cuda", AccelInfo { label: "NVIDIA (CUDA)", packages: vec!["ollama", "ollama-cuda"] }),
        ("rocm", AccelInfo { label: "AMD (ROCm)", packages: vec!["ollama", "ollama-rocm"] }),
        ("vulkan", AccelInfo { label: "Vulkan (AMD/Intel/NVIDIA)", packages: vec!["ollama", "ollama-vulkan", "vulkan-icd-loader"] }),
    ]
}

fn accel_info(key: &str) -> AccelInfo {
    ollama_accel().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_else(|| ollama_accel().remove(0).1)
}

#[derive(Debug, Clone)]
pub struct ModelPreset {
    pub name: &'static str,
    pub desc: &'static str,
    pub size_gb: f64,
}

pub fn model_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset { name: "llama3.2:3b", desc: "Meta Llama 3.2 – 3B, schnell", size_gb: 2.0 },
        ModelPreset { name: "llama3.1:8b", desc: "Meta Llama 3.1 – 8B, Allrounder", size_gb: 4.9 },
        ModelPreset { name: "qwen3:8b", desc: "Qwen 3 – 8B, stark mehrsprachig", size_gb: 5.2 },
        ModelPreset { name: "gemma3:4b", desc: "Google Gemma 3 – 4B", size_gb: 3.3 },
        ModelPreset { name: "mistral:7b", desc: "Mistral 7B", size_gb: 4.1 },
        ModelPreset { name: "deepseek-r1:8b", desc: "DeepSeek R1 – 8B, Reasoning", size_gb: 5.2 },
        ModelPreset { name: "phi4-mini", desc: "Microsoft Phi-4 mini – 3.8B", size_gb: 2.5 },
        ModelPreset { name: "nomic-embed-text", desc: "Embeddings für RAG/Suche", size_gb: 0.3 },
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

pub fn siglevels() -> Vec<&'static str> {
    vec!["Optional TrustAll", "Optional", "Required", "Never"]
}

/// Minimale Live-Pakete, wenn die Arch-Rettungswerkzeuge abgewählt sind
pub fn minimal_live() -> Vec<&'static str> {
    vec![
        "base", "linux", "linux-firmware", "mkinitcpio", "mkinitcpio-archiso", "syslinux",
        "memtest86+", "memtest86+-efi", "edk2-shell", "efibootmgr", "grub", "amd-ucode", "intel-ucode",
        "arch-install-scripts", "gptfdisk", "parted", "dosfstools", "e2fsprogs", "btrfs-progs",
        "sudo", "nano", "vim", "zsh", "grml-zsh-config", "iwd", "openssh", "reflector", "rsync",
        "less", "usbutils", "sof-firmware",
    ]
}

/// Dienst-Links aus releng, die im Minimalmodus ins Leere zeigen würden
pub fn minimal_drop_units() -> Vec<&'static str> {
    vec![
        "cloud-init.target.wants", "multi-user.target.wants/hv_fcopy_daemon.service",
        "multi-user.target.wants/hv_kvp_daemon.service", "multi-user.target.wants/hv_vss_daemon.service",
        "multi-user.target.wants/vboxservice.service", "multi-user.target.wants/vmtoolsd.service",
        "multi-user.target.wants/vmware-vmblock-fuse.service", "multi-user.target.wants/livecd-talk.service",
        "multi-user.target.wants/ModemManager.service", "dbus-org.freedesktop.ModemManager1.service",
        "sockets.target.wants/pcscd.socket", "sound.target.wants/livecd-alsa-unmuter.service",
    ]
}

pub fn installer_deps() -> Vec<&'static str> {
    vec![
        "arch-install-scripts", "gptfdisk", "parted", "dosfstools", "e2fsprogs", "btrfs-progs", "rsync", "sudo",
        "squashfs-tools", "cryptsetup",
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
pub fn model_name_valid(name: &str) -> bool {
    model_re().is_match(name)
}

pub fn needs_chafa(cfg: &Config) -> bool {
    matches!(cfg.branding.logo_mode.as_str(), "image" | "chafa") && !cfg.branding.logo.is_empty()
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
    let mut pkgs: Vec<String> = to_strings(vec![
        "base", "linux-firmware", "amd-ucode", "intel-ucode", "grub", "efibootmgr", "dosfstools",
        "networkmanager", "sudo", "nano", "vim", "bash-completion", "fastfetch",
    ]);
    pkgs.insert(1, p.kernel.clone());
    if needs_chafa(cfg) {
        pkgs.push("chafa".into());
    }
    if cfg.installer.zram {
        pkgs.push("zram-generator".into());
    }
    pkgs.extend(desktop_info(cfg).packages.iter().map(|s| s.to_string()));
    let gpu = if p.gpu.is_empty() { "mesa" } else { p.gpu.as_str() };
    pkgs.extend(gpu_info(gpu).packages.iter().map(|s| s.to_string()));
    if gpu == "nvidia" {
        pkgs.push(format!("{}-headers", p.kernel));
    }
    let eng = &cfg.containers.engine;
    pkgs.extend(engine_info(eng).packages.iter().map(|s| s.to_string()));
    if eng != "none" && gpu == "nvidia" {
        pkgs.push("nvidia-container-toolkit".into());
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
            [torch, "python-numpy", "python-pandas", "python-scikit-learn", "jupyterlab",
             "python-huggingface-hub", "python-pip"]
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
    let mut services: Vec<String> =
        to_strings(vec!["NetworkManager.service", "systemd-timesyncd.service", "fstrim.timer"]);
    if let Some(dm) = desktop_info(cfg).display_manager {
        services.push(format!("{dm}.service"));
    }
    let all_presets = presets();
    for key in &cfg.packages.presets {
        if let Some((_, info)) = all_presets.iter().find(|(k, _)| k == key) {
            services.extend(info.services.iter().map(|s| s.to_string()));
        }
    }
    services.extend(engine_info(&cfg.containers.engine).services.iter().map(|s| s.to_string()));
    if cfg.ai.ollama {
        services.push("ollama.service".into());
    }
    dedup(services)
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
    cfg.installer.enabled && cfg.packages.desktop != "none"
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
        errors.push("Die ID darf nur Kleinbuchstaben, Ziffern und '-' enthalten (max. 31 Zeichen).".to_string());
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
            errors.push(format!("Repository '{}' braucht einen gültigen Namen und eine Server-URL.", repo.name));
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
    let names: Vec<&str> = cfg.containers.stacks.iter().map(|s| s.name.as_str()).collect();
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
        warnings.push("CUDA-Beschleunigung ohne NVIDIA-Treiber – bitte unter Pakete NVIDIA wählen.".to_string());
    }
    if matches!(cfg.ai.accel.as_str(), "cuda" | "rocm") && (cfg.ai.ollama || cfg.ai.dev) {
        warnings.push("CUDA/ROCm sind mehrere GB groß – die ISO wird entsprechend größer.".to_string());
    }
    if cfg.installer.enabled && cfg.packages.desktop == "none" {
        warnings.push(
            "Der grafische Installer benötigt einen Desktop – ohne Desktop wird archinstall genutzt.".to_string(),
        );
    }
    if cfg.repos.core_testing || cfg.repos.extra_testing {
        warnings.push("Testing-Repositories können instabil sein.".to_string());
    }
    (errors, warnings)
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
    let mut missing: Vec<String> = re_de.captures_iter(&stderr).map(|c| c[1].to_string()).collect();
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
        for p in ["linux-zen", "linux-zen-headers", "nvidia-open-dkms", "docker", "nvidia-container-toolkit",
                  "ollama", "ollama-cuda"] {
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
        let preset = stack_presets().into_iter().find(|(k, _)| *k == "open-webui").unwrap().1;
        cfg.containers.stacks = vec![Stack {
            name: "open-webui".into(),
            title: preset.title.into(),
            compose: preset.compose.into(),
            autostart: true,
            url: String::new(),
        }];
        assert_eq!(
            container_images(&cfg),
            vec!["nginx:alpine".to_string(), "postgres:17".to_string(),
                 "ghcr.io/open-webui/open-webui:main".to_string()]
        );
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
