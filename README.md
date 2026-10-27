# Distro Builder

Baukasten für deine eigene Linux-Distribution auf Basis von **Arch Linux**. In einer
GTK4/libadwaita-Oberfläche stellst du Name, Logo, Repositories, Pakete, Desktop, KI-Modelle
und Container zusammen. Distro Builder erzeugt daraus eine bootfähige **ISO mit eigenem
grafischen Installer**.

![Branding-Seite mit fastfetch-Vorschau](docs/screenshots/builder-branding.png)

- **Branding:** Name, `/etc/os-release`, Logo für fastfetch/neofetch (Bild, Zeichengrafik oder ASCII-Art), Akzentfarbe, Hintergrundbild, Bootmenü, motd
- **Repositories:** core, extra, multilib, testing, eigener Mirror, eigene Repos mit SigLevel und Reihenfolge
- **Pakete:** Kernel (linux, lts, zen, hardened), Desktop (KDE Plasma, GNOME, Xfce, Cinnamon, LXQt), Grafiktreiber (Mesa, AMD, Intel, NVIDIA), Paket-Sammlungen, Paketsuche
- **KI:** Ollama mit CPU, CUDA, ROCm oder Vulkan. Modelle wie Llama, Qwen, Gemma oder Mistral werden **direkt in die ISO gepackt**. Optional PyTorch und Jupyter.
- **Container:** Docker oder Podman, vorgeladene Images, Container-Apps als Compose-Stacks (Open WebUI, Portainer, SearXNG oder eigene)
- **Grafischer Installer:** Installation **offline direkt von der ISO**, mehrere Festplatten (`/home`, Datenlaufwerke, btrfs-RAID 1), **LUKS2-Verschlüsselung**, ext4 oder btrfs, GRUB oder systemd-boot
- **Bauen:** ein Klick, Fortschritt und Log, danach die ISO direkt in QEMU testen

