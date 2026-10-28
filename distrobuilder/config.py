"""Projekt-Konfiguration: Standardwerte, Presets, Laden/Speichern und Paketlisten."""

import copy
import json
import os
import re
import subprocess

DEFAULTS = {
    "general": {
        "name": "MeinOS",
        "id": "meinos",
        "version": "1.0",
        "publisher": "",
        "homepage": "",
        "support_url": "",
        "bug_url": "",
        "hostname": "meinos",
    },
    "branding": {
        "logo": "",
        "logo_mode": "image",  # image | chafa | ascii | builtin
        "ascii_logo": "  __  __      _       ___  ____\n |  \\/  | ___(_)_ __ / _ \\/ ___|\n | |\\/| |/ _ \\ | '_ \\ | | \\___ \\\n | |  | |  __/ | | | | |_| |___) |\n |_|  |_|\\___|_|_| |_|\\___/|____/",
        "color": "#1793d1",
        "wallpaper": "",
        "motd": "Willkommen bei MeinOS!",
        "fastfetch_on_terminal": True,
    },
    "repos": {
        "core": True,
        "extra": True,
        "multilib": False,
        "core_testing": False,
        "extra_testing": False,
        "mirror": "",
        "custom": [],  # [{"name", "server", "siglevel"}]
    },
    "packages": {
        "kernel": "linux",
        "desktop": "kde",
        "presets": ["audio", "firefox", "fonts", "utils"],
        "custom": "",
        "rescue_tools": True,
        "gpu": "mesa",
    },
    "installer": {
        "enabled": True,
        "autostart": True,
        "locale": "de_DE.UTF-8",
        "timezone": "Europe/Berlin",
        "keyboard": "de",
        "filesystem": "ext4",
        "zram": True,
        "source": "offline",  # offline (von der ISO) | online (pacstrap)
        "allow_online": True,
        "encrypt": False,
        "bootloader": "grub",
        "swap_gb": 0,
        "welcome": "Dieser Assistent installiert das System auf deine Festplatte.",
    },
    "containers": {
        "engine": "none",  # none | docker | podman
        "preload": True,  # Images beim Bauen herunterladen und in die ISO packen
        "images": "",
        "stacks": [],  # [{"name", "compose", "autostart"}]
    },
    "ai": {
        "ollama": False,
        "accel": "cpu",  # cpu | cuda | rocm | vulkan
        "models": [],
        "expose": False,  # Ollama im Netzwerk freigeben (0.0.0.0)
        "dev": False,  # PyTorch, Jupyter & Co.
    },
    "flatpak": {
        "enabled": False,
        "flathub": True,
        "store": True,  # Discover bzw. GNOME Software mit Flatpak-Unterstützung
        "preload": True,  # Apps beim Bauen herunterladen und mitliefern
        "remotes": [],  # [{"name", "url"}] – URL einer .flatpakrepo-Datei
        "apps": [],  # App-IDs, optional mit Quelle: "quelle:org.example.App"
    },
    "distrobox": {
        "enabled": False,
        "boxes": [],  # [{"name", "image", "packages", "export"}]
    },
    "image": {  # nur für ARM/Raspberry Pi: fertiges System statt ISO mit Installer
        "username": "user",
        "password": "user",
        "expire_password": True,  # Passwort beim ersten Anmelden ändern
        "autologin": False,
        "size_gb": 8,
    },
    "build": {
        "arch": "x86_64",  # x86_64 | aarch64 | rpi
        "project_dir": os.path.expanduser("~/DistroBuilder"),
        "compression": "zstd",  # zstd (schnell) | xz (klein)
    },
}

# key: (Label, Beschreibung, pacman-Architektur, Container/Flatpak-Architektur)
ARCHES = {
    "x86_64": ("x86_64 (PC, Laptop)", "Bootfähige ISO mit Live-System und grafischem Installer", "x86_64", "amd64"),
    "aarch64": ("ARM64 (UEFI)", "Fertiges Disk-Image für ARM64-Rechner und VMs mit UEFI (Arch Linux ARM)",
                "aarch64", "arm64"),
    "rpi": ("Raspberry Pi 4/5 (ARM64)", "SD-Karten-Image für Raspberry Pi 4, 400 und 5 (Arch Linux ARM)",
            "aarch64", "arm64"),
}

