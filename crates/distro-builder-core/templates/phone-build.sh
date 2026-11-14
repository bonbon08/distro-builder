#!/bin/bash
# Erzeugt von Distro Builder – baut @@NAME@@ für das CMF Phone 1 mit cmf-phone1-arch.
# Kann auch manuell mit 'sudo bash build.sh' ausgeführt werden.
#   REBUILD_KERNEL=1 sudo bash build.sh   baut Kernel und Boot-Images in jedem Fall neu
set -euo pipefail
BASE=@@BASE@@
PROFILE=@@PROFILE@@
WORK=@@WORK@@
OUT=@@OUT@@
CACHE="$BASE/cache"
REPO=@@REPO@@
REPO_URL=@@REPO_URL@@
URL=@@URL@@
PHONE_PROFILE=@@PHONE_PROFILE@@
SIZE_GB=@@SIZE_GB@@
IMG_NAME=@@IMG_PREFIX@@"$(date +%Y.%m.%d)"
FILES="$OUT/$IMG_NAME-flash"
MNT="$WORK/mnt"
BUILD_USER=$(getent passwd @@UID@@ | cut -d: -f1)
USER_HOME=$(getent passwd @@UID@@ | cut -d: -f6)
as_user() { setpriv --reuid=@@UID@@ --regid=@@GID@@ --init-groups env HOME="$USER_HOME" "$@"; }

for t in arch-chroot:arch-install-scripts bsdtar:libarchive curl:curl git:git setpriv:util-linux \
         img2simg:android-tools mkbootimg:android-tools fastboot:android-tools fsck.erofs:erofs-utils \
         lz4:lz4 cpio:cpio mkfs.ext4:e2fsprogs resize2fs:e2fsprogs make:base-devel; do
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
  rm -rf -- "$WORK"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
rm -rf -- "$WORK"
mkdir -p -- "$MNT" "$OUT" "$CACHE"
chown @@UID@@:@@GID@@ -- "$CACHE"

echo "==> Hole cmf-phone1-arch"
if [ -z "$REPO" ]; then
  REPO="$CACHE/cmf-phone1-arch"
  if [ -d "$REPO/.git" ]; then
    as_user git -C "$REPO" pull -q --ff-only || echo "WARNUNG: cmf-phone1-arch konnte nicht aktualisiert werden"
  else
    as_user git clone -q --depth 1 "$REPO_URL" "$REPO"
  fi
fi
echo "cmf-phone1-arch: $REPO"

