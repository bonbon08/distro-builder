# Architektur

Distro Builder besteht aus zwei Teilen:

1. **Builder** (läuft auf deinem Rechner): GTK-Oberfläche, Profil-Generator und Downloader
2. **Live-System und Installer** (laufen in der ISO): Dateien aus `distrobuilder/templates/`

```
 Oberfläche (ui.py)
      │  Projekt-JSON (config.py)
      ▼
 profile.generate() ──► ~/DistroBuilder/<id>/profile/   (archiso-Profil)
      │
 fetch.prepare_assets() ──► cache/ ──► profile/airootfs/…  (Modelle, Images)
      │
 pkexec bash build.sh ──► mkarchiso ──► out/<id>-<version>-<datum>-x86_64.iso
```

## Projektordner

```
~/DistroBuilder/<id>/
  project.json     Kopie der Einstellungen beim letzten Erzeugen
  build.sh         ruft mkarchiso auf (auch manuell nutzbar: sudo bash build.sh)
  profile/         erzeugtes archiso-Profil
  cache/ollama/    heruntergeladene Modelle (Ollama-Format)
  cache/images/    Container-Images als docker-archive (.tar)
  work/            Arbeitsordner von mkarchiso (wird nach dem Build gelöscht)
  out/             fertige ISO-Dateien
```

`profile/` wird bei jedem Erzeugen komplett neu aufgebaut. Eigene Änderungen am Profil gehen also
verloren, wenn du danach erneut auf „Profil erzeugen“ oder „ISO bauen“ klickst. Der `cache/` bleibt
erhalten; Modelle und Images werden nur einmal geladen.

## Das erzeugte Profil

Grundlage ist `/usr/share/archiso/configs/releng`, das Profil des offiziellen Arch-ISOs. Daran
ändert `profile.py` Folgendes:

| Datei | Änderung |
|---|---|
| `profiledef.sh` | `iso_name`, `iso_label`, `iso_publisher`, `iso_application`, `iso_version`, Kompression (zstd oder xz), zusätzliche `file_permissions` |
| `packages.x86_64` | releng-Pakete (oder eine schlanke Liste) **plus alle Pakete des Zielsystems** plus Installer-Abhängigkeiten |
| `pacman.conf` | aus den Repository-Einstellungen erzeugt; dieselbe Datei landet als `/etc/pacman.conf` im System |
| `efiboot/`, `grub/`, `syslinux/` | „Arch Linux“ wird durch deinen Namen ersetzt, `syslinux/splash.png` durch das Hintergrundbild (640×480) |
| `airootfs/` | siehe unten |

Das Live-System enthält alle Pakete des Zielsystems. Nur so kann der Installer offline arbeiten:
Er kopiert das Live-Abbild und entfernt danach die Pakete, die nur im Live-System gebraucht werden.

### airootfs: Branding (Live-System und installiertes System)

Diese Dateien landen zweimal im Profil: direkt in `airootfs/` und als Kopie in
`airootfs/usr/share/<id>/overlay/`. Die Kopie braucht der Installer für die Online-Installation.

