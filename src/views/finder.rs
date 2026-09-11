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
  ContentUnavailableView, ContextMenu, MenuEntry, MenuItem, Sidebar, SidebarIcon, Toolbar,
  ToolbarItem,
};
use crate::UIKit::prelude::*;
use crate::UIKit::widget::{WidgetId, next_widget_id};
use gtk::prelude::*;
use std::cell::RefCell;

thread_local! {
  /// Keeps the downloads watcher alive for the app lifetime.
  static FOLDER_WATCHER: RefCell<Option<notify::RecommendedWatcher>> = RefCell::new(None);
}

const SF_PRO: &str = "SF Pro Display";
const SIDEBAR_WIDTH: f32 = 240.0;
const FOLDER_BLUE: &str = "#7fbeec";
const FOLDER_EDGE: &str = "#5ea3d8";

#[derive(Clone, Copy)]
struct Palette {
  bg: &'static str,
  fg: &'static str,
  secondary: &'static str,
}

fn palette(dark: bool) -> Palette {
  if dark {
    Palette {
      bg: "#1d1d1d",
      fg: "#F5F5F7",
      secondary: "#A1A1A6",
    }
  } else {
    Palette {
      bg: "#ececec",
      fg: "#1E1E1E",
      secondary: "#6E6E73",
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
  holder.append(&preview_art(path, false));
  holder
}

/// Grid artwork for one file: plain `GtkImage` for icons, or a
/// rounded-corner texture for photos and video frames (GTK CSS
/// `border-radius` does not clip image content). Falls back to the
/// plain image when rounding fails (covers SVGs, which the `image`
/// crate cannot decode).
fn preview_art(path: &std::path::Path, round: bool) -> gtk::Widget {
  if round {
    if let Ok(img) = image::open(path) {
      // 128px backing for the 64px display size (sharp on HiDPI).
      let mut square = icons::cover_square(&img, 128);
      icons::round_corners(&mut square, 20);
      let (w, h) = square.dimensions();
      let bytes = glib::Bytes::from(square.as_raw());
      let texture = gdk4::MemoryTexture::new(
        w as i32,
        h as i32,
        gdk4::MemoryFormat::R8g8b8a8,
        &bytes,
        (w * 4) as usize,
      );
      let preview = gtk::Image::from_paintable(Some(&texture));
      preview.set_pixel_size(64);
      preview.set_halign(gtk::Align::Center);
      return preview.upcast();
    }
  }
  let image = gtk::Image::from_file(path);
  image.set_pixel_size(64);
  image.set_halign(gtk::Align::Center);
  image.upcast()
}

/// Rounded photo/video preview in a centered holder box.
fn rounded_preview(path: &std::path::Path) -> gtk::Box {
  let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
  holder.set_halign(gtk::Align::Center);
  holder.append(&preview_art(path, true));
  holder
}

/// Right-click on a file cell claims the press so it never reaches
/// the empty-space `ContextMenu` wrapper: file right-clicks show
/// nothing (for now).
fn suppress_file_menu(cell: &gtk::Box) {
  let claim = gtk::GestureClick::new();
  claim.set_button(3);
  claim.connect_pressed(|_, _, _, _| {});
  cell.add_controller(claim);
}

fn folder_cell(base: &std::path::Path, entry: &model::DirEntry, pal: &Palette) -> gtk::Box {
  let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
  cell.set_size_request(112, -1);
  cell.set_halign(gtk::Align::Center);
  cell.set_valign(gtk::Align::Start);
  suppress_file_menu(&cell);

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
      model::FileKind::Image => cell.append(&rounded_preview(&full)),
      model::FileKind::Video => match icons::video_thumb(&full) {
        Some(thumb) => cell.append(&rounded_preview(&thumb)),
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
    ".fd-label {{ font-family: '{}'; font-size: 12px; font-weight: 600; color: {}; }}",
    SF_PRO, pal.fg
  );
  crate::UIKit::widget::apply_css(&label, &css);
  label.add_css_class("fd-label");
  cell.append(&label);

  cell
}

/// Empty-space context menu entries: New Folder, divider, Get Info,
/// divider, New File submenu with Text File. Actions log for now.
pub(crate) fn empty_space_menu() -> Vec<MenuEntry> {
  vec![
    MenuEntry::Item(
      MenuItem::new(lang::t("context.new_folder"))
        .on_activate(|| println!("Finder new folder")),
    ),
    MenuEntry::Divider,
    MenuEntry::Item(
      MenuItem::new(lang::t("context.get_info")).on_activate(|| println!("Finder get info")),
    ),
    MenuEntry::Divider,
    MenuEntry::Submenu {
      title: lang::t("context.new_file"),
      items: vec![MenuEntry::Item(MenuItem::new(lang::t("context.text_file")))],
    },
  ]
}

/// Wraps a raw GTK widget as a TontooUI `Widget` (e.g. to host the
/// scroll area inside a `ContextMenu`).
struct GtkWrap {
  id: WidgetId,
  widget: gtk::Widget,
}

impl GtkWrap {
  fn wrap(widget: impl IsA<gtk::Widget>) -> Self {
    Self {
      id: next_widget_id(),
      widget: widget.upcast(),
    }
  }
}

impl Widget for GtkWrap {
  fn id(&self) -> WidgetId {
    self.id
  }

  fn to_gtk(&self) -> gtk::Widget {
    self.widget.clone()
  }
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
      .item(ToolbarItem::new("chevron.backward").on_click(|| println!("Finder back")))
      .item(ToolbarItem::new("chevron.forward").on_click(|| println!("Finder forward")));
    toolbar_row.append(&nav.to_gtk());

    let title = markup_label(&lang::t("sidebar.downloads"), 15, "bold", pal.fg);
    title.set_halign(gtk::Align::Start);
    title.set_valign(gtk::Align::Center);
    title.set_hexpand(true);
    toolbar_row.append(&title);

    let views = Toolbar::new()
      .item(ToolbarItem::new("square.grid.2x2"))
      .item(ToolbarItem::new("list.bullet"));
    toolbar_row.append(&views.to_gtk());

    let actions = Toolbar::new()
      .item(ToolbarItem::new("magnifyingglass").on_click(|| println!("Finder search")))
      .item(ToolbarItem::new("square.and.arrow.up").on_click(|| println!("Finder share")));
    toolbar_row.append(&actions.to_gtk());

    detail.append(&toolbar_row);

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

    // Right-click on empty space shows the context menu; file cells
    // claim the press (see suppress_file_menu), so it stays hidden
    // over files.
    let menu = ContextMenu::new(GtkWrap::wrap(scroll.clone())).entries(empty_space_menu());
    let menu_gtk = menu.to_gtk();
    menu_gtk.set_hexpand(true);
    menu_gtk.set_vexpand(true);
    detail.append(&menu_gtk);

    let status = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status.set_margin_top(6);
    status.set_margin_bottom(8);
    let status_label = gtk::Label::new(None);
    status_label.set_use_markup(true);
    status_label.set_markup(&status_markup(&entries, &pal));
    status_label.set_halign(gtk::Align::Center);
    status.append(&status_label);
    detail.append(&status);

    // Live updates: a notify watcher signals changes in ~/Downloads/;
    // a 400ms main-thread tick drains bursts and rebuilds the grid once.
    if let Some((watcher, rx)) = crate::watch::watch_dir(&base) {
      FOLDER_WATCHER.with(|slot| *slot.borrow_mut() = Some(watcher));
      let grid_watch = grid.clone();
      let status_watch = status_label.clone();
      glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        let mut dirty = false;
        while rx.try_recv().is_ok() {
          dirty = true;
        }
        if dirty {
          refresh_grid(&grid_watch, &status_watch, &pal);
        }
        glib::ControlFlow::Continue
      });
    }

    outer.append(&detail);
    outer.upcast()
  }
}

