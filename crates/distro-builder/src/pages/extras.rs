use std::rc::Rc;

use adw::prelude::*;
use distro_builder_core::config::{self as C, Stack};
use distro_builder_core::fetch as F;
use gtk::glib;

use crate::ctx::Ctx;

pub fn page(ctx: &Rc<Ctx>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    // ---- KI
    let ai = adw::PreferencesGroup::builder()
        .title("KI mit Ollama")
        .description(
            "Lokale Sprachmodelle (Llama, Qwen, Gemma, Mistral …). Ausgewählte Modelle werden beim Bauen heruntergeladen und in die ISO gepackt – sofort offline nutzbar: 'ollama run llama3.2:3b'.",
        )
        .build();
    page.add(&ai);
    let ollama_row = ctx.switch_row(
        &ai,
        "Ollama installieren",
        Some("Dienst wird automatisch gestartet (Port 11434)"),
        |cfg| cfg.ai.ollama,
        |cfg, v| cfg.ai.ollama = v,
    );
    ctx.shared.borrow_mut().ollama_row = Some(ollama_row);

    let accels: Vec<(String, String)> = C::ollama_accel()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.label.to_string()))
        .collect();
    ctx.combo_row(
        &ai,
        "Beschleunigung",
        &accels,
        |cfg| cfg.ai.accel.clone(),
        |cfg, v| cfg.ai.accel = v,
        Some("CUDA/ROCm: mehrere GB groß"),
        false,
    );
    ctx.switch_row(
        &ai,
        "Im Netzwerk freigeben",
        Some("OLLAMA_HOST=0.0.0.0 – andere Geräte dürfen zugreifen"),
        |cfg| cfg.ai.expose,
        |cfg, v| cfg.ai.expose = v,
    );
    ctx.switch_row(
        &ai,
        "KI-Entwicklungsumgebung",
        Some("PyTorch (passend zur Beschleunigung), Jupyter, NumPy, pandas, scikit-learn, huggingface-hub"),
        |cfg| cfg.ai.dev,
        |cfg, v| cfg.ai.dev = v,
    );

    let models_grp = adw::PreferencesGroup::builder()
        .title("Modelle vorinstallieren")
        .description(
            "Größen sind ungefähre Downloadgrößen. Eigene Modelle: Name aus ollama.com/library oder hf.co/&lt;user&gt;/&lt;repo&gt;:&lt;quant&gt;.",
        )
        .build();
    let size_btn = gtk::Button::builder()
        .label("Größe prüfen")
        .css_classes(["flat"])
        .build();
    models_grp.set_header_suffix(Some(&size_btn));
    page.add(&models_grp);
    let preset_names: std::collections::HashSet<String> = C::model_presets()
        .iter()
        .map(|m| m.name.to_string())
        .collect();
    for preset in C::model_presets() {
        let active = ctx.cfg.borrow().ai.models.iter().any(|m| m == preset.name);
        let row = adw::SwitchRow::builder()
            .title(preset.name)
            .subtitle(format!("{} · ~{:.1} GB", preset.desc, preset.size_gb))
            .active(active)
            .build();
        let ctx2 = ctx.clone();
        let name = preset.name.to_string();
        row.connect_active_notify(move |r| toggle_model(&ctx2, &name, r.is_active()));
        models_grp.add(&row);
    }
    let custom_models_grp = adw::PreferencesGroup::builder()
        .title("Eigene Modelle")
        .build();
    page.add(&custom_models_grp);
    ctx.shared.borrow_mut().custom_models_grp = Some(custom_models_grp.clone());
    let add_model = adw::EntryRow::builder()
        .title("Modell hinzufügen (z. B. qwen2.5-coder:7b)")
        .show_apply_button(true)
        .build();
    {
        let ctx2 = ctx.clone();
        let preset_names2 = preset_names.clone();
        add_model.connect_apply(move |r| {
            let name = r.text().trim().to_string();
            if !C::model_name_valid(&name) {
                ctx2.toast("Ungültiger Modellname");
                return;
            }
            if !ctx2.cfg.borrow().ai.models.iter().any(|m| m == &name) {
                ctx2.cfg.borrow_mut().ai.models.push(name);
            }
            r.set_text("");
            refresh_custom_models(&ctx2, &preset_names2);
            ctx2.changed();
        });
    }
    custom_models_grp.add(&add_model);
    refresh_custom_models(ctx, &preset_names);
    let model_total = adw::ActionRow::builder()
        .title("Modelle gesamt")
        .css_classes(["property"])
        .build();
    custom_models_grp.add(&model_total);
    ctx.shared.borrow_mut().model_total = Some(model_total);
    update_model_total(ctx, None);

    {
        let ctx2 = ctx.clone();
        size_btn.connect_clicked(move |_| on_model_sizes(&ctx2));
    }

    // ---- Container
    let ct = adw::PreferencesGroup::builder()
        .title("Container")
        .description(
            "Docker oder Podman vorinstallieren. Images können direkt in die ISO gepackt werden und werden beim ersten Start importiert.",
        )
        .build();
    page.add(&ct);
    let engines: Vec<(String, String)> = C::container_engines()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.label.to_string()))
        .collect();
    ctx.combo_row(
        &ct,
        "Container-Engine",
        &engines,
        |cfg| cfg.containers.engine.clone(),
        |cfg, v| cfg.containers.engine = v,
        None,
        false,
    );
    ctx.switch_row(
        &ct,
        "Images beim Bauen in die ISO packen",
        Some("Sonst werden sie beim ersten Start aus dem Internet geladen"),
        |cfg| cfg.containers.preload,
        |cfg, v| cfg.containers.preload = v,
    );
    let imgs = adw::PreferencesGroup::builder()
        .title("Zusätzliche Images")
        .description("Ein Image pro Zeile, z. B. nginx:alpine, postgres:17, ghcr.io/…")
        .build();
    page.add(&imgs);
    ctx.textview_row(
        &imgs,
        |cfg| cfg.containers.images.clone(),
        |cfg, v| cfg.containers.images = v,
        70,
        true,
    );

    let stacks_grp = adw::PreferencesGroup::builder()
        .title("Container-Apps (Compose-Stacks)")
        .description("Werden als systemd-Dienst gestartet. Images aus den Stacks werden automatisch vorgeladen.")
        .build();
    let menu = gio::Menu::new();
    for (key, preset) in C::stack_presets() {
        menu.append(
            Some(preset.title),
            Some(&format!("win.stack-preset::{key}")),
        );
    }
    menu.append(Some("Eigener Stack …"), Some("win.stack-preset::custom"));
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("list-add-symbolic")
        .menu_model(&menu)
        .css_classes(["flat"])
        .tooltip_text("Stack hinzufügen")
        .build();
    stacks_grp.set_header_suffix(Some(&menu_btn));
    page.add(&stacks_grp);
    ctx.shared.borrow_mut().stacks_grp = Some(stacks_grp.clone());
    if ctx.window.lookup_action("stack-preset").is_none() {
        let action = gio::SimpleAction::new("stack-preset", Some(glib::VariantTy::STRING));
        let ctx2 = ctx.clone();
        action.connect_activate(move |_, v| {
            if let Some(key) = v.and_then(|v| v.str().map(|s| s.to_string())) {
                add_stack_preset(&ctx2, &key);
            }
        });
        ctx.window.add_action(&action);
    }
    refresh_stacks(ctx);

    page
}