# Arch Linux ARM: Basis-System pro Plattform
ALARM_TARBALLS = {
    "aarch64": "http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz",
    "rpi": "http://os.archlinuxarm.org/os/ArchLinuxARM-rpi-aarch64-latest.tar.gz",
}

ARM_KERNELS = {
    "aarch64": [("linux-aarch64", "Linux (Arch Linux ARM)")],
    "rpi": [("linux-rpi", "Raspberry Pi (Pi 4/400/5)"), ("linux-rpi-16k", "Raspberry Pi 5 (16K-Seiten, schneller)")],
}

KERNELS = [
    ("linux", "Linux (Standard)"),
    ("linux-lts", "Linux LTS"),
    ("linux-zen", "Linux Zen"),
    ("linux-hardened", "Linux Hardened"),
]

# key: (Label, Pakete, Display-Manager, Session für Autologin)
DESKTOPS = {
    "none": ("Kein Desktop (nur Konsole)", [], None, None),
    "kde": ("KDE Plasma", ["plasma-meta", "konsole", "dolphin", "kate", "ark", "spectacle", "sddm"], "sddm", "plasma"),
    "gnome": ("GNOME", ["gnome", "gnome-tweaks", "gdm"], "gdm", None),
    "xfce": ("Xfce", ["xorg-server", "xfce4", "xfce4-goodies", "lightdm", "lightdm-gtk-greeter",
                      "network-manager-applet", "gvfs"], "lightdm", "xfce"),
    "cinnamon": ("Cinnamon", ["xorg-server", "cinnamon", "gnome-terminal", "nemo", "lightdm",
                              "lightdm-gtk-greeter", "gvfs"], "lightdm", "cinnamon"),
    "lxqt": ("LXQt", ["xorg-server", "lxqt", "breeze-icons", "sddm", "network-manager-applet"], "sddm", "lxqt"),
}

# key: (Label, Beschreibung, Pakete, Dienste)
PRESETS = {
    "audio": ("Audio (PipeWire)", "pipewire, pulse, alsa, wireplumber",
              ["pipewire", "pipewire-pulse", "pipewire-alsa", "wireplumber"], []),
    "bluetooth": ("Bluetooth", "bluez + Dienst", ["bluez", "bluez-utils"], ["bluetooth.service"]),
    "printing": ("Drucken", "CUPS + Treiber", ["cups", "cups-pdf", "gutenprint"], ["cups.service"]),
    "firefox": ("Firefox", "Webbrowser", ["firefox"], []),
    "chromium": ("Chromium", "Webbrowser", ["chromium"], []),
    "office": ("LibreOffice", "Office-Paket", ["libreoffice-fresh"], []),
    "media": ("Multimedia", "VLC, mpv, GIMP", ["vlc", "mpv", "gimp"], []),
    "fonts": ("Schriften", "Noto + Emoji", ["noto-fonts", "noto-fonts-emoji", "ttf-dejavu"], []),
    "flatpak": ("Flatpak", "Flatpak-Unterstützung", ["flatpak"], []),
    "dev": ("Entwicklung", "base-devel, git", ["base-devel", "git"], []),
    "utils": ("Werkzeuge", "htop, wget, unzip, man", ["htop", "wget", "unzip", "zip", "man-db", "man-pages"], []),
    "gaming": ("Gaming", "Steam (benötigt multilib)", ["steam"], []),
}

# key: (Label, Pakete)
GPUS = {
    "mesa": ("Standard (Mesa, freie Treiber)", ["mesa", "vulkan-icd-loader"]),
    "amd": ("AMD Radeon (Mesa + Vulkan)", ["mesa", "vulkan-radeon", "vulkan-icd-loader"]),
    "intel": ("Intel (Mesa + Vulkan + VA-API)", ["mesa", "vulkan-intel", "intel-media-driver", "vulkan-icd-loader"]),
    "nvidia": ("NVIDIA (proprietär, nvidia-open)", ["nvidia-open-dkms", "nvidia-utils", "nvidia-settings",
                                                     "vulkan-icd-loader"]),
}

