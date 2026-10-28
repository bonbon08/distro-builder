#!/bin/bash
# Richtet den Live-Benutzer von @@NAME@@ ein (Name, Passwort und sudo legt Distro Builder fest)
U='@@LIVEUSER@@'
HASH='@@LIVEHASH@@'
SUDO='@@SUDO@@'
AUTOLOGIN='@@AUTOLOGIN@@'

GROUPS_="audio,video,network,storage"
[ "$SUDO" != none ] && GROUPS_="wheel,$GROUPS_"
id "$U" &>/dev/null || useradd -m -G "$GROUPS_" -s /bin/bash -c "@@NAME@@ Live" "$U"

if [ -n "$HASH" ]; then
  usermod -p "$HASH" "$U"
  # Mit Passwort soll auch root nicht ohne Passwort erreichbar sein
  passwd -l root >/dev/null
else
  passwd -d "$U" >/dev/null
fi

install -d -m 750 /etc/sudoers.d
case "$SUDO" in
  nopasswd) echo "$U ALL=(ALL:ALL) NOPASSWD: ALL" > /etc/sudoers.d/00-live ;;
  password) echo "$U ALL=(ALL:ALL) ALL" > /etc/sudoers.d/00-live ;;
  *) rm -f /etc/sudoers.d/00-live ;;
esac
[ -f /etc/sudoers.d/00-live ] && chmod 440 /etc/sudoers.d/00-live

if [ "$AUTOLOGIN" = 1 ]; then
  groupadd -rf autologin
  gpasswd -a "$U" autologin >/dev/null
fi

HOME_DIR=$(getent passwd "$U" | cut -d: -f6)
if [ -f /usr/share/applications/distro-installer.desktop ]; then
  install -d -o "$U" -g "$U" "$HOME_DIR/Desktop"
  install -m 755 -o "$U" -g "$U" /usr/share/applications/distro-installer.desktop "$HOME_DIR/Desktop/"
fi
if [ "@@FASTFETCH@@" = 1 ] && ! grep -q fastfetch "$HOME_DIR/.bashrc" 2>/dev/null; then
  printf '\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n' >> "$HOME_DIR/.bashrc"
  chown "$U:$U" "$HOME_DIR/.bashrc"
fi
exit 0
