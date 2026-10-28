"""Erzeugt aus einer Projekt-Konfiguration ein archiso-Profil (basierend auf releng)."""

import json
import os
import re
import shutil
import time

from . import config as C

RELENG = "/usr/share/archiso/configs/releng"
TEMPLATES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "templates")
INSTALLER_LIB = "/usr/local/lib/distro-installer"


class ProfileError(Exception):
    pass


def hex_to_ansi(hexcolor):
    h = hexcolor.lstrip("#")
    try:
        r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    except ValueError:
        r, g, b = 23, 147, 209
    return f"38;2;{r};{g};{b}"


def bash_str(s):
    return '"' + re.sub(r'([\\"$`])', r"\\\1", str(s)) + '"'


def osr_str(s):
    return '"' + re.sub(r'([\\"$`])', r"\\\1", str(s)) + '"'


def paths(cfg):
    base = os.path.join(os.path.expanduser(cfg["build"]["project_dir"]), cfg["general"]["id"])
    return {
        "base": base,
        "profile": os.path.join(base, "profile"),
        "work": os.path.join(base, "work"),
        "out": os.path.join(base, "out"),
        "script": os.path.join(base, "build.sh"),
    }


def _write(path, content, mode=None):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
    if mode:
        os.chmod(path, mode)


def _template(name, cfg):
    with open(os.path.join(TEMPLATES, name), encoding="utf-8") as f:
        text = f.read()
    return text.replace("@@ID@@", cfg["general"]["id"]).replace("@@NAME@@", cfg["general"]["name"])


def _symlink(target, link):
    os.makedirs(os.path.dirname(link), exist_ok=True)
    if os.path.lexists(link):
        os.remove(link)
    os.symlink(target, link)


def _remove(path):
    if os.path.isdir(path) and not os.path.islink(path):
        shutil.rmtree(path)
    elif os.path.lexists(path):
        os.remove(path)


def _save_image(src, dest, max_size=None, exact=None):
    """Konvertiert ein Bild nach PNG (optional skaliert). Fällt auf Kopieren zurück."""
    try:
        import gi
        gi.require_version("GdkPixbuf", "2.0")
        from gi.repository import GdkPixbuf
        pb = GdkPixbuf.Pixbuf.new_from_file(src)
        w, h = pb.get_width(), pb.get_height()
        if exact:
            pb = pb.scale_simple(exact[0], exact[1], GdkPixbuf.InterpType.BILINEAR)
        elif max_size and max(w, h) > max_size:
            f = max_size / max(w, h)
            pb = pb.scale_simple(max(1, int(w * f)), max(1, int(h * f)), GdkPixbuf.InterpType.BILINEAR)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        pb.savev(dest, "png", [], [])
        return True
    except Exception:
        if exact:
            return False
        shutil.copyfile(src, dest)
        return True


CONSOLE_KEYMAPS = {"de": "de-latin1", "at": "de-latin1", "ch": "de_CH-latin1", "gb": "uk", "us": "us",
                   "fr": "fr-latin1", "es": "es", "it": "it", "nl": "nl", "pl": "pl", "se": "sv-latin1",
                   "no": "no-latin1", "dk": "dk-latin1", "fi": "fi", "pt": "pt-latin1", "ru": "ru",
                   "tr": "trq", "cz": "cz", "hu": "hu", "be": "be-latin1", "br": "br-abnt2"}


def console_keymap(layout):
    return CONSOLE_KEYMAPS.get(layout, layout if re.match(r"^[a-z]{2,3}$", layout) else "us")


def os_release(cfg):
    g, b = cfg["general"], cfg["branding"]
    lines = [
        f"NAME={osr_str(g['name'])}",
        f"PRETTY_NAME={osr_str(g['name'] + ' ' + g['version'])}",
        f"ID={g['id']}",
        "ID_LIKE=arch",
        "BUILD_ID=rolling",
        f"VERSION_ID={osr_str(g['version'])}",
        f"ANSI_COLOR={osr_str(hex_to_ansi(b['color']))}",
        f"LOGO={g['id']}",
    ]
    for key, field in (("homepage", "HOME_URL"), ("support_url", "SUPPORT_URL"), ("bug_url", "BUG_REPORT_URL")):
        if g[key]:
            lines.append(f"{field}={osr_str(g[key])}")
    return "\n".join(lines) + "\n"


def fastfetch_config(cfg):
    b, gid = cfg["branding"], cfg["general"]["id"]
    ansi = hex_to_ansi(b["color"])
    mode = b["logo_mode"]
    if mode in ("image", "chafa") and b["logo"]:
        logo = {"type": "chafa", "source": f"/usr/share/{gid}/logo.png", "width": 32, "height": 16}
    elif mode == "ascii" and b["ascii_logo"].strip():
        logo = {"type": "file", "source": f"/usr/share/{gid}/logo.txt", "color": {"1": ansi}}
    else:
        logo = {"source": "arch"}
    conf = {
        "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
        "logo": logo,
        "display": {"separator": "  ", "color": {"keys": ansi, "title": ansi}},
        "modules": ["title", "separator", "os", "host", "kernel", "uptime", "packages", "shell",
                    "display", "de", "wm", "theme", "icons", "terminal", "cpu", "gpu", "memory",
                    "disk", "locale", "break", "colors"],
    }
    return json.dumps(conf, indent=2) + "\n"