CONTAINER_ENGINES = {
    "none": ("Keine", [], []),
    "docker": ("Docker", ["docker", "docker-compose", "docker-buildx", "lazydocker"], ["docker.service"]),
    "podman": ("Podman (ohne Daemon, rootless)", ["podman", "podman-compose", "podman-docker", "distrobox"],
               ["podman.socket"]),
}

OLLAMA_ACCEL = {
    "cpu": ("Nur CPU", ["ollama"]),
    "cuda": ("NVIDIA (CUDA)", ["ollama", "ollama-cuda"]),
    "rocm": ("AMD (ROCm)", ["ollama", "ollama-rocm"]),
    "vulkan": ("Vulkan (AMD/Intel/NVIDIA)", ["ollama", "ollama-vulkan", "vulkan-icd-loader"]),
}

# (Modell, Beschreibung, ungefähre Größe in GB)
MODEL_PRESETS = [
    ("llama3.2:3b", "Meta Llama 3.2 – 3B, schnell", 2.0),
    ("llama3.1:8b", "Meta Llama 3.1 – 8B, Allrounder", 4.9),
    ("qwen3:8b", "Qwen 3 – 8B, stark mehrsprachig", 5.2),
    ("gemma3:4b", "Google Gemma 3 – 4B", 3.3),
    ("mistral:7b", "Mistral 7B", 4.1),
    ("deepseek-r1:8b", "DeepSeek R1 – 8B, Reasoning", 5.2),
    ("phi4-mini", "Microsoft Phi-4 mini – 3.8B", 2.5),
    ("nomic-embed-text", "Embeddings für RAG/Suche", 0.3),
]

STACK_PRESETS = {
    "open-webui": ("Open WebUI", "ChatGPT-ähnliche Weboberfläche für Ollama – http://localhost:3000", """services:
  open-webui:
    image: ghcr.io/open-webui/open-webui:main
    network_mode: host
    environment:
      - PORT=3000
      - OLLAMA_BASE_URL=http://127.0.0.1:11434
    volumes:
      - open-webui:/app/backend/data
    restart: unless-stopped
volumes:
  open-webui: {}
"""),
    "portainer": ("Portainer", "Container-Verwaltung im Browser – https://localhost:9443", """services:
  portainer:
    image: docker.io/portainer/portainer-ce:latest
    ports:
      - "9443:9443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - portainer_data:/data
    restart: unless-stopped
volumes:
  portainer_data: {}
"""),
    "searxng": ("SearXNG", "Private Metasuchmaschine – http://localhost:8888", """services:
  searxng:
    image: docker.io/searxng/searxng:latest
    ports:
      - "8888:8080"
    restart: unless-stopped
"""),
}

# (App-ID, Name, nur x86_64)
FLATPAK_PRESETS = [
    ("com.github.tchx84.Flatseal", "Flatseal – Flatpak-Berechtigungen verwalten", False),
    ("org.mozilla.firefox", "Firefox", False),
    ("org.libreoffice.LibreOffice", "LibreOffice", False),
    ("org.gimp.GIMP", "GIMP", False),
    ("org.videolan.VLC", "VLC", False),
    ("org.kde.kdenlive", "Kdenlive – Videoschnitt", False),
    ("com.obsproject.Studio", "OBS Studio", False),
    ("org.signal.Signal", "Signal", False),
    ("com.spotify.Client", "Spotify", True),
    ("com.discordapp.Discord", "Discord", True),
    ("com.valvesoftware.Steam", "Steam", True),
    ("com.visualstudio.code", "Visual Studio Code", False),
]
FLATHUB_URL = "https://dl.flathub.org/repo/flathub.flatpakrepo"

# key: (Label, Image)
DISTROBOX_PRESETS = {
    "ubuntu": ("Ubuntu 24.04", "docker.io/library/ubuntu:24.04"),
    "debian": ("Debian 12", "docker.io/library/debian:12"),
    "fedora": ("Fedora 42", "registry.fedoraproject.org/fedora-toolbox:42"),
    "opensuse": ("openSUSE Tumbleweed", "registry.opensuse.org/opensuse/distrobox:latest"),
    "alpine": ("Alpine Linux", "docker.io/library/alpine:latest"),
    "arch": ("Arch Linux (nur x86_64)", "docker.io/library/archlinux:latest"),
}

