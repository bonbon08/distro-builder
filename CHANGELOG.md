# Änderungen

## Unreleased

- Zielplattform wählbar: x86_64 (ISO mit Installer), ARM64 mit UEFI und Raspberry Pi 4/400/5 (fertiges Disk-Image auf Basis von Arch Linux ARM, gebaut per QEMU-Emulation)
- Flatpak: Flathub und eigene Quellen, Apps beim Bauen mitliefern oder beim ersten Start laden, Discover/GNOME Software
- Distrobox: Container mit Ubuntu, Debian, Fedora, openSUSE, Alpine oder eigenen Images, eingerichtet beim ersten Anmelden
- Container-Images werden passend zur Plattform geladen (amd64/arm64)
- Seite „ISO bauen“ heißt jetzt „Bauen“
- „Nur Live-System“: ISO ohne Installer, z. B. für Rettungs-, Kiosk- oder Gaming-Sticks
- Live-Benutzer frei wählbar: Name, Passwort (nur als SHA-512-crypt-Hash gespeichert), sudo mit/ohne Passwort oder gar nicht, Autologin für Desktop und Konsole, `copytoram`
- Flatpak-Apps funktionieren im reinen Live-System (direkt im SquashFS)
- Seite „Installer“ heißt jetzt „Live & Installer“
- Eigener Spiegelserver: eingebauter Caching-Proxy (HTTP mit Range-Unterstützung, Datenbanken max. 5 Minuten gecacht, Auslieferung aus dem Cache bei Ausfall des Upstreams) oder vollständiger rsync-Spiegel; die Distro trägt ihn vor den offiziellen Mirrors ein
- Eigenes Paket-Repository per `repo-add`, ausgeliefert vom Spiegel und in der Distro als `[<id>]` eingebunden
- `distro-builder --mirror-server [projekt.json]` betreibt den Spiegel ohne Oberfläche; „Server-Paket exportieren“ erzeugt nginx-Konfiguration, `compose.yaml`, Sync-Skript und systemd-Timer
- Vor dem Build wird geprüft, ob ein zwingend benötigter Spiegel erreichbar ist

- Komplett in Rust neu geschrieben (vorher Python/PyGObject): Cargo-Workspace mit vier Crates
  (`distro-builder-core` als gemeinsame Bibliothek für Konfiguration/Profil-Erzeugung/Downloads,
  `distro-builder` als GTK4/libadwaita-Oberfläche, `distro-installer` und `distro-installer-backend`
  als grafischer Installer samt Root-Backend im Live-System). Funktionsumfang unverändert.
- Build/Install laufen jetzt über `cargo build --workspace` statt `python3`/PyGObject; `PKGBUILD` und
  CI entsprechend angepasst.

## 0.1.0 – 2026-10-27

Erste Veröffentlichung.

- GTK4/libadwaita-Oberfläche: Allgemein, Branding (mit fastfetch-Vorschau), Repositories, Pakete, Container & KI, Installer, ISO bauen
- archiso-Profil auf Basis von releng: Branding, Bootmenü, Live-Desktop mit Autologin, NetworkManager
- Grafischer Installer: Offline-Installation von der ISO oder online per pacstrap, mehrere Festplatten (`/home`, Datenlaufwerke, btrfs-RAID 1), LUKS2-Verschlüsselung, ext4/btrfs, Swap-Datei, GRUB/systemd-boot
- Ollama mit vorinstallierten Modellen (eigener Registry-Download mit SHA256-Prüfung), KI-Entwicklungsumgebung
- Docker/Podman mit vorgeladenen Images und Compose-Stacks als systemd-Dienste
- Grafiktreiber-Auswahl (Mesa, AMD, Intel, NVIDIA)
- Tests (unittest) und GitHub-CI im Arch-Container