def build_overlay(cfg, root):
    """Branding-Dateien, die im Live-System UND im installierten System landen."""
    g, b = cfg["general"], cfg["branding"]
    gid = g["id"]
    share = os.path.join(root, "usr/share", gid)
    os.makedirs(share, exist_ok=True)
    execs = []

    _write(os.path.join(share, "os-release"), os_release(cfg))
    _write(os.path.join(root, "usr/local/bin", f"{gid}-apply-branding"), _template("apply-branding.sh", cfg))
    execs.append(f"/usr/local/bin/{gid}-apply-branding")
    _write(os.path.join(root, "etc/pacman.d/hooks", f"90-{gid}-branding.hook"), _template("branding.hook", cfg))

    if b["logo"]:
        _save_image(b["logo"], os.path.join(share, "logo.png"), max_size=512)
        os.makedirs(os.path.join(root, "usr/share/pixmaps"), exist_ok=True)
        shutil.copyfile(os.path.join(share, "logo.png"), os.path.join(root, "usr/share/pixmaps", f"{gid}.png"))
    if b["ascii_logo"].strip():
        _write(os.path.join(share, "logo.txt"), b["ascii_logo"].rstrip("\n") + "\n")
    _write(os.path.join(root, "etc/xdg/fastfetch/config.jsonc"), fastfetch_config(cfg))
    _write(os.path.join(root, "usr/local/bin/neofetch"), '#!/bin/sh\nexec fastfetch "$@"\n')
    execs.append("/usr/local/bin/neofetch")
    if b["logo_mode"] == "image" and b["logo"]:
        # Echte Pixelgrafik nur in Terminals mit Kitty-Grafikprotokoll, sonst chafa (Zeichen)
        _write(os.path.join(root, "usr/local/bin/fastfetch"),
               '#!/bin/sh\n'
               'case "${TERM:-}:${TERM_PROGRAM:-}:${KONSOLE_VERSION:+konsole}" in\n'
               '  *kitty*|*WezTerm*|*ghostty*|*konsole*) exec /usr/bin/fastfetch --logo-type kitty-direct '
               '--logo-width 32 --logo-height 15 "$@" ;;\n'
               'esac\nexec /usr/bin/fastfetch "$@"\n')
        execs.append("/usr/local/bin/fastfetch")

    if b["wallpaper"]:
        ext = os.path.splitext(b["wallpaper"])[1].lower() or ".png"
        shutil.copyfile(b["wallpaper"], os.path.join(share, "wallpaper" + ext))
        os.makedirs(os.path.join(root, "usr/share/backgrounds"), exist_ok=True)
        _symlink(f"/usr/share/{gid}/wallpaper{ext}", os.path.join(root, "usr/share/backgrounds", f"{gid}{ext}"))
        script = _template("set-wallpaper.sh", cfg).replace("@@WALLPAPER@@", f"/usr/share/{gid}/wallpaper{ext}")
        _write(os.path.join(root, "usr/local/bin", f"{gid}-set-wallpaper"), script)
        execs.append(f"/usr/local/bin/{gid}-set-wallpaper")
        _write(os.path.join(root, "etc/xdg/autostart", f"{gid}-wallpaper.desktop"),
               f"[Desktop Entry]\nType=Application\nName={g['name']} Hintergrund\n"
               f"Exec=/usr/local/bin/{gid}-set-wallpaper\nNoDisplay=true\nX-GNOME-Autostart-Phase=Applications\n")

    if b["motd"].strip():
        _write(os.path.join(root, "etc/motd"), b["motd"].rstrip("\n") + "\n")

    dm = C.desktop_info(cfg)[2]
    if dm == "lightdm":
        _write(os.path.join(root, "etc/lightdm/lightdm.conf.d/10-greeter.conf"),
               "[Seat:*]\ngreeter-session=lightdm-gtk-greeter\n")
    if cfg["installer"]["zram"]:
        _write(os.path.join(root, "etc/systemd/zram-generator.conf"),
               "[zram0]\nzram-size = min(ram / 2, 8192)\ncompression-algorithm = zstd\n")
    execs += build_ai_containers(cfg, root)
    execs += build_apps(cfg, root)
    return execs


OLLAMA_MODELS_DIR = "/var/lib/ollama/models"
NOT_LIVE = "ConditionKernelCommandLine=!archisobasedir"


def build_ai_containers(cfg, root):
    """Ollama-Konfiguration, Container-Import beim ersten Start und Compose-Stacks."""
    g, ai, ct = cfg["general"], cfg["ai"], cfg["containers"]
    gid, execs = g["id"], []
    if ai["ollama"]:
        conf = ["[Service]", f'Environment="OLLAMA_MODELS={OLLAMA_MODELS_DIR}"']
        if ai["expose"]:
            conf.append('Environment="OLLAMA_HOST=0.0.0.0:11434"')
        # Vorab geladene Modelle gehören root -> Verzeichnisse und Manifeste dem ollama-Benutzer geben
        conf.append(f"ExecStartPre=+/bin/sh -c 'mkdir -p {OLLAMA_MODELS_DIR} && find {OLLAMA_MODELS_DIR} "
                    "\\( -type d -o -path \"*/manifests/*\" \\) ! -user ollama -exec chown ollama:ollama {} +'")
        _write(os.path.join(root, "etc/systemd/system/ollama.service.d", f"10-{gid}.conf"), "\n".join(conf) + "\n")

    engine = ct["engine"]
    if engine == "none":
        return execs
    tool = "/usr/bin/docker" if engine == "docker" else "/usr/bin/podman"
    dep = "docker.service" if engine == "docker" else "network-online.target"
    if ct["preload"] and C.container_images(cfg):
        script = (f"#!/bin/sh\n# Importiert die in die ISO gepackten Container-Images (einmalig)\n"
                  f"DIR=/var/lib/{gid}/container-images\n"
                  f'for f in "$DIR"/*.tar; do\n  [ -e "$f" ] || continue\n'
                  f'  echo "Importiere $f"\n  {tool} load -i "$f" && rm -f "$f"\ndone\n'
                  f'rmdir "$DIR" 2>/dev/null\nexit 0\n')
        _write(os.path.join(root, "usr/local/bin", f"{gid}-container-import"), script)
        execs.append(f"/usr/local/bin/{gid}-container-import")
        _write(os.path.join(root, "etc/systemd/system", f"{gid}-container-import.service"),
               f"[Unit]\nDescription={g['name']}: vorinstallierte Container-Images importieren\n"
               f"After={dep}\nRequires={dep}\nConditionDirectoryNotEmpty=/var/lib/{gid}/container-images\n"
               f"{NOT_LIVE}\n\n[Service]\nType=oneshot\nExecStart=/usr/local/bin/{gid}-container-import\n"
               f"TimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n")
    if ct["stacks"]:
        up = f"{tool} compose up -d --remove-orphans" if engine == "docker" else "/usr/bin/podman-compose up -d"
        down = f"{tool} compose down" if engine == "docker" else "/usr/bin/podman-compose down"
        _write(os.path.join(root, "etc/systemd/system", f"{gid}-compose@.service"),
               f"[Unit]\nDescription={g['name']} Container-Stack %i\n"
               f"After=network-online.target {dep} {gid}-container-import.service\n"
               f"Wants=network-online.target\nRequires={dep}\n{NOT_LIVE}\n\n"
               f"[Service]\nType=oneshot\nRemainAfterExit=yes\nWorkingDirectory=/etc/{gid}/compose/%i\n"
               f"ExecStart={up}\nExecStop={down}\nTimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n")
        for st in ct["stacks"]:
            _write(os.path.join(root, "etc", gid, "compose", st["name"], "compose.yaml"),
                   st["compose"].rstrip() + "\n")
            url = st.get("url") or (re.findall(r"https?://[^\s)]+", st.get("description", "")) or [""])[0]
            if url:
                _write(os.path.join(root, "usr/share/applications", f"{gid}-{st['name']}.desktop"),
                       f"[Desktop Entry]\nType=Application\nName={st.get('title') or st['name']}\n"
                       f"Comment=Container-App ({st['name']})\nExec=xdg-open {url}\n"
                       f"Icon=applications-internet\nCategories=Network;\n")
    return execs


