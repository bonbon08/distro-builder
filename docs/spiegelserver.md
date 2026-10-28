# Eigener Spiegelserver

Auf der Seite **Repositories** richtest du einen Spiegelserver für deine Distro ein. Die Distro
trägt ihn in ihre `pacman.conf` ein: beim ISO-Build, im Live-System und in jedem installierten
System. pacman fragt dann zuerst deinen Spiegel und nur, wenn der nicht erreichbar ist, die
offiziellen Mirrors.

## Zwei Arten

| Art | Speicherbedarf | Wofür |
|---|---|---|
| **Caching-Proxy** (Standard) | nur was gebraucht wird, meist wenige GB | mehrere Rechner im LAN, wiederholte ISO-Builds, langsames Internet |
| **Vollständiger Spiegel** (rsync) | alle genutzten Repos, für core + extra 80 GB und mehr | komplett offline arbeiten, eigener öffentlicher Mirror |

Der **Caching-Proxy** lädt jedes Paket beim ersten Abruf vom offiziellen Mirror und hält es
danach vor. Ab dem zweiten Rechner (oder dem zweiten ISO-Build) kommt alles aus dem LAN.
Repo-Datenbanken (`core.db` usw.) werden höchstens 5 Minuten zwischengespeichert, damit Updates
sofort ankommen. Ist der Upstream nicht erreichbar, liefert er weiter aus dem Cache.

Der **vollständige Spiegel** synchronisiert per rsync genau die Repos, die deine Distro nutzt
(z. B. `core`, `extra`, `multilib`). Das geht nur für x86_64, da Arch Linux ARM keine
öffentlichen rsync-Quellen anbietet. Für ARM gibt es deshalb den Caching-Proxy.

## Einrichten

1. **Repositories → Eigener Spiegelserver → „Distro nutzt den eigenen Spiegel“** einschalten.
2. **Adresse** eintragen. „Dieser Rechner“ setzt die LAN-Adresse dieses PCs mit dem Port ein, z. B. `http://192.168.1.10:8080`.
3. **Server → Starten.** Der eingebaute Server läuft, solange Distro Builder offen ist. „Letzte Anfrage“ zeigt, was gerade ausgeliefert wird, und der Status, wie viel schon gespeichert ist.
4. ISO bauen. `mkarchiso` nutzt den Spiegel bereits, der zweite Build ist also deutlich schneller.

> Damit andere Rechner den Spiegel erreichen, muss der Port in der Firewall offen sein, z. B.
> `sudo ufw allow 8080/tcp`. `localhost` als Adresse funktioniert nur auf diesem Rechner selbst;
> installierte Systeme erreichen ihn so nicht.

### Erzeugte pacman.conf

```ini
[meinos]                       # nur mit eigenem Repository
SigLevel = Optional TrustAll
Server = http://192.168.1.10:8080/$repo/os/$arch

[core]
Server = http://192.168.1.10:8080/$repo/os/$arch
Include = /etc/pacman.d/mirrorlist   # Ausweich, abschaltbar

[extra]
Server = http://192.168.1.10:8080/$repo/os/$arch
Include = /etc/pacman.d/mirrorlist
```

Ohne „Offizielle Mirrors als Ausweich“ fehlt die `Include`-Zeile. Dann braucht jeder Build
und jedes Update deinen Spiegel.

## Eigenes Paket-Repository

Unter **Eigenes Paket-Repository** fügst du mit **+** fertige Pakete (`.pkg.tar.zst`) hinzu,
zum Beispiel selbst gebaute AUR-Pakete:

```bash
git clone https://aur.archlinux.org/yay-bin.git && cd yay-bin && makepkg
# dann yay-bin-*.pkg.tar.zst in Distro Builder hinzufügen
```

Distro Builder kopiert die Pakete nach `~/DistroBuilder/<id>/mirror/repo/<id>/os/<arch>/` und
trägt sie mit `repo-add --new --remove` ein. Ältere Versionen desselben Pakets fliegen dabei
raus. Der Spiegel liefert das Repo als `[<id>]` aus, und die Distro bindet es **vor** den
offiziellen Repos ein. So kannst du eigene Pakete unter `Pakete → Eigene Pakete` eintragen,
und sie landen in der ISO und in jedem installierten System, inklusive Updates.

