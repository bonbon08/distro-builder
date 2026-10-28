# Änderungen

## Unveröffentlicht

- Zielplattform wählbar: x86_64 (ISO mit Installer), ARM64 mit UEFI und Raspberry Pi 4/400/5 (fertiges Disk-Image auf Basis von Arch Linux ARM, gebaut per QEMU-Emulation)
- Flatpak: Flathub und eigene Quellen, Apps beim Bauen mitliefern oder beim ersten Start laden, Discover/GNOME Software
- Distrobox: Container mit Ubuntu, Debian, Fedora, openSUSE, Alpine oder eigenen Images, eingerichtet beim ersten Anmelden
- Container-Images werden passend zur Plattform geladen (amd64/arm64)
- Seite „ISO bauen“ heißt jetzt „Bauen“

## 0.1.0 – 2026-10-27

Erste Veröffentlichung.

- GTK4/libadwaita-Oberfläche: Allgemein, Branding (mit fastfetch-Vorschau), Repositories, Pakete, Container & KI, Installer, ISO bauen
- archiso-Profil auf Basis von releng: Branding, Bootmenü, Live-Desktop mit Autologin, NetworkManager
- Grafischer Installer: Offline-Installation von der ISO oder online per pacstrap, mehrere Festplatten (`/home`, Datenlaufwerke, btrfs-RAID 1), LUKS2-Verschlüsselung, ext4/btrfs, Swap-Datei, GRUB/systemd-boot
- Ollama mit vorinstallierten Modellen (eigener Registry-Download mit SHA256-Prüfung), KI-Entwicklungsumgebung
- Docker/Podman mit vorgeladenen Images und Compose-Stacks als systemd-Dienste
- Grafiktreiber-Auswahl (Mesa, AMD, Intel, NVIDIA)
- Tests (unittest) und GitHub-CI im Arch-Container