def flatpak_setup_script(cfg):
    gid = cfg["general"]["id"]
    lines = ["#!/bin/sh",
             "# Richtet Flatpak-Quellen ein und installiert die vorgesehenen Apps (einmalig, braucht Internet,",
             "# wenn die Apps nicht schon beim Bauen mitgeliefert wurden)",
             f"TAR=/var/lib/{gid}/flatpak.tar",
             'if [ -f "$TAR" ]; then',
             '  mkdir -p /var/lib/flatpak && tar -xpf "$TAR" -C /var/lib/flatpak && rm -f "$TAR"',
             "fi",
             "ok=1"]
    for name, url in C.flatpak_remotes(cfg):
        lines.append(f"flatpak remote-add --system --if-not-exists {bash_str(name)} {bash_str(url)} || ok=0")
    for remote, app in C.flatpak_apps(cfg):
        lines.append(f"flatpak info --system {bash_str(app)} >/dev/null 2>&1 || "
                     f"flatpak install --system --noninteractive {bash_str(remote)} {bash_str(app)} || ok=0")
    lines += [f'[ "$ok" = 1 ] && mkdir -p /var/lib/{gid} && touch /var/lib/{gid}/flatpak-setup.done', "exit 0"]
    return "\n".join(lines) + "\n"


def distrobox_ini(cfg):
    out = ["# Erzeugt von Distro Builder – wird beim ersten Anmelden mit 'distrobox assemble' angelegt", ""]
    for box in cfg["distrobox"]["boxes"]:
        out += [f"[{box['name']}]", f"image={box['image']}", "pull=true", "replace=false"]
        pkgs = " ".join(p for p in re.split(r"[\s,]+", box.get("packages", "")) if p)
        if pkgs:
            out.append(f'additional_packages="{pkgs}"')
        apps = " ".join(a for a in re.split(r"[\s,]+", box.get("export", "")) if a)
        if apps:
            out.append(f'exported_apps="{apps}"')
        out.append("")
    return "\n".join(out)


def build_apps(cfg, root):
    """Flatpak-Quellen/Apps und Distrobox-Container (werden beim ersten Start bzw. Anmelden eingerichtet)."""
    g = cfg["general"]
    gid, execs = g["id"], []
    if cfg["flatpak"]["enabled"]:
        _write(os.path.join(root, "usr/local/bin", f"{gid}-flatpak-setup"), flatpak_setup_script(cfg))
        execs.append(f"/usr/local/bin/{gid}-flatpak-setup")
        _write(os.path.join(root, "etc/systemd/system", f"{gid}-flatpak-setup.service"),
               f"[Unit]\nDescription={g['name']}: Flatpak-Quellen und Apps einrichten\n"
               f"Wants=network-online.target\nAfter=network-online.target\n"
               f"ConditionPathExists=!/var/lib/{gid}/flatpak-setup.done\n{NOT_LIVE}\n\n"
               f"[Service]\nType=oneshot\nExecStart=/usr/local/bin/{gid}-flatpak-setup\nTimeoutStartSec=0\n\n"
               f"[Install]\nWantedBy=multi-user.target\n")
    if C.distrobox_active(cfg) and cfg["distrobox"]["boxes"]:
        _write(os.path.join(root, "etc", gid, "distrobox.ini"), distrobox_ini(cfg))
        if cfg["containers"]["engine"] == "docker":
            _write(os.path.join(root, "etc/distrobox/distrobox.conf"), 'container_manager="docker"\n')
        state = f"%h/.local/state/{gid}"
        unit = f"{gid}-distrobox.service"
        _write(os.path.join(root, "etc/systemd/user", unit),
               f"[Unit]\nDescription={g['name']}: Distrobox-Container einrichten\n"
               f"ConditionPathExists=!{state}/distrobox.done\n{NOT_LIVE}\n\n"
               f"[Service]\nType=oneshot\n"
               f"ExecStart=/usr/bin/distrobox assemble create --file /etc/{gid}/distrobox.ini\n"
               f"ExecStartPost=/usr/bin/mkdir -p {state}\nExecStartPost=/usr/bin/touch {state}/distrobox.done\n"
               f"TimeoutStartSec=0\n\n[Install]\nWantedBy=default.target\n")
        # global für alle Benutzer aktivieren (entspricht 'systemctl --global enable')
        _symlink(f"/etc/systemd/user/{unit}", os.path.join(root, "etc/systemd/user/default.target.wants", unit))
    return execs


def extra_services(cfg):
    """Eigene Dienste, die im installierten System aktiviert werden."""
    svcs = compose_services(cfg)
    if cfg["flatpak"]["enabled"]:
        svcs.append(f"{cfg['general']['id']}-flatpak-setup.service")
    return svcs


def compose_services(cfg):
    gid, ct = cfg["general"]["id"], cfg["containers"]
    if ct["engine"] == "none":
        return []
    svcs = []
    if ct["preload"] and C.container_images(cfg):
        svcs.append(f"{gid}-container-import.service")
    svcs += [f"{gid}-compose@{st['name']}.service" for st in ct["stacks"] if st.get("autostart", True)]
    if ct["engine"] == "podman" and ct["stacks"]:
        svcs.append("podman-restart.service")
    return svcs


# Dateien aus releng, die im installierten System bleiben sollen
RELENG_KEEP = {
    "etc/resolv.conf", "etc/localtime", "etc/passwd", "etc/shadow", "etc/hostname", "etc/locale.conf",
    "etc/systemd/system/dbus-org.freedesktop.resolve1.service",
    "etc/systemd/system/dbus-org.freedesktop.timesync1.service",
    "etc/systemd/system/multi-user.target.wants/systemd-resolved.service",
    "etc/systemd/system/sysinit.target.wants/systemd-timesyncd.service",
}