| Pfad | Zweck |
|---|---|
| `/usr/share/<id>/os-release` | Vorlage für `/usr/lib/os-release` |
| `/usr/local/bin/<id>-apply-branding` | schreibt `os-release`, `lsb-release` und `GRUB_DISTRIBUTOR` |
| `/etc/pacman.d/hooks/90-<id>-branding.hook` | ruft das Skript nach Updates von `filesystem`, `grub` und `lsb-release` erneut auf |
| `/usr/share/<id>/logo.png`, `/usr/share/pixmaps/<id>.png` | Logo, auch als `LOGO=<id>` in os-release |
| `/usr/share/<id>/logo.txt` | ASCII-Logo |
| `/etc/xdg/fastfetch/config.jsonc` | systemweite fastfetch-Konfiguration |
| `/usr/local/bin/neofetch` | Alias: ruft fastfetch auf |
| `/usr/local/bin/fastfetch` | nur im Modus „Bild“: echte Pixel in Kitty/Konsole/WezTerm/Ghostty, sonst chafa |
| `/usr/share/<id>/wallpaper.*`, `/usr/local/bin/<id>-set-wallpaper`, `/etc/xdg/autostart/<id>-wallpaper.desktop` | setzt das Hintergrundbild einmalig pro Benutzer (KDE, GNOME, Xfce, Cinnamon, LXQt) |
| `/etc/motd` | Begrüßungstext |
| `/etc/systemd/zram-generator.conf` | zram-Swap |
| `/etc/systemd/system/ollama.service.d/10-<id>.conf` | Ollama-Modellpfad (siehe [KI und Container](ki-und-container.md)) |
| `/etc/<id>/compose/…`, `<id>-compose@.service`, `<id>-container-import.service` | Container |

### airootfs: nur im Live-System

| Pfad | Zweck |
|---|---|
| `/usr/local/bin/live-setup` + `live-setup.service` | legt beim Booten den Benutzer `live` an (ohne Passwort, sudo ohne Passwort) |
| `/etc/systemd/system/display-manager.service` | Link auf sddm, gdm oder lightdm |
| `/etc/sddm.conf.d/autologin.conf` bzw. gdm/lightdm | automatische Anmeldung als `live` |
| `/usr/local/lib/distro-installer/{installer,backend}.py` | der Installer |
| `/etc/distro-installer.json` | Konfiguration für den Installer, siehe unten |
| `/usr/share/applications/distro-installer.desktop` (+ Autostart) | Starter |
| `/etc/vconsole.conf`, `/etc/X11/xorg.conf.d/00-keyboard.conf` | Tastaturlayout im Live-System |

Mit Desktop wird `systemd-networkd`/`iwd` aus releng durch NetworkManager ersetzt.

### /etc/distro-installer.json

Das Bindeglied zwischen Builder und Installer:

```json
{
  "name": "NebulaOS", "id": "nebulaos", "hostname": "nebula", "logo": "/usr/share/nebulaos/logo.png",
  "packages": ["base", "linux", "…"],
  "services": ["NetworkManager.service", "sddm.service", "ollama.service", "…"],
  "kernel_params": ["zswap.enabled=0"],
  "data_paths": ["/var/lib/ollama/models"],
  "live_files": ["/etc/mkinitcpio.conf.d/archiso.conf", "…"],
  "live_dirs": ["/etc/systemd/system/getty@tty1.service.d", "…"],
  "defaults": {"source": "offline", "encrypt": false, "bootloader": "grub", "filesystem": "ext4", "…": "…"}
}
```

- `packages`: Pakete des Zielsystems. Offline werden sie als „explizit installiert“ markiert, alles andere wird als verwaist entfernt.
- `live_files` / `live_dirs`: werden nach dem Offline-Kopieren gelöscht. Berechnet aus allen Dateien des releng-`airootfs` (außer `passwd`, `shadow`, `resolv.conf`, timesyncd/resolved u. a.) plus den Live-Dateien von Distro Builder.
- `data_paths`: werden bei der Online-Installation von der ISO ins Zielsystem kopiert.

## Rechte

`mkarchiso` kopiert `airootfs` mit `--no-preserve=ownership,mode`. Alle ausführbaren Dateien werden
deshalb über `file_permissions+=(…)` in `profiledef.sh` auf `0:0:755` gesetzt.

## Wer läuft mit welchen Rechten

| Schritt | Benutzer |
|---|---|
| Oberfläche, Profil erzeugen, Downloads | du |
| `build.sh` / `mkarchiso` | root über `pkexec`; `out/` gehört danach wieder dir |
| Installer-Oberfläche | `live` |
| Installer-Backend | root über `sudo -n` (der Benutzer `live` darf das ohne Passwort) |
