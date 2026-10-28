#!/bin/bash
# Erzeugt von Distro Builder – baut das Disk-Image für @@ARCH_LABEL@@.
# Kann auch manuell mit 'sudo bash build.sh' ausgeführt werden.
set -euo pipefail
BASE=@@BASE@@
PROFILE=@@PROFILE@@
WORK=@@WORK@@
OUT=@@OUT@@
CACHE="$BASE/cache"
URL=@@URL@@
IMG_NAME=@@IMG_PREFIX@@"$(date +%Y.%m.%d)"
SIZE_GB=@@SIZE_GB@@
MNT="$WORK/mnt"
LOOP=""

for t in arch-chroot:arch-install-scripts bsdtar:libarchive curl:curl sfdisk:util-linux losetup:util-linux \
         mkfs.vfat:dosfstools mkfs.ext4:e2fsprogs; do
  if ! command -v "${t%%:*}" >/dev/null; then
    echo "FEHLER: ${t%%:*} fehlt. Bitte 'pacman -S ${t#*:}' installieren." >&2
    exit 1
  fi
done
if [ "$(uname -m)" != aarch64 ] && [ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ]; then
  systemctl restart systemd-binfmt.service 2>/dev/null || true
  if [ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ]; then
    echo "FEHLER: ARM-Emulation fehlt. Bitte 'pacman -S qemu-user-static qemu-user-static-binfmt' installieren." >&2
    exit 1
  fi
fi

cleanup() {
  set +e
  mountpoint -q "$MNT" && umount -R "$MNT"
  mountpoint -q "$WORK/esp" && umount "$WORK/esp"
  [ -n "$LOOP" ] && losetup -d "$LOOP"
  rm -rf -- "$WORK"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
rm -rf -- "$WORK"
mkdir -p -- "$MNT" "$WORK/esp" "$OUT" "$CACHE"

echo "==> Lade das Arch-Linux-ARM-Basissystem"
TAR="$CACHE/$(basename "$URL")"
curl -fsSL --retry 3 -o "$TAR.md5" "$URL.md5"
if ! (cd "$CACHE" && md5sum --status -c "$(basename "$TAR").md5"); then
  curl -fL --retry 3 -o "$TAR" "$URL"
  (cd "$CACHE" && md5sum -c "$(basename "$TAR").md5")
fi
if command -v gpg >/dev/null; then
  export GNUPGHOME="$WORK/gnupg"
  mkdir -m 700 -- "$GNUPGHOME"
  if gpg --batch --quiet --keyserver hkps://keyserver.ubuntu.com --recv-keys @@ALARM_KEY@@ 2>/dev/null \
     && curl -fsSL --retry 3 -o "$TAR.sig" "$URL.sig"; then
    gpg --batch --quiet --verify "$TAR.sig" "$TAR" || { echo "FEHLER: Signatur des Basissystems ist ungültig" >&2; exit 1; }
    echo "Signatur geprüft ✔"
  else
    echo "WARNUNG: Signatur konnte nicht geprüft werden (Schlüsselserver nicht erreichbar) – nur MD5 geprüft."
  fi
  unset GNUPGHOME
fi

echo "==> Erzeuge Image ($SIZE_GB GB)"
IMG="$WORK/$IMG_NAME.img"
truncate -s "${SIZE_GB}G" "$IMG"
printf @@LAYOUT@@ | sfdisk -q "$IMG"
LOOP=$(losetup -fP --show "$IMG")
udevadm settle 2>/dev/null || true
mkfs.vfat -F 32 -n BOOT "${LOOP}p1" >/dev/null
mkfs.ext4 -q -F -L @@LABEL@@ "${LOOP}p2"
mount "${LOOP}p2" "$MNT"

echo "==> Entpacke das Basissystem"
bsdtar -xpf "$TAR" -C "$MNT"
# /boot liegt auf FAT32 (keine Symlinks/Rechte) – Inhalt hinüberkopieren
mount "${LOOP}p1" "$WORK/esp"
cp -rL "$MNT/boot/." "$WORK/esp/"
umount "$WORK/esp"
find "$MNT/boot" -mindepth 1 -delete
mount "${LOOP}p1" "$MNT/boot"

echo "==> Übernehme Branding und Einstellungen"
tar -C "$PROFILE/airootfs" --owner=0 --group=0 -cf - . | tar -C "$MNT" -xpf -
install -m644 "$PROFILE/pacman.conf" "$MNT/etc/pacman.conf"
install -m600 "$PROFILE/setup.sh" "$MNT/root/setup.sh"
install -m644 "$PROFILE/packages.txt" "$MNT/root/packages.txt"
@@FLATPAK@@
echo "==> System wird im Image eingerichtet (unter Emulation dauert das)"
arch-chroot "$MNT" /usr/bin/env \
  ROOT_UUID="$(blkid -s UUID -o value "${LOOP}p2")" \
  BOOT_UUID="$(blkid -s UUID -o value "${LOOP}p1")" \
  ROOT_PARTUUID="$(blkid -s PARTUUID -o value "${LOOP}p2")" \
  /bin/bash /root/setup.sh
rm -f -- "$MNT/root/setup.sh" "$MNT/root/packages.txt"
sync
umount -R "$MNT"
losetup -d "$LOOP"
LOOP=""

echo "==> Komprimiere Image"
if command -v @@TOOL@@ >/dev/null; then
  @@COMPRESS@@
else
  mv -- "$IMG" "$OUT/$IMG_NAME.img"
fi
chown -R @@UID@@:@@GID@@ -- "$OUT"
echo "Fertig: $(ls -1t "$OUT" | head -n1)"
echo "##DONE##"
