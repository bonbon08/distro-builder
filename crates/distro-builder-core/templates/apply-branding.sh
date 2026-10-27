#!/bin/sh
# @@NAME@@ Branding anwenden (wird auch per pacman-Hook nach Updates aufgerufen)
SRC=/usr/share/@@ID@@
[ -f "$SRC/os-release" ] && cp -f "$SRC/os-release" /usr/lib/os-release
if [ -f /etc/default/grub ]; then
  sed -i 's/^GRUB_DISTRIBUTOR=.*/GRUB_DISTRIBUTOR="@@NAME@@"/' /etc/default/grub
fi
if [ -f /etc/lsb-release ]; then
  printf 'DISTRIB_ID="@@NAME@@"\nDISTRIB_RELEASE="rolling"\nDISTRIB_DESCRIPTION="@@NAME@@"\n' > /etc/lsb-release
fi
exit 0
