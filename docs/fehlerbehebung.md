# Fehlerbehebung

## Beim Bauen

**„archiso-Vorlage nicht gefunden“ / „mkarchiso nicht gefunden“**
`archiso` installieren, entweder über den Button auf der Seite „ISO bauen“ oder mit `sudo pacman -S archiso`.

**Nach dem Passwort passiert nichts / „Abgebrochen (keine Berechtigung)“**
`pkexec` braucht einen laufenden Polkit-Agenten, den jeder gängige Desktop mitbringt. Alternativ
erst „Nur Profil erzeugen“ und dann im Terminal `sudo bash ~/DistroBuilder/<id>/build.sh` ausführen.

**`error: target not found: <paket>`**
Das Paket gibt es in den gewählten Repositories nicht. „Prüfen“ auf der Seite „Pakete“ findet
unbekannte Namen. Pakete aus eigenen Repos kennt dein lokales pacman nicht; die werden dort
immer als unbekannt gemeldet.

**Steam / 32-Bit-Pakete fehlen** → unter „Repositories“ **multilib** aktivieren.

**Signaturfehler bei eigenen Repos**
Für Repos ohne importierten Schlüssel `SigLevel = Optional TrustAll` wählen. Mit `Required` muss
der Schlüssel sowohl auf dem Build-Rechner (`pacman-key --recv-keys … && pacman-key --lsign-key …`)
als auch später im System bekannt sein.

**Zu wenig Speicher**
Ein Build braucht etwa 15–30 GB in `~/DistroBuilder/<id>/work`, plus Modelle und Images. Den
Projektordner kannst du auf eine andere Platte legen. Abgebrochene Builds hinterlassen `work/`;
der nächste Build räumt das auf, oder du löschst es selbst (`sudo rm -rf …/work`).

**Container-Images lassen sich nicht laden**
`skopeo` installieren (Button auf der Seite „ISO bauen“). Bei Docker als Fallback muss dein
Benutzer Docker verwenden dürfen. Private Registries: vorher `skopeo login` bzw. `docker login`.

**Modell nicht gefunden (404)**
Den genauen Namen auf ollama.com nachsehen, zum Beispiel `qwen3:8b` statt `qwen3-8b`. „Größe prüfen“ zeigt Fehler sofort an.

**Build dauert sehr lange**
xz-Kompression, NVIDIA-DKMS, CUDA/ROCm und große Modelle verlängern den Build deutlich. Für
Testbuilds zstd verwenden und die Arch-Rettungswerkzeuge abschalten.

## Im Live-System

**Kein grafischer Desktop**: Wurde unter „Pakete“ ein Desktop gewählt? Ohne Desktop startet das
System in die Konsole (root-Autologin wie beim Arch-ISO).

**Installer startet nicht**
Im Terminal `distro-installer` ausführen, dann siehst du die Fehlermeldung. Das Backend läuft per
`sudo -n`; der Benutzer `live` darf das ohne Passwort.

**NVIDIA: schwarzer Bildschirm**
Im Bootmenü mit `e` (systemd-boot) bzw. `Tab` (syslinux) den Parameter `nomodeset` anhängen.

## Installation

Das vollständige Protokoll zeigt „Details anzeigen“ auf der Fortschrittsseite.

**„Das System-Abbild der ISO (airootfs.sfs) wurde nicht gefunden“**
Tritt bei ungewöhnlichen Bootarten auf (PXE oder manche Multiboot-Sticks). Dann „Aus dem Internet“
als Quelle wählen.

**pacstrap ist fehlgeschlagen**: Keine Internetverbindung, oder ein Paket ist inzwischen nicht mehr
in den Repos. Die Offline-Quelle verwenden.

**Festplatte wird nicht angezeigt**: Es werden nur Platten ab 8 GB angezeigt; das Bootmedium ist ausgeblendet.

**Nach einem Abbruch neu installieren**: Den Installer einfach erneut starten. Er hängt vorher
alles aus und schließt offene LUKS-Container.

## Nach der Installation

**Bootet nicht (UEFI)**: Im Firmware-Bootmenü den Eintrag mit dem Distributionsnamen wählen. GRUB
wird zusätzlich als Fallback (`EFI/BOOT/BOOTX64.EFI`) installiert.

**`pacman -S` meldet fehlende Datenbanken**: Nach der Offline-Installation einmal `sudo pacman -Syu` ausführen.

**Ollama findet die Modelle nicht**: `systemctl cat ollama` sollte das Drop-in mit
`OLLAMA_MODELS=/var/lib/ollama/models` zeigen. Danach `sudo systemctl restart ollama`.

**Container-Apps laufen nicht**:
```bash
systemctl status <id>-container-import <id>-compose@<name>
journalctl -u <id>-compose@<name>
```

## AUR-Pakete

AUR-Pakete lassen sich nicht direkt eintragen. So geht es über ein eigenes Repository:

```bash
mkdir -p ~/myrepo && cd ~/aur-paket && makepkg -s
cp *.pkg.tar.zst ~/myrepo/
repo-add ~/myrepo/myrepo.db.tar.zst ~/myrepo/*.pkg.tar.zst
```

Unter „Repositories“ dann ein eigenes Repo `myrepo` mit `Server = file:///home/<du>/myrepo` und
`SigLevel = Optional TrustAll` hinzufügen und das Paket unter „Pakete“ eintragen. Damit das
installierte System Updates bekommt, legst du das Repo auf einen Webserver (zum Beispiel GitHub
Pages) und trägst dessen URL ein.
