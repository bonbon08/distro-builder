use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config::{self as C, CustomRepo};
use gtk::glib;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let off = adw::PreferencesGroup::builder().title("Offizielle Arch-Repositories").build();
    page.add(&off);
    let core = ctx.switch_row(&off, "core", Some("Basissystem – immer aktiv"), |cfg| cfg.repos.core, |cfg, v| cfg.repos.core = v);
    core.set_sensitive(false);
    ctx.switch_row(&off, "extra", Some("Desktop, Programme – empfohlen"), |cfg| cfg.repos.extra, |cfg, v| cfg.repos.extra = v);
    ctx.switch_row(&off, "multilib", Some("32-Bit-Bibliotheken (z. B. Steam, Wine)"), |cfg| cfg.repos.multilib, |cfg, v| cfg.repos.multilib = v);
    ctx.switch_row(&off, "core-testing", Some("⚠ Testpakete"), |cfg| cfg.repos.core_testing, |cfg, v| cfg.repos.core_testing = v);
    ctx.switch_row(&off, "extra-testing", Some("⚠ Testpakete"), |cfg| cfg.repos.extra_testing, |cfg, v| cfg.repos.extra_testing = v);

    let mir = adw::PreferencesGroup::builder()
        .title("Spiegelserver")
        .description(
            "Leer lassen, um /etc/pacman.d/mirrorlist zu nutzen. Sonst z. B. https://mirror.example.org/archlinux/$repo/os/$arch",
        )
        .build();
    page.add(&mir);
    ctx.entry_row(&mir, "Eigener Mirror (optional)", |cfg| cfg.repos.mirror.clone(), |cfg, v| cfg.repos.mirror = v, None);

    let custom_grp = adw::PreferencesGroup::builder()
        .title("Eigene Repositories")
        .description(
            "z. B. dein eigenes Repo mit selbstgebauten Paketen. Server-URL darf $repo und $arch enthalten. Ohne importierten Schlüssel „Optional TrustAll“ verwenden.",
        )
        .build();
    let add_btn = gtk::Button::builder().icon_name("list-add-symbolic").css_classes(["flat"]).tooltip_text("Repository hinzufügen").build();
    {
        let ctx2 = ctx.clone();
        add_btn.connect_clicked(move |_| repo_dialog(&ctx2, None));
    }
    custom_grp.set_header_suffix(Some(&add_btn));
    page.add(&custom_grp);
    ctx.shared.borrow_mut().custom_repo_grp = Some(custom_grp.clone());
    refresh_custom_repos(ctx);

    let prev = adw::PreferencesGroup::builder().title("Erzeugte pacman.conf").build();
    page.add(&prev);
    let pacman_label = gtk::Label::builder().xalign(0.0).selectable(true).wrap(true).css_classes(["log-view"]).build();
    prev.add(&gtk::Frame::builder().child(&pacman_label).build());
    ctx.shared.borrow_mut().pacman_label = Some(pacman_label);
    ctx.update_pacman_preview();

    page
}

fn refresh_custom_repos(ctx: &Rc<Ctx>) {
    let grp = ctx.shared.borrow().custom_repo_grp.clone();
    let Some(grp) = grp else { return };
    let old_rows: Vec<adw::ActionRow> = std::mem::take(&mut ctx.shared.borrow_mut().custom_repo_rows);
    for row in &old_rows {
        grp.remove(row);
    }
    let repos = ctx.cfg.borrow().repos.custom.clone();
    let mut rows = Vec::new();
    if repos.is_empty() {
        let row = adw::ActionRow::builder().title("Keine eigenen Repositories").subtitle("Mit + hinzufügen").build();
        grp.add(&row);
        rows.push(row);
    }
    for (i, repo) in repos.iter().enumerate() {
        let subtitle = format!(
            "{}  ·  {}{}",
            repo.server,
            repo.siglevel,
            if repo.first { "  ·  vor offiziellen" } else { "" }
        );
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&format!("[{}]", repo.name)))
            .subtitle(glib::markup_escape_text(&subtitle))
            .build();
        let edit = gtk::Button::builder().icon_name("document-edit-symbolic").css_classes(["flat"]).valign(gtk::Align::Center).build();
        {
            let ctx2 = ctx.clone();
            edit.connect_clicked(move |_| repo_dialog(&ctx2, Some(i)));
        }
        let rm = gtk::Button::builder().icon_name("user-trash-symbolic").css_classes(["flat"]).valign(gtk::Align::Center).build();
        {
            let ctx2 = ctx.clone();
            rm.connect_clicked(move |_| {
                ctx2.cfg.borrow_mut().repos.custom.remove(i);
                refresh_custom_repos(&ctx2);
                ctx2.changed();
            });
        }
        row.add_suffix(&edit);
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().custom_repo_rows = rows;
}

fn repo_dialog(ctx: &Rc<Ctx>, idx: Option<usize>) {
    let existing = idx.and_then(|i| ctx.cfg.borrow().repos.custom.get(i).cloned());
    let repo = existing.unwrap_or_else(|| CustomRepo {
        name: String::new(),
        server: String::new(),
        siglevel: "Optional TrustAll".to_string(),
        first: false,
    });
    let heading = if idx.is_some() { "Repository bearbeiten" } else { "Repository hinzufügen" };
    let dlg = adw::AlertDialog::builder().heading(heading).build();
    let lst = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).width_request(420).build();
    let name = adw::EntryRow::builder().title("Name (z. B. meinrepo)").text(repo.name.clone()).build();
    let server = adw::EntryRow::builder().title("Server-URL").text(repo.server.clone()).build();
    let siglevels: Vec<&str> = C::siglevels();
    let sig = adw::ComboRow::builder().title("SigLevel").model(&gtk::StringList::new(&siglevels)).build();
    if let Some(idx) = siglevels.iter().position(|s| *s == repo.siglevel) {
        sig.set_selected(idx as u32);
    }
    let first = adw::SwitchRow::builder()
        .title("Vor den offiziellen Repos")
        .subtitle("Pakete überschreiben Arch-Pakete")
        .active(repo.first)
        .build();
    lst.append(&name);
    lst.append(&server);
    lst.append(&sig);
    lst.append(&first);
    dlg.set_extra_child(Some(&lst));
    dlg.add_response("cancel", "Abbrechen");
    dlg.add_response("ok", "Speichern");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);

    let ctx2 = ctx.clone();
    let siglevels_owned: Vec<String> = siglevels.iter().map(|s| s.to_string()).collect();
    dlg.connect_response(None, move |_dlg, response| {
        if response != "ok" {
            return;
        }
        let new = CustomRepo {
            name: name.text().trim().to_string(),
            server: server.text().trim().to_string(),
            siglevel: siglevels_owned.get(sig.selected() as usize).cloned().unwrap_or_default(),
            first: first.is_active(),
        };
        let name_re = regex::Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
        if !name_re.is_match(&new.name) || new.server.is_empty() {
            ctx2.toast("Name (a-z, 0-9, -, _) und Server-URL sind erforderlich");
            return;
        }
        match idx {
            None => ctx2.cfg.borrow_mut().repos.custom.push(new),
            Some(i) => ctx2.cfg.borrow_mut().repos.custom[i] = new,
        }
        refresh_custom_repos(&ctx2);
        ctx2.changed();
    });
    dlg.present(Some(&ctx.window));
}