fn toggle_model(ctx: &Rc<Ctx>, name: &str, active: bool) {
    {
        let mut cfg = ctx.cfg.borrow_mut();
        let models = &mut cfg.ai.models;
        if active && !models.iter().any(|m| m == name) {
            models.push(name.to_string());
        } else if !active {
            models.retain(|m| m != name);
        }
    }
    let needs_enable = active && !ctx.cfg.borrow().ai.ollama;
    if needs_enable {
        ctx.cfg.borrow_mut().ai.ollama = true;
        if let Some(row) = &ctx.shared.borrow().ollama_row {
            row.set_active(true);
        }
        ctx.toast("Ollama wurde aktiviert");
    }
    let preset_names: std::collections::HashSet<String> = C::model_presets()
        .iter()
        .map(|m| m.name.to_string())
        .collect();
    update_model_total(ctx, None);
    refresh_custom_models(ctx, &preset_names);
    ctx.changed();
}

fn refresh_custom_models(ctx: &Rc<Ctx>, preset_names: &std::collections::HashSet<String>) {
    let grp = ctx.shared.borrow().custom_models_grp.clone();
    let Some(grp) = grp else { return };
    let old_rows = std::mem::take(&mut ctx.shared.borrow_mut().custom_model_rows);
    for row in &old_rows {
        grp.remove(row);
    }
    let custom_names: Vec<String> = ctx
        .cfg
        .borrow()
        .ai
        .models
        .iter()
        .filter(|m| !preset_names.contains(*m))
        .cloned()
        .collect();
    let mut rows = Vec::new();
    for name in custom_names {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&name))
            .build();
        let rm = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        let ctx2 = ctx.clone();
        let name2 = name.clone();
        let preset_names2 = preset_names.clone();
        rm.connect_clicked(move |_| {
            ctx2.cfg.borrow_mut().ai.models.retain(|m| m != &name2);
            refresh_custom_models(&ctx2, &preset_names2);
            update_model_total(&ctx2, None);
            ctx2.changed();
        });
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    // Keep the trailing "model_total" row (added after this group's models)
    // consistent: it is re-added by callers after rebuilding.
    ctx.shared.borrow_mut().custom_model_rows = rows;
}

