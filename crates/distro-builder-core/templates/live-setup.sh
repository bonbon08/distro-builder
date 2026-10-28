#!/bin/bash
# Richtet den Live-Benutzer "live" ein (ohne Passwort, sudo ohne Passwort)
id live &>/dev/null || useradd -m -G wheel,audio,video,network,storage -s /bin/bash -c "@@NAME@@ Live" live
passwd -d live >/dev/null
install -d -m 750 /etc/sudoers.d
echo 'live ALL=(ALL:ALL) NOPASSWD: ALL' > /etc/sudoers.d/00-live
chmod 440 /etc/sudoers.d/00-live
groupadd -rf autologin
gpasswd -a live autologin >/dev/null
HOME_DIR=/home/live
if [ -f /usr/share/applications/distro-installer.desktop ]; then
  for d in Desktop; do
    install -d -o live -g live "$HOME_DIR/$d"
    install -m 755 -o live -g live /usr/share/applications/distro-installer.desktop "$HOME_DIR/$d/"
  done
fi
if [ "@@FASTFETCH@@" = 1 ] && ! grep -q fastfetch "$HOME_DIR/.bashrc" 2>/dev/null; then
  printf '\n# Systeminfo beim Start\n[[ $- == *i* ]] && command -v fastfetch >/dev/null && fastfetch\n' >> "$HOME_DIR/.bashrc"
  chown live:live "$HOME_DIR/.bashrc"
fi
exit 0
