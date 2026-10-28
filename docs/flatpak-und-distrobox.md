# Flatpak und Distrobox

Die Seite **Flatpak & Distrobox** ergänzt die Pakete aus den Arch-Repositories um zwei weitere
Quellen: Apps aus Flatpak-Quellen wie Flathub und ganze andere Distributionen als Container.

## Flatpak

| Einstellung | Wirkung |
|---|---|
| Flatpak einrichten | Paket `flatpak`, Dienst `<id>-flatpak-setup.service` |
| Flathub als Quelle | Quelle `flathub` (`https://dl.flathub.org/repo/flathub.flatpakrepo`) |
| App-Store mit Flatpak-Unterstützung | KDE/LXQt: `discover`, sonst `gnome-software` |
| Apps beim Bauen mitliefern | die Apps werden auf dem Build-Rechner geladen und landen im Abbild – sie funktionieren dann ohne Internet |

Apps wählst du aus den Vorlagen oder trägst die App-ID ein, wie sie auf
[flathub.org](https://flathub.org) steht, z. B. `org.inkscape.Inkscape`.

### Eigene Quellen

Unter „Eigene Flatpak-Quellen“ gibst du einen Namen und die Adresse einer `.flatpakrepo`-Datei an,
etwa `kde` → `https://distribute.kde.org/kdeapps.flatpakrepo`. Apps daraus trägst du als
`quelle:app.id` ein, z. B. `kde:org.kde.kate`.

### Ablauf

1. **Beim Bauen** (nur mit „Apps beim Bauen mitliefern“ und installiertem `flatpak`) installiert `build.sh` die Apps in eine eigene Flatpak-Installation unter `~/DistroBuilder/<id>/cache/flatpak-<arch>`. Deine eigene Flatpak-Installation bleibt unberührt.
   - **ISO:** Die Installation wird als `/var/lib/<id>/flatpak.tar` ins Abbild gepackt (mkarchiso würde sonst die Dateirechte verlieren).
   - **ARM-Image:** Sie wird direkt nach `/var/lib/flatpak` kopiert.
2. **Beim ersten Start** des installierten Systems entpackt `<id>-flatpak-setup.service` das Archiv, richtet die Quellen ein und installiert fehlende Apps aus dem Internet. Erst wenn alles da ist, schreibt der Dienst `/var/lib/<id>/flatpak-setup.done`; sonst versucht er es beim nächsten Start erneut.

Schlägt das Vorladen fehl (kein `flatpak` auf dem Build-Rechner, kein Netz), bricht der Build nicht
ab. Die Apps werden dann beim ersten Start geladen.

Im Live-System einer ISO mit Installer sind die Flatpaks noch nicht eingerichtet, erst nach der
Installation. Bei „Nur Live-System“ sind sie direkt im Live-System nutzbar (siehe [Live-System](live-system.md)).

```bash
flatpak list
systemctl status <id>-flatpak-setup
```

## Distrobox

[Distrobox](https://distrobox.it) startet andere Distributionen als Container, die dein
Home-Verzeichnis, Wayland/X11, Audio und USB mitbenutzen. So laufen etwa Ubuntu- oder
Fedora-Programme auf deiner Distribution.

| Einstellung | Wirkung |
|---|---|
| Distrobox installieren | Pakete `distrobox` und – wenn keine Container-Engine gewählt ist – `podman` mit `podman.socket`. Mit Docker als Engine nutzt Distrobox Docker (`/etc/distrobox/distrobox.conf`). |
| Container | Name, Image, zusätzliche Pakete im Container und Apps, die ins Startmenü übernommen werden |

Vorlagen: Ubuntu 24.04, Debian 12, Fedora 42 (Toolbox-Image), openSUSE Tumbleweed, Alpine und
Arch Linux (nur x86_64).

### Ablauf

- Die Container stehen in `/etc/<id>/distrobox.ini` (Format von `distrobox assemble`).
- Der Benutzerdienst `/etc/systemd/user/<id>-distrobox.service` ist für alle Benutzer aktiviert. Er legt die Container **beim ersten Anmelden** mit `distrobox assemble create` an und merkt sich das in `~/.local/state/<id>/distrobox.done`.
- Distrobox-Container gehören dem jeweiligen Benutzer (rootless Podman). Deshalb werden sie nicht in die ISO gepackt; das Anlegen braucht Internet. Klappt es nicht, versucht es der Dienst bei der nächsten Anmeldung erneut.
- Im Live-System der ISO werden keine Container angelegt.

```bash
distrobox list
distrobox enter ubuntu
systemctl --user status <id>-distrobox
```
