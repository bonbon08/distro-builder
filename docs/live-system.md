# Live-System

Jede x86_64-ISO startet in ein Live-System. Auf der Seite **Live & Installer** legst du fest,
ob es dabei bleibt oder ob zusätzlich der grafische Installer mitkommt.

| Medium | Wofür |
|---|---|
| **Live-System mit grafischem Installer** (Standard) | ausprobieren und dann installieren |
| **Nur Live-System** | Systeme, die direkt vom Stick laufen sollen: Rettungs-, Kiosk-, Präsentations-, Schul- oder Gaming-ISO |

Mit „Nur Live-System“ fehlen der Installer und seine Abhängigkeiten. Alles andere (Branding,
Pakete, Desktop, KI-Modelle, Flatpaks) ist im Live-System enthalten.

## Live-Benutzer

| Einstellung | Standard | Wirkung |
|---|---|---|
| Benutzername | `live` | Konto, mit dem das Live-System startet |
| Passwort | keins | wird nur als SHA-512-Hash (`$6$…`) gespeichert, siehe unten |
| Administratorrechte | sudo ohne Passwort | `sudo ohne Passwort` · `sudo mit Passwort` · `kein sudo` |
| Automatisch anmelden | an | Desktop bzw. Konsole startet ohne Anmeldemaske |
| Komplett in den RAM laden | aus | Bootparameter `copytoram` |

- Mit Installer ist immer „sudo ohne Passwort“ eingestellt: Der Installer startet sein Root-Backend mit `sudo -n`.
- Mit Passwort wird zusätzlich das root-Konto gesperrt. Sonst wäre root im Live-System ohne Passwort erreichbar.
- Die automatische Anmeldung gilt für den Desktop (sddm, gdm oder lightdm) und für die Konsole auf tty1. Ohne Desktop meldet sich die Konsole also als dein Live-Benutzer an und nicht mehr als root wie beim Arch-ISO; root bekommst du über `sudo -i`.
- Ist die automatische Anmeldung aus und kein Passwort gesetzt, meldet man sich mit dem Benutzernamen ohne Passwort an.

### Wie das Passwort gespeichert wird

Das Passwort gibst du in das Feld „Neues Passwort setzen“ ein und bestätigst mit dem Haken.
Distro Builder berechnet daraus sofort einen SHA-512-crypt-Hash mit zufälligem Salt, im
selben Format wie `/etc/shadow`, und speichert nur diesen:

- in der Projektdatei als `live.password_hash`,
- im Live-System im Skript `/usr/local/bin/live-setup`, das den Benutzer beim Booten mit `usermod -p` anlegt.

Der Klartext steht nirgends: weder im Projekt noch in der ISO. Zum Ändern setzt du einfach ein
neues Passwort, „Entfernen“ macht das Konto wieder passwortlos. Einen Hash kannst du auch selbst
erzeugen und in die Projektdatei eintragen, z. B. mit `openssl passwd -6`.

> Ein Hash in einer ISO ist trotzdem öffentlich, sobald du die ISO weitergibst: Wer sie hat, kann
> ihn offline angreifen. Für weitergegebene ISOs deshalb ein starkes Passwort verwenden.

## Komplett in den RAM laden (copytoram)

Mit dieser Option hängt Distro Builder `copytoram` an alle Live-Booteinträge an (UEFI,
GRUB und Syslinux; PXE-Einträge bleiben unverändert). Beim Start wird das komplette System-Abbild
in den Arbeitsspeicher kopiert:

- Der Start dauert länger, danach läuft alles aus dem RAM und ist spürbar schneller.
- Der USB-Stick kann nach dem Start entfernt werden.
- Du brauchst mehr RAM als das System-Abbild groß ist (Größe von `arch/x86_64/airootfs.sfs` in der ISO plus Arbeitsspeicher für das System).

## Was im reinen Live-System funktioniert

| Funktion | Im reinen Live-System |
|---|---|
| Branding, Desktop, Pakete | ja |
| Ollama und vorinstallierte Modelle | ja (`ollama run …`) |
| Flatpak-Apps | ja. Mitgelieferte Apps werden beim Bauen direkt ins SquashFS übernommen, nicht beim Booten in den RAM entpackt |
| Docker/Podman | ja, aber nur das Programm selbst |
| Vorgeladene Container-Images, Container-Apps (Compose), Distrobox-Container | nein, die werden erst nach einer Installation eingerichtet |

Änderungen im Live-System (neue Dateien, installierte Pakete) liegen im RAM und sind nach dem
Neustart weg.

## Technische Details

- `/usr/local/bin/live-setup` und `live-setup.service` legen den Benutzer beim Booten an, vor dem Display-Manager und vor dem Konsolen-Login.
- `/etc/sudoers.d/00-live` enthält die sudo-Regel (oder fehlt bei „kein sudo“).
- `/etc/systemd/system/getty@tty1.service.d/autologin.conf` meldet die Konsole automatisch an. Ohne Autologin wird die Datei aus dem releng-Profil entfernt.
- Bei „Nur Live-System“ mit mitgelieferten Flatpaks entpackt der einmalige pacman-Hook `zz-<id>-flatpak-live.hook` das Flatpak-Archiv während `mkarchiso` im chroot. Er ist mit `# remove from airootfs!` markiert und wird danach vom releng-Hook wieder entfernt.
- Beim Installieren von einer ISO mit Installer werden alle diese Live-Dateien wieder entfernt; der Live-Benutzer existiert im installierten System nicht.
