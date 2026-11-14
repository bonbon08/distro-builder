# Nothing-Desktop

**Pakete → Desktop-Umgebung → Nothing** installiert [Nothing_Wayland](https://github.com/bonbon08/Nothing_Wayland).
Das ist eine Wayland-Desktopumgebung im Stil von Nothing OS mit eigenem Compositor (`nothing-wm`,
wlroots 0.20), Leiste, Dock, Widgets und einer eigenen Einstellungs-App. Angemeldet wird über SDDM mit
der Sitzung `nothing`. Als Terminal kommt GNOME Console mit, als Dateimanager Nautilus.

Das Paket `nothing-de` liegt in keinem Repository. Distro Builder baut es deshalb beim Bauen aus den
Quellen. Unter dem Desktop-Feld kannst du einen lokalen Checkout von Nothing_Wayland angeben. Bleibt
das Feld leer, wird der aktuelle Stand von GitHub geklont.

## ISO (x86_64)

`build.sh` baut `nothing-de` vor `mkarchiso` auf diesem Rechner:

1. Quellen nach `<projekt>/cache/nothing-de-src` kopieren bzw. klonen.
2. Die Bauabhängigkeiten aus dem `PKGBUILD` (`depends`, `makedepends`, dazu `base-devel`) per
   `pacman -S --needed --asdeps` **auf diesem Rechner** installieren.
3. `makepkg` als dein Benutzer, nicht als root.
4. Das Paket nach `<projekt>/cache/localrepo` legen und mit `repo-add` als Repo `[<id>-local]`
   eintragen.

Nur die `pacman.conf` für mkarchiso kennt `[<id>-local]`. Im Live-System und im installierten System
fehlt der Eintrag, weil es den Pfad dort nicht gibt. `nothing-de` erscheint dort als fremdes Paket
(`pacman -Qm`). Updates bekommt es, wenn du eine neue Version deiner Distro baust.

Der Installer installiert mit dem Nothing-Desktop nur von der ISO (offline). Online per `pacstrap`
ginge das nicht, weil es `nothing-de` auf keinem Mirror gibt.

## ARM64 und Raspberry Pi

Hier wird `nothing-de` im Image gebaut, also für aarch64 unter QEMU-Emulation. `image-build.sh` legt
die Quellen nach `/root/nothing-de-src`. `setup.sh` installiert die Bauwerkzeuge, baut das Paket mit
einem vorübergehenden Benutzer, installiert es und entfernt die Bauwerkzeuge danach wieder.

## CMF Phone 1

Auf dem Telefon nutzt die Nothing-Shell das GPU-wlroots von cmf-phone1-arch. Dieses entsteht erst in
der zweiten Stufe auf dem Telefon, deshalb baut `scripts/setup-nothing.sh` dort auch `nothing-de`.
Siehe [CMF Phone 1](cmf-phone1.md).
