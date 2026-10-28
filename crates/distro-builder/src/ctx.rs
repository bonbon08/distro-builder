//! Shared application state and generic Adwaita row helpers, mirroring the
//! `self.xxx` attributes and `entry()`/`switch()`/`combo()`/`textview()`/
//! `file_row()` helpers in the original `ui.py`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config::Config;
use gtk::glib;

pub const APP_ID: &str = "io.github.bonbon08.DistroBuilder";

pub const CSS: &str = "
.ff-preview { background: #16161e; color: #c0caf5; border-radius: 12px; padding: 18px; }
.ff-preview label { font-family: monospace; font-size: 10.5pt; }
.log-view text { background: transparent; }
.log-view { font-family: monospace; font-size: 9pt; padding: 8px; }
";

pub const PAGES: &[(&str, &str, &str)] = &[
    ("general", "Allgemein", "preferences-system-symbolic"),
    ("branding", "Branding", "applications-graphics-symbolic"),
    ("repos", "Repositories", "network-server-symbolic"),
    ("packages", "Pakete", "package-x-generic-symbolic"),
    ("extras", "Container & KI", "applications-science-symbolic"),
    ("apps", "Flatpak & Distrobox", "view-app-grid-symbolic"),
    (
        "installer",
        "Live & Installer",
        "system-software-install-symbolic",
    ),
    ("build", "Bauen", "media-optical-symbolic"),
];

pub const LOGO_MODES: &[(&str, &str)] = &[
    (
        "image",
        "Bild – echte Pixel in Kitty/Konsole/WezTerm, sonst Zeichengrafik",
    ),
    ("chafa", "Bild als Zeichengrafik (chafa) – überall gleich"),
    ("ascii", "Eigenes ASCII-Art"),
    ("builtin", "Arch-Standardlogo"),
];

pub const ARCH_ASCII: &str = r#"       /\
      /  \
     /\   \
    /      \
   /   ,,   \
  /   |  |  -\
 /_-''    ''-_\ "#;

/// Widgets referenced from more than one page (mirrors the `hasattr(self, …)`
/// pattern in ui.py, where unrelated pages reach into each other's widgets
/// via `update_preview` / `update_pacman_preview` / `update_package_count`).
#[derive(Default)]
pub struct Shared {
    pub prev_pic: Option<gtk::Picture>,
    pub prev_ascii: Option<gtk::Label>,
    pub prev_info: Option<gtk::Label>,
    pub pacman_label: Option<gtk::Label>,
    pub pkg_count: Option<adw::ActionRow>,
    pub custom_pkg_view: Option<gtk::TextView>,

    pub custom_repo_grp: Option<adw::PreferencesGroup>,
    pub custom_repo_rows: Vec<adw::ActionRow>,

    pub ollama_row: Option<adw::SwitchRow>,
    pub model_total: Option<adw::ActionRow>,
    pub custom_models_grp: Option<adw::PreferencesGroup>,
    pub custom_model_rows: Vec<adw::ActionRow>,
    pub stacks_grp: Option<adw::PreferencesGroup>,
    pub stack_rows: Vec<adw::ActionRow>,

    pub flatpak_row: Option<adw::SwitchRow>,
    pub flatpak_custom_grp: Option<adw::PreferencesGroup>,
    pub flatpak_custom_rows: Vec<adw::ActionRow>,
    pub remotes_grp: Option<adw::PreferencesGroup>,
    pub remote_rows: Vec<adw::ActionRow>,
    pub distrobox_row: Option<adw::SwitchRow>,
    pub boxes_grp: Option<adw::PreferencesGroup>,
    pub box_rows: Vec<adw::ActionRow>,

    pub archiso_row: Option<adw::ActionRow>,
    pub archiso_btn: Option<gtk::Button>,
    pub space_row: Option<adw::ActionRow>,
    pub skopeo_row: Option<adw::ActionRow>,
    pub skopeo_btn: Option<gtk::Button>,
    pub arm_row: Option<adw::ActionRow>,
    pub arm_btn: Option<gtk::Button>,
    pub flatpak_req_row: Option<adw::ActionRow>,
    pub flatpak_btn: Option<gtk::Button>,
    pub dir_row: Option<adw::ActionRow>,
    pub profile_btn: Option<gtk::Button>,
    pub build_btn: Option<gtk::Button>,
    pub cancel_btn: Option<gtk::Button>,
    pub build_status: Option<gtk::Label>,
    pub build_bar: Option<gtk::ProgressBar>,
    pub log_view: Option<gtk::TextView>,
    pub log_scroll: Option<gtk::ScrolledWindow>,
    pub result_grp: Option<adw::PreferencesGroup>,
    pub iso_row: Option<adw::ActionRow>,
    pub qemu_btn: Option<gtk::Button>,

    pub stage_frac: f64,
    pub pulse_source: Option<glib::SourceId>,
    pub build_out_dir: Option<PathBuf>,
    pub search_timer: Option<glib::SourceId>,
    pub search_entry: Option<gtk::SearchEntry>,
    pub search_list: Option<gtk::ListBox>,
}

enum StreamMsg {
    Line(String),
    Done(i32),
}

pub struct Ctx {
    pub cfg: RefCell<Config>,
    pub shared: RefCell<Shared>,
    pub project_path: RefCell<Option<PathBuf>>,
    pub last_iso: RefCell<Option<PathBuf>>,

    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub stack: gtk::Stack,
    pub content_title: adw::WindowTitle,
    pub content_page: adw::NavigationPage,
    pub split: adw::NavigationSplitView,

    pub locales: Vec<String>,
    pub timezones: Vec<String>,
    pub layouts: Vec<String>,
}

pub fn autosave_path() -> PathBuf {
    glib::user_config_dir()
        .join("distro-builder")
        .join("autosave.json")
}

fn list_locales() -> Vec<String> {
    match std::fs::read_to_string("/usr/share/i18n/SUPPORTED") {
        Ok(text) => {
            let out: Vec<String> = text
                .lines()
                .filter(|l| !l.trim().is_empty() && l.ends_with("UTF-8"))
                .filter_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
                .collect();
            if out.is_empty() {
                vec!["de_DE.UTF-8".into(), "en_US.UTF-8".into()]
            } else {
                out
            }
        }
        Err(_) => vec!["de_DE.UTF-8".into(), "en_US.UTF-8".into()],
    }
}

fn cmd_lines(cmd: &[&str], fallback: &[&str]) -> Vec<String> {
    let out = Command::new(cmd[0])
        .args(&cmd[1..])
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let lines: Vec<String> = text.split_whitespace().map(|s| s.to_string()).collect();
            if lines.is_empty() {
                fallback.iter().map(|s| s.to_string()).collect()
            } else {
                lines
            }
        }
        _ => fallback.iter().map(|s| s.to_string()).collect(),
    }
}