def live_only_paths(releng_air, cfg, overlay_files):
    """Pfade, die der Installer nach dem Offline-Kopieren entfernt (Live-Konfiguration)."""
    gid = cfg["general"]["id"]
    files, dirs = [], []
    for dirpath, dirnames, filenames in os.walk(releng_air):
        rel_dir = os.path.relpath(dirpath, releng_air)
        if rel_dir != ".":
            dirs.append("/" + rel_dir)
        for fn in filenames + [d for d in dirnames if os.path.islink(os.path.join(dirpath, d))]:
            rel = os.path.normpath(os.path.join(rel_dir, fn))
            if rel not in RELENG_KEEP and rel not in overlay_files:
                files.append("/" + rel)
    files += [
        "/etc/systemd/system/live-setup.service", "/etc/systemd/system/multi-user.target.wants/live-setup.service",
        "/usr/local/bin/live-setup", "/etc/sddm.conf.d/autologin.conf", "/etc/gdm/custom.conf",
        "/etc/lightdm/lightdm.conf.d/50-autologin.conf", "/etc/systemd/system/display-manager.service",
        "/usr/local/lib/distro-installer", "/etc/distro-installer.json", "/usr/local/bin/distro-installer",
        "/usr/share/applications/distro-installer.desktop", "/etc/xdg/autostart/distro-installer.desktop",
        f"/usr/share/{gid}/overlay", "/root/.zshrc.local", "/etc/sudoers.d/00-live",
    ]
    return sorted(set(files)), sorted(set(dirs))


def pacman_conf(cfg):
    r = cfg["repos"]
    server = f"Server = {r['mirror'].strip()}" if r["mirror"].strip() else "Include = /etc/pacman.d/mirrorlist"
    arm = C.is_image(cfg)
    out = [
        "# Erzeugt von Distro Builder",
        "[options]",
        "HoldPkg     = pacman glibc",
        f"Architecture = {C.pacman_arch(cfg) if arm else 'auto'}",
        "Color",
        "ILoveCandy",
        "ParallelDownloads = 5",
        "SigLevel    = Required DatabaseOptional",
        "LocalFileSigLevel = Optional",
        "",
    ]
    for repo in r["custom"]:
        if repo.get("first"):
            out += [f"[{repo['name']}]", f"SigLevel = {repo.get('siglevel', 'Optional TrustAll')}",
                    f"Server = {repo['server']}", ""]
    if arm:
        # Arch Linux ARM: keine Testing- und multilib-Repos, dafür [alarm] und [aur]
        official = ((None, "core"), ("extra", "extra"), (None, "alarm"), (None, "aur"))
    else:
        official = (("core_testing", "core-testing"), (None, "core"), ("extra_testing", "extra-testing"),
                    ("extra", "extra"), ("multilib", "multilib"))
    for key, name in official:
        if key is None or r.get(key):
            out += [f"[{name}]", server, ""]
    for repo in r["custom"]:
        if not repo.get("first"):
            out += [f"[{repo['name']}]", f"SigLevel = {repo.get('siglevel', 'Optional TrustAll')}",
                    f"Server = {repo['server']}", ""]
    return "\n".join(out)


def flatpak_preload(cfg):
    return cfg["flatpak"]["enabled"] and cfg["flatpak"]["preload"] and bool(C.flatpak_apps(cfg))


def flatpak_preload_script(cfg, target):
    """Bash-Abschnitt für build.sh: lädt Flatpaks in eine eigene Installation im Cache (als root)."""
    if not flatpak_preload(cfg):
        return ""
    farch = C.pacman_arch(cfg)
    lines = ["", "# Flatpak-Apps vorladen (schlägt das fehl, installiert sie das System beim ersten Start)",
             f'FLATPAK_CACHE="$BASE/cache/flatpak-{farch}"',
             "flatpak_preload() {",
             '  mkdir -p -- "$FLATPAK_CACHE"',
             '  export FLATPAK_SYSTEM_DIR="$FLATPAK_CACHE"']
    for name, url in C.flatpak_remotes(cfg):
        lines.append(f"  flatpak remote-add --system --if-not-exists {bash_str(name)} {bash_str(url)} || return 1")
    by_remote = {}
    for remote, app in C.flatpak_apps(cfg):
        by_remote.setdefault(remote, []).append(app)
    for remote, apps in by_remote.items():
        lines.append(f"  flatpak install --system --noninteractive --or-update --arch={farch} {bash_str(remote)} "
                     + " ".join(bash_str(a) for a in apps) + " || return 1")
    lines += ["}",
              "if ! command -v flatpak >/dev/null; then",
              "  echo \"WARNUNG: flatpak ist nicht installiert – die Apps werden erst beim ersten Start geladen.\"",
              "elif echo '==> Lade Flatpak-Apps' && flatpak_preload; then",
              target,
              "else",
              "  echo \"WARNUNG: Flatpak-Apps konnten nicht vorgeladen werden – sie werden beim ersten Start geladen.\"",
              "fi",
              f"chown -R {os.getuid()}:{os.getgid()} -- \"$BASE/cache\" 2>/dev/null || true",
              ""]
    return "\n".join(lines)


def installer_json(cfg, live_files=(), live_dirs=()):
    g, i = cfg["general"], cfg["installer"]
    data_paths = []
    if cfg["ai"]["ollama"] and cfg["ai"]["models"]:
        data_paths.append(OLLAMA_MODELS_DIR)
    if cfg["containers"]["preload"] and C.container_images(cfg):
        data_paths.append(f"/var/lib/{g['id']}/container-images")
    if flatpak_preload(cfg):
        data_paths.append(f"/var/lib/{g['id']}/flatpak.tar")
    return json.dumps({
        "name": g["name"],
        "id": g["id"],
        "hostname": g["hostname"],
        "logo": f"/usr/share/{g['id']}/logo.png" if cfg["branding"]["logo"] else "",
        "welcome": i["welcome"],
        "packages": C.target_packages(cfg),
        "services": C.target_services(cfg) + extra_services(cfg),
        "kernel_params": C.kernel_params(cfg),
        "zram": i["zram"],
        "allow_online": i.get("allow_online", True),
        "data_paths": data_paths,
        "live_files": list(live_files),
        "live_dirs": list(live_dirs),
        "desktop": cfg["packages"]["desktop"],
        "fastfetch_on_terminal": cfg["branding"]["fastfetch_on_terminal"],
        "defaults": {"locale": i["locale"], "timezone": i["timezone"], "keyboard": i["keyboard"],
                     "filesystem": i["filesystem"], "source": i.get("source", "offline"),
                     "encrypt": i.get("encrypt", False), "bootloader": i.get("bootloader", "grub"),
                     "swap_gb": i.get("swap_gb", 0)},
    }, indent=2, ensure_ascii=False)


