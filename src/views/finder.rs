//! Tahoe-style Finder root view for the Finder app.
//!
//! Left: TontooUI `Sidebar` (Favourites, Locations, Tags with CoreIcon SF
//! Symbols). Middle: toolbar row (navigation, view switch, actions,
//! search), folder icon grid, status line. Basis step: the grid shows
//! static folders from `crate::model`; selection wiring and real
//! filesystem listing are later steps.
//!
//! All text uses SF Pro Display and both `en_us` and `de_de` strings.

use crate::lang;
use crate::model;
use crate::TontooUI::{Sidebar, SidebarIcon, TextInput, Toolbar, ToolbarItem};
use crate::UIKit::prelude::*;
use crate::UIKit::widget::{WidgetId, next_widget_id};
use gtk::prelude::*;

const SF_PRO: &str = "SF Pro Display";
const SIDEBAR_WIDTH: f32 = 240.0;
const FOLDER_BLUE: &str = "#7fbeec";
const FOLDER_EDGE: &str = "#5ea3d8";

struct Palette {
  bg: &'static str,
  fg: &'static str,
  secondary: &'static str,
  separator: &'static str,
}

fn palette(dark: bool) -> Palette {
  if dark {
    Palette {
      bg: "#1d1d1d",
      fg: "#F5F5F7",
      secondary: "#A1A1A6",
      separator: "rgba(255,255,255,0.10)",
    }
  } else {
    Palette {
      bg: "#ececec",
      fg: "#1E1E1E",
      secondary: "#6E6E73",
      separator: "rgba(0,0,0,0.12)",
    }
  }
}

fn markup_label(text: &str, size: u32, weight: &str, color: &str) -> gtk::Label {
  let label = gtk::Label::new(None);
  label.set_use_markup(true);
  label.set_markup(&format!(
    "<span font_desc=\"{} {} {}\" foreground=\"{}\">{}</span>",
    SF_PRO,
    weight,
    size,
    color,
    glib::markup_escape_text(text),
  ));
  label
}

fn gray() -> Color {
  Color::from_rgb(142, 142, 147)
}

fn blue() -> Color {
  Color::from_rgb(0, 122, 255)
}

/// Static Tahoe-style folder artwork: tab plus body in Finder blue.
/// Pure CSS boxes, so the grid never depends on generated icon files.
fn folder_art() -> gtk::Box {
  let art = gtk::Box::new(gtk::Orientation::Vertical, 0);
  art.set_halign(gtk::Align::Center);
  art.set_valign(gtk::Align::Start);

  let tab = gtk::Box::new(gtk::Orientation::Vertical, 0);
  tab.set_size_request(30, 10);
  tab.set_halign(gtk::Align::Start);
  tab.set_margin_start(10);
  crate::UIKit::widget::apply_css(
    &tab,
    &format!(".fd-tab {{ background-color: {FOLDER_BLUE}; border-radius: 4px 4px 0px 0px; }}"),
  );
  tab.add_css_class("fd-tab");
  art.append(&tab);

  let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
  body.set_size_request(72, 46);
  crate::UIKit::widget::apply_css(
    &body,
    &format!(
      ".fd-body {{ background-color: {FOLDER_BLUE}; border-radius: 7px; \
        border: 1px solid {FOLDER_EDGE}; }}"
    ),
  );
  body.add_css_class("fd-body");
  art.append(&body);

  art
}

fn folder_cell(name: &str, pal: &Palette) -> gtk::Box {
  let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
  cell.set_size_request(112, -1);
  cell.set_halign(gtk::Align::Center);
  cell.set_valign(gtk::Align::Start);
  cell.append(&folder_art());

  let label = gtk::Label::new(Some(name));
  label.set_halign(gtk::Align::Center);
  label.set_justify(gtk::Justification::Center);
  label.set_wrap(true);
  label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
  label.set_lines(2);
  label.set_max_width_chars(16);
  label.set_ellipsize(gtk::pango::EllipsizeMode::End);
  let css = format!(
    ".fd-label {{ font-family: '{}'; font-size: 12px; color: {}; }}",
    SF_PRO, pal.fg
  );
  crate::UIKit::widget::apply_css(&label, &css);
  label.add_css_class("fd-label");
  cell.append(&label);

  cell
}

/// Root widget: sidebar on the left, toolbar plus grid plus status on the right.
pub struct FinderRoot {
  id: WidgetId,
}

impl FinderRoot {
  pub fn new() -> Self {
    Self {
      id: next_widget_id(),
    }
  }
}

impl Default for FinderRoot {
  fn default() -> Self {
    Self::new()
  }
}

impl Widget for FinderRoot {
  fn id(&self) -> WidgetId {
    self.id
  }