Das eigene Repository gibt es nur auf deinem Spiegel (ohne Ausweichquelle). Beim ISO-Build muss
der Spiegel deshalb erreichbar sein. Die Seite „Bauen“ prüft das vorher und bietet an, den
eingebauten Server zu starten. Dasselbe gilt, wenn „Offizielle Mirrors als Ausweich“ aus ist.

Pakete werden nicht signiert, deshalb `SigLevel = Optional TrustAll`. Liegt neben einem Paket
eine `.sig`-Datei, wird sie mitkopiert.

## Dauerhaft betreiben

Der Server in der Oberfläche ist zum Ausprobieren und für einen PC, der sowieso läuft. Für einen
Server oder Raspberry Pi im Dauerbetrieb gibt es zwei Wege.

### 1. Ohne Oberfläche

```bash
distro-builder --mirror-server ~/DistroBuilder/meinos/project.json
```

Startet denselben eingebauten Server ohne GTK-Fenster und protokolliert jede Anfrage im
Terminal. Als systemd-Dienst:

```ini
# /etc/systemd/system/meinos-mirror.service
[Unit]
Description=MeinOS Spiegelserver
After=network-online.target
Wants=network-online.target

[Service]
User=mirror
ExecStart=/usr/bin/distro-builder --mirror-server /srv/meinos/project.json
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Der Cache liegt dabei unter `<build.project_dir>/<id>/mirror/` aus der Projektdatei.

### 2. Server-Paket (nginx/Docker)

**Server → Server-Paket exportieren** schreibt nach `~/DistroBuilder/<id>/mirror/server-kit/`:

| Datei | Inhalt |
|---|---|
| `README.md` | Anleitung für genau dein Projekt |
| `nginx.conf` | Caching-Proxy (Pakete 90 Tage gecacht, Datenbanken nie) bzw. statischer Spiegel, plus eigenes Repo |
| `compose.yaml` | `nginx:alpine` mit dieser Konfiguration: `docker compose up -d` |
| `sync-mirror.sh` | nur vollständiger Spiegel: rsync der genutzten Repos, mit Lock gegen Doppelläufe |
| `<id>-mirror-sync.service` / `.timer` | nur vollständiger Spiegel: stündliche Synchronisation |
| `repo/` | Kopie deines eigenen Repositorys |

Den Ordner auf den Server kopieren und `docker compose up -d` ausführen. Für ARM bildet die
nginx-Konfiguration das andere Pfadschema von Arch Linux ARM (`$arch/$repo`) per `rewrite` ab.

## Technische Details

- Der eingebaute Server spricht HTTP/1.1 mit GET und HEAD und unterstützt `Range: bytes=N-`, damit pacman abgebrochene Downloads fortsetzen kann. Er liefert nur Pfade der Form `/<repo>/os/<arch>/<datei>` aus; `..` und andere Sonderzeichen werden abgewiesen.
- Upstream für den Proxy: das Feld „Quelle des Proxys“, sonst der eigene Mirror unter „Paketquelle“, sonst `https://geo.mirror.pkgbuild.com/$repo/os/$arch` (x86_64) bzw. `http://mirror.archlinuxarm.org/$arch/$repo` (ARM).
- Downloads landen erst in einer `.part`-Datei und werden dann umbenannt. Ein abgebrochener Download hinterlässt also keine kaputten Pakete im Cache.
- Die Paketintegrität prüft pacman wie immer selbst (Prüfsummen und Signaturen aus der Datenbank), der Spiegel ändert nichts an den Dateien.

| Pfad | Inhalt |
|---|---|
| `~/DistroBuilder/<id>/mirror/cache/` | Cache des Proxys (Layout wie ein Arch-Mirror) |
| `~/DistroBuilder/<id>/mirror/archlinux/` | vollständiger Spiegel |
| `~/DistroBuilder/<id>/mirror/repo/` | eigenes Repository |
| `~/DistroBuilder/<id>/mirror/server-kit/` | exportiertes Server-Paket |

Den Cache darfst du jederzeit löschen, er füllt sich bei Bedarf neu.
