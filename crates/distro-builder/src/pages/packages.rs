use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config as C;
use gtk::glib;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let base = adw::PreferencesGroup::builder().title("Grundsystem").build();
    page.add(&base);

    let kernels: Vec<(String, String)> = C::kernels().into_iter().map(|(k, l)| (k.to_string(), l.to_string())).collect();
    ctx.combo_row(&base, "Kernel", &kernels, |cfg| cfg.packages.kernel.clone(), |cfg, v| cfg.packages.kernel = v, None, false);

    let desktops: Vec<(String, String)> = C::desktops().into_iter().map(|(k, v)| (k.to_string(), v.label.to_string())).collect();
    ctx.combo_row(
        &base,
        "Desktop-Umgebung",
        &desktops,
        |cfg| cfg.packages.desktop.clone(),
        |cfg, v| cfg.packages.desktop = v,
        Some("Inklusive Display-Manager und automatischer Anmeldung im Live-System"),
        false,
    );
    let gpus: Vec<(String, String)> = C::gpus().into_iter().map(|(k, v)| (k.to_string(), v.label.to_string())).collect();
    ctx.combo_row(
        &base,
        "Grafiktreiber",
        &gpus,
        |cfg| cfg.packages.gpu.clone(),
        |cfg, v| cfg.packages.gpu = v,
        Some("NVIDIA nutzt DKMS – der Build dauert dadurch etwas länger"),
        false,
    );
    ctx.switch_row(
        &base,
        "Arch-Rettungswerkzeuge im Live-System",
        Some("Alle Tools des offiziellen Arch-ISOs (größer). Aus = schlankes Live-System"),
        |cfg| cfg.packages.rescue_tools,
        |cfg, v| cfg.packages.rescue_tools = v,
    );
    let pkg_count = adw::ActionRow::builder().title("Paketanzahl").css_classes(["property"]).build();
    base.add(&pkg_count);
    ctx.shared.borrow_mut().pkg_count = Some(pkg_count);

    let pre = adw::PreferencesGroup::builder().title("Paket-Sammlungen").build();
    page.add(&pre);
    for (key, info) in C::presets() {
        let active = ctx.cfg.borrow().packages.presets.iter().any(|p| p == key);
        let row = adw::SwitchRow::builder()
            .title(info.label)
            .subtitle(format!("{} – {}", info.desc, info.packages.join(", ")))
            .active(active)
            .build();
        let ctx2 = ctx.clone();
        let key = key.to_string();
        row.connect_active_notify(move |r| {
            let mut cfg = ctx2.cfg.borrow_mut();
            let presets = &mut cfg.packages.presets;
            if r.is_active() && !presets.iter().any(|p| p == &key) {
                presets.push(key.clone());
            } else if !r.is_active() {
                presets.retain(|p| p != &key);
            }
            drop(cfg);
            ctx2.changed();
        });
        pre.add(&row);
    }

    let custom = adw::PreferencesGroup::builder()
        .title("Eigene Pakete")
        .description("Paketnamen durch Leerzeichen oder Zeilen getrennt. Pakete aus eigenen Repos sind ebenfalls möglich.")
        .build();
    let check_btn = gtk::Button::builder().label("Prüfen").css_classes(["flat"]).build();
    custom.set_header_suffix(Some(&check_btn));
    page.add(&custom);
    let custom_view =
        ctx.textview_row(&custom, |cfg| cfg.packages.custom.clone(), |cfg, v| cfg.packages.custom = v, 100, true);
    ctx.shared.borrow_mut().custom_pkg_view = Some(custom_view.clone());

    {
        let ctx2 = ctx.clone();
        check_btn.connect_clicked(move |_| {
            let names = C::custom_packages(&ctx2.cfg.borrow());
            if names.is_empty() {
                ctx2.toast("Keine eigenen Pakete eingetragen");
                return;
            }
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let missing = C::check_packages(&names);
                let _ = tx.send_blocking((names, missing));
            });
            let ctx3 = ctx2.clone();
            glib::spawn_future_local(async move {
                if let Ok((names, missing)) = rx.recv().await {
                    if missing.is_empty() {
                        ctx3.toast(&format!("Alle {} Pakete gefunden \u{2714}", names.len()));
                    } else {
                        ctx3.error(
                            "Unbekannte Pakete",
                            &format!(
                                "Folgende Pakete wurden in den lokal bekannten Repos nicht gefunden (bei eigenen Repos ist das normal):\n\n{}",
                                missing.join(", ")
                            ),
                        );
                    }
                }
            });
        });
    }

    let search_grp = adw::PreferencesGroup::builder().title("Paketsuche").description("Durchsucht die lokale pacman-Datenbank").build();
    page.add(&search_grp);
    let search_entry = gtk::SearchEntry::builder().placeholder_text("z. B. kdenlive, neovim, obs …").margin_bottom(6).build();
    search_grp.add(&search_entry);
    let search_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).visible(false).build();
    search_grp.add(&search_list);
    {
        let mut shared = ctx.shared.borrow_mut();
        shared.search_entry = Some(search_entry.clone());
        shared.search_list = Some(search_list.clone());
    }
    {
        let ctx2 = ctx.clone();
        search_entry.connect_search_changed(move |entry| on_search(&ctx2, entry));
    }
    ctx.update_package_count();
    page
}