SIGLEVELS = ["Optional TrustAll", "Optional", "Required", "Never"]

# Minimale Live-Pakete, wenn die Arch-Rettungswerkzeuge abgewählt sind
MINIMAL_LIVE = [
    "base", "linux", "linux-firmware", "mkinitcpio", "mkinitcpio-archiso", "syslinux",
    "memtest86+", "memtest86+-efi", "edk2-shell", "efibootmgr", "grub", "amd-ucode", "intel-ucode",
    "arch-install-scripts", "gptfdisk", "parted", "dosfstools", "e2fsprogs", "btrfs-progs",
    "sudo", "nano", "vim", "zsh", "grml-zsh-config", "iwd", "openssh", "reflector", "rsync",
    "less", "usbutils", "sof-firmware",
]

# Dienst-Links aus releng, die im Minimalmodus ins Leere zeigen würden
MINIMAL_DROP_UNITS = [
    "cloud-init.target.wants", "multi-user.target.wants/hv_fcopy_daemon.service",
    "multi-user.target.wants/hv_kvp_daemon.service", "multi-user.target.wants/hv_vss_daemon.service",
    "multi-user.target.wants/vboxservice.service", "multi-user.target.wants/vmtoolsd.service",
    "multi-user.target.wants/vmware-vmblock-fuse.service", "multi-user.target.wants/livecd-talk.service",
    "multi-user.target.wants/ModemManager.service", "dbus-org.freedesktop.ModemManager1.service",
    "sockets.target.wants/pcscd.socket", "sound.target.wants/livecd-alsa-unmuter.service",
]

INSTALLER_DEPS = ["python", "python-gobject", "gtk4", "libadwaita", "arch-install-scripts",
                  "gptfdisk", "parted", "dosfstools", "e2fsprogs", "btrfs-progs", "rsync", "sudo",
                  "squashfs-tools", "cryptsetup"]

IMAGE_RE = re.compile(r"^[a-z0-9][a-z0-9._/:@-]*$")
MODEL_RE = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9._/:-]*$")
STACK_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,40}$")

ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,30}$")
USER_RE = re.compile(r"^[a-z_][a-z0-9_-]{0,31}$")
FLATPAK_APP_RE = re.compile(r"^[A-Za-z_][\w-]*(\.[A-Za-z0-9_-]+)+$")
REMOTE_RE = re.compile(r"^[A-Za-z0-9_-]+$")
BOX_RE = re.compile(r"^[a-z0-9][a-z0-9_.-]{0,40}$")
PKG_RE = re.compile(r"^[a-zA-Z0-9@._+-]+$")


def default_config():
    return copy.deepcopy(DEFAULTS)


def _merge(base, data):
    for k, v in data.items():
        if isinstance(v, dict) and isinstance(base.get(k), dict):
            _merge(base[k], v)
        else:
            base[k] = v
    return base


def load(path):
    with open(path, encoding="utf-8") as f:
        return _merge(default_config(), json.load(f))


def save(cfg, path):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(cfg, f, indent=2, ensure_ascii=False)


def slugify(name):
    s = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    return s[:31] or "meinos"


def custom_packages(cfg):
    return [p for p in re.split(r"[\s,]+", cfg["packages"]["custom"]) if p]


def desktop_info(cfg):
    return DESKTOPS.get(cfg["packages"]["desktop"], DESKTOPS["none"])


def needs_chafa(cfg):
    return cfg["branding"]["logo_mode"] in ("image", "chafa") and bool(cfg["branding"]["logo"])


def arch(cfg):
    a = cfg["build"].get("arch", "x86_64")
    return a if a in ARCHES else "x86_64"


def is_image(cfg):
    """ARM-Plattformen werden als fertiges Disk-Image gebaut, x86_64 als ISO."""
    return arch(cfg) != "x86_64"


def pacman_arch(cfg):
    return ARCHES[arch(cfg)][2]


def oci_arch(cfg):
    return ARCHES[arch(cfg)][3]


