# Der grafische Installer

Der Installer ist eine GTK4/libadwaita-App in der ISO. Er wird eingebunden, wenn unter
**Installer** „Installer einbinden“ aktiv ist **und** ein Desktop gewählt wurde. Ohne Desktop
steht im Live-System nur `archinstall` aus dem Arch-ISO bereit.

> ⚠️ Der Installer **löscht alle Festplatten vollständig**, die du als Systemplatte oder als
> weitere Platte auswählst. Vorhandene Partitionen lassen sich nicht weiterverwenden, und eine
> manuelle Partitionierung gibt es nicht.

## Ablauf

| Seite | Inhalt |
|---|---|
| Willkommen | Logo, Willkommenstext, Prüfung der Internetverbindung |
| Region | Sprache (alle UTF-8-Locales), Zeitzone, Tastaturlayout mit Testfeld |
| Festplatten | Systemplatte und Verwendung der übrigen Platten |
| Optionen | Dateisystem, Swap-Datei, Verschlüsselung, Bootloader, Installationsquelle |
| Benutzer | Name, Benutzername, Computername, Passwort, root-Passwort, automatische Anmeldung |
| Zusammenfassung | alles auf einen Blick, Bestätigungsdialog |
| Installation | Fortschritt und aufklappbares Protokoll |
| Fertig | Neustart (oder Fehlermeldung mit Protokoll) |

Angezeigt werden Festplatten ab 8 GB. Das Bootmedium selbst, schreibgeschützte Geräte, Loop-,
zram- und optische Laufwerke werden ausgeblendet.

## Partitionierung

Alle Platten bekommen GPT.

**Systemplatte, UEFI**

| Nr. | Größe | Typ | Inhalt |
|---|---|---|---|
| 1 | 1 GB | EFI (`ef00`) | FAT32, eingehängt als `/boot` (Kernel und Initramfs liegen hier) |
| 2 | Rest | Linux root (`8304`) bzw. LUKS (`8309`) | System |

**Systemplatte, BIOS**

| Nr. | Größe | Typ | Inhalt |
|---|---|---|---|
| 1 | 1 MB | BIOS boot (`ef02`) | Platz für GRUB |
| 2 | 1 GB | Linux (`8300`) | ext4, `/boot` (unverschlüsselt, damit GRUB lesen kann) |
| 3 | Rest | Linux root bzw. LUKS | System |

**Weitere Festplatten**: je eine Partition über die ganze Platte.

| Verwendung | Ergebnis |
|---|---|
| `/home (Benutzerdaten)` | eigenes Dateisystem für `/home` (höchstens eine Platte) |
| `Datenlaufwerk (/data)` | `/data`, `/data2`, `/data3` … |
| `btrfs-Spiegel (RAID 1)` | wird zusammen mit der Systempartition zu einem btrfs-Dateisystem mit `-d raid1 -m raid1` (nur mit btrfs) |

## Dateisysteme

**ext4**: eine Partition für `/`.

**btrfs** mit zstd-Kompression und `noatime`:

| Subvolume | Einhängepunkt |
|---|---|
| `@` | `/` |
| `@home` | `/home` (entfällt, wenn eine eigene `/home`-Platte gewählt ist) |
| `@log` | `/var/log` |
| `@pkg` | `/var/cache/pacman/pkg` |
| `@snapshots` | `/.snapshots` |
| `@swap` | `/swap` (nur mit Swap-Datei) |

Mit btrfs wird zusätzlich `btrfs-scrub@-.timer` aktiviert.

**Swap-Datei** (0–128 GB): ext4 → `/swapfile`, btrfs → `/swap/swapfile`. Auf btrfs-RAID ist keine
Swap-Datei möglich. Unabhängig davon gibt es zram, wenn es im Builder aktiviert ist; dann wird
zusätzlich `zswap.enabled=0` gesetzt.

## Verschlüsselung (LUKS2)

- Die Systempartition (und gegebenenfalls der RAID-Spiegel) wird mit LUKS2 und deiner Passphrase verschlüsselt.
- Entsperrt wird in der Initramfs mit `sd-encrypt` über `rd.luks.name=<UUID>=cryptroot` (`cryptroot2` usw. für Spiegel). systemd fragt **einmal** nach dem Passwort und probiert es für alle Geräte.
- `/home` und Datenlaufwerke bekommen eine zufällige 512-Byte-Schlüsseldatei unter `/etc/cryptsetup-keys.d/<name>.key` auf dem verschlüsselten System und einen Eintrag in `/etc/crypttab`. Zusätzlich wird die Passphrase als zweiter Schlüssel eingetragen, damit du die Platte notfalls auch von Hand öffnen kannst.
- `/boot` bleibt unverschlüsselt: die EFI-Partition bzw. unter BIOS eine eigene ext4-Partition.
- Das Passwort ist entweder das Benutzerpasswort (mindestens 8 Zeichen) oder ein eigenes.

