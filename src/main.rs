//! Finder: TontooOS file manager basis built with TontooUI.
//!
//! Tahoe-style window (1080x720): sidebar on the left (Favourites,
//! Locations, Tags with CoreIcon SF Symbols), toolbar plus folder grid in
//! the middle, status line at the bottom. Follows the live system color
//! scheme (Dark `#1d1d1d`, Light `#ececec`).

mod icons;
mod lang;
mod model;
mod views;
mod watch;

sdk::preinclude!();

use UIKit::prelude::*;

struct FinderDelegate;

impl AppDelegate for FinderDelegate {
  fn view(&self) -> Box<dyn Widget> {
    Box::new(views::finder::FinderRoot::new())
  }
}

fn main() {
  lang::init();
  let mut app = App::with_delegate(lang::t("app.title"), 1080, 720, FinderDelegate);
  // No extra window bar: the sidebar draws the only traffic lights.
  app.no_window_bar();
  app.auto_color_scheme();
  app.run();
}