def kernels(cfg):
    return ARM_KERNELS.get(arch(cfg), KERNELS)


def kernel(cfg):
    """Gewählter Kernel – fällt auf den Standard der Plattform zurück, wenn er dort nicht existiert."""
    options = [k for k, _l in kernels(cfg)]
    k = cfg["packages"]["kernel"]
    return k if k in options else options[0]


def gpus(cfg):
    return {"mesa": GPUS["mesa"]} if is_image(cfg) else GPUS


def flatpak_apps(cfg):
    """[(Quelle, App-ID)] – 'quelle:app.id' oder nur 'app.id' (Flathub)."""
    if not cfg["flatpak"]["enabled"]:
        return []
    out = []
    for entry in cfg["flatpak"]["apps"]:
        remote, _, app = entry.rpartition(":")
        out.append((remote or "flathub", app))
    return out


def flatpak_remotes(cfg):
    """[(Name, URL der .flatpakrepo-Datei)]"""
    f = cfg["flatpak"]
    if not f["enabled"]:
        return []
    remotes = [("flathub", FLATHUB_URL)] if f["flathub"] else []
    return remotes + [(r.get("name", ""), r.get("url", "")) for r in f["remotes"]]


def distrobox_active(cfg):
    return cfg["distrobox"]["enabled"]


def target_packages(cfg):
    """Pakete, die der Installer auf dem Zielsystem installiert."""
    p = cfg["packages"]
    plat = arch(cfg)
    kern = kernel(cfg)
    pkgs = ["base", kern, "linux-firmware"]
    if plat == "x86_64":
        pkgs += ["amd-ucode", "intel-ucode", "grub", "efibootmgr"]
    elif plat == "aarch64":
        pkgs += ["archlinuxarm-keyring", "grub", "efibootmgr"]
    else:
        pkgs += ["archlinuxarm-keyring", "raspberrypi-bootloader", "firmware-raspberrypi"]
    pkgs += ["dosfstools", "networkmanager", "sudo", "nano", "vim", "bash-completion", "fastfetch"]
    if needs_chafa(cfg):
        pkgs.append("chafa")
    if cfg["installer"]["zram"]:
        pkgs.append("zram-generator")
    pkgs += desktop_info(cfg)[1]
    gpu = p.get("gpu", "mesa")
    pkgs += gpus(cfg).get(gpu, GPUS["mesa"])[1]
    if gpu == "nvidia":
        pkgs.append(f"{kern}-headers")
    eng = cfg["containers"]["engine"]
    pkgs += CONTAINER_ENGINES.get(eng, CONTAINER_ENGINES["none"])[1]
    if eng != "none" and gpu == "nvidia":
        pkgs.append("nvidia-container-toolkit")
    if distrobox_active(cfg):
        pkgs.append("distrobox")
        if eng == "none":
            pkgs.append("podman")
    if cfg["flatpak"]["enabled"]:
        pkgs.append("flatpak")
        if cfg["flatpak"]["store"]:
            pkgs += flatpak_store(cfg)
    ai = cfg["ai"]
    if ai["ollama"]:
        pkgs += OLLAMA_ACCEL.get(ai["accel"], OLLAMA_ACCEL["cpu"])[1]
    if ai["dev"]:
        torch = {"cuda": "python-pytorch-cuda", "rocm": "python-pytorch-rocm"}.get(ai["accel"], "python-pytorch")
        pkgs += [torch, "python-numpy", "python-pandas", "python-scikit-learn", "jupyterlab",
                 "python-huggingface-hub", "python-pip"]
    for key in p["presets"]:
        if key in PRESETS:
            pkgs += PRESETS[key][2]
    pkgs += custom_packages(cfg)
    return _dedup(pkgs)


def flatpak_store(cfg):
    """App-Store passend zum Desktop, der Flatpaks anzeigt und aktualisiert."""
    desktop = cfg["packages"]["desktop"]
    if desktop == "none":
        return []
    if desktop in ("kde", "lxqt"):
        return ["discover"]
    return ["gnome-software"]