  fn to_gtk(&self) -> gtk::Widget {
    let dark = crate::UIKit::app::current_color_scheme()
      .unwrap_or_else(ColorScheme::detect_system)
      == ColorScheme::Dark;
    let pal = palette(dark);

    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.set_hexpand(true);
    outer.set_vexpand(true);
    crate::UIKit::widget::apply_css(
      &outer,
      &format!(".finder {{ background-color: {}; }}", pal.bg),
    );
    outer.add_css_class("finder");

    let sidebar = Sidebar::new()
      .section(lang::t("sidebar.favourites"))
      .item(
        lang::t("sidebar.recents"),
        SidebarIcon::sf("clock.fill", gray()),
      )
      .item(
        lang::t("sidebar.applications"),
        SidebarIcon::sf("square.stack.3d.up.fill", blue()),
      )
      .item(
        lang::t("sidebar.downloads"),
        SidebarIcon::sf("arrow.down.circle.fill", blue()),
      )
      .item(
        lang::t("sidebar.documents"),
        SidebarIcon::sf("doc.fill", gray()),
      )
      .item(
        lang::t("sidebar.desktop"),
        SidebarIcon::sf("desktopcomputer", gray()),
      )
      .section(lang::t("sidebar.locations"))
      .item(
        lang::t("sidebar.osx"),
        SidebarIcon::sf("internaldrive.fill", gray()),
      )
      .item(
        lang::t("sidebar.network"),
        SidebarIcon::sf("network", gray()),
      )
      .section(lang::t("sidebar.tags"))
      .item(
        lang::t("tags.professional"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(0, 122, 255)),
      )
      .item(
        lang::t("tags.urgent"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(255, 59, 48)),
      )
      .item(
        lang::t("tags.active"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(255, 149, 0)),
      )
      .item(
        lang::t("tags.reference"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(255, 204, 0)),
      )
      .item(
        lang::t("tags.personal"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(52, 199, 89)),
      )
      .item(
        lang::t("tags.creative"),
        SidebarIcon::sf("circle.fill", Color::from_rgb(175, 82, 222)),
      )
      .item(
        lang::t("tags.archive"),
        SidebarIcon::sf("circle.fill", gray()),
      )
      .selected(0)
      .search_placeholder(lang::t("sidebar.search"))
      .width(SIDEBAR_WIDTH)
      .on_select(|i| println!("Finder selected: {}", i));
    outer.append(&sidebar.to_gtk());

    let detail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    detail.set_hexpand(true);
    detail.set_vexpand(true);
    crate::UIKit::widget::apply_css(
      &detail,
      &format!(".finder-detail {{ background-color: {}; }}", pal.bg),
    );
    detail.add_css_class("finder-detail");

    let toolbar_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    toolbar_row.set_margin_top(8);
    toolbar_row.set_margin_bottom(8);
    toolbar_row.set_margin_start(12);
    toolbar_row.set_margin_end(12);

    let nav = Toolbar::new()
      .item(ToolbarItem::new("chevron.left").on_click(|| println!("Finder back")))
      .item(ToolbarItem::new("chevron.right").on_click(|| println!("Finder forward")));
    toolbar_row.append(&nav.to_gtk());

    let title = markup_label(&lang::t("detail.title"), 15, "bold", pal.fg);
    title.set_halign(gtk::Align::Start);
    title.set_valign(gtk::Align::Center);
    title.set_hexpand(true);
    toolbar_row.append(&title);

    let views = Toolbar::new()
      .item(ToolbarItem::new("square.grid.2x2").on_click(|| println!("Finder view icons")))
      .item(ToolbarItem::new("list.bullet").on_click(|| println!("Finder view list")))
      .item(ToolbarItem::new("rectangle.split.3x1").on_click(|| println!("Finder view columns")))
      .item(ToolbarItem::new("rectangle.stack").on_click(|| println!("Finder view gallery")));
    toolbar_row.append(&views.to_gtk());

    let actions = Toolbar::new()
      .item(ToolbarItem::new("arrow.up.arrow.down").on_click(|| println!("Finder sort")))
      .item(ToolbarItem::new("square.and.arrow.up").on_click(|| println!("Finder share")))
      .item(ToolbarItem::new("tag").on_click(|| println!("Finder tag")));
    toolbar_row.append(&actions.to_gtk());

    let search = TextInput::new(lang::t("detail.search")).on_change(|text| {
      println!("Finder search: {}", text);
    });
    let search_gtk = search.to_gtk();
    search_gtk.set_size_request(170, -1);
    toolbar_row.append(&search_gtk);

    detail.append(&toolbar_row);

    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    crate::UIKit::widget::apply_css(
      &sep,
      &format!(".finder-sep {{ background-color: {}; }}", pal.separator),
    );
    sep.add_css_class("finder-sep");
    detail.append(&sep);

    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    grid.set_min_children_per_line(4);
    grid.set_max_children_per_line(8);
    grid.set_row_spacing(14);
    grid.set_column_spacing(8);
    grid.set_margin_top(16);
    grid.set_margin_bottom(16);
    grid.set_margin_start(16);
    grid.set_margin_end(16);
    crate::UIKit::widget::apply_css(
      &grid,
      &format!(".finder-grid {{ background-color: {}; }}", pal.bg),
    );
    grid.add_css_class("finder-grid");
    for name in model::folders() {
      grid.insert(&folder_cell(name, &pal), -1);
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&grid));
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    crate::UIKit::widget::apply_css(
      &scroll,
      &format!(".finder-scroll {{ background-color: {}; }}", pal.bg),
    );
    scroll.add_css_class("finder-scroll");
    detail.append(&scroll);

    let status = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status.set_margin_top(6);
    status.set_margin_bottom(8);
    let line = lang::t("status.line")
      .replace("{count}", &model::item_count().to_string())
      .replace("{free}", &lang::t("status.free"));
    let status_label = markup_label(&line, 11, "normal", pal.secondary);
    status_label.set_halign(gtk::Align::Center);
    status.append(&status_label);
    detail.append(&status);

    outer.append(&detail);
    outer.upcast()
  }
}
