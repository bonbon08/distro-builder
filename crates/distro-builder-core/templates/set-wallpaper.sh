#!/bin/sh
# Setzt einmalig pro Benutzer das @@NAME@@ Hintergrundbild
W="@@WALLPAPER@@"
MARK="${XDG_CONFIG_HOME:-$HOME/.config}/@@ID@@/wallpaper-set"
[ -e "$MARK" ] && exit 0
[ -f "$W" ] || exit 0
sleep 4
ok=1
case "${XDG_CURRENT_DESKTOP:-}" in
  *KDE*)
    for i in 1 2 3 4 5; do plasma-apply-wallpaperimage "$W" && ok=0 && break; sleep 3; done ;;
  *GNOME*)
    gsettings set org.gnome.desktop.background picture-uri "file://$W" &&
    gsettings set org.gnome.desktop.background picture-uri-dark "file://$W" &&
    gsettings set org.gnome.desktop.screensaver picture-uri "file://$W" && ok=0 ;;
  *X-Cinnamon*|*Cinnamon*)
    gsettings set org.cinnamon.desktop.background picture-uri "file://$W" && ok=0 ;;
  *XFCE*)
    for p in $(xfconf-query -c xfce4-desktop -l 2>/dev/null | grep -E 'last-image$'); do
      xfconf-query -c xfce4-desktop -p "$p" -s "$W" && ok=0
    done
    if [ $ok -ne 0 ]; then
      xfconf-query -c xfce4-desktop -n -t string -p /backdrop/screen0/monitor0/workspace0/last-image -s "$W" && ok=0
    fi ;;
  *LXQt*)
    pcmanfm-qt --set-wallpaper "$W" && ok=0 ;;
esac
if [ $ok -eq 0 ]; then mkdir -p "$(dirname "$MARK")" && touch "$MARK"; fi