def target_services(cfg):
    services = ["NetworkManager.service", "systemd-timesyncd.service", "fstrim.timer"]
    dm = desktop_info(cfg)[2]
    if dm:
        services.append(f"{dm}.service")
    for key in cfg["packages"]["presets"]:
        if key in PRESETS:
            services += PRESETS[key][3]
    services += CONTAINER_ENGINES.get(cfg["containers"]["engine"], CONTAINER_ENGINES["none"])[2]
    if distrobox_active(cfg) and cfg["containers"]["engine"] == "none":
        services.append("podman.socket")
    if cfg["ai"]["ollama"]:
        services.append("ollama.service")
    return _dedup(services)


def container_images(cfg):
    """Alle Images, die vorab in die ISO geladen werden sollen (Liste + aus Compose-Stacks)."""
    c = cfg["containers"]
    if c["engine"] == "none":
        return []
    images = [i for i in re.split(r"[\s,]+", c["images"]) if i]
    for st in c["stacks"]:
        images += re.findall(r"^\s*image:\s*[\"']?([^\s\"']+)", st.get("compose", ""), flags=re.M)
    return _dedup(images)


def kernel_params(cfg):
    params = []
    if cfg["packages"].get("gpu") == "nvidia":
        params.append("nvidia_drm.modeset=1")
    if cfg["installer"]["zram"]:
        params.append("zswap.enabled=0")
    return params


def image_packages(cfg):
    """Pakete für das fertige ARM-Image (zusätzlich zum Arch-Linux-ARM-Basissystem)."""
    return _dedup(target_packages(cfg) + ["e2fsprogs", "util-linux"])


def live_packages(cfg, releng_packages):
    base = list(releng_packages) if cfg["packages"]["rescue_tools"] else list(MINIMAL_LIVE)
    # Das Live-System enthält alle Zielpakete, damit der Installer offline kopieren kann
    pkgs = base + target_packages(cfg)
    if cfg["packages"].get("gpu") == "nvidia":
        pkgs.append("linux-headers")
    if installer_active(cfg):
        pkgs += INSTALLER_DEPS
    return _dedup(pkgs)


def installer_active(cfg):
    return not is_image(cfg) and cfg["installer"]["enabled"] and cfg["packages"]["desktop"] != "none"


def _dedup(items):
    seen, out = set(), []
    for i in items:
        if i not in seen:
            seen.add(i)
            out.append(i)
    return out


def validate(cfg):
    """Gibt (Fehler, Warnungen) zurück."""
    errors, warnings = [], []
    g, b = cfg["general"], cfg["branding"]
    if not g["name"].strip():
        errors.append("Der Distro-Name darf nicht leer sein.")
    if not ID_RE.match(g["id"]):
        errors.append("Die ID darf nur Kleinbuchstaben, Ziffern und '-' enthalten (max. 31 Zeichen).")
    if not re.match(r"^[a-zA-Z0-9-]{1,63}$", g["hostname"]):
        errors.append("Ungültiger Hostname.")
    for key, label in (("logo", "Logo"), ("wallpaper", "Hintergrundbild")):
        if b[key] and not os.path.isfile(b[key]):
            errors.append(f"{label}-Datei nicht gefunden: {b[key]}")
    if b["logo_mode"] in ("image", "chafa") and not b["logo"]:
        warnings.append("Kein Logo-Bild gewählt – fastfetch zeigt das Arch-Logo.")
    for pkg in custom_packages(cfg):
        if not PKG_RE.match(pkg):
            errors.append(f"Ungültiger Paketname: {pkg}")
    for repo in cfg["repos"]["custom"]:
        if not re.match(r"^[a-zA-Z0-9_-]+$", repo.get("name", "")) or not repo.get("server"):
            errors.append(f"Repository '{repo.get('name')}' braucht einen gültigen Namen und eine Server-URL.")
    if "gaming" in cfg["packages"]["presets"] and not cfg["repos"]["multilib"]:
        errors.append("Steam benötigt das multilib-Repository.")
    for img in container_images(cfg):
        if not IMAGE_RE.match(img):
            errors.append(f"Ungültiger Image-Name: {img}")
    names = [st.get("name", "") for st in cfg["containers"]["stacks"]]
    for n in names:
        if not STACK_RE.match(n):
            errors.append(f"Ungültiger Stack-Name '{n}' (a-z, 0-9, -)")
    if len(set(names)) != len(names):
        errors.append("Stack-Namen müssen eindeutig sein.")
    if cfg["containers"]["stacks"] and cfg["containers"]["engine"] == "none":
        errors.append("Compose-Stacks benötigen Docker oder Podman.")
    for m in cfg["ai"]["models"]:
        if not MODEL_RE.match(m):
            errors.append(f"Ungültiger Modellname: {m}")
    if cfg["ai"]["models"] and not cfg["ai"]["ollama"]:
        warnings.append("KI-Modelle sind ausgewählt, aber Ollama ist deaktiviert.")
    if cfg["ai"]["accel"] == "cuda" and cfg["packages"].get("gpu") != "nvidia":
        warnings.append("CUDA-Beschleunigung ohne NVIDIA-Treiber – bitte unter Pakete NVIDIA wählen.")
    if cfg["ai"]["accel"] in ("cuda", "rocm") and (cfg["ai"]["ollama"] or cfg["ai"]["dev"]):
        warnings.append("CUDA/ROCm sind mehrere GB groß – die ISO wird entsprechend größer.")
    if not is_image(cfg) and cfg["installer"]["enabled"] and cfg["packages"]["desktop"] == "none":
        warnings.append("Der grafische Installer benötigt einen Desktop – ohne Desktop wird archinstall genutzt.")
    if cfg["repos"]["core_testing"] or cfg["repos"]["extra_testing"]:
        warnings.append("Testing-Repositories können instabil sein.")
    _validate_arch(cfg, errors, warnings)
    _validate_flatpak(cfg, errors, warnings)
    _validate_distrobox(cfg, errors, warnings)
    return errors, warnings