/// Status line markup for the current entries.
fn status_markup(entries: &[model::DirEntry], pal: &Palette) -> String {
  let line = lang::t("status.line")
    .replace("{count}", &model::item_count(entries).to_string())
    .replace("{free}", &lang::t("status.free"));
  format!(
    "<span font_desc=\"{} normal 11\" foreground=\"{}\">{}</span>",
    SF_PRO,
    pal.secondary,
    glib::markup_escape_text(&line),
  )
}

/// Rebuild the grid and status line from the live directory.
fn refresh_grid(grid: &gtk::FlowBox, status: &gtk::Label, pal: &Palette) {
  let mut child = grid.first_child();
  while let Some(widget) = child {
    child = widget.next_sibling();
    grid.remove(&widget);
  }
  let base = model::downloads_dir();
  let entries = model::list_downloads();
  for entry in &entries {
    grid.insert(&folder_cell(&base, entry, pal), -1);
  }
  status.set_markup(&status_markup(&entries, pal));
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item_label(entry: &MenuEntry) -> Option<&str> {
    match entry {
      MenuEntry::Item(item) => Some(item.label.as_str()),
      _ => None,
    }
  }

  #[test]
  fn empty_space_menu_structure() {
    let entries = empty_space_menu();
    assert_eq!(entries.len(), 5);
    assert!(matches!(entries[1], MenuEntry::Divider));
    assert!(matches!(entries[3], MenuEntry::Divider));
    match &entries[4] {
      MenuEntry::Submenu { title, items } => {
        assert_eq!(title, &lang::t("context.new_file"));
        assert_eq!(items.len(), 1);
        assert_eq!(item_label(&items[0]), Some(lang::t("context.text_file").as_str()));
      }
      _ => panic!("last entry must be the New File submenu"),
    }
  }
}