def _rebrand_boot(profile, name):
    for sub in ("efiboot", "grub", "syslinux"):
        for dirpath, _, files in os.walk(os.path.join(profile, sub)):
            for fn in files:
                if not fn.endswith((".cfg", ".conf")):
                    continue
                p = os.path.join(dirpath, fn)
                with open(p, encoding="utf-8") as f:
                    text = f.read()
                new = text.replace("Arch Linux install medium", f"{name} Live")
                new = new.replace("install Arch Linux", f"install {name}").replace("Arch Linux", name)
                if new != text:
                    with open(p, "w", encoding="utf-8") as f:
                        f.write(new)


def _edit_profiledef(cfg, profile, perms):
    g = cfg["general"]
    p = os.path.join(profile, "profiledef.sh")
    with open(p, encoding="utf-8") as f:
        text = f.read()
    label = re.sub(r"[^A-Z0-9_]", "_", g["id"].upper())[:22]
    repl = {
        "iso_name": bash_str(g["id"]),
        "iso_label": f'"{label}_$(date --date="@${{SOURCE_DATE_EPOCH:-$(date +%s)}}" +%Y%m)"',
        "iso_publisher": bash_str(g["publisher"] or g["name"]),
        "iso_application": bash_str(f"{g['name']} Live/Installations-Medium"),
        "iso_version": bash_str(f"{g['version']}-") + '"$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y.%m.%d)"',
    }
    for key, val in repl.items():
        text = re.sub(rf"^{key}=.*$", lambda _m, k=key, v=val: f"{k}={v}", text, flags=re.M)
    if cfg["build"]["compression"] == "zstd":
        text = re.sub(r"^airootfs_image_tool_options=.*$",
                      "airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '15' '-b' '1M')",
                      text, flags=re.M)
    text += "\n# Distro Builder\nfile_permissions+=(\n"
    for path, mode in perms.items():
        text += f'  ["{path}"]="{mode}"\n'
    text += ")\n"
    with open(p, "w", encoding="utf-8") as f:
        f.write(text)


def autologin_files(dm, session, user):
    """Konfigurationsdateien für die automatische Anmeldung (Pfad -> Inhalt)."""
    if dm == "sddm":
        return {"etc/sddm.conf.d/autologin.conf": f"[Autologin]\nUser={user}\nSession={session}\n"}
    if dm == "gdm":
        return {"etc/gdm/custom.conf": f"[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin={user}\n"}
    if dm == "lightdm":
        return {"etc/lightdm/lightdm.conf.d/50-autologin.conf":
                f"[Seat:*]\nautologin-user={user}\nautologin-session={session}\n"}
    return {}


def build_script(cfg):
    pth = paths(cfg)
    uid, gid = os.getuid(), os.getgid()
    # Flatpaks als Tar-Archiv: mkarchiso kopiert airootfs ohne Dateirechte, das Archiv behält sie
    tar = f'"$PROFILE/airootfs/var/lib/{cfg["general"]["id"]}/flatpak.tar"'
    flatpak = flatpak_preload_script(cfg, f'  tar -C "$FLATPAK_CACHE" -cf {tar} . && chown {uid}:{gid} -- {tar}')
    return f"""#!/bin/bash
# Erzeugt von Distro Builder – kann auch manuell mit 'sudo bash build.sh' ausgeführt werden.
set -euo pipefail
BASE={bash_str(pth['base'])}
PROFILE={bash_str(pth['profile'])}
WORK={bash_str(pth['work'])}
OUT={bash_str(pth['out'])}
if ! command -v mkarchiso >/dev/null; then
  echo "FEHLER: mkarchiso nicht gefunden. Bitte 'pacman -S archiso' installieren." >&2
  exit 1
fi
cleanup() {{ rm -rf -- "$WORK"; }}
trap cleanup EXIT
rm -rf -- "$WORK"
mkdir -p -- "$OUT"
{flatpak}
mkarchiso -v -r -w "$WORK" -o "$OUT" "$PROFILE"
chown -R {uid}:{gid} -- "$OUT"
echo "##DONE##"
"""


