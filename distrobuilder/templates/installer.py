#!/usr/bin/env python3
"""@@NAME@@ – grafischer Installer (GTK4 / libadwaita)."""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, GLib, Gtk  # noqa: E402

CONF_PATH = "/etc/distro-installer.json"
BACKEND = os.path.join(os.path.dirname(os.path.abspath(__file__)), "backend.py")


def load_conf():
    try:
        with open(CONF_PATH, encoding="utf-8") as f:
            return json.load(f)
    except OSError:
        return {"name": "@@NAME@@", "id": "@@ID@@", "hostname": "@@ID@@", "logo": "", "welcome": "",
                "defaults": {"locale": "en_US.UTF-8", "timezone": "UTC", "keyboard": "us", "filesystem": "ext4"}}


def cmd_lines(cmd):
    try:
        return [l.strip() for l in subprocess.run(cmd, capture_output=True, text=True).stdout.splitlines()
                if l.strip()]
    except OSError:
        return []


def list_locales():
    locs = []
    try:
        with open("/usr/share/i18n/SUPPORTED", encoding="utf-8") as f:
            for line in f:
                name, _, charset = line.strip().partition(" ")
                if charset == "UTF-8":
                    locs.append(name)
    except OSError:
        pass
    return locs or ["en_US.UTF-8", "de_DE.UTF-8"]


def list_timezones():
    return cmd_lines(["timedatectl", "list-timezones"]) or ["UTC", "Europe/Berlin"]


def list_layouts():
    return cmd_lines(["localectl", "list-x11-keymap-layouts"]) or ["us", "de"]


def boot_disk():
    """Datenträger, von dem das Live-System gestartet wurde (wird ausgeblendet)."""
    src = cmd_lines(["findmnt", "-no", "SOURCE", "/run/archiso/bootmnt"])
    if not src:
        return None
    parent = cmd_lines(["lsblk", "-npo", "PKNAME", src[0]])
    return parent[0] if parent and parent[0] else src[0]


def list_disks():
    try:
        r = subprocess.run(["lsblk", "-J", "-b", "-d", "-o", "NAME,PATH,SIZE,MODEL,TYPE,TRAN,RO"],
                           capture_output=True, text=True, check=True)
        devices = json.loads(r.stdout).get("blockdevices", [])
    except (OSError, subprocess.CalledProcessError, ValueError):
        return []
    skip = boot_disk()
    return [d for d in devices
            if d.get("type") == "disk" and not d.get("ro") and d.get("path") != skip
            and not d["name"].startswith(("loop", "zram", "sr")) and int(d.get("size") or 0) > 8 * 1024**3]


DISK_ROLES = [("none", "Nicht verwenden"), ("home", "/home (Benutzerdaten)"),
              ("data", "Datenlaufwerk (/data)"), ("raid", "btrfs-Spiegel der Systemplatte (RAID 1)")]


def has_offline_image():
    return any(os.path.exists(p) for p in ("/run/archiso/copytoram/airootfs.sfs",
                                           "/run/archiso/bootmnt/arch/x86_64/airootfs.sfs"))


def human(size):
    size = float(size)
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if size < 1024:
            return f"{size:.1f} {unit}"
        size /= 1024
    return f"{size:.1f} PB"


def combo(title, items, selected):
    model = Gtk.StringList.new(items)
    row = Adw.ComboRow(title=title, model=model, enable_search=True)
    row.set_expression(Gtk.PropertyExpression.new(Gtk.StringObject, None, "string"))
    if selected in items:
        row.set_selected(items.index(selected))
    return row


def combo_value(row):
    item = row.get_selected_item()
    return item.get_string() if item else ""


