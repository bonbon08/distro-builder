mod app;
mod ctx;
mod pages;

use std::path::PathBuf;
use std::sync::Arc;

use distro_builder_core::{config, mirror};

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--mirror-server") => mirror_server(args.get(2).map(PathBuf::from)),
        Some("--help" | "-h") => {
            println!(
                "distro-builder {}\n\n  distro-builder                         Oberfläche starten\n  distro-builder --mirror-server [JSON]  Spiegelserver ohne Oberfläche (Projekt oder letzter Stand)",
                env!("CARGO_PKG_VERSION")
            );
            glib::ExitCode::SUCCESS
        }
        _ => app::run(),
    }
}

/// Spiegelserver ohne GTK, z. B. auf einem Heimserver (Strg+C beendet).
fn mirror_server(project: Option<PathBuf>) -> glib::ExitCode {
    let path = project.unwrap_or_else(ctx::autosave_path);
    let cfg = match config::load(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "Projekt {} konnte nicht geladen werden: {e}",
                path.display()
            );
            return glib::ExitCode::FAILURE;
        }
    };
    let sc = mirror::ServerConfig::from_config(&cfg);
    let what = if sc.mode == "full" {
        format!("vollständiger Spiegel aus {}", sc.full.display())
    } else {
        format!("Caching-Proxy für {}", sc.upstream)
    };
    let log: mirror::LogFn = Arc::new(|line| println!("{line}"));
    match mirror::start_server(sc, log) {
        Ok(handle) => {
            println!(
                "Spiegelserver für {} läuft auf {} ({what}) – Strg+C beendet.",
                cfg.general.name,
                mirror::suggested_url(handle.port)
            );
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            eprintln!("Spiegelserver konnte nicht starten: {e}");
            glib::ExitCode::FAILURE
        }
    }
}
