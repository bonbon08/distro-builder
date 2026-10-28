"""Distro Builder – GTK4/libadwaita-Oberfläche."""

import html
import os
import re
import shutil
import subprocess
import threading

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gdk", "4.0")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk  # noqa: E402

from . import __version__  # noqa: E402
from . import config as C  # noqa: E402
from . import fetch as F  # noqa: E402
from . import profile as P  # noqa: E402

APP_ID = "io.github.bonbon08.DistroBuilder"
STATE_DIR = os.path.join(GLib.get_user_config_dir(), "distro-builder")
AUTOSAVE = os.path.join(STATE_DIR, "autosave.json")

CSS = """
.ff-preview { background: #16161e; color: #c0caf5; border-radius: 12px; padding: 18px; }
.ff-preview label { font-family: monospace; font-size: 10.5pt; }
.log-view text { background: transparent; }
.log-view { font-family: monospace; font-size: 9pt; padding: 8px; }
"""

PAGES = [
    ("general", "Allgemein", "preferences-system-symbolic"),
    ("branding", "Branding", "applications-graphics-symbolic"),
    ("repos", "Repositories", "network-server-symbolic"),
    ("packages", "Pakete", "package-x-generic-symbolic"),
    ("extras", "Container & KI", "applications-science-symbolic"),
    ("apps", "Flatpak & Distrobox", "view-app-grid-symbolic"),
    ("installer", "Installer", "system-software-install-symbolic"),
    ("build", "Bauen", "media-optical-symbolic"),
]

LOGO_MODES = [
    ("image", "Bild – echte Pixel in Kitty/Konsole/WezTerm, sonst Zeichengrafik"),
    ("chafa", "Bild als Zeichengrafik (chafa) – überall gleich"),
    ("ascii", "Eigenes ASCII-Art"),
    ("builtin", "Arch-Standardlogo"),
]

BUILD_STAGES = [
    ("Installing packages to", 0.04, "Pakete werden installiert"),
    ("Creating a list of installed packages", 0.52, "Paketliste wird erstellt"),
    ("Preparing kernel and initrd", 0.56, "Kernel und Initramfs"),
    ("Creating SquashFS image", 0.60, "SquashFS-Abbild wird komprimiert (dauert)"),
    ("Creating checksum", 0.86, "Prüfsummen werden erstellt"),
    ("Setting up SYSLINUX", 0.88, "Bootloader wird eingerichtet"),
    ("Setting up systemd-boot", 0.89, "Bootloader wird eingerichtet"),
    ("Creating ISO image", 0.92, "ISO-Datei wird geschrieben"),
    ("Done!", 1.0, "Fertig"),
    # ARM-Image (build.sh aus profile.image_build_script)
    ("==> Lade das Arch-Linux-ARM-Basissystem", 0.02, "Basissystem wird geladen"),
    ("==> Erzeuge Image", 0.04, "Image wird angelegt"),
    ("==> Lade Flatpak-Apps", 0.05, "Flatpak-Apps werden geladen"),
    ("==> System wird im Image eingerichtet", 0.06, "System wird eingerichtet (Emulation, dauert)"),
    ("==> System wird konfiguriert", 0.80, "System wird konfiguriert"),
    ("==> Bootloader wird eingerichtet", 0.86, "Bootloader wird eingerichtet"),
    ("==> Komprimiere Image", 0.90, "Image wird komprimiert"),
]

IMAGE_SUFFIXES = (".iso", ".img", ".img.zst", ".img.xz")
ARM_TOOLS = ("arch-install-scripts", "qemu-user-static", "qemu-user-static-binfmt")


def arm_tools_ready():
    emulated = os.uname().machine == "aarch64" or os.path.exists("/proc/sys/fs/binfmt_misc/qemu-aarch64") \
        or os.path.exists("/usr/lib/binfmt.d/qemu-aarch64-static.conf")
    return bool(shutil.which("arch-chroot")) and emulated


def list_locales():
    try:
        with open("/usr/share/i18n/SUPPORTED", encoding="utf-8") as f:
            return [l.split()[0] for l in f if l.strip().endswith("UTF-8")]
    except OSError:
        return ["de_DE.UTF-8", "en_US.UTF-8"]


def cmd_lines(cmd, fallback):
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=10).stdout.split()
        return out or fallback
    except (OSError, subprocess.TimeoutExpired):
        return fallback


class BuilderWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Distro Builder", default_width=1120, default_height=780)
        self.cfg = self.load_autosave()
        self.project_path = None
        self.proc = None
        self.last_iso = None
        self._locales = list_locales()
        self._timezones = cmd_lines(["timedatectl", "list-timezones"], ["UTC", "Europe/Berlin"])
        self._layouts = cmd_lines(["localectl", "list-x11-keymap-layouts"], ["de", "us"])

        provider = Gtk.CssProvider()
        provider.load_from_string(CSS)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider,
                                                  Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        self.toasts = Adw.ToastOverlay()
        self.split = Adw.NavigationSplitView(min_sidebar_width=220, max_sidebar_width=280)
        self.toasts.set_child(self.split)
        self.set_content(self.toasts)

        # Seitenleiste
        side_view = Adw.ToolbarView()
        side_header = Adw.HeaderBar()
        side_header.set_title_widget(Adw.WindowTitle(title="Distro Builder", subtitle=f"Version {__version__}"))
        side_view.add_top_bar(side_header)
        self.sidebar = Gtk.ListBox(css_classes=["navigation-sidebar"])
        for key, label, icon in PAGES:
            row = Gtk.ListBoxRow(name=key)
            box = Gtk.Box(spacing=12, margin_top=8, margin_bottom=8, margin_start=6)
            box.append(Gtk.Image(icon_name=icon))
            box.append(Gtk.Label(label=label, xalign=0))
            row.set_child(box)
            self.sidebar.append(row)
        self.sidebar.connect("row-selected", self.on_nav)
        side_view.set_content(Gtk.ScrolledWindow(child=self.sidebar, vexpand=True))
        self.split.set_sidebar(Adw.NavigationPage(title="Distro Builder", child=side_view))

        # Inhalt
        content_view = Adw.ToolbarView()
        self.content_header = Adw.HeaderBar()
        self.content_title = Adw.WindowTitle(title="Allgemein")
        self.content_header.set_title_widget(self.content_title)
        menu = Gio.Menu()
        menu.append("Neues Projekt", "win.new")
        menu.append("Projekt öffnen …", "win.open")
        menu.append("Projekt speichern unter …", "win.save-as")
        self.content_header.pack_end(Gtk.MenuButton(icon_name="open-menu-symbolic", menu_model=menu,
                                                    tooltip_text="Projekt"))
        save_btn = Gtk.Button(icon_name="document-save-symbolic", tooltip_text="Projekt speichern (Strg+S)")
        save_btn.connect("clicked", lambda *_: self.save_project())
        self.content_header.pack_end(save_btn)
        content_view.add_top_bar(self.content_header)
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE)
        content_view.set_content(self.stack)
        self.content_page = Adw.NavigationPage(title="Allgemein", child=content_view)
        self.split.set_content(self.content_page)

        bp = Adw.Breakpoint.new(Adw.BreakpointCondition.parse("max-width: 700sp"))
        bp.add_setter(self.split, "collapsed", True)
        self.add_breakpoint(bp)

        for name, cb in (("new", self.new_project), ("open", self.open_project), ("save-as", self.save_as)):
            act = Gio.SimpleAction.new(name, None)
            act.connect("activate", lambda *_a, f=cb: f())
            self.add_action(act)
        app.set_accels_for_action("win.open", ["<Control>o"])
        save = Gio.SimpleAction.new("save", None)
        save.connect("activate", lambda *_: self.save_project())
        self.add_action(save)
        app.set_accels_for_action("win.save", ["<Control>s"])

        self.connect("close-request", self.on_close)
        self.rebuild()
        self.sidebar.select_row(self.sidebar.get_row_at_index(0))

    # ------------------------------------------------------------------ Projekt
    def load_autosave(self):
        try:
            return C.load(AUTOSAVE)
        except (OSError, ValueError):
            return C.default_config()

    def on_close(self, *_):
        os.makedirs(STATE_DIR, exist_ok=True)
        try:
            C.save(self.cfg, AUTOSAVE)
        except OSError:
            pass
        return False

    def rebuild(self):
        current = self.stack.get_visible_child_name()
        while (child := self.stack.get_first_child()) is not None:
            self.stack.remove(child)
        builders = {"general": self.page_general, "branding": self.page_branding, "repos": self.page_repos,
                    "packages": self.page_packages, "extras": self.page_extras, "apps": self.page_apps,
                    "installer": self.page_installer,
                    "build": self.page_build}
        for key, _label, _icon in PAGES:
            self.stack.add_named(builders[key](), key)
        if current:
            self.stack.set_visible_child_name(current)
        self.update_preview()
        self.update_pacman_preview()

    def new_project(self):
        self.cfg = C.default_config()
        self.project_path = None
        self.rebuild()
        self.toast("Neues Projekt erstellt")

    def open_project(self):
        dlg = Gtk.FileDialog(title="Projekt öffnen")
        flt = Gtk.FileFilter(name="Distro-Builder-Projekt (*.json)")
        flt.add_pattern("*.json")
        dlg.set_default_filter(flt)

        def done(d, res):
            try:
                f = d.open_finish(res)
            except GLib.Error:
                return
            try:
                self.cfg = C.load(f.get_path())
                self.project_path = f.get_path()
                self.rebuild()
                self.toast(f"Projekt geladen: {os.path.basename(self.project_path)}")
            except (OSError, ValueError) as e:
                self.error("Projekt konnte nicht geladen werden", str(e))
        dlg.open(self, None, done)

    def save_project(self):
        if not self.project_path:
            self.save_as()
            return
        C.save(self.cfg, self.project_path)
        self.toast("Projekt gespeichert")

    def save_as(self):
        dlg = Gtk.FileDialog(title="Projekt speichern", initial_name=f"{self.cfg['general']['id']}.json")

        def done(d, res):
            try:
                f = d.save_finish(res)
            except GLib.Error:
                return
            self.project_path = f.get_path()
            self.save_project()
        dlg.save(self, None, done)

    # ------------------------------------------------------------------ Helfer
    def toast(self, text):
        self.toasts.add_toast(Adw.Toast(title=text, timeout=3))

    def error(self, heading, body):
        dlg = Adw.AlertDialog(heading=heading, body=body)
        dlg.add_response("ok", "OK")
        dlg.present(self)

    def on_nav(self, _box, row):
        if not row:
            return
        key = row.get_name()
        label = next(l for k, l, _ in PAGES if k == key)
        self.stack.set_visible_child_name(key)
        self.content_title.set_title(label)
        self.content_page.set_title(label)
        self.split.set_show_content(True)
        if key == "build":
            self.refresh_build_info()

    def changed(self):
        self.update_preview()
        self.update_pacman_preview()
        self.update_package_count()

    def entry(self, grp, title, sec, key, subtitle=None, callback=None):
        row = Adw.EntryRow(title=title, text=str(self.cfg[sec][key]))
        if subtitle:
            row.set_tooltip_text(subtitle)

        def on_change(r):
            self.cfg[sec][key] = r.get_text()
            if callback:
                callback(r.get_text())
            self.changed()
        row.connect("changed", on_change)
        grp.add(row)
        return row

    def switch(self, grp, title, sec, key, subtitle=None):
        row = Adw.SwitchRow(title=title, active=bool(self.cfg[sec][key]))
        if subtitle:
            row.set_subtitle(subtitle)

        def on_change(r, _p):
            self.cfg[sec][key] = r.get_active()
            self.changed()
        row.connect("notify::active", on_change)
        grp.add(row)
        return row

    def combo(self, grp, title, options, sec, key, subtitle=None, search=False):
        """options: Liste von (Wert, Anzeigetext) oder Liste von Strings."""
        options = [(o, o) if isinstance(o, str) else o for o in options]
        model = Gtk.StringList.new([label for _v, label in options])
        row = Adw.ComboRow(title=title, model=model, enable_search=search)
        if search:
            row.set_expression(Gtk.PropertyExpression.new(Gtk.StringObject, None, "string"))
        if subtitle:
            row.set_subtitle(subtitle)
        values = [v for v, _l in options]
        if self.cfg[sec][key] in values:
            row.set_selected(values.index(self.cfg[sec][key]))

        def on_change(r, _p):
            idx = r.get_selected()
            if 0 <= idx < len(values):
                self.cfg[sec][key] = values[idx]
                self.changed()
        row.connect("notify::selected", on_change)
        grp.add(row)
        return row

    def textview(self, grp, sec, key, height=120, monospace=False):
        view = Gtk.TextView(monospace=monospace, wrap_mode=Gtk.WrapMode.WORD_CHAR, top_margin=8,
                            bottom_margin=8, left_margin=10, right_margin=10)
        view.get_buffer().set_text(self.cfg[sec][key])

        def on_change(buf):
            self.cfg[sec][key] = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)
            self.changed()
        view.get_buffer().connect("changed", on_change)
        frame = Gtk.Frame(child=Gtk.ScrolledWindow(child=view, min_content_height=height))
        grp.add(frame)
        return view

    def file_row(self, grp, title, sec, key, subtitle):
        row = Adw.ActionRow(title=title)
        thumb = Gtk.Picture(content_fit=Gtk.ContentFit.CONTAIN, width_request=56, height_request=40,
                            can_shrink=True)
        clear = Gtk.Button(icon_name="edit-clear-symbolic", valign=Gtk.Align.CENTER, css_classes=["flat"],
                           tooltip_text="Entfernen")
        pick = Gtk.Button(label="Wählen …", valign=Gtk.Align.CENTER)

        def refresh():
            path = self.cfg[sec][key]
            row.set_subtitle(html.escape(path) if path else subtitle)
            if path and os.path.isfile(path):
                thumb.set_filename(path)
            else:
                thumb.set_paintable(None)
            clear.set_visible(bool(path))

        def on_pick(*_):
            dlg = Gtk.FileDialog(title=title)
            flt = Gtk.FileFilter(name="Bilder")
            flt.add_pixbuf_formats()
            dlg.set_default_filter(flt)

            def done(d, res):
                try:
                    f = d.open_finish(res)
                except GLib.Error:
                    return
                self.cfg[sec][key] = f.get_path()
                refresh()
                self.changed()
            dlg.open(self, None, done)

        def on_clear(*_):
            self.cfg[sec][key] = ""
            refresh()
            self.changed()

        pick.connect("clicked", on_pick)
        clear.connect("clicked", on_clear)
        row.add_prefix(thumb)
        row.add_suffix(clear)
        row.add_suffix(pick)
        refresh()
        grp.add(row)
        return row

    def on_arch_changed(self):
        # Kernel und Grafiktreiber an die Plattform anpassen
        self.cfg["packages"]["kernel"] = C.kernel(self.cfg)
        if self.cfg["packages"].get("gpu") not in C.gpus(self.cfg):
            self.cfg["packages"]["gpu"] = "mesa"
        if C.is_image(self.cfg) and self.cfg["ai"]["accel"] in ("cuda", "rocm"):
            self.cfg["ai"]["accel"] = "cpu"
        self.rebuild()
        self.toast(C.ARCHES[C.arch(self.cfg)][1])
        return False

    # ------------------------------------------------------------------ Seiten
    def page_general(self):
        page = Adw.PreferencesPage()
        grp = Adw.PreferencesGroup(title="Deine Distribution",
                                   description="Diese Angaben landen in /etc/os-release, im Bootmenü, in "
                                               "fastfetch/neofetch und im Installer.")
        page.add(grp)
        self._id_row = None

        def name_changed(text):
            if self._id_row and self._id_row.get_text() in ("", getattr(self, "_auto_id", "")):
                self._auto_id = C.slugify(text)
                self._id_row.set_text(self._auto_id)
        self._auto_id = self.cfg["general"]["id"] if self.cfg["general"]["id"] == C.slugify(
            self.cfg["general"]["name"]) else ""
        self.entry(grp, "Name der Distro", "general", "name", callback=name_changed)
        self._id_row = self.entry(grp, "ID (kleingeschrieben, für Dateinamen)", "general", "id")
        self.entry(grp, "Version", "general", "version")
        self.entry(grp, "Standard-Hostname", "general", "hostname")

        plat = Adw.PreferencesGroup(title="Zielplattform",
                                    description="x86_64 ergibt eine ISO mit Live-System und Installer. Für ARM und "
                                                "den Raspberry Pi entsteht ein fertig installiertes Disk-Image "
                                                "(Arch Linux ARM), das du direkt auf SD-Karte oder USB-Stick schreibst.")
        page.add(plat)
        row = self.combo(plat, "Architektur", [(k, v[0]) for k, v in C.ARCHES.items()], "build", "arch",
                         subtitle=C.ARCHES[C.arch(self.cfg)][1])
        row.connect("notify::selected", lambda *_: GLib.idle_add(self.on_arch_changed))

        grp2 = Adw.PreferencesGroup(title="Herausgeber &amp; Links", description="Optional")
        page.add(grp2)
        self.entry(grp2, "Herausgeber", "general", "publisher")
        self.entry(grp2, "Webseite (HOME_URL)", "general", "homepage")
        self.entry(grp2, "Support-URL", "general", "support_url")
        self.entry(grp2, "Fehlerberichte-URL", "general", "bug_url")
        return page

    def page_branding(self):
        page = Adw.PreferencesPage()

        logo = Adw.PreferencesGroup(title="Logo",
                                    description="Wird in fastfetch/neofetch, im Installer und als Systemlogo "
                                                "(os-release LOGO) verwendet.")
        page.add(logo)
        self.file_row(logo, "Logo-Bild", "branding", "logo", "PNG, JPG oder SVG – quadratisch empfohlen")
        self.combo(logo, "Logo in fastfetch / neofetch", LOGO_MODES, "branding", "logo_mode")

        color_row = Adw.ActionRow(title="Akzentfarbe", subtitle="ANSI_COLOR und Farbe der fastfetch-Schlüssel")
        rgba = Gdk.RGBA()
        rgba.parse(self.cfg["branding"]["color"])
        btn = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False), rgba=rgba, valign=Gtk.Align.CENTER)

        def color_changed(b, _p):
            c = b.get_rgba()
            self.cfg["branding"]["color"] = "#{:02x}{:02x}{:02x}".format(
                int(c.red * 255), int(c.green * 255), int(c.blue * 255))
            self.changed()
        btn.connect("notify::rgba", color_changed)
        color_row.add_suffix(btn)
        logo.add(color_row)

        ascii_grp = Adw.PreferencesGroup(title="ASCII-Logo",
                                         description="Wird für den Modus „Eigenes ASCII-Art“ genutzt. Tipp: "
                                                     "Mit 'figlet' oder 'toilet' erzeugen.")
        page.add(ascii_grp)
        self.textview(ascii_grp, "branding", "ascii_logo", height=140, monospace=True)

        prev = Adw.PreferencesGroup(title="Vorschau", description="So ungefähr sieht fastfetch aus")
        page.add(prev)
        box = Gtk.Box(spacing=24, css_classes=["ff-preview"])
        self.prev_pic = Gtk.Picture(content_fit=Gtk.ContentFit.CONTAIN, width_request=160, height_request=160,
                                    valign=Gtk.Align.START)
        self.prev_ascii = Gtk.Label(xalign=0, yalign=0, valign=Gtk.Align.START)
        self.prev_info = Gtk.Label(xalign=0, yalign=0, valign=Gtk.Align.START, selectable=True)
        box.append(self.prev_pic)
        box.append(self.prev_ascii)
        box.append(self.prev_info)
        prev.add(box)

        wall = Adw.PreferencesGroup(title="Hintergrundbild",
                                    description="Wird beim ersten Login gesetzt (KDE, GNOME, Xfce, Cinnamon, LXQt) "
                                                "und als Bootmenü-Hintergrund (BIOS) genutzt.")
        page.add(wall)
        self.file_row(wall, "Hintergrundbild", "branding", "wallpaper", "Am besten 1920×1080 oder größer")

        term = Adw.PreferencesGroup(title="Terminal")
        page.add(term)
        self.switch(term, "fastfetch beim Öffnen eines Terminals anzeigen", "branding", "fastfetch_on_terminal",
                    "'neofetch' ist zusätzlich als Alias für fastfetch verfügbar")
        motd = Adw.PreferencesGroup(title="Begrüßungstext (/etc/motd)",
                                    description="Wird nach dem Konsolen-Login angezeigt")
        page.add(motd)
        self.textview(motd, "branding", "motd", height=70)
        return page

    def update_preview(self):
        if not hasattr(self, "prev_info"):
            return
        b, g = self.cfg["branding"], self.cfg["general"]
        color = b["color"]
        mode = b["logo_mode"]
        show_img = mode in ("image", "chafa") and b["logo"] and os.path.isfile(b["logo"])
        self.prev_pic.set_visible(bool(show_img))
        if show_img:
            self.prev_pic.set_filename(b["logo"])
        ascii_text = b["ascii_logo"] if mode == "ascii" else ARCH_ASCII if not show_img else ""
        self.prev_ascii.set_visible(bool(ascii_text))
        ascii_color = color if mode == "ascii" else "#1793d1"
        self.prev_ascii.set_markup(f'<span foreground="{ascii_color}">{html.escape(ascii_text)}</span>')
        desktop = C.desktop_info(self.cfg)[0]
        kernel = dict(C.KERNELS).get(self.cfg["packages"]["kernel"], "linux")
        host = html.escape(g["hostname"] or "host")
        lines = [
            f'<span foreground="{color}"><b>user</b></span>@<span foreground="{color}"><b>{host}</b></span>',
            "-" * (5 + len(g["hostname"] or "host")),
        ]
        for k, v in (("OS", f"{g['name']} {g['version']} x86_64"), ("Kernel", f"{kernel} 6.x"),
                     ("Packages", f"{len(C.target_packages(self.cfg))}+ (pacman)"), ("Shell", "bash 5.3"),
                     ("DE", desktop), ("Terminal", "konsole"), ("CPU", "AMD Ryzen 7 (16)"),
                     ("Memory", "2.1 GiB / 15.5 GiB")):
            lines.append(f'<span foreground="{color}"><b>{k}</b></span>: {html.escape(v)}')
        palette = "".join(f'<span background="{c}">   </span>' for c in
                          ("#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6"))
        lines += ["", palette]
        self.prev_info.set_markup("\n".join(lines))

    def page_repos(self):
        page = Adw.PreferencesPage()
        off = Adw.PreferencesGroup(title="Offizielle Arch-Repositories")
        page.add(off)
        core = self.switch(off, "core", "repos", "core", "Basissystem – immer aktiv")
        core.set_sensitive(False)
        self.switch(off, "extra", "repos", "extra", "Desktop, Programme – empfohlen")
        self.switch(off, "multilib", "repos", "multilib", "32-Bit-Bibliotheken (z. B. Steam, Wine)")
        self.switch(off, "core-testing", "repos", "core_testing", "⚠ Testpakete")
        self.switch(off, "extra-testing", "repos", "extra_testing", "⚠ Testpakete")

        mir = Adw.PreferencesGroup(title="Spiegelserver",
                                   description="Leer lassen, um /etc/pacman.d/mirrorlist zu nutzen. "
                                               "Sonst z. B. https://mirror.example.org/archlinux/$repo/os/$arch")
        page.add(mir)
        self.entry(mir, "Eigener Mirror (optional)", "repos", "mirror")

        self.custom_grp = Adw.PreferencesGroup(
            title="Eigene Repositories",
            description="z. B. dein eigenes Repo mit selbstgebauten Paketen. Server-URL darf $repo und $arch "
                        "enthalten. Ohne importierten Schlüssel „Optional TrustAll“ verwenden.")
        add = Gtk.Button(icon_name="list-add-symbolic", css_classes=["flat"], tooltip_text="Repository hinzufügen")
        add.connect("clicked", lambda *_: self.repo_dialog())
        self.custom_grp.set_header_suffix(add)
        page.add(self.custom_grp)
        self.custom_rows = []
        self.refresh_custom_repos()

        prev = Adw.PreferencesGroup(title="Erzeugte pacman.conf")
        page.add(prev)
        self.pacman_label = Gtk.Label(xalign=0, selectable=True, wrap=True, css_classes=["log-view"])
        prev.add(Gtk.Frame(child=self.pacman_label))
        return page

    def refresh_custom_repos(self):
        for r in self.custom_rows:
            self.custom_grp.remove(r)
        self.custom_rows = []
        repos = self.cfg["repos"]["custom"]
        if not repos:
            row = Adw.ActionRow(title="Keine eigenen Repositories", subtitle="Mit + hinzufügen")
            self.custom_grp.add(row)
            self.custom_rows.append(row)
        for i, repo in enumerate(repos):
            row = Adw.ActionRow(title=html.escape(f"[{repo['name']}]"),
                                subtitle=html.escape(f"{repo['server']}  ·  {repo.get('siglevel')}"
                                                     + ("  ·  vor offiziellen" if repo.get("first") else "")))
            edit = Gtk.Button(icon_name="document-edit-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            edit.connect("clicked", lambda *_a, idx=i: self.repo_dialog(idx))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, idx=i: self.remove_repo(idx))
            row.add_suffix(edit)
            row.add_suffix(rm)
            self.custom_grp.add(row)
            self.custom_rows.append(row)

    def remove_repo(self, idx):
        del self.cfg["repos"]["custom"][idx]
        self.refresh_custom_repos()
        self.changed()

    def repo_dialog(self, idx=None):
        repo = dict(self.cfg["repos"]["custom"][idx]) if idx is not None else \
            {"name": "", "server": "", "siglevel": "Optional TrustAll", "first": False}
        dlg = Adw.AlertDialog(heading="Repository bearbeiten" if idx is not None else "Repository hinzufügen")
        lst = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE, width_request=420)
        name = Adw.EntryRow(title="Name (z. B. meinrepo)", text=repo["name"])
        server = Adw.EntryRow(title="Server-URL", text=repo["server"])
        sig = Adw.ComboRow(title="SigLevel", model=Gtk.StringList.new(C.SIGLEVELS))
        if repo.get("siglevel") in C.SIGLEVELS:
            sig.set_selected(C.SIGLEVELS.index(repo["siglevel"]))
        first = Adw.SwitchRow(title="Vor den offiziellen Repos", subtitle="Pakete überschreiben Arch-Pakete",
                              active=repo.get("first", False))
        for w in (name, server, sig, first):
            lst.append(w)
        dlg.set_extra_child(lst)
        dlg.add_response("cancel", "Abbrechen")
        dlg.add_response("ok", "Speichern")
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)

        def resp(_d, r):
            if r != "ok":
                return
            new = {"name": name.get_text().strip(), "server": server.get_text().strip(),
                   "siglevel": C.SIGLEVELS[sig.get_selected()], "first": first.get_active()}
            if not re.match(r"^[a-zA-Z0-9_-]+$", new["name"]) or not new["server"]:
                self.toast("Name (a-z, 0-9, -, _) und Server-URL sind erforderlich")
                return
            if idx is None:
                self.cfg["repos"]["custom"].append(new)
            else:
                self.cfg["repos"]["custom"][idx] = new
            self.refresh_custom_repos()
            self.changed()
        dlg.connect("response", resp)
        dlg.present(self)

    def update_pacman_preview(self):
        if hasattr(self, "pacman_label"):
            text = "\n".join(l for l in P.pacman_conf(self.cfg).splitlines() if not l.startswith("#"))
            self.pacman_label.set_text(text.strip())

    def page_packages(self):
        page = Adw.PreferencesPage()
        base = Adw.PreferencesGroup(title="Grundsystem")
        page.add(base)
        self.combo(base, "Kernel", C.kernels(self.cfg), "packages", "kernel")
        self.combo(base, "Desktop-Umgebung", [(k, v[0]) for k, v in C.DESKTOPS.items()], "packages", "desktop",
                   subtitle="Inklusive Display-Manager und automatischer Anmeldung im Live-System")
        self.combo(base, "Grafiktreiber", [(k, v[0]) for k, v in C.gpus(self.cfg).items()], "packages", "gpu",
                   subtitle="NVIDIA nutzt DKMS – der Build dauert dadurch etwas länger")
        if not C.is_image(self.cfg):
            self.switch(base, "Arch-Rettungswerkzeuge im Live-System", "packages", "rescue_tools",
                        "Alle Tools des offiziellen Arch-ISOs (größer). Aus = schlankes Live-System")
        self.pkg_count = Adw.ActionRow(title="Paketanzahl", css_classes=["property"])
        base.add(self.pkg_count)

        pre = Adw.PreferencesGroup(title="Paket-Sammlungen")
        page.add(pre)
        for key, (label, desc, pkgs, _svc) in C.PRESETS.items():
            row = Adw.SwitchRow(title=label, subtitle=f"{desc} – {', '.join(pkgs)}",
                                active=key in self.cfg["packages"]["presets"])

            def toggled(r, _p, k=key):
                presets = self.cfg["packages"]["presets"]
                if r.get_active() and k not in presets:
                    presets.append(k)
                elif not r.get_active() and k in presets:
                    presets.remove(k)
                self.changed()
            row.connect("notify::active", toggled)
            pre.add(row)

        custom = Adw.PreferencesGroup(title="Eigene Pakete",
                                      description="Paketnamen durch Leerzeichen oder Zeilen getrennt. "
                                                  "Pakete aus eigenen Repos sind ebenfalls möglich.")
        check = Gtk.Button(label="Prüfen", css_classes=["flat"])
        check.connect("clicked", self.on_check_packages)
        custom.set_header_suffix(check)
        page.add(custom)
        self.custom_view = self.textview(custom, "packages", "custom", height=100, monospace=True)

        search = Adw.PreferencesGroup(title="Paketsuche", description="Durchsucht die lokale pacman-Datenbank")
        page.add(search)
        self.search_entry = Gtk.SearchEntry(placeholder_text="z. B. kdenlive, neovim, obs …",
                                            margin_bottom=6)
        self.search_entry.connect("search-changed", self.on_search)
        search.add(self.search_entry)
        self.search_list = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
        self.search_list.set_visible(False)
        search.add(self.search_list)
        self.update_package_count()
        return page

    def update_package_count(self):
        if hasattr(self, "pkg_count"):
            self.pkg_count.set_subtitle(f"{len(C.target_packages(self.cfg))} Pakete/Gruppen für das installierte "
                                        "System (ohne Abhängigkeiten)")

    def on_check_packages(self, *_):
        names = C.custom_packages(self.cfg)
        if not names:
            self.toast("Keine eigenen Pakete eingetragen")
            return

        def work():
            missing = C.check_packages(names)
            GLib.idle_add(done, missing)

        def done(missing):
            note = ("\n\nHinweis: Geprüft wird gegen die x86_64-Repos dieses Rechners. Für ARM können "
                    "Pakete fehlen oder anders heißen.") if C.is_image(self.cfg) else ""
            if missing:
                self.error("Unbekannte Pakete",
                           "Folgende Pakete wurden in den lokal bekannten Repos nicht gefunden "
                           "(bei eigenen Repos ist das normal):\n\n" + ", ".join(missing) + note)
            else:
                self.toast(f"Alle {len(names)} Pakete gefunden ✔")
        threading.Thread(target=work, daemon=True).start()

    def on_search(self, entry):
        query = entry.get_text().strip()
        if hasattr(self, "_search_timer") and self._search_timer:
            GLib.source_remove(self._search_timer)
        self._search_timer = GLib.timeout_add(300, self._run_search, query)

    def _run_search(self, query):
        self._search_timer = None
        if len(query) < 2:
            self.search_list.remove_all()
            self.search_list.set_visible(False)
            return False

        def work():
            res = C.search_packages(query, limit=40)
            GLib.idle_add(self._show_results, query, res)
        threading.Thread(target=work, daemon=True).start()
        return False

    def _show_results(self, query, results):
        if self.search_entry.get_text().strip() != query:
            return
        self.search_list.remove_all()
        self.search_list.set_visible(True)
        if not results:
            self.search_list.append(Adw.ActionRow(title="Keine Treffer"))
        current = set(C.custom_packages(self.cfg))
        for r in results:
            row = Adw.ActionRow(title=html.escape(r["name"]),
                                subtitle=html.escape(f"{r['repo']} · {r['version']} — {r['desc']}"),
                                subtitle_lines=2)
            btn = Gtk.Button(icon_name="emblem-ok-symbolic" if r["name"] in current else "list-add-symbolic",
                             valign=Gtk.Align.CENTER, css_classes=["flat"], tooltip_text="Hinzufügen")
            btn.connect("clicked", lambda b, n=r["name"]: self.add_package(n, b))
            row.add_suffix(btn)
            self.search_list.append(row)

    def add_package(self, name, btn):
        if name in C.custom_packages(self.cfg):
            return
        buf = self.custom_view.get_buffer()
        text = self.cfg["packages"]["custom"].rstrip()
        buf.set_text((text + "\n" if text else "") + name)
        btn.set_icon_name("emblem-ok-symbolic")
        self.toast(f"{name} hinzugefügt")

    def page_extras(self):
        page = Adw.PreferencesPage()

        # ---- KI
        ai = Adw.PreferencesGroup(title="KI mit Ollama",
                                  description="Lokale Sprachmodelle (Llama, Qwen, Gemma, Mistral …). Ausgewählte "
                                              "Modelle werden beim Bauen heruntergeladen und in die ISO gepackt – "
                                              "sofort offline nutzbar: 'ollama run llama3.2:3b'.")
        page.add(ai)
        self.ollama_row = self.switch(ai, "Ollama installieren", "ai", "ollama",
                                      "Dienst wird automatisch gestartet (Port 11434)")
        accel = [(k, v[0]) for k, v in C.OLLAMA_ACCEL.items()
                 if not (C.is_image(self.cfg) and k in ("cuda", "rocm"))]
        self.combo(ai, "Beschleunigung", accel, "ai", "accel", subtitle="CUDA/ROCm: mehrere GB groß, nur x86_64")
        self.switch(ai, "Im Netzwerk freigeben", "ai", "expose", "OLLAMA_HOST=0.0.0.0 – andere Geräte dürfen zugreifen")
        self.switch(ai, "KI-Entwicklungsumgebung", "ai", "dev",
                    "PyTorch (passend zur Beschleunigung), Jupyter, NumPy, pandas, scikit-learn, huggingface-hub")

        models = Adw.PreferencesGroup(title="Modelle vorinstallieren",
                                      description="Größen sind ungefähre Downloadgrößen. Eigene Modelle: Name aus "
                                                  "ollama.com/library oder hf.co/&lt;user&gt;/&lt;repo&gt;:&lt;quant&gt;.")
        size_btn = Gtk.Button(label="Größe prüfen", css_classes=["flat"])
        size_btn.connect("clicked", self.on_model_sizes)
        models.set_header_suffix(size_btn)
        page.add(models)
        preset_names = {m for m, _d, _s in C.MODEL_PRESETS}
        for name, desc, size in C.MODEL_PRESETS:
            row = Adw.SwitchRow(title=name, subtitle=f"{desc} · ~{size:.1f} GB",
                                active=name in self.cfg["ai"]["models"])
            row.connect("notify::active", lambda r, _p, n=name: self.toggle_model(n, r.get_active()))
            models.add(row)
        self.custom_models_grp = Adw.PreferencesGroup(title="Eigene Modelle")
        page.add(self.custom_models_grp)
        add = Adw.EntryRow(title="Modell hinzufügen (z. B. qwen2.5-coder:7b)", show_apply_button=True)

        def on_apply(r):
            name = r.get_text().strip()
            if not C.MODEL_RE.match(name):
                self.toast("Ungültiger Modellname")
                return
            if name not in self.cfg["ai"]["models"]:
                self.cfg["ai"]["models"].append(name)
            r.set_text("")
            self.refresh_custom_models(preset_names)
            self.changed()
        add.connect("apply", on_apply)
        self.custom_models_grp.add(add)
        self.custom_model_rows = []
        self.refresh_custom_models(preset_names)
        self.model_total = Adw.ActionRow(title="Modelle gesamt", css_classes=["property"])
        self.custom_models_grp.add(self.model_total)
        self.update_model_total()

        # ---- Container
        ct = Adw.PreferencesGroup(title="Container",
                                  description="Docker oder Podman vorinstallieren. Images können direkt in die ISO "
                                              "gepackt werden und werden beim ersten Start importiert.")
        page.add(ct)
        self.combo(ct, "Container-Engine", [(k, v[0]) for k, v in C.CONTAINER_ENGINES.items()],
                   "containers", "engine")
        self.switch(ct, "Images beim Bauen in die ISO packen", "containers", "preload",
                    "Sonst werden sie beim ersten Start aus dem Internet geladen")
        imgs = Adw.PreferencesGroup(title="Zusätzliche Images", description="Ein Image pro Zeile, z. B. nginx:alpine, "
                                                                           "postgres:17, ghcr.io/…")
        page.add(imgs)
        self.textview(imgs, "containers", "images", height=70, monospace=True)

        self.stacks_grp = Adw.PreferencesGroup(
            title="Container-Apps (Compose-Stacks)",
            description="Werden als systemd-Dienst gestartet. Images aus den Stacks werden automatisch vorgeladen.")
        menu = Gio.Menu()
        for key, (title, _desc, _y) in C.STACK_PRESETS.items():
            menu.append(title, f"win.stack-preset::{key}")
        menu.append("Eigener Stack …", "win.stack-preset::custom")
        self.stacks_grp.set_header_suffix(Gtk.MenuButton(icon_name="list-add-symbolic", menu_model=menu,
                                                         css_classes=["flat"], tooltip_text="Stack hinzufügen"))
        if not self.lookup_action("stack-preset"):
            act = Gio.SimpleAction.new("stack-preset", GLib.VariantType.new("s"))
            act.connect("activate", lambda _a, v: self.add_stack_preset(v.get_string()))
            self.add_action(act)
        page.add(self.stacks_grp)
        self.stack_rows = []
        self.refresh_stacks()
        return page

    def toggle_model(self, name, active):
        models = self.cfg["ai"]["models"]
        if active and name not in models:
            models.append(name)
            if not self.cfg["ai"]["ollama"]:
                self.ollama_row.set_active(True)
                self.toast("Ollama wurde aktiviert")
        elif not active and name in models:
            models.remove(name)
        self.update_model_total()
        self.changed()

    def refresh_custom_models(self, preset_names):
        for r in self.custom_model_rows:
            self.custom_models_grp.remove(r)
        self.custom_model_rows = []
        for name in [m for m in self.cfg["ai"]["models"] if m not in preset_names]:
            row = Adw.ActionRow(title=html.escape(name))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, n=name: (self.cfg["ai"]["models"].remove(n),
                                                       self.refresh_custom_models(preset_names),
                                                       self.update_model_total()))
            row.add_suffix(rm)
            self.custom_models_grp.add(row)
            self.custom_model_rows.append(row)

    def update_model_total(self, exact=None):
        if not hasattr(self, "model_total"):
            return
        known = {m: sz for m, _d, sz in C.MODEL_PRESETS}
        models = self.cfg["ai"]["models"]
        if exact is not None:
            self.model_total.set_subtitle(f"{len(models)} Modell(e), {exact / 1024**3:.2f} GB (geprüft)")
            return
        est = sum(known.get(m, 0) for m in models)
        unknown = sum(1 for m in models if m not in known)
        extra = f" + {unknown} unbekannte" if unknown else ""
        self.model_total.set_subtitle(f"{len(models)} Modell(e), ca. {est:.1f} GB{extra}"
                                      + ("  ⚠ ISO wird groß" if est > 8 else ""))

    def on_model_sizes(self, *_):
        if not self.cfg["ai"]["models"]:
            self.toast("Keine Modelle ausgewählt")
            return
        self.model_total.set_subtitle("Prüfe …")

        def work():
            total, errors = F.estimate_sizes(self.cfg, lambda *_a: None)
            GLib.idle_add(done, total, errors)

        def done(total, errors):
            self.update_model_total(exact=total)
            if errors:
                self.error("Modelle nicht gefunden", "\n".join(errors))
        threading.Thread(target=work, daemon=True).start()

    def refresh_stacks(self):
        for r in self.stack_rows:
            self.stacks_grp.remove(r)
        self.stack_rows = []
        stacks = self.cfg["containers"]["stacks"]
        if not stacks:
            row = Adw.ActionRow(title="Keine Container-Apps", subtitle="Mit + eine Vorlage (Open WebUI, Portainer …) "
                                                                      "oder einen eigenen Stack hinzufügen")
            self.stacks_grp.add(row)
            self.stack_rows.append(row)
        for i, st in enumerate(stacks):
            images = re.findall(r"^\s*image:\s*[\"']?([^\s\"']+)", st["compose"], flags=re.M)
            row = Adw.ActionRow(title=html.escape(st.get("title") or st["name"]),
                                subtitle=html.escape(", ".join(images) or "kein Image"))
            auto = Gtk.Switch(active=st.get("autostart", True), valign=Gtk.Align.CENTER,
                              tooltip_text="Beim Systemstart starten")
            auto.connect("notify::active", lambda w, _p, idx=i: self.cfg["containers"]["stacks"][idx].__setitem__(
                "autostart", w.get_active()))
            edit = Gtk.Button(icon_name="document-edit-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            edit.connect("clicked", lambda *_a, idx=i: self.stack_dialog(idx))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, idx=i: (self.cfg["containers"]["stacks"].pop(idx),
                                                      self.refresh_stacks(), self.changed()))
            for w in (auto, edit, rm):
                row.add_suffix(w)
            self.stacks_grp.add(row)
            self.stack_rows.append(row)

    def add_stack_preset(self, key):
        if self.cfg["containers"]["engine"] == "none":
            self.cfg["containers"]["engine"] = "docker"
            self.rebuild()
            self.toast("Docker wurde aktiviert")
        if key == "custom":
            self.stack_dialog()
            return
        title, desc, compose = C.STACK_PRESETS[key]
        if any(st["name"] == key for st in self.cfg["containers"]["stacks"]):
            self.toast(f"{title} ist bereits vorhanden")
            return
        url = (re.findall(r"https?://\S+", desc) or [""])[0]
        self.cfg["containers"]["stacks"].append({"name": key, "title": title, "compose": compose,
                                                 "autostart": True, "url": url})
        if key == "open-webui" and not self.cfg["ai"]["ollama"]:
            self.toast("Tipp: Open WebUI braucht Ollama – unter „KI mit Ollama“ aktivieren")
        self.refresh_stacks()
        self.changed()

    def stack_dialog(self, idx=None):
        st = dict(self.cfg["containers"]["stacks"][idx]) if idx is not None else \
            {"name": "", "title": "", "compose": "services:\n  app:\n    image: \n    restart: unless-stopped\n",
             "autostart": True, "url": ""}
        dlg = Adw.AlertDialog(heading="Container-App bearbeiten" if idx is not None else "Eigene Container-App")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12, width_request=560)
        lst = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE)
        name = Adw.EntryRow(title="Name (a-z, 0-9, -)", text=st["name"])
        url = Adw.EntryRow(title="Web-Adresse für Menüeintrag (optional)", text=st.get("url", ""))
        for w in (name, url):
            lst.append(w)
        box.append(lst)
        view = Gtk.TextView(monospace=True, top_margin=8, bottom_margin=8, left_margin=10, right_margin=10)
        view.get_buffer().set_text(st["compose"])
        box.append(Gtk.Label(label="compose.yaml", xalign=0, css_classes=["heading"]))
        box.append(Gtk.Frame(child=Gtk.ScrolledWindow(child=view, min_content_height=260)))
        dlg.set_extra_child(box)
        dlg.add_response("cancel", "Abbrechen")
        dlg.add_response("ok", "Speichern")
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)

        def resp(_d, r):
            if r != "ok":
                return
            buf = view.get_buffer()
            new = {**st, "name": name.get_text().strip(), "url": url.get_text().strip(),
                   "compose": buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)}
            new["title"] = st.get("title") or new["name"]
            if not C.STACK_RE.match(new["name"]):
                self.toast("Ungültiger Name (a-z, 0-9, -)")
                return
            if not re.search(r"^services:", new["compose"], flags=re.M):
                self.toast("Die Compose-Datei braucht einen 'services:'-Abschnitt")
                return
            if idx is None:
                self.cfg["containers"]["stacks"].append(new)
            else:
                self.cfg["containers"]["stacks"][idx] = new
            self.refresh_stacks()
            self.changed()
        dlg.connect("response", resp)
        dlg.present(self)

    def page_apps(self):
        page = Adw.PreferencesPage()

        # ---- Flatpak
        fp = Adw.PreferencesGroup(title="Flatpak",
                                  description="Apps aus Flathub oder anderen Flatpak-Quellen vorinstallieren. Sie "
                                              "werden beim Bauen geladen und beim ersten Start eingerichtet – fehlt "
                                              "etwas, lädt das System es nach, sobald Internet da ist.")
        page.add(fp)
        self.flatpak_row = self.switch(fp, "Flatpak einrichten", "flatpak", "enabled")
        self.switch(fp, "Flathub als Quelle", "flatpak", "flathub", "dl.flathub.org – die größte Flatpak-Quelle")
        self.switch(fp, "App-Store mit Flatpak-Unterstützung", "flatpak", "store",
                    "KDE/LXQt: Discover, sonst GNOME Software")
        self.switch(fp, "Apps beim Bauen mitliefern", "flatpak", "preload",
                    "Braucht flatpak auf diesem Rechner. Aus = Download beim ersten Start")

        apps = Adw.PreferencesGroup(title="Flatpak-Apps")
        page.add(apps)
        x86 = not C.is_image(self.cfg)
        preset_ids = set()
        for app_id, label, x86_only in C.FLATPAK_PRESETS:
            if x86_only and not x86:
                continue
            preset_ids.add(app_id)
            row = Adw.SwitchRow(title=label, subtitle=app_id, active=app_id in self.cfg["flatpak"]["apps"])
            row.connect("notify::active", lambda r, _p, a=app_id: self.toggle_flatpak(a, r.get_active()))
            apps.add(row)

        self.flatpak_custom_grp = Adw.PreferencesGroup(
            title="Weitere Flatpak-Apps",
            description="App-ID wie auf flathub.org, z. B. org.inkscape.Inkscape. Aus einer eigenen Quelle: "
                        "quelle:app.id")
        page.add(self.flatpak_custom_grp)
        add = Adw.EntryRow(title="App hinzufügen", show_apply_button=True)

        def on_apply(r):
            entry = r.get_text().strip()
            if not C.FLATPAK_APP_RE.match(entry.rpartition(":")[2]):
                self.toast("Ungültige App-ID")
                return
            self.toggle_flatpak(entry, True)
            r.set_text("")
            self.refresh_flatpak_custom(preset_ids)
        add.connect("apply", on_apply)
        self.flatpak_custom_grp.add(add)
        self.flatpak_custom_rows = []
        self.refresh_flatpak_custom(preset_ids)

        self.remotes_grp = Adw.PreferencesGroup(
            title="Eigene Flatpak-Quellen",
            description="Name und Adresse einer .flatpakrepo-Datei, z. B. "
                        "kde → https://distribute.kde.org/kdeapps.flatpakrepo")
        page.add(self.remotes_grp)
        rname = Adw.EntryRow(title="Name der Quelle")
        rurl = Adw.EntryRow(title="URL (.flatpakrepo)", show_apply_button=True)

        def on_remote(_r):
            name, url = rname.get_text().strip(), rurl.get_text().strip()
            if not C.REMOTE_RE.match(name) or not url.startswith(("http://", "https://")):
                self.toast("Bitte Namen (a-z, 0-9, _ -) und URL angeben")
                return
            self.cfg["flatpak"]["remotes"] = [x for x in self.cfg["flatpak"]["remotes"] if x["name"] != name]
            self.cfg["flatpak"]["remotes"].append({"name": name, "url": url})
            rname.set_text("")
            rurl.set_text("")
            self.refresh_remotes()
            self.changed()
        rurl.connect("apply", on_remote)
        self.remotes_grp.add(rname)
        self.remotes_grp.add(rurl)
        self.remote_rows = []
        self.refresh_remotes()

        # ---- Distrobox
        db = Adw.PreferencesGroup(
            title="Distrobox",
            description="Andere Distributionen als Container im Terminal – mit Zugriff auf das Home-Verzeichnis. "
                        "Die Container werden beim ersten Anmelden jedes Benutzers angelegt (braucht Internet).")
        page.add(db)
        self.distrobox_row = self.switch(db, "Distrobox installieren", "distrobox", "enabled",
                                         "Nutzt Podman (wird bei Bedarf mitinstalliert) oder Docker")
        self.boxes_grp = Adw.PreferencesGroup(title="Container", description="Starten mit 'distrobox enter &lt;name&gt;'")
        menu = Gio.Menu()
        for key, (label, _img) in C.DISTROBOX_PRESETS.items():
            menu.append(label, f"win.box-preset::{key}")
        menu.append("Eigener Container …", "win.box-preset::custom")
        self.boxes_grp.set_header_suffix(Gtk.MenuButton(icon_name="list-add-symbolic", menu_model=menu,
                                                        css_classes=["flat"], tooltip_text="Container hinzufügen"))
        if not self.lookup_action("box-preset"):
            act = Gio.SimpleAction.new("box-preset", GLib.VariantType.new("s"))
            act.connect("activate", lambda _a, v: self.add_box_preset(v.get_string()))
            self.add_action(act)
        page.add(self.boxes_grp)
        self.box_rows = []
        self.refresh_boxes()
        return page

    def toggle_flatpak(self, app_id, active):
        apps = self.cfg["flatpak"]["apps"]
        if active and app_id not in apps:
            apps.append(app_id)
            if not self.cfg["flatpak"]["enabled"]:
                self.flatpak_row.set_active(True)
                self.toast("Flatpak wurde aktiviert")
        elif not active and app_id in apps:
            apps.remove(app_id)
        self.changed()

    def refresh_flatpak_custom(self, preset_ids):
        for r in self.flatpak_custom_rows:
            self.flatpak_custom_grp.remove(r)
        self.flatpak_custom_rows = []
        for app in [a for a in self.cfg["flatpak"]["apps"] if a not in preset_ids]:
            row = Adw.ActionRow(title=html.escape(app))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, a=app: (self.toggle_flatpak(a, False),
                                                      self.refresh_flatpak_custom(preset_ids)))
            row.add_suffix(rm)
            self.flatpak_custom_grp.add(row)
            self.flatpak_custom_rows.append(row)

    def refresh_remotes(self):
        for r in self.remote_rows:
            self.remotes_grp.remove(r)
        self.remote_rows = []
        for i, rem in enumerate(self.cfg["flatpak"]["remotes"]):
            row = Adw.ActionRow(title=html.escape(rem["name"]), subtitle=html.escape(rem["url"]))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, idx=i: (self.cfg["flatpak"]["remotes"].pop(idx),
                                                      self.refresh_remotes(), self.changed()))
            row.add_suffix(rm)
            self.remotes_grp.add(row)
            self.remote_rows.append(row)

    def refresh_boxes(self):
        for r in self.box_rows:
            self.boxes_grp.remove(r)
        self.box_rows = []
        boxes = self.cfg["distrobox"]["boxes"]
        if not boxes:
            row = Adw.ActionRow(title="Keine Container",
                                subtitle="Mit + eine Vorlage (Ubuntu, Fedora, Debian …) oder ein eigenes Image wählen")
            self.boxes_grp.add(row)
            self.box_rows.append(row)
        for i, box in enumerate(boxes):
            extra = [x for x in (box.get("packages"), box.get("export") and "Apps: " + box["export"]) if x]
            row = Adw.ActionRow(title=html.escape(box["name"]),
                                subtitle=html.escape(" · ".join([box["image"]] + extra)))
            edit = Gtk.Button(icon_name="document-edit-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            edit.connect("clicked", lambda *_a, idx=i: self.box_dialog(idx))
            rm = Gtk.Button(icon_name="user-trash-symbolic", css_classes=["flat"], valign=Gtk.Align.CENTER)
            rm.connect("clicked", lambda *_a, idx=i: (self.cfg["distrobox"]["boxes"].pop(idx),
                                                      self.refresh_boxes(), self.changed()))
            row.add_suffix(edit)
            row.add_suffix(rm)
            self.boxes_grp.add(row)
            self.box_rows.append(row)

    def add_box_preset(self, key):
        if not self.cfg["distrobox"]["enabled"]:
            self.distrobox_row.set_active(True)
            self.toast("Distrobox wurde aktiviert")
        if key == "custom":
            self.box_dialog()
            return
        label, image = C.DISTROBOX_PRESETS[key]
        if any(b["name"] == key for b in self.cfg["distrobox"]["boxes"]):
            self.toast(f"{label} ist bereits vorhanden")
            return
        self.cfg["distrobox"]["boxes"].append({"name": key, "image": image, "packages": "", "export": ""})
        self.refresh_boxes()
        self.changed()

    def box_dialog(self, idx=None):
        box = dict(self.cfg["distrobox"]["boxes"][idx]) if idx is not None else \
            {"name": "", "image": "", "packages": "", "export": ""}
        dlg = Adw.AlertDialog(heading="Distrobox-Container" if idx is not None else "Eigener Distrobox-Container")
        lst = Gtk.ListBox(css_classes=["boxed-list"], selection_mode=Gtk.SelectionMode.NONE, width_request=480)
        name = Adw.EntryRow(title="Name (a-z, 0-9, _ . -)", text=box["name"])
        image = Adw.EntryRow(title="Image, z. B. docker.io/library/ubuntu:24.04", text=box["image"])
        pkgs = Adw.EntryRow(title="Zusätzliche Pakete im Container (optional)", text=box.get("packages", ""))
        export = Adw.EntryRow(title="Apps ins Menü übernehmen (optional)", text=box.get("export", ""))
        for w in (name, image, pkgs, export):
            lst.append(w)
        dlg.set_extra_child(lst)
        dlg.add_response("cancel", "Abbrechen")
        dlg.add_response("ok", "Speichern")
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)

        def resp(_d, r):
            if r != "ok":
                return
            new = {"name": name.get_text().strip(), "image": image.get_text().strip(),
                   "packages": pkgs.get_text().strip(), "export": export.get_text().strip()}
            if not C.BOX_RE.match(new["name"]):
                self.toast("Ungültiger Name (a-z, 0-9, _ . -)")
                return
            if not C.IMAGE_RE.match(new["image"]):
                self.toast("Ungültiger Image-Name")
                return
            boxes = self.cfg["distrobox"]["boxes"]
            if any(b["name"] == new["name"] for j, b in enumerate(boxes) if j != idx):
                self.toast("Der Name ist bereits vergeben")
                return
            if idx is None:
                boxes.append(new)
            else:
                boxes[idx] = new
            self.refresh_boxes()
            self.changed()
        dlg.connect("response", resp)
        dlg.present(self)

    def page_image(self):
        page = Adw.PreferencesPage()
        label = C.ARCHES[C.arch(self.cfg)][0]
        grp = Adw.PreferencesGroup(
            title="Fertiges System",
            description=f"Für {label} gibt es keinen Installer: Das Image enthält ein fertig eingerichtetes System. "
                        "Beim ersten Start wächst die Partition auf die Größe der SD-Karte bzw. des Datenträgers.")
        page.add(grp)
        self.entry(grp, "Benutzername", "image", "username")
        pw = Adw.PasswordEntryRow(title="Startpasswort", text=self.cfg["image"]["password"])
        pw.connect("changed", lambda r: (self.cfg["image"].__setitem__("password", r.get_text()), self.changed()))
        grp.add(pw)
        self.switch(grp, "Passwort beim ersten Anmelden ändern", "image", "expire_password")
        self.switch(grp, "Automatisch anmelden", "image", "autologin", "Nur mit Desktop-Umgebung")
        self.switch(grp, "zram-Swap aktivieren", "installer", "zram", "Komprimierter Swap im RAM")
        size = Adw.SpinRow.new_with_range(3, 256, 1)
        size.set_title("Image-Größe (GB)")
        size.set_subtitle("Platz für System, Modelle und Apps – wächst beim ersten Start automatisch")
        size.set_value(self.cfg["image"].get("size_gb", 8))
        size.connect("notify::value", lambda r, _p: (self.cfg["image"].__setitem__("size_gb", int(r.get_value())),
                                                     self.changed()))
        grp.add(size)

        d = Adw.PreferencesGroup(title="Region", description="Sprache, Zeitzone und Tastatur des Systems")
        page.add(d)
        self.combo(d, "Sprache", self._locales, "installer", "locale", search=True)
        self.combo(d, "Zeitzone", self._timezones, "installer", "timezone", search=True)
        self.combo(d, "Tastaturlayout", self._layouts, "installer", "keyboard", search=True)
        return page

    def page_installer(self):
        if C.is_image(self.cfg):
            return self.page_image()
        page = Adw.PreferencesPage()
        grp = Adw.PreferencesGroup(title="Grafischer Installer",
                                   description="Ein eigener GTK-Installer: Festplatte wählen, Benutzer anlegen, "
                                               "fertig. Er installiert die Pakete aus dem Internet (pacstrap) und "
                                               "übernimmt dein Branding. Benötigt eine Desktop-Umgebung.")
        page.add(grp)
        self.switch(grp, "Installer einbinden", "installer", "enabled")
        self.switch(grp, "Installer beim Start des Live-Systems öffnen", "installer", "autostart")
        self.switch(grp, "zram-Swap aktivieren", "installer", "zram", "Komprimierter Swap im RAM")

        src = Adw.PreferencesGroup(title="Installation",
                                   description="Offline kopiert der Installer das fertige System direkt von der ISO "
                                               "– ohne Internet, inklusive aller Pakete, KI-Modelle und Images.")
        page.add(src)
        self.combo(src, "Standard-Installationsquelle", [("offline", "Von der ISO (offline)"),
                                                         ("online", "Aus dem Internet (pacstrap)")],
                   "installer", "source")
        self.switch(src, "Online-Installation zur Auswahl anbieten", "installer", "allow_online")
        self.switch(src, "Verschlüsselung (LUKS2) vorauswählen", "installer", "encrypt",
                    "Der Benutzer kann es im Installer trotzdem ändern")
        self.combo(src, "Standard-Bootloader", [("grub", "GRUB"), ("systemd-boot", "systemd-boot (nur UEFI)")],
                   "installer", "bootloader")
        swap = Adw.SpinRow.new_with_range(0, 128, 1)
        swap.set_title("Standard-Swap-Datei (GB)")
        swap.set_subtitle("0 = keine (zram reicht meist)")
        swap.set_value(self.cfg["installer"].get("swap_gb", 0))
        swap.connect("notify::value", lambda r, _p: self.cfg["installer"].__setitem__("swap_gb", int(r.get_value())))
        src.add(swap)

        d = Adw.PreferencesGroup(title="Standardwerte",
                                 description="Vorauswahl im Installer, gilt auch für die Tastatur im Live-System")
        page.add(d)
        self.combo(d, "Sprache", self._locales, "installer", "locale", search=True)
        self.combo(d, "Zeitzone", self._timezones, "installer", "timezone", search=True)
        self.combo(d, "Tastaturlayout", self._layouts, "installer", "keyboard", search=True)
        self.combo(d, "Dateisystem", [("ext4", "ext4"), ("btrfs", "btrfs (Subvolumes, Kompression)")],
                   "installer", "filesystem")

        w = Adw.PreferencesGroup(title="Willkommenstext")
        page.add(w)
        self.textview(w, "installer", "welcome", height=70)
        return page

    def page_build(self):
        page = Adw.PreferencesPage()
        req = Adw.PreferencesGroup(title="Voraussetzungen")
        page.add(req)
        self.archiso_row = Adw.ActionRow(title="archiso")
        self.archiso_btn = Gtk.Button(label="Installieren", valign=Gtk.Align.CENTER)
        self.archiso_btn.connect("clicked", self.install_archiso)
        self.archiso_row.add_suffix(self.archiso_btn)
        req.add(self.archiso_row)
        self.space_row = Adw.ActionRow(title="Freier Speicher", css_classes=["property"])
        req.add(self.space_row)
        self.skopeo_row = Adw.ActionRow(title="Container-Images vorladen")
        self.skopeo_btn = Gtk.Button(label="skopeo installieren", valign=Gtk.Align.CENTER)
        self.skopeo_btn.connect("clicked", lambda *_: self.install_pkg("skopeo"))
        self.skopeo_row.add_suffix(self.skopeo_btn)
        req.add(self.skopeo_row)
        self.arm_row = Adw.ActionRow(title="ARM-Image bauen",
                                     subtitle="arch-install-scripts, QEMU-Emulation (qemu-user-static-binfmt)")
        self.arm_btn = Gtk.Button(label="Installieren", valign=Gtk.Align.CENTER)
        self.arm_btn.connect("clicked", lambda *_: self.install_pkg(*ARM_TOOLS))
        self.arm_row.add_suffix(self.arm_btn)
        req.add(self.arm_row)
        self.flatpak_req_row = Adw.ActionRow(title="Flatpak-Apps mitliefern")
        self.flatpak_btn = Gtk.Button(label="flatpak installieren", valign=Gtk.Align.CENTER)
        self.flatpak_btn.connect("clicked", lambda *_: self.install_pkg("flatpak"))
        self.flatpak_req_row.add_suffix(self.flatpak_btn)
        req.add(self.flatpak_req_row)

        out = Adw.PreferencesGroup(title="Ausgabe")
        page.add(out)
        self.dir_row = Adw.ActionRow(title="Projektordner")
        pick = Gtk.Button(icon_name="folder-open-symbolic", valign=Gtk.Align.CENTER, css_classes=["flat"])
        pick.connect("clicked", self.pick_dir)
        self.dir_row.add_suffix(pick)
        out.add(self.dir_row)
        self.combo(out, "Kompression", [("zstd", "zstd – schneller Build"), ("xz", "xz – kleinere Datei")],
                   "build", "compression")

        act = Adw.PreferencesGroup()
        page.add(act)
        btns = Gtk.Box(spacing=12, halign=Gtk.Align.CENTER, margin_top=6, margin_bottom=6)
        self.profile_btn = Gtk.Button(label="Nur Profil erzeugen", css_classes=["pill"])
        self.profile_btn.connect("clicked", self.on_profile_only)
        self.build_btn = Gtk.Button(label="Image bauen" if C.is_image(self.cfg) else "ISO bauen",
                                    css_classes=["pill", "suggested-action"])
        self.build_btn.connect("clicked", self.on_build)
        self.cancel_btn = Gtk.Button(label="Abbrechen", css_classes=["pill", "destructive-action"], visible=False)
        self.cancel_btn.connect("clicked", self.on_cancel)
        for b in (self.profile_btn, self.build_btn, self.cancel_btn):
            btns.append(b)
        act.add(btns)

        prog = Adw.PreferencesGroup(title="Fortschritt")
        page.add(prog)
        self.build_status = Gtk.Label(label="Bereit", xalign=0, css_classes=["dim-label"], margin_bottom=6)
        self.build_bar = Gtk.ProgressBar(margin_bottom=10)
        prog.add(self.build_status)
        prog.add(self.build_bar)
        self.log_view = Gtk.TextView(editable=False, cursor_visible=False, monospace=True,
                                     wrap_mode=Gtk.WrapMode.CHAR, css_classes=["log-view"])
        self.log_scroll = Gtk.ScrolledWindow(child=self.log_view, min_content_height=280)
        prog.add(Gtk.Frame(child=self.log_scroll))

        self.result_grp = Adw.PreferencesGroup(title="Ergebnis", visible=False)
        page.add(self.result_grp)
        self.iso_row = Adw.ActionRow(title="Image-Datei" if C.is_image(self.cfg) else "ISO-Datei",
                                     subtitle_selectable=True)
        open_btn = Gtk.Button(label="Ordner öffnen", valign=Gtk.Align.CENTER)
        open_btn.connect("clicked", lambda *_: subprocess.Popen(["xdg-open", os.path.dirname(self.last_iso)]))
        self.qemu_btn = Gtk.Button(label="In VM testen", valign=Gtk.Align.CENTER, css_classes=["suggested-action"])
        self.qemu_btn.connect("clicked", self.run_vm)
        self.iso_row.add_suffix(open_btn)
        self.iso_row.add_suffix(self.qemu_btn)
        self.result_grp.add(self.iso_row)
        self.refresh_build_info()
        return page

    # ------------------------------------------------------------------ Bauen
    def refresh_build_info(self):
        if not hasattr(self, "archiso_row"):
            return
        need_images = bool(C.container_images(self.cfg)) and self.cfg["containers"]["preload"]
        self.skopeo_row.set_visible(need_images)
        tool = F.container_tool()
        self.skopeo_row.set_subtitle(f"✔ über {tool}" if tool else "✘ skopeo, docker oder podman wird benötigt")
        self.skopeo_btn.set_visible(not tool)
        image = C.is_image(self.cfg)
        has = shutil.which("mkarchiso") is not None and os.path.isdir(P.RELENG)
        self.archiso_row.set_visible(not image)
        self.archiso_row.set_subtitle("✔ installiert" if has else "✘ nicht installiert – wird zum Bauen benötigt")
        self.archiso_btn.set_visible(not has)
        self.arm_row.set_visible(image)
        arm_ok = arm_tools_ready()
        self.arm_row.set_subtitle("✔ bereit" if arm_ok else
                                  "✘ arch-install-scripts und QEMU-Emulation (qemu-user-static-binfmt) fehlen")
        self.arm_btn.set_visible(not arm_ok)
        self.flatpak_req_row.set_visible(P.flatpak_preload(self.cfg))
        has_fp = shutil.which("flatpak") is not None
        self.flatpak_req_row.set_subtitle("✔ flatpak installiert" if has_fp else
                                          "✘ flatpak fehlt – sonst werden die Apps erst beim ersten Start geladen")
        self.flatpak_btn.set_visible(not has_fp)
        pth = P.paths(self.cfg)
        self.dir_row.set_subtitle(html.escape(pth["base"]))
        probe = os.path.expanduser(self.cfg["build"]["project_dir"])
        while probe and not os.path.exists(probe):
            probe = os.path.dirname(probe)
        try:
            free = shutil.disk_usage(probe or "/").free / 1024**3
            self.space_row.set_subtitle(f"{free:.1f} GB frei" + ("  ⚠ empfohlen: mind. 20 GB" if free < 20 else ""))
        except OSError:
            self.space_row.set_subtitle("unbekannt")
        isos = sorted(f for f in os.listdir(pth["out"]) if f.endswith(IMAGE_SUFFIXES)) \
            if os.path.isdir(pth["out"]) else []
        if isos:
            self.show_result(os.path.join(pth["out"], isos[-1]))

    def pick_dir(self, *_):
        dlg = Gtk.FileDialog(title="Projektordner wählen")

        def done(d, res):
            try:
                f = d.select_folder_finish(res)
            except GLib.Error:
                return
            self.cfg["build"]["project_dir"] = f.get_path()
            self.refresh_build_info()
        dlg.select_folder(self, None, done)

    def log(self, text):
        buf = self.log_view.get_buffer()
        buf.insert(buf.get_end_iter(), text + "\n")
        if buf.get_line_count() > 5000:
            start = buf.get_start_iter()
            cut = buf.get_iter_at_line(1000)[1]
            buf.delete(start, cut)
        adj = self.log_scroll.get_vadjustment()
        GLib.idle_add(lambda: adj.set_value(adj.get_upper()) and False)

    def set_busy(self, busy):
        self.build_btn.set_sensitive(not busy)
        self.profile_btn.set_sensitive(not busy)
        self.cancel_btn.set_visible(busy)

    def run_streaming(self, cmd, on_line, on_done):
        self.set_busy(True)

        def work():
            rc = -1
            try:
                self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                                             errors="replace", env={**os.environ, "LC_ALL": "C"})
                for line in self.proc.stdout:
                    GLib.idle_add(on_line, line.rstrip("\n"))
                rc = self.proc.wait()
            except OSError as e:
                GLib.idle_add(on_line, f"FEHLER: {e}")
            GLib.idle_add(finish, rc)

        def finish(rc):
            self.proc = None
            self.set_busy(False)
            on_done(rc)
        threading.Thread(target=work, daemon=True).start()

    def install_archiso(self, *_):
        self.install_pkg("archiso")

    def install_pkg(self, *pkgs):
        names = " ".join(pkgs)
        self.log(f"Installiere {names} …")
        self.run_streaming(["pkexec", "pacman", "-S", "--needed", "--noconfirm", *pkgs], self.log,
                           lambda rc: (self.toast(f"{names} installiert" if rc == 0 else "Installation fehlgeschlagen"),
                                       self.refresh_build_info()))

    def prepare(self):
        errors, warnings = C.validate(self.cfg)
        if errors:
            self.error("Bitte korrigieren", "\n".join("• " + e for e in errors))
            return None
        for w in warnings:
            self.log("Hinweis: " + w)
        try:
            pth = P.generate(self.cfg, log=self.log)
        except (P.ProfileError, OSError) as e:
            self.error("Profil konnte nicht erzeugt werden", str(e))
            return None
        return pth

    def fetch_assets(self, pth, then):
        """Lädt KI-Modelle und Container-Images (im Hintergrund), danach then()."""
        needs = (self.cfg["ai"]["ollama"] and self.cfg["ai"]["models"]) or \
            (self.cfg["containers"]["preload"] and C.container_images(self.cfg))
        if not needs:
            then()
            return
        self.set_busy(True)
        self.build_status.set_label("Lade KI-Modelle und Container-Images …")

        def prog(frac, text):
            GLib.idle_add(lambda: (self.build_bar.set_fraction(frac), self.build_status.set_label(text)) and False)

        def work():
            err = None
            try:
                F.prepare_assets(self.cfg, pth, lambda t: GLib.idle_add(self.log, t), prog)
            except (F.FetchError, OSError) as e:
                err = str(e)
            GLib.idle_add(done, err)

        def done(err):
            self.set_busy(False)
            if err:
                self.build_status.set_label("Download fehlgeschlagen")
                self.error("Download fehlgeschlagen", err)
            else:
                then()
        threading.Thread(target=work, daemon=True).start()

    def on_profile_only(self, *_):
        pth = self.prepare()
        if pth:
            def finished():
                self.build_status.set_label("Profil erzeugt")
                self.toast("Profil erzeugt")
                self.log(f"Zum manuellen Bauen: sudo bash {pth['script']}")
            self.fetch_assets(pth, finished)

    def on_build(self, *_):
        if C.is_image(self.cfg):
            if not arm_tools_ready():
                self.error("Werkzeuge fehlen", "Für ARM-Images werden arch-install-scripts und die QEMU-Emulation "
                                               "benötigt (Button „Installieren“ oben).")
                return
        elif not shutil.which("mkarchiso"):
            self.error("archiso fehlt", "Bitte zuerst archiso installieren (Button oben).")
            return
        self.log_view.get_buffer().set_text("")
        pth = self.prepare()
        if not pth:
            return
        self.result_grp.set_visible(False)
        self.build_bar.set_fraction(0)
        self.fetch_assets(pth, lambda: self.start_mkarchiso(pth))

    def start_mkarchiso(self, pth):
        self.build_bar.set_fraction(0)
        self.build_status.set_label("Warte auf Passwort-Bestätigung …")
        self._stage_frac = 0.0
        self._build_out = pth["out"]
        started = set(os.listdir(pth["out"])) if os.path.isdir(pth["out"]) else set()
        self._pulse = GLib.timeout_add(250, self._pulse_tick)
        self.run_streaming(["pkexec", "bash", pth["script"]], self._build_line,
                           lambda rc: self._build_done(rc, started))

    def _pulse_tick(self):
        if self._stage_frac <= 0:
            self.build_bar.pulse()
        return True

    def _build_line(self, line):
        self.log(line)
        for key, frac, label in BUILD_STAGES:
            if key in line:
                self._stage_frac = frac
                self.build_bar.set_fraction(frac)
                self.build_status.set_label(label)
                return
        m = re.search(r"\(\s*(\d+)/(\d+)\) installing (\S+)", line)
        if m:
            frac = 0.06 + 0.44 * int(m[1]) / max(int(m[2]), 1)
            self._stage_frac = frac
            self.build_bar.set_fraction(frac)
            self.build_status.set_label(f"Installiere Pakete: {m[3]} ({m[1]}/{m[2]})")
        elif "downloading" in line and self._stage_frac < 0.06:
            self.build_status.set_label("Pakete werden heruntergeladen …")

    def _build_done(self, rc, started):
        if self._pulse:
            GLib.source_remove(self._pulse)
            self._pulse = None
        out = self._build_out
        new = sorted(f for f in os.listdir(out) if f.endswith(IMAGE_SUFFIXES) and f not in started) \
            if os.path.isdir(out) else []
        kind = "Image" if C.is_image(self.cfg) else "ISO"
        if rc == 0 and new:
            self.build_bar.set_fraction(1)
            self.build_status.set_label(f"{kind} erfolgreich gebaut 🎉")
            self.show_result(os.path.join(out, new[-1]))
            self.toast(f"{kind} fertig!")
        elif rc in (126, 127):
            self.build_status.set_label("Abgebrochen (keine Berechtigung)")
        else:
            self.build_bar.set_fraction(0)
            self.build_status.set_label(f"Build fehlgeschlagen (Code {rc}) – siehe Protokoll")
            self.error("Build fehlgeschlagen", "Die Ursache steht im Protokoll. Häufig: unbekannter "
                                               "Paketname, nicht erreichbares Repository oder zu wenig Speicher.")

    def show_result(self, iso):
        self.last_iso = iso
        try:
            size = os.path.getsize(iso) / 1024**3
        except OSError:
            size = 0
        self.iso_row.set_subtitle(html.escape(f"{iso}  ({size:.2f} GB)"))
        self.qemu_btn.set_visible(iso.endswith(".iso") and bool(shutil.which("run_archiso")
                                                                and shutil.which("qemu-system-x86_64")))
        self.result_grp.set_visible(True)

    def on_cancel(self, *_):
        self.log("Abbruch angefordert …")
        pattern = P.paths(self.cfg)["script"] if C.is_image(self.cfg) else "mkarchiso"
        subprocess.Popen(["pkexec", "pkill", "-TERM", "-f", pattern])

    def run_vm(self, *_):
        if not self.last_iso:
            return
        uefi = os.path.exists("/usr/share/edk2/x64/OVMF_CODE.4m.fd")
        cmd = ["run_archiso", "-i", self.last_iso] + (["-u"] if uefi else [])
        self.log("Starte VM: " + " ".join(cmd))
        subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


ARCH_ASCII = r"""       /\
      /  \
     /\   \
    /      \
   /   ,,   \
  /   |  |  -\
 /_-''    ''-_\ """


class BuilderApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)

    def do_activate(self):
        win = self.get_active_window() or BuilderWindow(self)
        win.present()


def main():
    import sys
    return BuilderApp().run(sys.argv)
