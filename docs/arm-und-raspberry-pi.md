# ARM und Raspberry Pi

Unter **Allgemein → Zielplattform** wählst du, für welche Hardware gebaut wird:

| Plattform | Ergebnis | Basis |
|---|---|---|
| **x86_64** (Standard) | bootfähige ISO mit Live-System und grafischem Installer | Arch Linux, archiso |
| **ARM64 (UEFI)** | fertig installiertes Disk-Image (`.img.zst` bzw. `.img.xz`), GPT, GRUB | Arch Linux ARM, `ArchLinuxARM-aarch64-latest` |
| **Raspberry Pi 4/400/5** | fertig installiertes SD-Karten-Image, MBR, Raspberry-Pi-Firmware | Arch Linux ARM, `ArchLinuxARM-rpi-aarch64-latest` |

Für ARM gibt es keinen Installer: Das Image enthält ein komplett eingerichtetes System. Du
schreibst es auf eine SD-Karte, einen USB-Stick oder eine SSD und startest direkt.

```bash
# z. B. mit dem Raspberry Pi Imager („Eigenes Image“) oder:
zstd -dc meinos-1.0-rpi-2026.10.28.img.zst | sudo dd of=/dev/sdX bs=4M status=progress conv=fsync
```

Beim ersten Start vergrößert `<id>-growfs.service` die Root-Partition auf den ganzen Datenträger.

## Voraussetzungen auf dem Build-Rechner

Gebaut wird weiterhin auf Arch Linux (x86_64). Die ARM-Programme laufen dabei unter QEMU-Emulation:

```bash
sudo pacman -S --needed arch-install-scripts qemu-user-static qemu-user-static-binfmt
```

Auf der Seite **Bauen** erledigt das der Button „Installieren“. Auf einem ARM64-Rechner mit Arch
Linux ARM braucht es keine Emulation.

## Einstellungen

Für ARM zeigt die Seite **Installer** stattdessen die Einstellungen des fertigen Systems:

| Einstellung | Wirkung |
|---|---|
| Benutzername, Startpasswort | Benutzer mit sudo-Rechten (Gruppe `wheel`). `root` ist gesperrt. |
| Passwort beim ersten Anmelden ändern | `chage -d 0` – das Startpasswort muss sofort geändert werden (nicht bei automatischer Anmeldung) |
| Automatisch anmelden | Autologin über sddm, gdm oder lightdm |
| Image-Größe | Größe der Image-Datei. Sie muss System, KI-Modelle, Flatpaks und Container-Images fassen. Auf dem Datenträger wächst sie beim ersten Start. |
| Sprache, Zeitzone, Tastatur | werden direkt im System gesetzt |

Kernel:

| Plattform | Kernel |
|---|---|
| ARM64 | `linux-aarch64` |
| Raspberry Pi | `linux-rpi` (Pi 4/400/5) oder `linux-rpi-16k` (nur Pi 5, etwas schneller) |

Nicht verfügbar auf ARM (die Prüfung meldet es): NVIDIA-Treiber, CUDA/ROCm, multilib und Steam.
Testing-Repositories gibt es bei Arch Linux ARM nicht; die Einstellung wird ignoriert. Statt
`[multilib]` stehen `[alarm]` und `[aur]` in der `pacman.conf`.

> Nicht jedes Arch-Paket gibt es für ARM. „Prüfen“ auf der Seite **Pakete** fragt die
> x86_64-Repos deines Rechners ab und ist für ARM deshalb nur ein Anhaltspunkt. Ein unbekanntes
> Paket bricht den Build mit `target not found` ab.

## So entsteht das Image

`profile.generate()` erzeugt statt eines archiso-Profils:

```
~/DistroBuilder/<id>/
  build.sh               baut das Image (sudo bash build.sh)
  profile/airootfs/      Branding, Einstellungen, Dienste, Modelle, Container-Images
  profile/pacman.conf    für Arch Linux ARM (Architecture = aarch64)
  profile/packages.txt   zu installierende Pakete
  profile/setup.sh       Einrichtung im Image (läuft per arch-chroot)
  cache/ArchLinuxARM-…   Basissystem (wird nur bei Änderungen neu geladen)
  out/<id>-<version>-<plattform>-<datum>.img.zst
```

`build.sh`:

1. lädt das Arch-Linux-ARM-Basissystem und prüft MD5 und – wenn `gpg` und der Schlüsselserver erreichbar sind – die Signatur (Schlüssel `68B3537F39A313B3E574D06777193F152BDBE6A6`)
2. legt eine Image-Datei an: 512 MB FAT32 für `/boot` plus ext4 für `/`
3. entpackt das Basissystem und kopiert `profile/airootfs` hinein
4. lädt optional die Flatpak-Apps für aarch64 (siehe [Flatpak und Distrobox](flatpak-und-distrobox.md))
5. führt `setup.sh` per `arch-chroot` aus: Paketschlüssel, Kernel, Pakete, Locale, Benutzer, Dienste, `fstab`, Bootloader (`cmdline.txt`/`config.txt` beim Pi, GRUB für ARM64)
6. entfernt Paket-Cache, pacman-Schlüssel, SSH-Hostschlüssel und `machine-id` – jedes Gerät erzeugt beim ersten Start eigene
7. komprimiert das Image mit zstd oder xz (Einstellung „Kompression“)

Unter Emulation dauert das Installieren der Pakete deutlich länger als bei einer ISO, mit Desktop
oft 30–60 Minuten.

Container-Images werden für `linux/arm64` geladen (eigener Cache `cache/images-arm64`), KI-Modelle
sind plattformunabhängig.