## Installationsquellen

### Offline von der ISO (Standard)

1. `airootfs.sfs` vom Bootmedium (oder aus `copytoram`) wird mit `unsquashfs` ins Zielsystem entpackt.
2. Live-Dateien aus `live_files` und `live_dirs` werden gelöscht: Autologin, Installer, archiso-Hooks, releng-Dienste und Ähnliches.
3. Der pacman-Schlüsselbund wird neu erzeugt (`pacman-key --init`, `--populate`).
4. Alle Pakete werden als Abhängigkeit markiert, die Zielpakete (Gruppen aufgelöst) als explizit. Danach werden verwaiste Pakete mit `pacman -Rns` entfernt. So verschwinden `mkinitcpio-archiso`, die Rettungswerkzeuge, die Installer-Abhängigkeiten usw.
5. `mkarchiso` leert `/boot` im Abbild. Deshalb wird `vmlinuz-<pkgbase>` aus `/usr/lib/modules/*/` wiederhergestellt.

Kein Internet nötig. KI-Modelle, Container-Images und Pakete aus eigenen Repos sind sofort da.
Die pacman-Sync-Datenbanken fehlen nach der Installation und werden beim ersten `pacman -Syu` geladen.

### Online (pacstrap)

`pacstrap` installiert die Zielpakete mit der `pacman.conf` der ISO aus dem Internet und bekommt
damit die neuesten Versionen. Branding-Overlay und `data_paths` (Modelle, Images) werden von der
ISO übernommen. Diese Quelle lässt sich im Builder ausblenden („Online-Installation zur Auswahl anbieten“).

## Konfiguration des Zielsystems

- `genfstab -U`, Swap-Eintrag, `crypttab`
- `pacman.conf` und Mirrorliste der ISO
- neue `machine-id`
- Locale (plus `en_US.UTF-8`), Zeitzone, Hardware-Uhr, Konsolen- und X11-Tastatur
- Hostname und `/etc/hosts`
- Benutzer in den Gruppen `wheel`, `audio`, `video`, `storage`, `network`, `lp` sowie, falls vorhanden, `docker`, `render`, `ollama`; sudo für `wheel`
- root bekommt entweder dasselbe Passwort oder wird gesperrt
- optional automatische Anmeldung (sddm, gdm oder lightdm)
- alle Dienste aus `services` (Display-Manager, NetworkManager, Docker, Ollama, Container-Stacks …)
- Branding-Skript

**Initramfs**: `/etc/mkinitcpio.conf.d/20-<id>.conf` mit
`HOOKS=(base systemd autodetect microcode modconf kms keyboard sd-vconsole block [sd-encrypt] filesystems fsck)`,
danach `mkinitcpio -P`.

**Bootloader**

- **GRUB**: UEFI mit `--bootloader-id=<Name>` plus Fallback `--removable`, BIOS auf die Systemplatte. Kernel-Parameter kommen in `GRUB_CMDLINE_LINUX`.
- **systemd-boot** (nur UEFI): `bootctl install`, je Kernel ein normaler und ein Notfall-Eintrag, `systemd-boot-update.service`.

## Backend direkt aufrufen

Die Oberfläche schreibt deine Auswahl in eine temporäre JSON-Datei (mit Rechten `0600`, wird
danach gelöscht) und startet:

```bash
sudo /usr/local/lib/distro-installer/distro-installer-backend auswahl.json
```

Beispiel für `auswahl.json`:

```json
{
  "disk": "/dev/nvme0n1",
  "extra_disks": [{"path": "/dev/sda", "role": "home"}],
  "filesystem": "btrfs", "swap_gb": 8,
  "encrypt": true, "luks_password": "…",
  "bootloader": "systemd-boot", "source": "offline",
  "locale": "de_DE.UTF-8", "timezone": "Europe/Berlin", "keyboard": "de",
  "fullname": "Max Muster", "username": "max", "hostname": "pc", "password": "…",
  "root_same": false, "autologin": false
}
```

Das Backend gibt Logzeilen aus. Fortschritt meldet es als `##PROGRESS <0..1> <Text>`, Fehler als
`##ERROR## <Text>`. Bei einem Fehler hängt es alles aus und schließt geöffnete LUKS-Container.