def generate(cfg, log=print, releng=RELENG):
    errors, _ = C.validate(cfg)
    if errors:
        raise ProfileError("\n".join(errors))
    if C.is_image(cfg):
        return generate_image(cfg, log)
    if not os.path.isdir(releng):
        raise ProfileError(f"archiso-Vorlage nicht gefunden ({releng}). Ist 'archiso' installiert?")

    g = cfg["general"]
    gid = g["id"]
    pth = paths(cfg)
    os.makedirs(pth["base"], exist_ok=True)
    profile = pth["profile"]
    log(f"Erzeuge Profil in {profile}")
    _remove(profile)
    shutil.copytree(releng, profile, symlinks=True)
    air = os.path.join(profile, "airootfs")
    systemd = os.path.join(air, "etc/systemd/system")
    perms = {}

    # Pakete
    with open(os.path.join(releng, "packages.x86_64"), encoding="utf-8") as f:
        releng_pkgs = [l.strip() for l in f if l.strip() and not l.startswith("#")]
    pkgs = C.live_packages(cfg, releng_pkgs)
    _write(os.path.join(profile, "packages.x86_64"), "\n".join(pkgs) + "\n")
    log(f"{len(pkgs)} Pakete für das Live-System")
    if not cfg["packages"]["rescue_tools"]:
        for unit in C.MINIMAL_DROP_UNITS:
            _remove(os.path.join(systemd, unit))

    # Repositories
    pconf = pacman_conf(cfg)
    _write(os.path.join(profile, "pacman.conf"), pconf)
    _write(os.path.join(air, "etc/pacman.conf"), pconf)

    # Branding
    _write(os.path.join(air, "etc/hostname"), g["hostname"] + "\n")
    kb = cfg["installer"]["keyboard"]
    _write(os.path.join(air, "etc/vconsole.conf"), f"KEYMAP={console_keymap(kb)}\n")
    _write(os.path.join(air, "etc/X11/xorg.conf.d/00-keyboard.conf"),
           f'Section "InputClass"\n    Identifier "system-keyboard"\n    MatchIsKeyboard "on"\n'
           f'    Option "XkbLayout" "{kb}"\nEndSection\n')
    execs = build_overlay(cfg, air)
    if flatpak_preload(cfg):
        os.makedirs(os.path.join(air, "var/lib", gid), exist_ok=True)
    staging = os.path.join(air, "usr/share", gid, "overlay")
    build_overlay(cfg, staging)
    overlay_files = set()
    for dirpath, _d, filenames in os.walk(staging):
        for fn in filenames:
            overlay_files.add(os.path.relpath(os.path.join(dirpath, fn), staging))
    for e in execs:
        perms[e] = "0:0:755"
    _rebrand_boot(profile, g["name"])
    if cfg["branding"]["wallpaper"]:
        if _save_image(cfg["branding"]["wallpaper"], os.path.join(profile, "syslinux/splash.png"), exact=(640, 480)):
            log("Bootmenü-Hintergrund ersetzt")

    # Desktop-Live-Umgebung
    label, _, dm, session = C.desktop_info(cfg)
    if dm:
        log(f"Live-Desktop: {label} ({dm})")
        for unit in ("multi-user.target.wants/systemd-networkd.service", "multi-user.target.wants/iwd.service",
                     "sockets.target.wants/systemd-networkd.socket", "dbus-org.freedesktop.network1.service",
                     "network-online.target.wants/systemd-networkd-wait-online.service"):
            _remove(os.path.join(systemd, unit))
        _symlink("/usr/lib/systemd/system/NetworkManager.service",
                 os.path.join(systemd, "multi-user.target.wants/NetworkManager.service"))
        _symlink(f"/usr/lib/systemd/system/{dm}.service", os.path.join(systemd, "display-manager.service"))
        for rel, content in autologin_files(dm, session, "live").items():
            _write(os.path.join(air, rel), content)

        _write(os.path.join(air, "usr/local/bin/live-setup"),
               _template("live-setup.sh", cfg).replace(
                   "@@FASTFETCH@@", "1" if cfg["branding"]["fastfetch_on_terminal"] else "0"))
        perms["/usr/local/bin/live-setup"] = "0:0:755"
        _write(os.path.join(systemd, "live-setup.service"), _template("live-setup.service", cfg))
        _symlink("/etc/systemd/system/live-setup.service",
                 os.path.join(systemd, "multi-user.target.wants/live-setup.service"))

    # Installer
    if C.installer_active(cfg):
        log("Grafischer Installer wird eingebunden")
        lib = air + INSTALLER_LIB
        for fn in ("installer.py", "backend.py"):
            _write(os.path.join(lib, fn), _template(fn, cfg))
            perms[f"{INSTALLER_LIB}/{fn}"] = "0:0:755"
        live_files, live_dirs = live_only_paths(os.path.join(releng, "airootfs"), cfg, overlay_files)
        _write(os.path.join(air, "etc/distro-installer.json"), installer_json(cfg, live_files, live_dirs))
        _write(os.path.join(air, "usr/local/bin/distro-installer"),
               f"#!/bin/sh\nexec python3 {INSTALLER_LIB}/installer.py \"$@\"\n")
        perms["/usr/local/bin/distro-installer"] = "0:0:755"
        icon = gid if cfg["branding"]["logo"] else "system-software-install"
        desktop = (f"[Desktop Entry]\nType=Application\nName={g['name']} installieren\n"
                   f"Comment={g['name']} auf der Festplatte installieren\nExec=distro-installer\n"
                   f"Icon={icon}\nTerminal=false\nCategories=System;\n")
        _write(os.path.join(air, "usr/share/applications/distro-installer.desktop"), desktop)
        if cfg["installer"]["autostart"]:
            _write(os.path.join(air, "etc/xdg/autostart/distro-installer.desktop"), desktop)
    elif cfg["branding"]["fastfetch_on_terminal"]:
        _write(os.path.join(air, "root/.zshrc.local"), "fastfetch\n")

    # KI im Live-System direkt nutzbar
    if cfg["ai"]["ollama"]:
        _symlink("/usr/lib/systemd/system/ollama.service",
                 os.path.join(systemd, "multi-user.target.wants/ollama.service"))
        if cfg["ai"]["models"]:
            log(f"KI-Modelle: {', '.join(cfg['ai']['models'])}")
    if cfg["containers"]["engine"] != "none":
        log(f"Container: {cfg['containers']['engine']}, {len(cfg['containers']['stacks'])} Stack(s), "
            f"{len(C.container_images(cfg)) if cfg['containers']['preload'] else 0} vorab geladene Image(s)")

    _edit_profiledef(cfg, profile, perms)
    _write(pth["script"], build_script(cfg), 0o755)
    _write(os.path.join(pth["base"], "project.json"), json.dumps(cfg, indent=2, ensure_ascii=False))
    log(f"Profil fertig ({time.strftime('%H:%M:%S')})")
    return pth


# ---------------------------------------------------------------- ARM: fertiges Disk-Image
ALARM_KEY = "68B3537F39A313B3E574D06777193F152BDBE6A6"  # Arch Linux ARM Build System


def image_services(cfg):
    gid = cfg["general"]["id"]
    return C._dedup(C.target_services(cfg) + extra_services(cfg)
                    + [f"{gid}-growfs.service", f"{gid}-pacman-init.service"])