impl Ctx {
    pub fn new(app: &adw::Application) -> Rc<Ctx> {
        let cfg = load_autosave();

        let toasts = adw::ToastOverlay::new();
        let split = adw::NavigationSplitView::builder()
            .min_sidebar_width(220.0)
            .max_sidebar_width(280.0)
            .build();
        toasts.set_child(Some(&split));

        // Sidebar.
        let side_view = adw::ToolbarView::new();
        let side_header = adw::HeaderBar::new();
        side_header.set_title_widget(Some(&adw::WindowTitle::new(
            "Distro Builder",
            &format!("Version {}", env!("CARGO_PKG_VERSION")),
        )));
        side_view.add_top_bar(&side_header);
        let sidebar = gtk::ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        for (key, label, icon) in PAGES {
            let row = gtk::ListBoxRow::new();
            row.set_widget_name(key);
            let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            box_.set_margin_top(8);
            box_.set_margin_bottom(8);
            box_.set_margin_start(6);
            box_.append(&gtk::Image::from_icon_name(icon));
            let lbl = gtk::Label::new(Some(label));
            lbl.set_xalign(0.0);
            box_.append(&lbl);
            row.set_child(Some(&box_));
            sidebar.append(&row);
        }
        let sidebar_scroll = gtk::ScrolledWindow::builder()
            .child(&sidebar)
            .vexpand(true)
            .build();
        side_view.set_content(Some(&sidebar_scroll));
        let sidebar_page = adw::NavigationPage::new(&side_view, "Distro Builder");
        split.set_sidebar(Some(&sidebar_page));

        // Content.
        let content_view = adw::ToolbarView::new();
        let content_header = adw::HeaderBar::new();
        let content_title = adw::WindowTitle::new("Allgemein", "");
        content_header.set_title_widget(Some(&content_title));
        let menu = gio::Menu::new();
        menu.append(Some("Neues Projekt"), Some("win.new"));
        menu.append(Some("Projekt öffnen …"), Some("win.open"));
        menu.append(Some("Projekt speichern unter …"), Some("win.save-as"));
        let menu_btn = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .tooltip_text("Projekt")
            .build();
        content_header.pack_end(&menu_btn);
        let save_btn = gtk::Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text("Projekt speichern (Strg+S)")
            .build();
        content_header.pack_end(&save_btn);
        content_view.add_top_bar(&content_header);
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .build();
        content_view.set_content(Some(&stack));
        let content_page = adw::NavigationPage::new(&content_view, "Allgemein");
        split.set_content(Some(&content_page));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Distro Builder")
            .default_width(1120)
            .default_height(780)
            .content(&toasts)
            .build();

        let bp = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 700sp").unwrap());
        bp.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(bp);

        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        let ctx = Rc::new(Ctx {
            cfg: RefCell::new(cfg),
            shared: RefCell::new(Shared::default()),
            project_path: RefCell::new(None),
            last_iso: RefCell::new(None),
            window,
            toasts,
            stack,
            content_title,
            content_page,
            split,
            locales: list_locales(),
            timezones: cmd_lines(
                &["timedatectl", "list-timezones"],
                &["UTC", "Europe/Berlin"],
            ),
            layouts: cmd_lines(&["localectl", "list-x11-keymap-layouts"], &["de", "us"]),
        });

        {
            let ctx2 = ctx.clone();
            save_btn.connect_clicked(move |_| ctx2.save_project());
        }
        {
            let ctx2 = ctx.clone();
            sidebar.connect_row_selected(move |_, row| ctx2.on_nav(row.cloned()));
        }
        {
            let ctx2 = ctx.clone();
            ctx.window.connect_close_request(move |_| {
                ctx2.autosave();
                glib::Propagation::Proceed
            });
        }

        let actions = [
            (
                "new",
                Box::new({
                    let ctx2 = ctx.clone();
                    move || ctx2.new_project()
                }) as Box<dyn Fn()>,
            ),
            (
                "open",
                Box::new({
                    let ctx2 = ctx.clone();
                    move || ctx2.open_project()
                }) as Box<dyn Fn()>,
            ),
            (
                "save-as",
                Box::new({
                    let ctx2 = ctx.clone();
                    move || ctx2.save_as()
                }) as Box<dyn Fn()>,
            ),
            (
                "save",
                Box::new({
                    let ctx2 = ctx.clone();
                    move || ctx2.save_project()
                }) as Box<dyn Fn()>,
            ),
        ];
        for (name, cb) in actions {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |_, _| cb());
            ctx.window.add_action(&action);
        }
        app.set_accels_for_action("win.open", &["<Control>o"]);
        app.set_accels_for_action("win.save", &["<Control>s"]);