def _validate_arch(cfg, errors, warnings):
    plat = cfg["build"].get("arch", "x86_64")
    if plat not in ARCHES:
        errors.append(f"Unbekannte Architektur: {plat}")
        return
    if not is_image(cfg):
        return
    label = ARCHES[plat][0]
    if cfg["packages"].get("gpu", "mesa") != "mesa":
        errors.append(f"Der Grafiktreiber '{cfg['packages']['gpu']}' ist für {label} nicht verfügbar – bitte Mesa wählen.")
    if cfg["ai"]["accel"] in ("cuda", "rocm") and (cfg["ai"]["ollama"] or cfg["ai"]["dev"]):
        errors.append(f"CUDA/ROCm gibt es für {label} nicht – bitte CPU oder Vulkan wählen.")
    if cfg["repos"]["multilib"]:
        errors.append("Das multilib-Repository gibt es nur für x86_64.")
    if "gaming" in cfg["packages"]["presets"]:
        errors.append(f"Steam ist für {label} nicht verfügbar.")
    if cfg["repos"]["core_testing"] or cfg["repos"]["extra_testing"]:
        warnings.append("Arch Linux ARM hat keine Testing-Repositories – die Einstellung wird ignoriert.")
    if cfg["packages"]["kernel"] != kernel(cfg):
        warnings.append(f"Kernel '{cfg['packages']['kernel']}' gibt es für {label} nicht – es wird {kernel(cfg)} "
                        "verwendet.")
    img = cfg["image"]
    if not USER_RE.match(img["username"]) or img["username"] == "root":
        errors.append("Ungültiger Benutzername für das Image (a-z, 0-9, _ und -).")
    if not img["password"]:
        errors.append("Das Image braucht ein Startpasswort für den Benutzer.")
    if img["autologin"] and img.get("expire_password", True) and cfg["packages"]["desktop"] != "none":
        warnings.append("Mit automatischer Anmeldung muss das Startpasswort nicht geändert werden – bitte selbst ändern.")
    if int(img.get("size_gb", 0)) < 3:
        errors.append("Das Image muss mindestens 3 GB groß sein.")
    known = {m: sz for m, _d, sz in MODEL_PRESETS}
    need = 3 + (sum(known.get(m, 0) for m in cfg["ai"]["models"]) if cfg["ai"]["ollama"] else 0)
    if cfg["packages"]["desktop"] != "none":
        need += 3
    if need > int(img.get("size_gb", 0)):
        warnings.append(f"Das Image ist mit {img['size_gb']} GB vermutlich zu klein (geschätzt mind. {need:.0f} GB).")
    warnings.append(f"{label}: Es entsteht ein fertig installiertes Image (Arch Linux ARM). Nicht jedes Paket "
                    "aus Arch Linux gibt es auch für ARM.")


