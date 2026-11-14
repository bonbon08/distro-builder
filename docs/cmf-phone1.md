# CMF Phone 1

Unter **Allgemein → Zielplattform → CMF Phone 1 (Nothing)** baut Distro Builder deine Distribution
für das CMF Phone 1 (Codename Tetris, MediaTek Dimensity 7300). Die Hardware-Anpassung stammt aus
[cmf-phone1-arch](https://github.com/bonbon08/cmf-phone1-arch): Nothings Android-Kernel 6.1, eine
kleine Initramfs, ein Halium-Container für GPU, Modem und Kamera sowie eigene Dienste. Distro Builder
bringt Name, Branding, Pakete, Flatpaks, Container und Region dazu.

> Das Flashen löscht alle Daten auf dem Telefon. Der Bootloader muss entsperrt sein. Nothing OS
> bleibt auf Slot a erhalten, deine Distribution läuft auf Slot b. Lies vorher die Abschnitte
> „Status“ und „Risks“ im README von cmf-phone1-arch.

## Oberflächen

Die Seite **Pakete** zeigt für das Telefon eigene Oberflächen. Jede entspricht einem Profil von
cmf-phone1-arch:

| Oberfläche | Profil | Was entsteht |
|---|---|---|
| Nothing-Shell (Telefon-Layout) | `phone` | [Nothing_Wayland](https://github.com/bonbon08/Nothing_Wayland) im Telefon-Layout. Die zweite Stufe baut und aktiviert es. |
| Phosh | `phone` | Phosh, wie bei cmf-phone1-arch ohne die Nothing-Shell |
| GNOME über RDP | `desktop` | GNOME als Headless-Sitzung, Zugriff per RDP (Port 3389) |
| Server ohne Oberfläche | `server` | Docker, Ollama als Container, SSH, `<hostname>.local` |

## Was du selbst besorgen musst

Diese Dateien liegen nicht im Repository, weil sie proprietär oder zu groß sind. Lege sie in den
cmf-phone1-arch-Checkout:

| Pfad im Checkout | Inhalt |
|---|---|
| `firmware/Tetris_B4.1-260928-1140/images/` | fastboot-Images von Nothing OS **B4.1** |
| `toolchain/clang-r487747c/` | Android-Clang **r487747c** |
| `downloads/alarm/usr/bin/busybox` | statisches busybox aus dem Paket `busybox` von Arch Linux ARM |
| Halium-`system.img` (beliebiger Ort) | LuneOS halium_arm64, wird erst in der zweiten Stufe gebraucht |

Unter **Allgemein → CMF Phone 1** trägst du den Pfad zu deinem Checkout ein. Bleibt das Feld leer,
klont der Build das Repository nach `<projekt>/cache/cmf-phone1-arch`. Dann gehören die Dateien dort
hinein. Das Basissystem `ArchLinuxARM-aarch64-latest.tar.gz` lädt der Build selbst.

Auf dem Build-Rechner (Arch Linux, x86_64) brauchst du außerdem:

```bash
sudo pacman -S --needed arch-install-scripts qemu-user-static qemu-user-static-binfmt \
    android-tools erofs-utils lz4 cpio git base-devel bc
```

Auf der Seite **Bauen** installiert das der Button „Installieren“.

## Ablauf von build.sh

1. cmf-phone1-arch holen bzw. aktualisieren und prüfen, ob die Dateien von oben vorhanden sind.
2. `scripts/fetch-sources.sh`: Nothings Kernel-Quellen und die Patches.
3. `scripts/build-kernel.sh` und `scripts/build-debug-boot.sh`. Sie laufen nur, wenn sich `kernel/`,
   `patches/`, `mainline/` oder `initramfs/` seit dem letzten Bau geändert haben. Mit
   `REBUILD_KERNEL=1 sudo bash build.sh` erzwingst du den Neubau.
4. `scripts/build-rootfs.sh` mit dem passenden Profil (`PROFILE=phone|server|desktop`).
5. Das Rootfs wird auf die eingestellte Image-Größe vergrößert. Overlay, `pacman.conf`, Paketliste und
   Flatpaks von Distro Builder kommen hinein, `setup.sh` richtet im chroot Pakete, Sprache, Zeitzone,
   Benutzer und Dienste ein. Danach wird das Image wieder auf die minimale Größe geschrumpft.
6. Ergebnis in `<projekt>/out/`:

```
meinos-1.0-cmf-phone1-2026.11.14.simg          userdata (Sparse-Image)
meinos-1.0-cmf-phone1-2026.11.14-flash/
  boot_b.img  init_boot_b.img  vendor_boot_b.img
  flash.sh                                     alles flashen, Slot b aktivieren, neu starten
  zweite-stufe.sh                              zweite Stufe auf dem Telefon
```

Kernel, Initramfs und die Vendor-Module kommen aus cmf-phone1-arch. Kernel und Bootloader-Pakete von
Arch Linux ARM landen deshalb nicht im Image.

## Flashen und zweite Stufe

```bash
# Telefon im fastboot-Modus (Ein + Leiser). Einmalig vorher, falls nötig:
# cmf-phone1-arch/scripts/flash-stock.sh (Stock auf beiden Slots, AVB aus)
out/meinos-1.0-cmf-phone1-…-flash/flash.sh
```

Nach dem ersten Start bleibt der Bildschirm schwarz. Das ist so gewollt: GPU-wlroots, libhybris und
das Modem-Plugin müssen noch auf dem Telefon gebaut werden. Das Telefon ist per USB unter
`172.16.42.1` erreichbar. Gib dem Rechner am USB-Netz die Adresse `172.16.42.2/24`, teile das
Internet (`sudo cmf-phone1-arch/scripts/host-nat.sh up`) und starte die zweite Stufe:

```bash
out/meinos-1.0-cmf-phone1-…-flash/zweite-stufe.sh /pfad/zu/halium/system.img
```

`zweite-stufe.sh` ruft `scripts/setup-phone.sh` auf. Für Phosh ist `PHOSH=1` gesetzt, für die
Nothing-Shell wird `NOTHING_SRC` an den eingestellten Nothing_Wayland-Checkout übergeben. Beim
GNOME-Profil wartet das Skript nach dem Neustart auf das Telefon und führt danach
`scripts/setup-desktop.sh` aus. Die Datei enthält das Passwort für sudo auf dem Telefon und ist
deshalb nur für dich lesbar (`chmod 700`).

## Einstellungen und Grenzen

| Einstellung | Auf dem Telefon |
|---|---|
| Benutzer | immer `alarm` (UID 1000), weil die Dienste von cmf-phone1-arch ihn voraussetzen. SSH ist an. |
| Startpasswort | wird für `alarm` gesetzt und an die zweite Stufe weitergegeben. Es darf kein `'` enthalten und läuft nicht ab. |
| Automatisch anmelden | gibt es nicht. Die Sitzung startet cmf-phone1-arch selbst. |
| Image-Größe | Platz während des Baus (mindestens 8 GB). Das fertige Image wird geschrumpft und wächst auf dem Telefon über `tetris-growfs` auf die ganze userdata-Partition. |
| Grafiktreiber | nur Mesa. Die GPU (Mali-G615) nutzt cmf-phone1-arch über libhybris. |
| Kernel | `tetris-6.1`, wird aus Nothings Quellen gebaut |
| Hostname | ersetzt `tetris`, das Telefon heißt im LAN dann `<hostname>.local` |

Arch Linux ARM hat kein Ollama-Paket. Im Server-Profil läuft Ollama bei cmf-phone1-arch deshalb als
Container (`tetris-ai.service`).