        ctx.rebuild();
        if let Some(row) = sidebar.row_at_index(0) {
            sidebar.select_row(Some(&row));
        }
        ctx
    }

    // ------------------------------------------------------------ Projekt
    pub fn autosave(&self) {
        let path = autosave_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = distro_builder_core::config::save(&self.cfg.borrow(), &path);
    }

    pub fn new_project(self: &Rc<Self>) {
        *self.cfg.borrow_mut() = distro_builder_core::config::default_config();
        *self.project_path.borrow_mut() = None;
        self.rebuild();
        self.toast("Neues Projekt erstellt");
    }

    pub fn open_project(self: &Rc<Self>) {
        let dlg = gtk::FileDialog::builder().title("Projekt öffnen").build();
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Distro-Builder-Projekt (*.json)"));
        filter.add_pattern("*.json");
        dlg.set_default_filter(Some(&filter));
        let ctx = self.clone();
        dlg.open(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            let Ok(file) = res else { return };
            let Some(path) = file.path() else { return };
            match distro_builder_core::config::load(&path) {
                Ok(cfg) => {
                    *ctx.cfg.borrow_mut() = cfg;
                    *ctx.project_path.borrow_mut() = Some(path.clone());
                    ctx.rebuild();
                    ctx.toast(&format!(
                        "Projekt geladen: {}",
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    ));
                }
                Err(e) => ctx.error("Projekt konnte nicht geladen werden", &e.to_string()),
            }
        });
    }

    pub fn save_project(self: &Rc<Self>) {
        let path = self.project_path.borrow().clone();
        match path {
            None => self.save_as(),
            Some(path) => {
                if let Err(e) = distro_builder_core::config::save(&self.cfg.borrow(), &path) {
                    self.error("Projekt konnte nicht gespeichert werden", &e.to_string());
                } else {
                    self.toast("Projekt gespeichert");
                }
            }
        }
    }

    pub fn save_as(self: &Rc<Self>) {
        let initial = format!("{}.json", self.cfg.borrow().general.id);
        let dlg = gtk::FileDialog::builder()
            .title("Projekt speichern")
            .initial_name(initial)
            .build();
        let ctx = self.clone();
        dlg.save(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            let Ok(file) = res else { return };
            let Some(path) = file.path() else { return };
            *ctx.project_path.borrow_mut() = Some(path);
            ctx.save_project();
        });
    }

    // ------------------------------------------------------------ Helfer
    pub fn toast(&self, text: &str) {
        let toast = adw::Toast::builder().title(text).timeout(3).build();
        self.toasts.add_toast(toast);
    }

    pub fn error(&self, heading: &str, body: &str) {
        let dlg = adw::AlertDialog::builder()
            .heading(heading)
            .body(body)
            .build();
        dlg.add_response("ok", "OK");
        dlg.present(Some(&self.window));
    }

    pub fn on_nav(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let Some(row) = row else { return };
        let key = row.widget_name().to_string();
        let label = PAGES
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, l, _)| *l)
            .unwrap_or("");
        self.stack.set_visible_child_name(&key);
        self.content_title.set_title(label);
        self.content_page.set_title(label);
        self.split.set_show_content(true);
        if key == "build" {
            crate::pages::build::refresh_build_info(self);
        }
    }

    /// Mirrors ui.py's `changed()`: refresh every cross-page preview.
    pub fn changed(self: &Rc<Self>) {
        crate::pages::branding::update_preview(self);
        self.update_pacman_preview();
        self.update_package_count();
    }

    pub fn update_pacman_preview(&self) {
        let shared = self.shared.borrow();
        if let Some(label) = &shared.pacman_label {
            let text = distro_builder_core::profile::pacman_conf(&self.cfg.borrow());
            let stripped: String = text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            label.set_text(stripped.trim());
        }
    }

    pub fn update_package_count(&self) {
        let shared = self.shared.borrow();
        if let Some(row) = &shared.pkg_count {
            let n = distro_builder_core::config::target_packages(&self.cfg.borrow()).len();
            row.set_subtitle(&format!(
                "{n} Pakete/Gruppen für das installierte System (ohne Abhängigkeiten)"
            ));
        }
    }

    /// Append a line to the build-page log view, trimming old lines like the
    /// Python version's 5000-line cap.
    pub fn log(&self, text: &str) {
        let shared = self.shared.borrow();
        let Some(view) = &shared.log_view else { return };
        let buf = view.buffer();
        let mut end = buf.end_iter();
        buf.insert(&mut end, &format!("{text}\n"));
        if buf.line_count() > 5000 {
            let start = buf.start_iter();
            let cut = buf.iter_at_line(1000).unwrap_or(buf.start_iter());
            buf.delete(&mut start.clone(), &mut cut.clone());
        }
        if let Some(scroll) = &shared.log_scroll {
            let adj = scroll.vadjustment();
            adj.set_value(adj.upper());
        }
    }

    pub fn set_busy(&self, busy: bool) {
        let shared = self.shared.borrow();
        if let Some(b) = &shared.build_btn {
            b.set_sensitive(!busy);
        }
        if let Some(b) = &shared.profile_btn {
            b.set_sensitive(!busy);
        }
        if let Some(b) = &shared.cancel_btn {
            b.set_visible(busy);
        }
    }

    /// Runs `cmd` in a worker thread, streaming its combined stdout/stderr
    /// line by line back to the main thread via `on_line`, then `on_done`
    /// with the process exit code (-1 on spawn failure).
    pub fn run_streaming(
        self: &Rc<Self>,
        cmd: Vec<String>,
        on_line: impl Fn(&Rc<Ctx>, &str) + 'static,
        on_done: impl Fn(&Rc<Ctx>, i32) + 'static,
    ) {
        self.set_busy(true);
        let (tx, rx) = async_channel::unbounded::<StreamMsg>();
        std::thread::spawn(move || {
            let mut command = Command::new(&cmd[0]);
            command.args(&cmd[1..]);
            command.env("LC_ALL", "C");
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            match command.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();
                    let tx2 = tx.clone();
                    let out_thread = std::thread::spawn(move || {
                        use std::io::{BufRead, BufReader};
                        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                            let _ = tx2.send_blocking(StreamMsg::Line(line));
                        }
                    });
                    {
                        use std::io::{BufRead, BufReader};
                        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                            let _ = tx.send_blocking(StreamMsg::Line(line));
                        }
                    }
                    let _ = out_thread.join();
                    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                    let _ = tx.send_blocking(StreamMsg::Done(code));
                }
                Err(e) => {
                    let _ = tx.send_blocking(StreamMsg::Line(format!("FEHLER: {e}")));
                    let _ = tx.send_blocking(StreamMsg::Done(-1));
                }
            }
        });
        let ctx = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(msg) = rx.recv().await {
                match msg {
                    StreamMsg::Line(l) => on_line(&ctx, &l),
                    StreamMsg::Done(code) => {
                        ctx.set_busy(false);
                        on_done(&ctx, code);
                        break;
                    }
                }
            }
        });
    }

    pub fn rebuild(self: &Rc<Self>) {
        let current = self.stack.visible_child_name().map(|s| s.to_string());
        while let Some(child) = self.stack.first_child() {
            self.stack.remove(&child);
        }
        *self.shared.borrow_mut() = Shared::default();
        for (key, _label, _icon) in PAGES {
            let page = crate::pages::build_page(self, key);
            self.stack.add_named(&page, Some(key));
        }
        if let Some(name) = current {
            self.stack.set_visible_child_name(&name);
        }
        self.changed();
    }

    // -------------------------------------------------------- Zeilen-Helfer
    pub fn entry_row(
        self: &Rc<Self>,
        grp: &adw::PreferencesGroup,
        title: &str,
        get: impl Fn(&Config) -> String + 'static,
        set: impl Fn(&mut Config, String) + 'static,
        on_change: Option<Box<dyn Fn(&Rc<Ctx>, &str)>>,
    ) -> adw::EntryRow {
        let row = adw::EntryRow::builder()
            .title(title)
            .text(get(&self.cfg.borrow()))
            .build();
        let ctx = self.clone();
        row.connect_changed(move |r| {
            let text = r.text().to_string();
            set(&mut ctx.cfg.borrow_mut(), text.clone());
            if let Some(cb) = &on_change {
                cb(&ctx, &text);
            }
            ctx.changed();
        });
        grp.add(&row);
        row
    }

    pub fn switch_row(
        self: &Rc<Self>,
        grp: &adw::PreferencesGroup,
        title: &str,
        subtitle: Option<&str>,
        get: impl Fn(&Config) -> bool + 'static,
        set: impl Fn(&mut Config, bool) + 'static,
    ) -> adw::SwitchRow {
        let row = adw::SwitchRow::builder()
            .title(title)
            .active(get(&self.cfg.borrow()))
            .build();
        if let Some(s) = subtitle {
            row.set_subtitle(s);
        }
        let ctx = self.clone();
        row.connect_active_notify(move |r| {
            set(&mut ctx.cfg.borrow_mut(), r.is_active());
            ctx.changed();
        });
        grp.add(&row);
        row
    }

    pub fn combo_row(
        self: &Rc<Self>,
        grp: &adw::PreferencesGroup,
        title: &str,
        options: &[(String, String)],
        get: impl Fn(&Config) -> String + 'static,
        set: impl Fn(&mut Config, String) + 'static,
        subtitle: Option<&str>,
        search: bool,
    ) -> adw::ComboRow {
        let labels: Vec<&str> = options.iter().map(|(_, l)| l.as_str()).collect();
        let model = gtk::StringList::new(&labels);
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&model)
            .enable_search(search)
            .build();
        if search {
            row.set_expression(Some(&gtk::PropertyExpression::new(
                gtk::StringObject::static_type(),
                None::<gtk::Expression>,
                "string",
            )));
        }
        if let Some(s) = subtitle {
            row.set_subtitle(s);
        }
        let values: Vec<String> = options.iter().map(|(v, _)| v.clone()).collect();
        let current = get(&self.cfg.borrow());
        if let Some(idx) = values.iter().position(|v| *v == current) {
            row.set_selected(idx as u32);
        }
        let ctx = self.clone();
        let values2 = values.clone();
        row.connect_selected_notify(move |r| {
            let idx = r.selected() as usize;
            if let Some(v) = values2.get(idx) {
                set(&mut ctx.cfg.borrow_mut(), v.clone());
                ctx.changed();
            }
        });
        grp.add(&row);
        row
    }

    pub fn textview_row(
        self: &Rc<Self>,
        grp: &adw::PreferencesGroup,
        get: impl Fn(&Config) -> String + 'static,
        set: impl Fn(&mut Config, String) + 'static,
        height: i32,
        monospace: bool,
    ) -> gtk::TextView {
        let view = gtk::TextView::builder()
            .monospace(monospace)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(10)
            .right_margin(10)
            .build();
        view.buffer().set_text(&get(&self.cfg.borrow()));
        let ctx = self.clone();
        view.buffer().connect_changed(move |buf| {
            let text = buf
                .text(&buf.start_iter(), &buf.end_iter(), false)
                .to_string();
            set(&mut ctx.cfg.borrow_mut(), text);
            ctx.changed();
        });
        let scroller = gtk::ScrolledWindow::builder()
            .child(&view)
            .min_content_height(height)
            .build();
        let frame = gtk::Frame::builder().child(&scroller).build();
        grp.add(&frame);
        view
    }

    pub fn file_row(
        self: &Rc<Self>,
        grp: &adw::PreferencesGroup,
        title: &str,
        default_subtitle: &str,
        get: impl Fn(&Config) -> String + 'static,
        set: impl Fn(&mut Config, String) + 'static,
    ) -> adw::ActionRow {
        let row = adw::ActionRow::builder().title(title).build();
        let thumb = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .width_request(56)
            .height_request(40)
            .can_shrink(true)
            .build();
        let clear = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .tooltip_text("Entfernen")
            .build();
        let pick = gtk::Button::builder()
            .label("Wählen …")
            .valign(gtk::Align::Center)
            .build();

        let get: Rc<dyn Fn(&Config) -> String> = Rc::new(get);
        let set: Rc<dyn Fn(&mut Config, String)> = Rc::new(set);
        let default_subtitle = default_subtitle.to_string();

        let refresh: Rc<dyn Fn()> = {
            let row = row.clone();
            let thumb = thumb.clone();
            let clear = clear.clone();
            let ctx = self.clone();
            let get = get.clone();
            Rc::new(move || {
                let path = get(&ctx.cfg.borrow());
                if path.is_empty() {
                    row.set_subtitle(&default_subtitle);
                } else {
                    row.set_subtitle(&glib::markup_escape_text(&path));
                }
                if !path.is_empty() && std::path::Path::new(&path).is_file() {
                    thumb.set_filename(Some(&path));
                } else {
                    thumb.set_paintable(None::<&gdk::Paintable>);
                }
                clear.set_visible(!path.is_empty());
            })
        };
        refresh();

        {
            let ctx = self.clone();
            let title = title.to_string();
            let refresh = refresh.clone();
            let set = set.clone();
            pick.connect_clicked(move |_| {
                let dlg = gtk::FileDialog::builder().title(title.clone()).build();
                let filter = gtk::FileFilter::new();
                filter.set_name(Some("Bilder"));
                filter.add_pixbuf_formats();
                dlg.set_default_filter(Some(&filter));
                let window = ctx.window.clone();
                let ctx = ctx.clone();
                let refresh = refresh.clone();
                let set = set.clone();
                dlg.open(Some(&window), None::<&gio::Cancellable>, move |res| {
                    let Ok(file) = res else { return };
                    let Some(path) = file.path() else { return };
                    set(
                        &mut ctx.cfg.borrow_mut(),
                        path.to_string_lossy().into_owned(),
                    );
                    refresh();
                    ctx.changed();
                });
            });
        }
        {
            let ctx = self.clone();
            let refresh = refresh.clone();
            let set = set.clone();
            clear.connect_clicked(move |_| {
                set(&mut ctx.cfg.borrow_mut(), String::new());
                refresh();
                ctx.changed();
            });
        }
        row.add_prefix(&thumb);
        row.add_suffix(&clear);
        row.add_suffix(&pick);
        grp.add(&row);
        row
    }
}

fn load_autosave() -> Config {
    distro_builder_core::config::load(&autosave_path())
        .unwrap_or_else(|_| distro_builder_core::config::default_config())
}