fn update_model_total(ctx: &Rc<Ctx>, exact: Option<u64>) {
    let shared = ctx.shared.borrow();
    let Some(row) = &shared.model_total else {
        return;
    };
    let models = ctx.cfg.borrow().ai.models.clone();
    if let Some(exact) = exact {
        row.set_subtitle(&format!(
            "{} Modell(e), {:.2} GB (geprüft)",
            models.len(),
            exact as f64 / 1024f64.powi(3)
        ));
        return;
    }
    let known: std::collections::HashMap<&str, f64> = C::model_presets()
        .into_iter()
        .map(|m| (m.name, m.size_gb))
        .collect();
    let est: f64 = models
        .iter()
        .map(|m| known.get(m.as_str()).copied().unwrap_or(0.0))
        .sum();
    let unknown = models
        .iter()
        .filter(|m| !known.contains_key(m.as_str()))
        .count();
    let extra = if unknown > 0 {
        format!(" + {unknown} unbekannte")
    } else {
        String::new()
    };
    let warn = if est > 8.0 {
        "  \u{26a0} ISO wird groß"
    } else {
        ""
    };
    row.set_subtitle(&format!(
        "{} Modell(e), ca. {:.1} GB{extra}{warn}",
        models.len(),
        est
    ));
}

fn on_model_sizes(ctx: &Rc<Ctx>) {
    if ctx.cfg.borrow().ai.models.is_empty() {
        ctx.toast("Keine Modelle ausgewählt");
        return;
    }
    if let Some(row) = &ctx.shared.borrow().model_total {
        row.set_subtitle("Prüfe …");
    }
    let cfg = ctx.cfg.borrow().clone();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let (total, errors) = F::estimate_sizes(&cfg);
        let _ = tx.send_blocking((total, errors));
    });
    let ctx2 = ctx.clone();
    glib::spawn_future_local(async move {
        if let Ok((total, errors)) = rx.recv().await {
            update_model_total(&ctx2, Some(total));
            if !errors.is_empty() {
                ctx2.error("Modelle nicht gefunden", &errors.join("\n"));
            }
        }
    });
}

fn refresh_stacks(ctx: &Rc<Ctx>) {
    let grp = ctx.shared.borrow().stacks_grp.clone();
    let Some(grp) = grp else { return };
    let old_rows = std::mem::take(&mut ctx.shared.borrow_mut().stack_rows);
    for row in &old_rows {
        grp.remove(row);
    }
    let stacks = ctx.cfg.borrow().containers.stacks.clone();
    let mut rows = Vec::new();
    if stacks.is_empty() {
        let row = adw::ActionRow::builder()
            .title("Keine Container-Apps")
            .subtitle(
                "Mit + eine Vorlage (Open WebUI, Portainer …) oder einen eigenen Stack hinzufügen",
            )
            .build();
        grp.add(&row);
        rows.push(row);
    }
    let image_re = regex::Regex::new(r#"(?m)^\s*image:\s*["']?([^\s"']+)"#).unwrap();
    for (i, st) in stacks.iter().enumerate() {
        let images: Vec<&str> = image_re
            .captures_iter(&st.compose)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();
        let title = if st.title.is_empty() {
            &st.name
        } else {
            &st.title
        };
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(title))
            .subtitle(glib::markup_escape_text(&if images.is_empty() {
                "kein Image".to_string()
            } else {
                images.join(", ")
            }))
            .build();
        let auto = gtk::Switch::builder()
            .active(st.autostart)
            .valign(gtk::Align::Center)
            .tooltip_text("Beim Systemstart starten")
            .build();
        {
            let ctx2 = ctx.clone();
            auto.connect_active_notify(move |w| {
                if let Some(s) = ctx2.cfg.borrow_mut().containers.stacks.get_mut(i) {
                    s.autostart = w.is_active();
                }
            });
        }
        let edit = gtk::Button::builder()
            .icon_name("document-edit-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        {
            let ctx2 = ctx.clone();
            edit.connect_clicked(move |_| stack_dialog(&ctx2, Some(i)));
        }
        let rm = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        {
            let ctx2 = ctx.clone();
            rm.connect_clicked(move |_| {
                ctx2.cfg.borrow_mut().containers.stacks.remove(i);
                refresh_stacks(&ctx2);
                ctx2.changed();
            });
        }
        row.add_suffix(&auto);
        row.add_suffix(&edit);
        row.add_suffix(&rm);
        grp.add(&row);
        rows.push(row);
    }
    ctx.shared.borrow_mut().stack_rows = rows;
}