> **Projektstatus 0.1.0 – frühe Version.** Profilerzeugung, Oberflächen und Downloads sind
> getestet (siehe [Tests](#entwicklung)). Der komplette Ablauf aus ISO-Build und Installation
> auf echter Hardware ist noch nicht umfassend erprobt. Teste deine ISO zuerst in einer VM.
> Der Installer **löscht die gewählten Festplatten vollständig**.

## Installation

### Arch Linux

```bash
git clone https://github.com/bonbon08/distro-builder.git
cd distro-builder
makepkg -si
```

### Direkt aus dem Quellordner

```bash
sudo pacman -S --needed python python-gobject gtk4 libadwaita archiso
./distro-builder
```

Benötigt: Python ≥ 3.10, PyGObject, GTK ≥ 4.12, libadwaita ≥ 1.5, `archiso`, `polkit` (für `pkexec`).
Bauen lassen sich die ISOs nur auf Arch Linux oder einer Arch-basierten Distribution, weil `mkarchiso` pacman braucht.

Optional:

| Paket | Wofür |
|---|---|
| `skopeo` (oder docker/podman) | Container-Images in die ISO packen |
| `qemu-desktop`, `edk2-ovmf` | Fertige ISO mit „In VM testen“ starten (UEFI) |

## Schnellstart

1. `distro-builder` starten.
2. **Allgemein:** Namen eingeben. Die ID für Dateinamen wird automatisch erzeugt.
3. **Branding:** Logo und Hintergrundbild wählen. Die Vorschau zeigt, wie fastfetch aussieht.
4. **Pakete:** Desktop und Kernel wählen, Sammlungen und eigene Pakete hinzufügen.
5. Optional **Container & KI:** Ollama und Modelle, Docker und Container-Apps.
6. **ISO bauen** → „ISO bauen“ klicken und das Passwort für `pkexec` eingeben.
7. Die ISO liegt in `~/DistroBuilder/<id>/out/`. Mit **In VM testen** startest du sie direkt in QEMU.

Projekte speicherst du mit Strg+S als JSON. Der letzte Stand wird beim Schließen automatisch gesichert.

## Oberfläche

| Seite | Inhalt |
|---|---|
| **Allgemein** | Name, ID, Version, Hostname, Herausgeber, URLs |
| **Branding** | Logo, Logo-Modus, Akzentfarbe, ASCII-Logo, Live-Vorschau, Hintergrund, motd, fastfetch beim Terminalstart |
| **Repositories** | Offizielle Repos, Mirror, eigene Repos, Vorschau der erzeugten `pacman.conf` |
| **Pakete** | Kernel, Desktop, Grafiktreiber, Rettungswerkzeuge, Sammlungen, eigene Pakete, Suche, Prüfung |
| **Container & KI** | Ollama, Beschleunigung, Modelle mit Größenprüfung, Entwicklungsumgebung, Docker/Podman, Images, Compose-Stacks |
| **Installer** | An/aus, Autostart im Live-System, Standardwerte (Quelle, Verschlüsselung, Bootloader, Swap, Sprache, Zeitzone, Tastatur, Dateisystem) |
| **ISO bauen** | Voraussetzungen, Projektordner, Kompression, Build, Log, Ergebnis |

<p>
  <img src="docs/screenshots/builder-extras.png" width="49%" alt="Container & KI">
  <img src="docs/screenshots/builder-repos.png" width="49%" alt="Repositories">
</p>

### Der Installer im Live-System

<p>
  <img src="docs/screenshots/installer-welcome.png" width="49%" alt="Installer: Willkommen">
  <img src="docs/screenshots/installer-options.png" width="49%" alt="Installer: Optionen">
</p>

Willkommen → Region → Festplatten → Optionen → Benutzer → Zusammenfassung → Installation.
Details stehen in [docs/installer.md](docs/installer.md).

## Wie es funktioniert

1. Distro Builder kopiert das offizielle archiso-Profil `releng` nach `~/DistroBuilder/<id>/profile` und passt es an: Pakete, `pacman.conf`, Branding, Live-Benutzer mit Autologin, NetworkManager und Installer.
2. KI-Modelle und Container-Images werden in `~/DistroBuilder/<id>/cache` geladen und ins Profil gelegt.
3. `build.sh` ruft `mkarchiso` über `pkexec` auf. Du kannst `sudo bash build.sh` auch selbst ausführen, etwa nachdem du das Profil von Hand angepasst hast.

Mehr dazu in [docs/architektur.md](docs/architektur.md).

## Dokumentation

- [Architektur und erzeugte Dateien](docs/architektur.md)
- [Der grafische Installer](docs/installer.md): Partitionierung, Verschlüsselung, Offline-Installation
- [KI und Container](docs/ki-und-container.md): Ollama, Modelle, Docker/Podman, Compose-Stacks
- [Projektdatei (JSON)](docs/projektdatei.md): alle Einstellungen
- [Fehlerbehebung](docs/fehlerbehebung.md)
- [Änderungen](CHANGELOG.md)

## Entwicklung

```bash
make check                     # = python3 -m unittest discover -s tests -v
ruff check --select E,F,W,B --ignore E741,E501 .
```

Die Profil-Tests nutzen die releng-Vorlage aus `/usr/share/archiso/configs/releng`. Einen anderen Pfad
setzt du mit `DISTRO_BUILDER_RELENG`. Die GitHub-CI führt Lint und Tests in einem `archlinux`-Container aus.

```
distro-builder              Startskript
distrobuilder/
  ui.py                     GTK4/libadwaita-Oberfläche
  config.py                 Standardwerte, Presets, Paketlisten, Validierung
  profile.py                Erzeugt das archiso-Profil
  fetch.py                  Lädt Ollama-Modelle und Container-Images
  templates/                Dateien für die ISO (Installer, Backend, Branding, Live-Setup)
tests/                      unittest-Tests
docs/                       Dokumentation
```

Fehler und Ideen gerne als [Issue](https://github.com/bonbon08/distro-builder/issues) melden.

## Hinweise

- Distro Builder ist ein unabhängiges Projekt und steht in keiner Verbindung zu Arch Linux. Bevor du eine Distribution veröffentlichst, beachte die [Markenrichtlinien von Arch Linux](https://terms.archlinux.org/docs/trademark-policy/).
- AUR-Pakete lassen sich nicht direkt einbinden. Baue sie selbst und stelle sie über ein eigenes Repository bereit (siehe [Fehlerbehebung](docs/fehlerbehebung.md#aur-pakete)).

## Lizenz

[MIT](LICENSE)