class Installer(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title=f"{CONF['name']} installieren", default_width=760,
                         default_height=640)
        self.disk = None
        self.proc = None
        self.toast = Adw.ToastOverlay()
        view = Adw.ToolbarView()
        header = Adw.HeaderBar()
        self.back_btn = Gtk.Button(label="Zurück")
        self.back_btn.connect("clicked", lambda *_: self.go(-1))
        self.next_btn = Gtk.Button(label="Weiter", css_classes=["suggested-action"])
        self.next_btn.connect("clicked", lambda *_: self.go(1))
        header.pack_start(self.back_btn)
        header.pack_end(self.next_btn)
        view.add_top_bar(header)
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)
        view.set_content(self.stack)
        self.toast.set_child(view)
        self.set_content(self.toast)

        self.pages = ["welcome", "region", "disk", "options", "user", "summary", "progress", "done"]
        self.stack.add_named(self.page_welcome(), "welcome")
        self.stack.add_named(self.page_region(), "region")
        self.stack.add_named(self.page_disk(), "disk")
        self.stack.add_named(self.page_options(), "options")
        self.stack.add_named(self.page_user(), "user")
        self.stack.add_named(self.page_summary(), "summary")
        self.stack.add_named(self.page_progress(), "progress")
        self.stack.add_named(self.page_done(), "done")
        self.index = 0
        self.update_nav()

    # ---------- Seiten ----------
    def page_welcome(self):
        page = Adw.StatusPage(title=f"Willkommen bei {CONF['name']}",
                              description=CONF.get("welcome") or "Dieser Assistent installiert das System.")
        if CONF.get("logo") and os.path.exists(CONF["logo"]):
            page.set_paintable(self._texture(CONF["logo"]))
        else:
            page.set_icon_name("system-software-install-symbolic")
        net = Gtk.Label(css_classes=["dim-label"])
        page.set_child(net)

        def check():
            ok = subprocess.run(["ping", "-c1", "-W2", "archlinux.org"], capture_output=True).returncode == 0
            GLib.idle_add(net.set_label, "✔ Internetverbindung vorhanden" if ok else
                          "Keine Internetverbindung – die Offline-Installation von der ISO funktioniert trotzdem.")
        threading.Thread(target=check, daemon=True).start()
        return page

    def _texture(self, path):
        from gi.repository import Gdk
        try:
            return Gdk.Texture.new_from_filename(path)
        except GLib.Error:
            return None

    def page_region(self):
        d = CONF["defaults"]
        page = Adw.PreferencesPage(title="Region")
        grp = Adw.PreferencesGroup(title="Sprache und Region",
                                   description="Diese Einstellungen gelten für das installierte System.")
        self.locale_row = combo("Sprache", list_locales(), d.get("locale"))
        self.tz_row = combo("Zeitzone", list_timezones(), d.get("timezone"))
        self.kb_row = combo("Tastaturlayout", list_layouts(), d.get("keyboard"))
        self.kb_row.connect("notify::selected", self.on_keyboard)
        for r in (self.locale_row, self.tz_row, self.kb_row):
            grp.add(r)
        page.add(grp)
        test = Adw.PreferencesGroup(title="Tastatur testen")
        test.add(Adw.EntryRow(title="Hier tippen …"))
        page.add(test)
        return page

    def on_keyboard(self, *_):
        layout = combo_value(self.kb_row)
        if layout and shutil.which("setxkbmap"):
            subprocess.Popen(["setxkbmap", layout], stderr=subprocess.DEVNULL, stdout=subprocess.DEVNULL)

    def page_disk(self):
        page = Adw.PreferencesPage(title="Festplatten")
        grp = Adw.PreferencesGroup(title="Systemfestplatte",
                                   description="⚠ Die gewählte Festplatte wird vollständig gelöscht!")
        self.disks = list_disks()
        self.extra_rows = {}
        first = None
        for d in self.disks:
            check = Gtk.CheckButton()
            if first:
                check.set_group(first)
            else:
                first = check
            row = Adw.ActionRow(title=f"{d.get('model') or 'Datenträger'} ({human(d['size'])})",
                                subtitle=f"{d['path']}  ·  {(d.get('tran') or '').upper()}")
            row.add_prefix(check)
            row.set_activatable_widget(check)
            check.connect("toggled", self.on_system_disk, d["path"])
            grp.add(row)
        page.add(grp)

        extra = Adw.PreferencesGroup(
            title="Weitere Festplatten",
            description="Optional: zusätzliche Platten als /home, als Datenlaufwerk (/data) oder als "
                        "btrfs-Spiegel (RAID 1) der Systemplatte nutzen. Genutzte Platten werden gelöscht.")
        for d in self.disks:
            row = Adw.ComboRow(title=f"{d.get('model') or 'Datenträger'} ({human(d['size'])})",
                               subtitle=d["path"], model=Gtk.StringList.new([label for _k, label in DISK_ROLES]))
            row.connect("notify::selected", lambda *_: self.update_disk_hint())
            extra.add(row)
            self.extra_rows[d["path"]] = row
        if len(self.disks) < 2:
            extra.set_visible(False)
        page.add(extra)
        self.disk_hint = Adw.PreferencesGroup()
        page.add(self.disk_hint)
        self.disk_hint_row = Adw.ActionRow(title="", css_classes=["property"])
        self.disk_hint.add(self.disk_hint_row)
        if not self.disks:
            grp.add(Adw.ActionRow(title="Keine geeignete Festplatte gefunden (mind. 8 GB)."))
        elif first:
            first.set_active(True)
        return page

    def on_system_disk(self, check, path):
        if not check.get_active():
            return
        self.disk = path
        for p, row in self.extra_rows.items():
            row.set_sensitive(p != path)
            if p == path:
                row.set_selected(0)
        self.update_disk_hint()

    def extra_disks(self):
        result = []
        for path, row in self.extra_rows.items():
            role = DISK_ROLES[row.get_selected()][0]
            if path != self.disk and role != "none":
                result.append({"path": path, "role": role})
        return result

    def update_disk_hint(self):
        if not hasattr(self, "disk_hint_row"):
            return
        n = len(self.extra_disks()) + (1 if self.disk else 0)
        self.disk_hint_row.set_title("Werden gelöscht")
        self.disk_hint_row.set_subtitle(", ".join([self.disk or "–"] + [e["path"] for e in self.extra_disks()])
                                        + f"  ({n} Festplatte{'n' if n != 1 else ''})")

    def page_options(self):
        d = CONF["defaults"]
        uefi = os.path.isdir("/sys/firmware/efi")
        page = Adw.PreferencesPage(title="Optionen")

        fsg = Adw.PreferencesGroup(title="Dateisystem und Speicher")
        self.fs_row = combo("Dateisystem", ["ext4", "btrfs"], d.get("filesystem", "ext4"))
        self.fs_row.set_subtitle("btrfs: Subvolumes, Kompression, Snapshots, RAID")
        self.swap_row = Adw.SpinRow.new_with_range(0, 128, 1)
        self.swap_row.set_title("Swap-Datei (GB)")
        self.swap_row.set_subtitle("0 = keine Swap-Datei" + (" (zram ist aktiv)" if CONF.get("zram") else ""))
        self.swap_row.set_value(d.get("swap_gb", 0))
        fsg.add(self.fs_row)
        fsg.add(self.swap_row)
        page.add(fsg)

        enc = Adw.PreferencesGroup(title="Verschlüsselung",
                                   description="Vollständige Festplattenverschlüsselung mit LUKS2. Beim Start "
                                               "wird einmal das Passwort abgefragt – auch für weitere Platten.")
        self.encrypt = Adw.SwitchRow(title="Festplatten verschlüsseln", active=d.get("encrypt", False))
        self.luks_same = Adw.SwitchRow(title="Benutzerpasswort verwenden", active=True)
        self.luks1 = Adw.PasswordEntryRow(title="Verschlüsselungspasswort")
        self.luks2 = Adw.PasswordEntryRow(title="Passwort wiederholen")
        for w in (self.encrypt, self.luks_same, self.luks1, self.luks2):
            enc.add(w)

        def sync(*_):
            on = self.encrypt.get_active()
            self.luks_same.set_visible(on)
            self.luks1.set_visible(on and not self.luks_same.get_active())
            self.luks2.set_visible(on and not self.luks_same.get_active())
        self.encrypt.connect("notify::active", sync)
        self.luks_same.connect("notify::active", sync)
        sync()
        page.add(enc)

        boot = Adw.PreferencesGroup(title="System")
        loaders = [("grub", "GRUB"), ("systemd-boot", "systemd-boot (schlank, nur UEFI)")]
        if not uefi:
            loaders = loaders[:1]
        self.loader_row = Adw.ComboRow(title="Bootloader", model=Gtk.StringList.new([l for _k, l in loaders]))
        self.loader_keys = [k for k, _l in loaders]
        if d.get("bootloader") in self.loader_keys:
            self.loader_row.set_selected(self.loader_keys.index(d["bootloader"]))
        boot.add(self.loader_row)
        boot.add(Adw.ActionRow(title="Startmodus", subtitle="UEFI" if uefi else "BIOS (Legacy)",
                               css_classes=["property"]))
        sources = []
        if has_offline_image():
            sources.append(("offline", "Von der ISO (offline, schnell)"))
        if CONF.get("allow_online", True) or not sources:
            sources.append(("online", "Aus dem Internet (neueste Pakete)"))
        self.source_keys = [k for k, _l in sources]
        self.source_row = Adw.ComboRow(title="Installationsquelle",
                                       model=Gtk.StringList.new([l for _k, l in sources]))
        if d.get("source") in self.source_keys:
            self.source_row.set_selected(self.source_keys.index(d["source"]))
        boot.add(self.source_row)
        page.add(boot)
        return page

    def page_user(self):
        page = Adw.PreferencesPage(title="Benutzer")
        grp = Adw.PreferencesGroup(title="Dein Benutzerkonto")
        self.fullname = Adw.EntryRow(title="Vollständiger Name")
        self.username = Adw.EntryRow(title="Benutzername")
        self.hostname = Adw.EntryRow(title="Computername", text=CONF.get("hostname", "computer"))
        self.pw1 = Adw.PasswordEntryRow(title="Passwort")
        self.pw2 = Adw.PasswordEntryRow(title="Passwort wiederholen")
        self.fullname.connect("changed", self.on_fullname)
        for r in (self.fullname, self.username, self.hostname, self.pw1, self.pw2):
            grp.add(r)
        page.add(grp)
        opt = Adw.PreferencesGroup(title="Optionen")
        self.root_same = Adw.SwitchRow(title="Gleiches Passwort für root",
                                       subtitle="Sonst wird das root-Konto gesperrt (sudo bleibt nutzbar)")
        self.autologin = Adw.SwitchRow(title="Automatisch anmelden")
        opt.add(self.root_same)
        if CONF.get("desktop") not in (None, "none"):
            opt.add(self.autologin)
        page.add(opt)
        return page

    def on_fullname(self, row):
        first = (row.get_text().split() or [""])[0].lower()
        suggestion = re.sub(r"[^a-z0-9_-]", "", first)
        if self.username.get_text() in ("", getattr(self, "_auto_user", "")):
            self.username.set_text(suggestion)
        self._auto_user = suggestion

    def page_summary(self):
        page = Adw.PreferencesPage(title="Zusammenfassung")
        self.summary = Adw.PreferencesGroup(title="Bereit zur Installation",
                                            description="Bitte prüfe die Einstellungen.")
        page.add(self.summary)
        self.summary_rows = []
        return page

    def fill_summary(self):
        for r in self.summary_rows:
            self.summary.remove(r)
        self.summary_rows = []
        s = self.selection()
        role_names = dict(DISK_ROLES)
        extras = ", ".join(f"{e['path']} → {role_names[e['role']]}" for e in s["extra_disks"]) or "keine"
        loader = {"grub": "GRUB", "systemd-boot": "systemd-boot"}[s["bootloader"]]
        for title, value in (("Systemfestplatte (wird gelöscht!)", s["disk"]),
                             ("Weitere Festplatten (werden gelöscht!)", extras),
                             ("Dateisystem", s["filesystem"] + (f", Swap {s['swap_gb']} GB" if s["swap_gb"] else "")),
                             ("Verschlüsselung", "LUKS2" if s["encrypt"] else "aus"),
                             ("Bootloader", loader),
                             ("Quelle", "ISO (offline)" if s["source"] == "offline" else "Internet"),
                             ("Sprache", s["locale"]), ("Zeitzone", s["timezone"]), ("Tastatur", s["keyboard"]),
                             ("Benutzer", f"{s['fullname']} ({s['username']})"), ("Computername", s["hostname"]),
                             ("Pakete", f"{len(CONF.get('packages', []))} Pakete/Gruppen")):
            row = Adw.ActionRow(title=title, subtitle=value or "–", css_classes=["property"])
            self.summary.add(row)
            self.summary_rows.append(row)

    def page_progress(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12, margin_top=24, margin_bottom=24,
                      margin_start=24, margin_end=24)
        title = Gtk.Label(label=f"{CONF['name']} wird installiert …", css_classes=["title-2"])
        self.status = Gtk.Label(label="Starte …", xalign=0, ellipsize=3)
        self.bar = Gtk.ProgressBar(show_text=True)
        self.log = Gtk.TextView(editable=False, monospace=True, cursor_visible=False,
                                wrap_mode=Gtk.WrapMode.WORD_CHAR)
        scroller = Gtk.ScrolledWindow(vexpand=True, child=self.log)
        expander = Gtk.Expander(label="Details anzeigen", child=scroller, vexpand=True)
        for w in (title, self.status, self.bar, expander):
            box.append(w)
        return box

    def page_done(self):
        self.done_page = Adw.StatusPage(icon_name="emblem-ok-symbolic", title="Installation abgeschlossen",
                                        description=f"{CONF['name']} wurde installiert. Entferne das "
                                                    "Installationsmedium und starte neu.")
        btn = Gtk.Button(label="Jetzt neu starten", halign=Gtk.Align.CENTER,
                         css_classes=["pill", "suggested-action"])
        btn.connect("clicked", lambda *_: subprocess.Popen(["systemctl", "reboot"]))
        self.done_page.set_child(btn)
        return self.done_page

    # ---------- Navigation ----------
    def selection(self):
        return {
            "disk": self.disk or "",
            "extra_disks": self.extra_disks(),
            "filesystem": combo_value(self.fs_row),
            "swap_gb": int(self.swap_row.get_value()),
            "encrypt": self.encrypt.get_active(),
            "luks_password": (self.pw1.get_text() if self.luks_same.get_active() else self.luks1.get_text())
            if self.encrypt.get_active() else "",
            "bootloader": self.loader_keys[self.loader_row.get_selected()],
            "source": self.source_keys[self.source_row.get_selected()],
            "locale": combo_value(self.locale_row),
            "timezone": combo_value(self.tz_row),
            "keyboard": combo_value(self.kb_row),
            "fullname": self.fullname.get_text().strip(),
            "username": self.username.get_text().strip(),
            "hostname": self.hostname.get_text().strip(),
            "password": self.pw1.get_text(),
            "root_same": self.root_same.get_active(),
            "autologin": self.autologin.get_active(),
        }

    def validate(self, name):
        s = self.selection()
        if name == "disk":
            if not s["disk"]:
                return "Bitte eine Festplatte wählen."
            roles = [e["role"] for e in s["extra_disks"]]
            if roles.count("home") > 1:
                return "Nur eine Festplatte kann /home sein."
        if name == "options":
            roles = [e["role"] for e in s["extra_disks"]]
            if "raid" in roles and s["filesystem"] != "btrfs":
                return "Der RAID-1-Spiegel benötigt btrfs als Dateisystem."
            if "raid" in roles and s["swap_gb"]:
                return "Eine Swap-Datei ist auf btrfs-RAID nicht möglich – bitte 0 wählen (zram nutzen)."
            if s["encrypt"] and not self.luks_same.get_active():
                if len(self.luks1.get_text()) < 8:
                    return "Das Verschlüsselungspasswort muss mindestens 8 Zeichen haben."
                if self.luks1.get_text() != self.luks2.get_text():
                    return "Die Verschlüsselungspasswörter stimmen nicht überein."
        if name == "user":
            if not re.match(r"^[a-z_][a-z0-9_-]{0,31}$", s["username"]):
                return "Benutzername: nur Kleinbuchstaben, Ziffern, '-' und '_'."
            if s["username"] in ("root", "live"):
                return "Dieser Benutzername ist reserviert."
            if not re.match(r"^[a-zA-Z0-9-]{1,63}$", s["hostname"]):
                return "Ungültiger Computername."
            if not s["password"]:
                return "Bitte ein Passwort eingeben."
            if s["password"] != self.pw2.get_text():
                return "Die Passwörter stimmen nicht überein."
            if s["encrypt"] and self.luks_same.get_active() and len(s["password"]) < 8:
                return "Für die Verschlüsselung muss das Passwort mindestens 8 Zeichen haben."
        return None

    def go(self, delta):
        name = self.pages[self.index]
        if delta > 0:
            err = self.validate(name)
            if err:
                self.toast.add_toast(Adw.Toast(title=err))
                return
            if name == "summary":
                self.confirm()
                return
        self.index = max(0, min(self.index + delta, len(self.pages) - 1))
        if self.pages[self.index] == "summary":
            self.fill_summary()
        self.update_nav()

    def update_nav(self):
        name = self.pages[self.index]
        self.stack.set_visible_child_name(name)
        busy = name in ("progress", "done")
        self.back_btn.set_visible(self.index > 0 and not busy)
        self.next_btn.set_visible(not busy)
        self.next_btn.set_label("Installieren" if name == "summary" else "Weiter")
        self.next_btn.set_css_classes(["destructive-action"] if name == "summary" else ["suggested-action"])

    def confirm(self):
        disks = ", ".join([self.disk] + [e["path"] for e in self.extra_disks()])
        dlg = Adw.AlertDialog(heading="Festplatten löschen?",
                              body=f"Alle Daten auf {disks} gehen unwiderruflich verloren.")
        dlg.add_response("cancel", "Abbrechen")
        dlg.add_response("install", "Löschen und installieren")
        dlg.set_response_appearance("install", Adw.ResponseAppearance.DESTRUCTIVE)
        dlg.connect("response", lambda _d, r: self.start() if r == "install" else None)
        dlg.present(self)

    # ---------- Installation ----------
    def start(self):
        self.index = self.pages.index("progress")
        self.update_nav()
        fd, path = tempfile.mkstemp(prefix="installer-", suffix=".json")
        with os.fdopen(fd, "w") as f:
            json.dump(self.selection(), f)
        cmd = [sys.executable, BACKEND, path]
        if os.geteuid() != 0:
            cmd = ["sudo", "-n"] + cmd
        threading.Thread(target=self.worker, args=(cmd, path), daemon=True).start()

    def worker(self, cmd, path):
        error = None
        try:
            self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            for line in self.proc.stdout:
                line = line.rstrip("\n")
                if line.startswith("##PROGRESS "):
                    _, frac, text = line.split(" ", 2)
                    GLib.idle_add(self.set_progress, float(frac), text)
                elif line.startswith("##ERROR## "):
                    error = line[10:]
                elif line != "##DONE##":
                    GLib.idle_add(self.append_log, line)
            if self.proc.wait() != 0 and not error:
                error = "Das Installationsprogramm wurde mit einem Fehler beendet."
        except OSError as e:
            error = str(e)
        finally:
            try:
                os.remove(path)
            except OSError:
                pass
        GLib.idle_add(self.finished, error)

    def set_progress(self, frac, text):
        self.bar.set_fraction(frac)
        self.status.set_label(text)

    def append_log(self, line):
        buf = self.log.get_buffer()
        buf.insert(buf.get_end_iter(), line + "\n")
        self.log.scroll_to_mark(buf.get_insert(), 0, False, 0, 0)

    def finished(self, error):
        if error:
            self.done_page.set_icon_name("dialog-error-symbolic")
            self.done_page.set_title("Installation fehlgeschlagen")
            self.done_page.set_description(error + "\n\nDetails stehen im Protokoll. Du kannst den Installer "
                                                   "neu starten.")
            btn = Gtk.Button(label="Protokoll anzeigen", halign=Gtk.Align.CENTER, css_classes=["pill"])
            btn.connect("clicked", lambda *_: self.stack.set_visible_child_name("progress"))
            self.done_page.set_child(btn)
        self.index = self.pages.index("done")
        self.update_nav()


CONF = load_conf()


class App(Adw.Application):
    def __init__(self):
        super().__init__(application_id="io.github.bonbon08.DistroInstaller", flags=Gio.ApplicationFlags.NON_UNIQUE)

    def do_activate(self):
        Installer(self).present()


if __name__ == "__main__":
    sys.exit(App().run(sys.argv[:1]))
