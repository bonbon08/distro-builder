pub mod branding;
pub mod build;
pub mod extras;
pub mod general;
pub mod installer;
pub mod packages;
pub mod repos;

use std::rc::Rc;

use crate::ctx::Ctx;

pub fn build_page(ctx: &Rc<Ctx>, key: &str) -> gtk::Widget {
    match key {
        "general" => general::page(ctx).into(),
        "branding" => branding::page(ctx).into(),
        "repos" => repos::page(ctx).into(),
        "packages" => packages::page(ctx).into(),
        "extras" => extras::page(ctx).into(),
        "installer" => installer::page(ctx).into(),
        "build" => build::page(ctx).into(),
        _ => unreachable!("unknown page {key}"),
    }
}
