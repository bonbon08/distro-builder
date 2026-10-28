# Projektdatei

Ein Projekt ist eine JSON-Datei (Menü → „Projekt speichern unter …“, Strg+S). Fehlende Schlüssel
werden beim Laden mit Standardwerten ergänzt; ältere Projekte bleiben also gültig. Beim Bauen wird
eine Kopie als `~/DistroBuilder/<id>/project.json` abgelegt. Den letzten Stand speichert die
Oberfläche automatisch in `~/.config/distro-builder/autosave.json`.

## Alle Einstellungen

| Schlüssel | Standard | Bedeutung |
|---|---|---|
| `general.name` | `"MeinOS"` | Anzeigename der Distribution |
| `general.id` | `"meinos"` | `[a-z0-9-]`, max. 31 Zeichen; ISO-Name, Pfade, Dienstnamen |
| `general.version` | `"1.0"` | Version (`VERSION_ID`, ISO-Version) |
| `general.publisher` | `""` | ISO-Herausgeber (Standard: Name) |
| `general.homepage` | `""` | `HOME_URL` |
| `general.support_url` | `""` | `SUPPORT_URL` |
| `general.bug_url` | `""` | `BUG_REPORT_URL` |
| `general.hostname` | `"meinos"` | Hostname im Live-System und Vorschlag im Installer |
| `branding.logo` | `""` | Pfad zum Logo (wird als PNG, max. 512 px, übernommen) |
| `branding.logo_mode` | `"image"` | `image` · `chafa` · `ascii` · `builtin` |
| `branding.ascii_logo` | `"  __  __      _       ___  ____\n | …"` | ASCII-Logo für `ascii` |
| `branding.color` | `"#1793d1"` | Akzentfarbe `#rrggbb` |
| `branding.wallpaper` | `""` | Pfad zum Hintergrundbild |
| `branding.motd` | `"Willkommen bei MeinOS!"` | Inhalt von `/etc/motd` |
| `branding.fastfetch_on_terminal` | `true` | fastfetch in neuen Terminals |
| `repos.core` | `true` | immer aktiv |
| `repos.extra` | `true` |  |
| `repos.multilib` | `false` | 32-Bit |
| `repos.core_testing` | `false` |  |
| `repos.extra_testing` | `false` |  |
| `repos.mirror` | `""` | eigener `Server =`-Eintrag statt Mirrorliste |
| `repos.custom` | `[]` | Liste von `{name, server, siglevel, first}` |
| `packages.kernel` | `"linux"` | `linux` · `linux-lts` · `linux-zen` · `linux-hardened`; ARM: `linux-aarch64`, Raspberry Pi: `linux-rpi` · `linux-rpi-16k` |
| `packages.desktop` | `"kde"` | `none` · `kde` · `gnome` · `xfce` · `cinnamon` · `lxqt` |
| `packages.presets` | `["audio", "firefox", "fonts", "utils"]` | Sammlungen: `audio`, `bluetooth`, `printing`, `firefox`, `chromium`, `office`, `media`, `fonts`, `flatpak`, `dev`, `utils`, `gaming` |
| `packages.custom` | `""` | eigene Pakete, durch Leerzeichen/Zeilen getrennt |
| `packages.rescue_tools` | `true` | releng-Paketliste statt Minimalliste im Live-System |
| `packages.gpu` | `"mesa"` | `mesa` · `amd` · `intel` · `nvidia` |
| `installer.enabled` | `true` | Installer einbinden (braucht Desktop) |
| `installer.autostart` | `true` | Installer im Live-System automatisch öffnen |
| `installer.locale` | `"de_DE.UTF-8"` | Vorgabe |
| `installer.timezone` | `"Europe/Berlin"` | Vorgabe |
| `installer.keyboard` | `"de"` | X11-Layout, Vorgabe und Live-Tastatur |
| `installer.filesystem` | `"ext4"` | `ext4` · `btrfs` |
| `installer.zram` | `true` | zram-Swap |
| `installer.source` | `"offline"` | `offline` · `online` |
| `installer.allow_online` | `true` | Online-Quelle anbieten |
| `installer.encrypt` | `false` | Verschlüsselung vorauswählen |
| `installer.bootloader` | `"grub"` | `grub` · `systemd-boot` |
| `installer.swap_gb` | `0` | Swap-Datei in GB |
| `installer.welcome` | `"Dieser Assistent installiert das Sys…"` | Text auf der Willkommensseite |
| `containers.engine` | `"none"` | `none` · `docker` · `podman` |
| `containers.preload` | `true` | Images in die ISO packen |
| `containers.images` | `""` | zusätzliche Images, eins pro Zeile |
| `containers.stacks` | `[]` | Liste von `{name, title, compose, autostart, url}` |
| `ai.ollama` | `false` | Ollama installieren |
| `ai.accel` | `"cpu"` | `cpu` · `cuda` · `rocm` · `vulkan` |
| `ai.models` | `[]` | Modellnamen, z. B. `llama3.2:3b` |
| `ai.expose` | `false` | `OLLAMA_HOST=0.0.0.0` |
| `ai.dev` | `false` | PyTorch, Jupyter & Co. |
| `flatpak.enabled` | `false` | Flatpak einrichten |
| `flatpak.flathub` | `true` | Flathub als Quelle |
| `flatpak.store` | `true` | Discover bzw. GNOME Software installieren |
| `flatpak.preload` | `true` | Apps beim Bauen herunterladen und mitliefern |
| `flatpak.remotes` | `[]` | eigene Quellen: Liste von `{name, url}` (URL einer `.flatpakrepo`-Datei) |
| `flatpak.apps` | `[]` | App-IDs, z. B. `org.gimp.GIMP` oder `quelle:app.id` |
| `distrobox.enabled` | `false` | Distrobox installieren |
| `distrobox.boxes` | `[]` | Liste von `{name, image, packages, export}` |
| `image.username` | `"user"` | nur ARM: Benutzer im fertigen Image |
| `image.password` | `"user"` | nur ARM: Startpasswort |
| `image.expire_password` | `true` | nur ARM: Passwort beim ersten Anmelden ändern |
| `image.autologin` | `false` | nur ARM: automatisch anmelden |
| `image.size_gb` | `8` | nur ARM: Größe der Image-Datei |
| `build.arch` | `"x86_64"` | `x86_64` (ISO) · `aarch64` (ARM64-Image) · `rpi` (Raspberry-Pi-Image) |
| `build.project_dir` | `"~/DistroBuilder"` | Basisordner, Projekt liegt in `<project_dir>/<id>` |
| `build.compression` | `"zstd"` | `zstd` (schnell) · `xz` (klein); gilt für SquashFS bzw. das ARM-Image |