# Dateien, die nicht im Repository liegen (proprietär oder zu groß)
FW="$REPO/firmware/Tetris_B4.1-260928-1140/images"
MISSING=()
[ -f "$FW/vendor_boot.img" ] || MISSING+=("Stock-Firmware Nothing OS B4.1 (fastboot-Images): $FW/")
[ -x "$REPO/toolchain/clang-r487747c/bin/clang" ] || MISSING+=("Android-Clang r487747c (AOSP prebuilts/clang/host/linux-x86): $REPO/toolchain/clang-r487747c/")
[ -x "$REPO/downloads/alarm/usr/bin/busybox" ] || MISSING+=("statisches busybox aus dem busybox-Paket von Arch Linux ARM: $REPO/downloads/alarm/usr/bin/busybox")
if [ ${#MISSING[@]} -gt 0 ]; then
  echo "FEHLER: Für das CMF Phone 1 fehlen Dateien, die du selbst besorgen musst:" >&2
  printf '  - %s\n' "${MISSING[@]}" >&2
  echo "Details: README von cmf-phone1-arch, Abschnitt „Inputs you have to get yourself“." >&2
  exit 1
fi

echo "==> Lade das Arch-Linux-ARM-Basissystem"
TAR="$REPO/downloads/alarm/$(basename "$URL")"
as_user curl -fsSL --retry 3 -o "$TAR.md5" "$URL.md5"
if ! (cd "$(dirname "$TAR")" && md5sum --status -c "$(basename "$TAR").md5"); then
  as_user curl -fL --retry 3 -o "$TAR" "$URL"
  (cd "$(dirname "$TAR")" && md5sum -c "$(basename "$TAR").md5")
fi

echo "==> Kernel-Quellen von Nothing"
as_user "$REPO/scripts/fetch-sources.sh"
KREL="$REPO/out/kernel/include/config/kernel.release"
VBOOT="$REPO/out/debug-vendor_boot.img"
newer() { [ -n "$(find "$@" -type f -newer "$REF" 2>/dev/null | head -n1)" ]; }
NEED_KERNEL=0
NEED_BOOT=0
REF=$KREL
if [ "${REBUILD_KERNEL:-0}" = 1 ] || [ ! -f "$KREL" ] || newer "$REPO/kernel" "$REPO/patches" "$REPO/mainline"; then
  NEED_KERNEL=1
fi
REF=$VBOOT
if [ "$NEED_KERNEL" = 1 ] || [ ! -f "$VBOOT" ] || newer "$REPO/initramfs" "$REPO/scripts/build-debug-boot.sh"; then
  NEED_BOOT=1
fi
if [ "$NEED_KERNEL" = 1 ]; then
  echo "==> Baue Kernel und Module (dauert lange)"
  as_user "$REPO/scripts/build-kernel.sh"
else
  echo "Kernel $(cat "$KREL") ist aktuell – wird nicht neu gebaut (REBUILD_KERNEL=1 erzwingt es)."
fi
if [ "$NEED_BOOT" = 1 ]; then
  echo "==> Baue Boot-Images"
  as_user "$REPO/scripts/build-debug-boot.sh"
fi

echo "==> Baue das Grundsystem (Profil $PHONE_PROFILE)"
SUDO_USER="$BUILD_USER" PROFILE="$PHONE_PROFILE" SIZE="${SIZE_GB}G" "$REPO/scripts/build-rootfs.sh"

echo "==> Übernehme Branding und Einstellungen"
IMG="$WORK/$IMG_NAME.img"
cp --sparse=always -- "$REPO/out/rootfs.img" "$IMG"
e2fsck -fy "$IMG" >/dev/null || true
resize2fs "$IMG" "${SIZE_GB}G" >/dev/null
mount -o loop "$IMG" "$MNT"
tar -C "$PROFILE/airootfs" --owner=0 --group=0 -cf - . | tar -C "$MNT" -xpf -
install -m644 "$PROFILE/pacman.conf" "$MNT/etc/pacman.conf"
install -m600 "$PROFILE/setup.sh" "$MNT/root/setup.sh"
install -m644 "$PROFILE/packages.txt" "$MNT/root/packages.txt"
@@FLATPAK@@
echo "==> System wird im Image eingerichtet (unter Emulation dauert das)"
arch-chroot "$MNT" /bin/bash /root/setup.sh
rm -f -- "$MNT/root/setup.sh" "$MNT/root/packages.txt"
sync
umount -R "$MNT"
e2fsck -fy "$IMG" >/dev/null || true
resize2fs -M "$IMG" >/dev/null

echo "==> Erzeuge Flash-Dateien"
mkdir -p -- "$FILES"
img2simg "$IMG" "$OUT/$IMG_NAME.simg"
install -m644 "$REPO/out/debug-boot.img" "$FILES/boot_b.img"
install -m644 "$REPO/out/debug-init_boot.img" "$FILES/init_boot_b.img"
install -m644 "$REPO/out/debug-vendor_boot.img" "$FILES/vendor_boot_b.img"
cat > "$FILES/flash.sh" <<EOF
#!/bin/bash
# Flasht @@NAME@@ auf das CMF Phone 1 (Telefon im fastboot-Modus: Ein + Leiser).
# ACHTUNG: löscht alle Android-Daten. Nothing OS auf Slot a bleibt erhalten;
# zurück mit: fastboot --set-active=a && fastboot reboot
# Der Bootloader muss entsperrt und AVB aus sein (cmf-phone1-arch: scripts/flash-stock.sh).
set -euo pipefail
D=\$(dirname "\$(realpath "\$0")")
read -r -p "Alle Daten auf dem Telefon werden gelöscht. Zum Fortfahren LOESCHEN eingeben: " a
[ "\$a" = LOESCHEN ] || exit 1
fastboot flash userdata "\$D/../$IMG_NAME.simg"
fastboot flash boot_b "\$D/boot_b.img"
fastboot flash init_boot_b "\$D/init_boot_b.img"
fastboot flash vendor_boot_b "\$D/vendor_boot_b.img"
fastboot --set-active=b
fastboot reboot
echo "Fertig. Danach die zweite Stufe: \$D/zweite-stufe.sh <halium-system.img>"
EOF
{
  cat <<'EOF'
#!/bin/bash
# Zweite Stufe nach dem ersten Start (Telefon per USB unter 172.16.42.1): baut libhybris, wlroots,
# das ModemManager-Plugin und die Oberfläche auf dem Telefon und installiert das Halium-Image.
#   zweite-stufe.sh <halium-system.img>   (LuneOS halium_arm64, entpacktes system.img)
# Der Rechner braucht die Adresse 172.16.42.2/24 am USB-Netz und teilt sein Internet:
#   sudo "$REPO/scripts/host-nat.sh" up
set -euo pipefail
EOF
  printf 'REPO=%q\n' "$REPO"
  cat <<'EOF'
@@STAGE2@@
EOF
} > "$FILES/zweite-stufe.sh"
chmod 755 "$FILES/flash.sh"
# enthält das Passwort für sudo auf dem Telefon
chmod 700 "$FILES/zweite-stufe.sh"
rm -f -- "$IMG"
chown -R @@UID@@:@@GID@@ -- "$OUT"
echo "Flash-Dateien: $FILES"
echo "Fertig: $IMG_NAME.simg"
echo "##DONE##"