def image_overlay(cfg, air):
    """Systemeinstellungen, die beim ISO der Installer setzt – hier direkt ins Image."""
    g, i, img = cfg["general"], cfg["installer"], cfg["image"]
    gid, host, kb = g["id"], g["hostname"], i["keyboard"]
    _write(os.path.join(air, "etc/hostname"), host + "\n")
    _write(os.path.join(air, "etc/hosts"), f"127.0.0.1 localhost\n::1 localhost\n127.0.1.1 {host}.localdomain {host}\n")
    _write(os.path.join(air, "etc/locale.conf"), f"LANG={i['locale']}\n")
    _write(os.path.join(air, "etc/vconsole.conf"), f"KEYMAP={console_keymap(kb)}\n")
    _write(os.path.join(air, "etc/X11/xorg.conf.d/00-keyboard.conf"),
           f'Section "InputClass"\n    Identifier "system-keyboard"\n    MatchIsKeyboard "on"\n'
           f'    Option "XkbLayout" "{kb}"\nEndSection\n')
    _write(os.path.join(air, "etc/sudoers.d/10-wheel"), "%wheel ALL=(ALL:ALL) ALL\n")
    _label, _p, dm, session = C.desktop_info(cfg)
    if dm and img["autologin"]:
        for rel, content in autologin_files(dm, session, img["username"]).items():
            _write(os.path.join(air, rel), content)
    if dm == "lightdm":
        _write(os.path.join(air, "etc/lightdm/lightdm.conf.d/10-greeter.conf"),
               "[Seat:*]\ngreeter-session=lightdm-gtk-greeter\n")
    _write(os.path.join(air, "usr/local/bin", f"{gid}-growfs"),
           "#!/bin/sh\n# Vergrößert die Root-Partition beim ersten Start auf den ganzen Datenträger\nset -e\n"
           "ROOT=$(findmnt -no SOURCE /)\nDISK=/dev/$(lsblk -no PKNAME \"$ROOT\")\n"
           "PART=$(cat \"/sys/class/block/$(basename \"$ROOT\")/partition\")\n"
           "sfdisk --relocate gpt-bak-std \"$DISK\" 2>/dev/null || true\n"
           "echo ', +' | sfdisk --force --no-reread -N \"$PART\" \"$DISK\" || true\n"
           "partx -u \"$DISK\" || true\nresize2fs \"$ROOT\"\n"
           f"mkdir -p /var/lib/{gid} && touch /var/lib/{gid}/growfs.done\n", 0o755)
    _write(os.path.join(air, "etc/systemd/system", f"{gid}-growfs.service"),
           f"[Unit]\nDescription={g['name']}: Root-Partition vergrößern\n"
           f"ConditionPathExists=!/var/lib/{gid}/growfs.done\nAfter=local-fs.target\n\n"
           f"[Service]\nType=oneshot\nExecStart=/usr/local/bin/{gid}-growfs\n\n"
           f"[Install]\nWantedBy=multi-user.target\n")
    _write(os.path.join(air, "etc/systemd/system", f"{gid}-pacman-init.service"),
           f"[Unit]\nDescription={g['name']}: Paketschlüssel einrichten\n"
           f"ConditionPathExists=!/etc/pacman.d/gnupg\nBefore=archlinux-keyring-wkd-sync.service\n\n"
           f"[Service]\nType=oneshot\nExecStart=/usr/bin/pacman-key --init\n"
           f"ExecStart=/usr/bin/pacman-key --populate archlinuxarm\n\n[Install]\nWantedBy=multi-user.target\n")


def image_setup_script(cfg):
    g, i, img = cfg["general"], cfg["installer"], cfg["image"]
    services = " ".join(bash_str(s) for s in image_services(cfg))
    variables = "\n".join([
        f"PLATFORM={bash_str(C.arch(cfg))}",
        f"KERNEL={bash_str(C.kernel(cfg))}",
        f"NAME={bash_str(g['name'])}",
        f"GID={bash_str(g['id'])}",
        f"USERNAME={bash_str(img['username'])}",
        f"PASSWORD={bash_str(img['password'])}",
        f"EXPIRE={1 if img.get('expire_password', True) else 0}",
        f"AUTOLOGIN={1 if img['autologin'] and C.desktop_info(cfg)[2] else 0}",
        f"FASTFETCH={1 if cfg['branding']['fastfetch_on_terminal'] else 0}",
        f"DESKTOP={0 if cfg['packages']['desktop'] == 'none' else 1}",
        f"LOCALE={bash_str(i['locale'])}",
        f"TIMEZONE={bash_str(i['timezone'])}",
        f"KPARAMS={bash_str(' '.join(C.kernel_params(cfg)))}",
        f"SERVICES=({services})",
    ])
    with open(os.path.join(TEMPLATES, "image-setup.sh"), encoding="utf-8") as f:
        text = f.read()
    return text.replace("@@NAME@@", g["name"]).replace("@@VARS@@", variables)


