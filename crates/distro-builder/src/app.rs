use adw::prelude::*;

use crate::ctx::{Ctx, APP_ID};

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let ctx = Ctx::new(app);
        ctx.window.present();
    });
    app.run()
}