fn add_stack_preset(ctx: &Rc<Ctx>, key: &str) {
    if ctx.cfg.borrow().containers.engine == "none" {
        ctx.cfg.borrow_mut().containers.engine = "docker".to_string();
        ctx.rebuild();
        ctx.toast("Docker wurde aktiviert");
    }
    if key == "custom" {
        stack_dialog(ctx, None);
        return;
    }
    let Some((_, preset)) = C::stack_presets().into_iter().find(|(k, _)| *k == key) else {
        return;
    };
    if ctx
        .cfg
        .borrow()
        .containers
        .stacks
        .iter()
        .any(|s| s.name == key)
    {
        ctx.toast(&format!("{} ist bereits vorhanden", preset.title));
        return;
    }
    let url_re = regex::Regex::new(r"https?://\S+").unwrap();
    let url = url_re
        .find(preset.desc)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    ctx.cfg.borrow_mut().containers.stacks.push(Stack {
        name: key.to_string(),
        title: preset.title.to_string(),
        compose: preset.compose.to_string(),
        autostart: true,
        url,
    });
    if key == "open-webui" && !ctx.cfg.borrow().ai.ollama {
        ctx.toast("Tipp: Open WebUI braucht Ollama – unter „KI mit Ollama“ aktivieren");
    }
    refresh_stacks(ctx);
    ctx.changed();
}

fn stack_dialog(ctx: &Rc<Ctx>, idx: Option<usize>) {
    let existing = idx.and_then(|i| ctx.cfg.borrow().containers.stacks.get(i).cloned());
    let st = existing.unwrap_or_else(|| Stack {
        name: String::new(),
        title: String::new(),
        compose: "services:\n  app:\n    image: \n    restart: unless-stopped\n".to_string(),
        autostart: true,
        url: String::new(),
    });
    let heading = if idx.is_some() {
        "Container-App bearbeiten"
    } else {
        "Eigene Container-App"
    };
    let dlg = adw::AlertDialog::builder().heading(heading).build();
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_width_request(560);
    let lst = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();
    let name = adw::EntryRow::builder()
        .title("Name (a-z, 0-9, -)")
        .text(st.name.clone())
        .build();
    let url = adw::EntryRow::builder()
        .title("Web-Adresse für Menüeintrag (optional)")
        .text(st.url.clone())
        .build();
    lst.append(&name);
    lst.append(&url);
    box_.append(&lst);
    let view = gtk::TextView::builder()
        .monospace(true)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(10)
        .right_margin(10)
        .build();
    view.buffer().set_text(&st.compose);
    let heading_lbl = gtk::Label::builder()
        .label("compose.yaml")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    box_.append(&heading_lbl);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(260)
        .build();
    box_.append(&gtk::Frame::builder().child(&scroller).build());
    dlg.set_extra_child(Some(&box_));
    dlg.add_response("cancel", "Abbrechen");
    dlg.add_response("ok", "Speichern");
    dlg.set_response_appearance("ok", adw::ResponseAppearance::Suggested);

    let ctx2 = ctx.clone();
    let title_fallback = st.title.clone();
    dlg.connect_response(None, move |_dlg, response| {
        if response != "ok" {
            return;
        }
        let buf = view.buffer();
        let compose = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        let new_name = name.text().trim().to_string();
        let new = Stack {
            name: new_name.clone(),
            title: if title_fallback.is_empty() {
                new_name.clone()
            } else {
                title_fallback.clone()
            },
            compose,
            autostart: st.autostart,
            url: url.text().trim().to_string(),
        };
        if !C::stack_re().is_match(&new.name) {
            ctx2.toast("Ungültiger Name (a-z, 0-9, -)");
            return;
        }
        if !regex::Regex::new(r"(?m)^services:")
            .unwrap()
            .is_match(&new.compose)
        {
            ctx2.toast("Die Compose-Datei braucht einen 'services:'-Abschnitt");
            return;
        }
        match idx {
            None => ctx2.cfg.borrow_mut().containers.stacks.push(new),
            Some(i) => ctx2.cfg.borrow_mut().containers.stacks[i] = new,
        }
        refresh_stacks(&ctx2);
        ctx2.changed();
    });
    dlg.present(Some(&ctx.window));
}