def image_build_script(cfg):
    pth = paths(cfg)
    g, plat = cfg["general"], C.arch(cfg)
    uid, gid = os.getuid(), os.getgid()
    url = C.ALARM_TARBALLS[plat]
    label = re.sub(r"[^A-Za-z0-9_]", "_", g["id"])[:16]
    layout = "label: dos\n,512MiB,c,*\n,,L\n" if plat == "rpi" else "label: gpt\n,512MiB,U\n,,L\n"
    flatpak = flatpak_preload_script(
        cfg, '  mkdir -p "$MNT/var/lib/flatpak" && cp -a "$FLATPAK_CACHE/." "$MNT/var/lib/flatpak/" '
             '&& chown -R 0:0 "$MNT/var/lib/flatpak"')
    if cfg["build"]["compression"] == "xz":
        compress = 'xz -T0 -c "$IMG" > "$OUT/$IMG_NAME.img.xz"'
        tool = "xz"
    else:
        compress = 'zstd -T0 -q -f -o "$OUT/$IMG_NAME.img.zst" "$IMG"'
        tool = "zstd"
    return f"""#!/bin/bash
# Erzeugt von Distro Builder – baut das Disk-Image für {C.ARCHES[plat][0]}.
# Kann auch manuell mit 'sudo bash build.sh' ausgeführt werden.
set -euo pipefail
BASE={bash_str(pth['base'])}
PROFILE={bash_str(pth['profile'])}
WORK={bash_str(pth['work'])}
OUT={bash_str(pth['out'])}
CACHE="$BASE/cache"
URL={bash_str(url)}
IMG_NAME={bash_str(f"{g['id']}-{g['version']}-{plat}-")}"$(date +%Y.%m.%d)"
SIZE_GB={int(cfg['image']['size_gb'])}
MNT="$WORK/mnt"
LOOP=""

for t in arch-chroot:arch-install-scripts bsdtar:libarchive curl:curl sfdisk:util-linux losetup:util-linux \\
         mkfs.vfat:dosfstools mkfs.ext4:e2fsprogs; do
  if ! command -v "${{t%%:*}}" >/dev/null; then
    echo "FEHLER: ${{t%%:*}} fehlt. Bitte 'pacman -S ${{t#*:}}' installieren." >&2
    exit 1
  fi
done
if [ "$(uname -m)" != aarch64 ] && [ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ]; then
  systemctl restart systemd-binfmt.service 2>/dev/null || true
  if [ ! -e /proc/sys/fs/binfmt_misc/qemu-aarch64 ]; then
    echo "FEHLER: ARM-Emulation fehlt. Bitte 'pacman -S qemu-user-static qemu-user-static-binfmt' installieren." >&2
    exit 1
  fi
fi

cleanup() {{
  set +e
  mountpoint -q "$MNT" && umount -R "$MNT"
  mountpoint -q "$WORK/esp" && umount "$WORK/esp"
  [ -n "$LOOP" ] && losetup -d "$LOOP"
  rm -rf -- "$WORK"
}}
trap cleanup EXIT
trap 'exit 130' INT TERM
rm -rf -- "$WORK"
mkdir -p -- "$MNT" "$WORK/esp" "$OUT" "$CACHE"

echo "==> Lade das Arch-Linux-ARM-Basissystem"
TAR="$CACHE/$(basename "$URL")"
curl -fsSL --retry 3 -o "$TAR.md5" "$URL.md5"
if ! (cd "$CACHE" && md5sum --status -c "$(basename "$TAR").md5"); then
  curl -fL --retry 3 -o "$TAR" "$URL"
  (cd "$CACHE" && md5sum -c "$(basename "$TAR").md5")
fi
if command -v gpg >/dev/null; then
  export GNUPGHOME="$WORK/gnupg"
  mkdir -m 700 -- "$GNUPGHOME"
  if gpg --batch --quiet --keyserver hkps://keyserver.ubuntu.com --recv-keys {ALARM_KEY} 2>/dev/null \\
     && curl -fsSL --retry 3 -o "$TAR.sig" "$URL.sig"; then
    gpg --batch --quiet --verify "$TAR.sig" "$TAR" || {{ echo "FEHLER: Signatur des Basissystems ist ungültig" >&2; exit 1; }}
    echo "Signatur geprüft ✔"
  else
    echo "WARNUNG: Signatur konnte nicht geprüft werden (Schlüsselserver nicht erreichbar) – nur MD5 geprüft."
  fi
  unset GNUPGHOME
fi

echo "==> Erzeuge Image ($SIZE_GB GB)"
IMG="$WORK/$IMG_NAME.img"
truncate -s "${{SIZE_GB}}G" "$IMG"
printf {bash_str(layout)} | sfdisk -q "$IMG"
LOOP=$(losetup -fP --show "$IMG")
udevadm settle 2>/dev/null || true
mkfs.vfat -F 32 -n BOOT "${{LOOP}}p1" >/dev/null
mkfs.ext4 -q -F -L {bash_str(label)} "${{LOOP}}p2"
mount "${{LOOP}}p2" "$MNT"

echo "==> Entpacke das Basissystem"
bsdtar -xpf "$TAR" -C "$MNT"
# /boot liegt auf FAT32 (keine Symlinks/Rechte) – Inhalt hinüberkopieren
mount "${{LOOP}}p1" "$WORK/esp"
cp -rL "$MNT/boot/." "$WORK/esp/"
umount "$WORK/esp"
find "$MNT/boot" -mindepth 1 -delete
mount "${{LOOP}}p1" "$MNT/boot"

echo "==> Übernehme Branding und Einstellungen"
tar -C "$PROFILE/airootfs" --owner=0 --group=0 -cf - . | tar -C "$MNT" -xpf -
install -m644 "$PROFILE/pacman.conf" "$MNT/etc/pacman.conf"
install -m600 "$PROFILE/setup.sh" "$MNT/root/setup.sh"
install -m644 "$PROFILE/packages.txt" "$MNT/root/packages.txt"
{flatpak}
echo "==> System wird im Image eingerichtet (unter Emulation dauert das)"
arch-chroot "$MNT" /usr/bin/env \\
  ROOT_UUID="$(blkid -s UUID -o value "${{LOOP}}p2")" \\
  BOOT_UUID="$(blkid -s UUID -o value "${{LOOP}}p1")" \\
  ROOT_PARTUUID="$(blkid -s PARTUUID -o value "${{LOOP}}p2")" \\
  /bin/bash /root/setup.sh
rm -f -- "$MNT/root/setup.sh" "$MNT/root/packages.txt"
sync
umount -R "$MNT"
losetup -d "$LOOP"
LOOP=""

echo "==> Komprimiere Image"
if command -v {tool} >/dev/null; then
  {compress}
else
  mv -- "$IMG" "$OUT/$IMG_NAME.img"
fi
chown -R {uid}:{gid} -- "$OUT"
echo "Fertig: $(ls -1t "$OUT" | head -n1)"
echo "##DONE##"
"""


def generate_image(cfg, log=print):
    """Profil für ein fertiges ARM-Image: Overlay, Paketliste, Einrichtungsskript und build.sh."""
    pth = paths(cfg)
    os.makedirs(pth["base"], exist_ok=True)
    profile = pth["profile"]
    log(f"Erzeuge Image-Profil für {C.ARCHES[C.arch(cfg)][0]} in {profile}")
    _remove(profile)
    air = os.path.join(profile, "airootfs")
    os.makedirs(air)
    execs = build_overlay(cfg, air)
    image_overlay(cfg, air)
    for e in execs:
        os.chmod(air + e, 0o755)
    pkgs = C.image_packages(cfg)
    _write(os.path.join(profile, "packages.txt"), "\n".join(pkgs) + "\n")
    log(f"{len(pkgs)} Pakete, Kernel {C.kernel(cfg)}")
    pconf = pacman_conf(cfg)
    _write(os.path.join(profile, "pacman.conf"), pconf)
    _write(os.path.join(profile, "setup.sh"), image_setup_script(cfg), 0o600)
    if cfg["ai"]["ollama"] and cfg["ai"]["models"]:
        log(f"KI-Modelle: {', '.join(cfg['ai']['models'])}")
    _write(pth["script"], image_build_script(cfg), 0o755)
    _write(os.path.join(pth["base"], "project.json"), json.dumps(cfg, indent=2, ensure_ascii=False))
    log(f"Profil fertig ({time.strftime('%H:%M:%S')})")
    return pth