def _validate_flatpak(cfg, errors, warnings):
    f = cfg["flatpak"]
    if not f["enabled"]:
        if f["apps"]:
            warnings.append("Flatpak-Apps sind ausgewählt, aber Flatpak ist deaktiviert.")
        return
    names = [n for n, _u in flatpak_remotes(cfg)]
    for name, url in flatpak_remotes(cfg):
        if not REMOTE_RE.match(name) or not re.match(r"^https?://\S+$", url):
            errors.append(f"Flatpak-Quelle '{name}' braucht einen gültigen Namen und eine URL.")
    if len(set(names)) != len(names):
        errors.append("Flatpak-Quellen müssen eindeutige Namen haben.")
    for remote, app in flatpak_apps(cfg):
        if not FLATPAK_APP_RE.match(app):
            errors.append(f"Ungültige Flatpak-App-ID: {app}")
        elif remote not in names:
            errors.append(f"Flatpak-App {app}: Quelle '{remote}' ist nicht eingerichtet.")
    if is_image(cfg):
        x86_only = {a for a, _l, only in FLATPAK_PRESETS if only}
        for _r, app in flatpak_apps(cfg):
            if app in x86_only:
                errors.append(f"Die Flatpak-App {app} gibt es nur für x86_64.")
    if f["store"] and cfg["packages"]["desktop"] == "none":
        warnings.append("Ein App-Store für Flatpaks braucht einen Desktop.")


def _validate_distrobox(cfg, errors, warnings):
    if not distrobox_active(cfg):
        return
    boxes = cfg["distrobox"]["boxes"]
    names = [b.get("name", "") for b in boxes]
    for b in boxes:
        if not BOX_RE.match(b.get("name", "")):
            errors.append(f"Ungültiger Distrobox-Name '{b.get('name')}' (a-z, 0-9, _ . -)")
        if not IMAGE_RE.match(b.get("image", "")):
            errors.append(f"Ungültiges Image für Distrobox '{b.get('name')}': {b.get('image')}")
        for pkg in re.split(r"[\s,]+", b.get("packages", "")):
            if pkg and not PKG_RE.match(pkg):
                errors.append(f"Distrobox '{b.get('name')}': ungültiger Paketname {pkg}")
        for app in re.split(r"[\s,]+", b.get("export", "")):
            if app and not PKG_RE.match(app):
                errors.append(f"Distrobox '{b.get('name')}': ungültiger App-Name {app}")
        if is_image(cfg) and "archlinux" in b.get("image", ""):
            errors.append(f"Distrobox '{b.get('name')}': das Arch-Linux-Image gibt es nur für x86_64.")
    if len(set(names)) != len(names):
        errors.append("Distrobox-Namen müssen eindeutig sein.")
    if not boxes:
        warnings.append("Distrobox ist aktiv, aber es sind keine Container angelegt.")


def check_packages(names):
    """Prüft Pakete gegen die lokale pacman-Datenbank. Gibt unbekannte zurück."""
    if not names:
        return []
    r = subprocess.run(["pacman", "-Sp", "--print-format", "%n", *names],
                       capture_output=True, text=True)
    missing = re.findall(r"target not found: (\S+)", r.stderr)
    missing += re.findall(r"Ziel nicht gefunden: (\S+)", r.stderr)
    return _dedup(missing)


def search_packages(query, limit=60):
    r = subprocess.run(["pacman", "-Ss", query], capture_output=True, text=True)
    results, lines = [], r.stdout.splitlines()
    for i in range(0, len(lines) - 1, 2):
        head, desc = lines[i], lines[i + 1].strip()
        m = re.match(r"^(\S+)/(\S+) (\S+)", head)
        if m:
            results.append({"repo": m[1], "name": m[2], "version": m[3], "desc": desc,
                            "installed": "[" in head})
        if len(results) >= limit:
            break
    return results