## Beispiel

```json
{
  "general": {
    "name": "NebulaOS",
    "id": "nebulaos",
    "version": "1.0",
    "publisher": "",
    "homepage": "",
    "support_url": "",
    "bug_url": "",
    "hostname": "nebula"
  },
  "branding": {
    "logo": "",
    "logo_mode": "image",
    "ascii_logo": "",
    "color": "#1793d1",
    "wallpaper": "",
    "motd": "Willkommen bei MeinOS!",
    "fastfetch_on_terminal": true
  },
  "repos": {
    "core": true,
    "extra": true,
    "multilib": false,
    "core_testing": false,
    "extra_testing": false,
    "mirror": "",
    "custom": [
      {
        "name": "nebula",
        "server": "https://repo.example.org/$arch",
        "siglevel": "Optional TrustAll",
        "first": true
      }
    ]
  },
  "packages": {
    "kernel": "linux",
    "desktop": "kde",
    "presets": [
      "audio",
      "firefox",
      "fonts",
      "utils"
    ],
    "custom": "",
    "rescue_tools": true,
    "gpu": "mesa"
  },
  "installer": {
    "enabled": true,
    "autostart": true,
    "locale": "de_DE.UTF-8",
    "timezone": "Europe/Berlin",
    "keyboard": "de",
    "filesystem": "ext4",
    "zram": true,
    "source": "offline",
    "allow_online": true,
    "encrypt": false,
    "bootloader": "grub",
    "swap_gb": 0,
    "welcome": "Dieser Assistent installiert das System auf deine Festplatte."
  },
  "containers": {
    "engine": "docker",
    "preload": true,
    "images": "",
    "stacks": [
      {
        "name": "portainer",
        "title": "Portainer",
        "compose": "services:\n  portainer:\n    image: docker.io/portainer/portainer-ce:latest\n",
        "autostart": true,
        "url": "https://localhost:9443"
      }
    ]
  },
  "ai": {
    "ollama": true,
    "accel": "vulkan",
    "models": [
      "llama3.2:3b"
    ],
    "expose": false,
    "dev": false
  },
  "flatpak": {
    "enabled": true,
    "flathub": true,
    "store": true,
    "preload": true,
    "remotes": [],
    "apps": [
      "org.mozilla.firefox",
      "com.github.tchx84.Flatseal"
    ]
  },
  "distrobox": {
    "enabled": true,
    "boxes": [
      {
        "name": "ubuntu",
        "image": "docker.io/library/ubuntu:24.04",
        "packages": "build-essential git",
        "export": ""
      }
    ]
  },
  "image": {
    "username": "user",
    "password": "user",
    "expire_password": true,
    "autologin": false,
    "size_gb": 8
  },
  "build": {
    "arch": "x86_64",
    "project_dir": "~/DistroBuilder",
    "compression": "zstd"
  }
}
```

## Ohne Oberfläche bauen

```python
from distrobuilder import config, profile, fetch

cfg = config.load("nebulaos.json")
pth = profile.generate(cfg)            # erzeugt ~/DistroBuilder/<id>/profile und build.sh (ISO oder ARM-Image)
fetch.prepare_assets(cfg, pth, print)  # Modelle und Images
```

```bash
sudo bash ~/DistroBuilder/nebulaos/build.sh
```