fn on_search(ctx: &Rc<Ctx>, entry: &gtk::SearchEntry) {
    let query = entry.text().trim().to_string();
    {
        let mut shared = ctx.shared.borrow_mut();
        if let Some(id) = shared.search_timer.take() {
            id.remove();
        }
    }
    let ctx2 = ctx.clone();
    let id = glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
        ctx2.shared.borrow_mut().search_timer = None;
        run_search(&ctx2, query);
    });
    ctx.shared.borrow_mut().search_timer = Some(id);
}

fn run_search(ctx: &Rc<Ctx>, query: String) {
    let list = ctx.shared.borrow().search_list.clone();
    let Some(list) = list else { return };
    if query.chars().count() < 2 {
        list.remove_all();
        list.set_visible(false);
        return;
    }
    let (tx, rx) = async_channel::bounded(1);
    let query2 = query.clone();
    std::thread::spawn(move || {
        let results = C::search_packages(&query2, 40);
        let _ = tx.send_blocking(results);
    });
    let ctx2 = ctx.clone();
    glib::spawn_future_local(async move {
        if let Ok(results) = rx.recv().await {
            show_results(&ctx2, &query, results);
        }
    });
}

fn show_results(ctx: &Rc<Ctx>, query: &str, results: Vec<C::PackageSearchResult>) {
    let shared = ctx.shared.borrow();
    let (Some(entry), Some(list)) = (&shared.search_entry, &shared.search_list) else { return };
    if entry.text().trim() != query {
        return;
    }
    list.remove_all();
    list.set_visible(true);
    if results.is_empty() {
        list.append(&adw::ActionRow::builder().title("Keine Treffer").build());
    }
    let current: std::collections::HashSet<String> = C::custom_packages(&ctx.cfg.borrow()).into_iter().collect();
    for r in results {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&r.name))
            .subtitle(glib::markup_escape_text(&format!("{} · {} — {}", r.repo, r.version, r.desc)))
            .subtitle_lines(2)
            .build();
        let has = current.contains(&r.name);
        let btn = gtk::Button::builder()
            .icon_name(if has { "emblem-ok-symbolic" } else { "list-add-symbolic" })
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .tooltip_text("Hinzufügen")
            .build();
        let ctx2 = ctx.clone();
        let name = r.name.clone();
        btn.connect_clicked(move |b| add_package(&ctx2, &name, b));
        row.add_suffix(&btn);
        list.append(&row);
    }
}

fn add_package(ctx: &Rc<Ctx>, name: &str, btn: &gtk::Button) {
    if C::custom_packages(&ctx.cfg.borrow()).iter().any(|p| p == name) {
        return;
    }
    let view = ctx.shared.borrow().custom_pkg_view.clone();
    if let Some(view) = view {
        let buf = view.buffer();
        let text = ctx.cfg.borrow().packages.custom.trim_end().to_string();
        let new_text = if text.is_empty() { name.to_string() } else { format!("{text}\n{name}") };
        buf.set_text(&new_text);
    }
    btn.set_icon_name("emblem-ok-symbolic");
    ctx.toast(&format!("{name} hinzugefügt"));
}
