#!/bin/bash
# Richtet @@NAME@@ im ARM-Image ein. Läuft per arch-chroot (auf x86_64 unter QEMU-Emulation).
# ROOT_UUID, BOOT_UUID und ROOT_PARTUUID setzt build.sh.
set -euo pipefail
@@VARS@@

echo "==> Paketschlüssel werden eingerichtet"
pacman-key --init
pacman-key --populate archlinuxarm

PHONE=0; [ "$PLATFORM" = cmf-phone1 ] && PHONE=1

# Kernel der Plattform tauschen, falls ein anderer gewählt ist (Kernel schließen sich gegenseitig aus).
# Das Telefon startet seinen Kernel von boot_b, im Image liegt keiner.
if [ "$PHONE" = 0 ]; then
  for k in linux-aarch64 linux-rpi linux-rpi-16k uboot-raspberrypi; do
    if [ "$k" != "$KERNEL" ] && pacman -Q "$k" &>/dev/null; then
      echo "Entferne $k"
      pacman -Rdd --noconfirm "$k"
    fi
  done
fi

echo "==> Pakete werden installiert"
pacman -Syu --noconfirm --needed - < /root/packages.txt

if [ -d /root/nothing-de-src ]; then
  echo "==> Baue nothing-de aus den Quellen (unter Emulation dauert das)"
  pacman -S --noconfirm --needed --asdeps base-devel git meson ninja wayland-protocols
  B=/var/tmp/nothing-de-build
  rm -rf "$B"
  useradd -r -M -d "$B" -s /bin/bash nothingbuild
  mv /root/nothing-de-src "$B"
  chown -R nothingbuild: "$B"
  runuser -u nothingbuild -- env HOME="$B" bash -c 'cd "$HOME" && makepkg -f --noconfirm --nocheck'
  pacman -U --noconfirm "$B"/nothing-de-[0-9]*.pkg.tar.*
  userdel nothingbuild
  rm -rf "$B"
  ORPHANS=$(pacman -Qdtq || true)
  [ -n "$ORPHANS" ] && pacman -Rns --noconfirm $ORPHANS
fi

echo "==> System wird konfiguriert"
for loc in "$LOCALE" en_US.UTF-8; do
  sed -i "s|^#\s*\($loc UTF-8\)|\1|" /etc/locale.gen
  grep -q "^$loc UTF-8" /etc/locale.gen || echo "$loc UTF-8" >> /etc/locale.gen
done
locale-gen
ln -sf "/usr/share/zoneinfo/$TIMEZONE" /etc/localtime
chmod 440 /etc/sudoers.d/10-wheel

if [ "$FASTFETCH" = 1 ] && ! grep -q fastfetch /etc/skel/.bashrc 2>/dev/null; then
  printf '\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n' >> /etc/skel/.bashrc
fi
GRP=""
for g in wheel audio video storage network lp input docker render ollama; do
  getent group "$g" >/dev/null && GRP="$GRP,$g"
done
if [ "$PHONE" = 1 ]; then
  # Die Dienste von cmf-phone1-arch setzen den Benutzer alarm (UID 1000) voraus
  echo "==> Benutzer $USERNAME wird eingerichtet"
  usermod -a -G "${GRP#,}" -c "$NAME" "$USERNAME"
  if [ "$FASTFETCH" = 1 ] && ! grep -q fastfetch "/home/$USERNAME/.bashrc" 2>/dev/null; then
    printf '\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n' >> "/home/$USERNAME/.bashrc"
    chown 1000:1000 "/home/$USERNAME/.bashrc"
  fi
else
  echo "==> Benutzer $USERNAME wird angelegt"
  userdel -r alarm 2>/dev/null || true
  useradd -m -G "${GRP#,}" -s /bin/bash -c "$NAME" "$USERNAME"
fi
printf '%s:%s\n' "$USERNAME" "$PASSWORD" | chpasswd
if [ "$AUTOLOGIN" = 1 ]; then
  groupadd -rf autologin
  gpasswd -a "$USERNAME" autologin >/dev/null
elif [ "$EXPIRE" = 1 ]; then
  chage -d 0 "$USERNAME"
fi
passwd -l root >/dev/null

echo "==> Dienste werden aktiviert"
# NetworkManager übernimmt das Netzwerk (das Telefon braucht systemd-networkd für USB)
[ "$PHONE" = 0 ] && { systemctl disable systemd-networkd.service systemd-networkd.socket 2>/dev/null || true; }
for svc in "${SERVICES[@]}"; do
  systemctl enable "$svc" || echo "WARNUNG: Dienst $svc konnte nicht aktiviert werden"
done
[ -x "/usr/local/bin/$GID-apply-branding" ] && "/usr/local/bin/$GID-apply-branding"

if [ "$PHONE" = 0 ]; then
cat > /etc/fstab <<EOF
# Erzeugt von Distro Builder
UUID=$ROOT_UUID  /      ext4  rw,relatime  0 1
UUID=$BOOT_UUID  /boot  vfat  rw,relatime,fmask=0022,dmask=0022,errors=remount-ro  0 2
EOF

echo "==> Bootloader wird eingerichtet"
if [ "$PLATFORM" = rpi ]; then
  echo "root=PARTUUID=$ROOT_PARTUUID rw rootwait console=serial0,115200 console=tty1 fsck.repair=yes $KPARAMS" \
    > /boot/cmdline.txt
  [ -f /boot/config.txt ] || printf '[all]\narm_64bit=1\nenable_uart=1\n' > /boot/config.txt
  if [ "$DESKTOP" = 1 ] && ! grep -q '^dtoverlay=vc4-kms-v3d' /boot/config.txt; then
    printf '\n[all]\ndtoverlay=vc4-kms-v3d\n' >> /boot/config.txt
  fi
else
  grub-install --target=arm64-efi --efi-directory=/boot --boot-directory=/boot --removable --no-nvram
  KIMG=$(cd /boot && ls Image Image.gz vmlinuz-* 2>/dev/null | head -n1)
  INITRD=$(cd /boot && ls initramfs-*.img 2>/dev/null | grep -v fallback | head -n1)
  [ -n "$KIMG" ] || { echo "FEHLER: Kein Kernel in /boot gefunden" >&2; exit 1; }
  cat > /boot/grub/grub.cfg <<EOF
# Erzeugt von Distro Builder
set timeout=3
menuentry "$NAME" {
  search --no-floppy --fs-uuid --set=root $BOOT_UUID
  linux /$KIMG root=UUID=$ROOT_UUID rw $KPARAMS
  initrd /$INITRD
}
EOF
fi
fi

echo "==> Aufräumen"
rm -rf /var/cache/pacman/pkg/*
# Jedes Gerät bekommt eigene Schlüssel und eine eigene machine-id
gpgconf --homedir /etc/pacman.d/gnupg --kill all 2>/dev/null || true
rm -rf /etc/pacman.d/gnupg /etc/ssh/ssh_host_*
: > /etc/machine-id
exit 0
