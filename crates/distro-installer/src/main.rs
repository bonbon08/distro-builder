//! Graphical installer that runs inside the live ISO, porting
//! `templates/installer.py`.

mod sysinfo;
mod wizard;

use adw::prelude::*;
use gtk::gio;

const APP_ID: &str = "io.github.bonbon08.DistroInstaller";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(wizard::build);
    app.run()
}
