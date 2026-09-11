//! Tahoe-style Finder root view for the Finder app.
//!
//! Left: TontooUI `Sidebar` (Favourites, Locations, Tags with CoreIcon SF
//! Symbols). Middle: toolbar row (navigation, view switch, actions,
//! search), folder icon grid, status line. Basis step: the grid shows
//! static folders from `crate::model`; selection wiring and real
//! filesystem listing are later steps.
//!
//! All text uses SF Pro Display and both `en_us` and `de_de` strings.

use crate::icons;
use crate::lang;
use crate::model;
use crate::TontooUI::{
  ContentUnavailableView, Sidebar, SidebarIcon, TextInput, Toolbar, ToolbarItem,
};
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

fn blue() -> Color {
  Color::from_rgb(0, 122, 255)
}

/// Default grid folder icon (`scalable/folder.svg`). Falls back to CSS
/// artwork when the file is missing so the cell never renders empty.
fn folder_icon_art() -> gtk::Box {
  match icons::folder_icon("folder.svg") {
    Some(path) => {
      let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
      holder.set_halign(gtk::Align::Center);
      let image = gtk::Image::from_file(&path);
      image.set_pixel_size(64);
      image.set_halign(gtk::Align::Center);
      holder.append(&image);
      holder
    }
    None => folder_art(),
  }
}

/// Static Tahoe-style folder artwork: tab plus body in Finder blue.
/// Pure CSS boxes, used only when the themed SVG icon file is missing.
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

fn preview_image(path: &std::path::Path) -> gtk::Box {
  let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
  holder.set_halign(gtk::Align::Center);
  let image = gtk::Image::from_file(path);
  image.set_pixel_size(64);
  image.set_halign(gtk::Align::Center);
  holder.append(&image);
  holder
}

fn folder_cell(base: &std::path::Path, entry: &model::DirEntry, pal: &Palette) -> gtk::Box {
  let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
  cell.set_size_request(112, -1);
  cell.set_halign(gtk::Align::Center);
  cell.set_valign(gtk::Align::Start);

  // `.app` bundles show the app icon rendered once through CoreIcon.
  // Directories show the default folder icon. Images show the picture
  // itself, videos show the cached first-second frame (themed icon
  // while ffmpeg is missing), audio files show the music icon. Other
  // files show no icon, only the name without extension.
  if entry.is_app {
    let full = base.join(&entry.name);
    match icons::app_icon(&full) {
      Some(icon) => cell.append(&preview_image(&icon)),
      None => cell.append(&folder_art()),
    }
  } else if entry.is_dir {
    cell.append(&folder_icon_art());
  } else {
    let full = base.join(&entry.name);
    match model::file_kind(&entry.ext) {
      model::FileKind::Image => cell.append(&preview_image(&full)),
      model::FileKind::Video => match icons::video_thumb(&full) {
        Some(thumb) => cell.append(&preview_image(&thumb)),
        None => match icons::video_icon() {
          Some(icon) => cell.append(&preview_image(&icon)),
          None => cell.append(&folder_art()),
        },
      },
      model::FileKind::Audio => match icons::audio_icon() {
        Some(icon) => cell.append(&preview_image(&icon)),
        None => cell.append(&folder_art()),
      },
      model::FileKind::Archive => match icons::archive_icon() {
        Some(icon) => cell.append(&preview_image(&icon)),
        None => cell.append(&folder_art()),
      },
      model::FileKind::Other => {
        let icon = model::document_icon(&entry.ext)
          .and_then(icons::extension_icon)
          .or_else(icons::generic_file_icon);
        if let Some(path) = icon {
          cell.append(&preview_image(&path));
        }
      }
    }
  }

  let shown = model::display_name(&entry.name, entry.is_dir);
  let label = gtk::Label::new(Some(&shown));
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
        lang::t("sidebar.downloads"),
        SidebarIcon::sf("arrow.down.circle.fill", blue()),
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

    let title = markup_label(&lang::t("sidebar.downloads"), 15, "bold", pal.fg);
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

    let base = model::downloads_dir();
    let entries = model::list_downloads();

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
    for entry in &entries {
      grid.insert(&folder_cell(&base, entry, &pal), -1);
    }

    let scroll = gtk::ScrolledWindow::new();
    if entries.is_empty() {
      let empty = ContentUnavailableView::new()
        .title(lang::t("detail.empty"))
        .message(lang::t("detail.empty.hint"));
      let empty_gtk = empty.to_gtk();
      empty_gtk.set_hexpand(true);
      empty_gtk.set_vexpand(true);
      scroll.set_child(Some(&empty_gtk));
    } else {
      scroll.set_child(Some(&grid));
    }
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
      .replace("{count}", &model::item_count(&entries).to_string())
      .replace("{free}", &lang::t("status.free"));
    let status_label = markup_label(&line, 11, "normal", pal.secondary);
    status_label.set_halign(gtk::Align::Center);
    status.append(&status_label);
    detail.append(&status);

    outer.append(&detail);
    outer.upcast()
  }
}
