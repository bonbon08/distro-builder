# Änderungen

## 0.1.0 – 2026-10-27

Erste Veröffentlichung.

- GTK4/libadwaita-Oberfläche: Allgemein, Branding (mit fastfetch-Vorschau), Repositories, Pakete, Container & KI, Installer, ISO bauen
- archiso-Profil auf Basis von releng: Branding, Bootmenü, Live-Desktop mit Autologin, NetworkManager
- Grafischer Installer: Offline-Installation von der ISO oder online per pacstrap, mehrere Festplatten (`/home`, Datenlaufwerke, btrfs-RAID 1), LUKS2-Verschlüsselung, ext4/btrfs, Swap-Datei, GRUB/systemd-boot
- Ollama mit vorinstallierten Modellen (eigener Registry-Download mit SHA256-Prüfung), KI-Entwicklungsumgebung
- Docker/Podman mit vorgeladenen Images und Compose-Stacks als systemd-Dienste
- Grafiktreiber-Auswahl (Mesa, AMD, Intel, NVIDIA)
- Tests (unittest) und GitHub-CI im Arch-Container
