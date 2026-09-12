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
use std::sync::{Arc, Mutex};

thread_local! {
  /// Keeps the downloads watcher alive for the app lifetime.
  static FOLDER_WATCHER: RefCell<Option<notify::RecommendedWatcher>> = RefCell::new(None);
}

/// Inline rename session: the entry path currently edited.
#[derive(Clone, Debug)]
struct EditSession {
  path: std::path::PathBuf,
}

/// Shared rename session (main thread only in practice, `Send` for the
/// menu callbacks).
type SharedSession = Arc<Mutex<Option<EditSession>>>;

/// Grid refresh signal shared by menu actions and edit commits.
/// The 400ms main-thread tick picks it up and rebuilds the grid, so
/// `Send`-bound menu callbacks never touch GTK widgets directly.
type Refresh = Arc<dyn Fn() + Send + Sync>;

/// Create a uniquely named folder (`Untitled Folder`, `Untitled
/// Folder 2`, ...) and return its path.
fn create_folder(base: &std::path::Path) -> Option<std::path::PathBuf> {
  let t0 = std::time::Instant::now();
  let stem = lang::t("folder.untitled");
  let mut candidate = base.join(&stem);
  let mut counter = 2;
  while candidate.exists() {
    candidate = base.join(format!("{stem} {counter}"));
    counter += 1;
  }
  let created = std::fs::create_dir(&candidate);
  eprintln!(
    "[finder][create_folder] create_dir {} ok={} in {}ms",
    candidate.display(),
    created.is_ok(),
    t0.elapsed().as_millis()
  );
  created.ok()?;
  Some(candidate)
}

/// Commit an inline rename. Returns true on success (caller refreshes);
/// on failure the session stays active so the name can be fixed.
fn commit_rename(base: &std::path::Path, entry: &model::DirEntry, typed: &str) -> bool {
  let t0 = std::time::Instant::now();
  let Some(new_name) = model::resolve_new_name(entry, typed) else {
    eprintln!(
      "[finder][commit_rename] rejected typed text {typed:?} for {}",
      entry.name
    );
    return false;
  };
  let target = base.join(&new_name);
  if target.exists() {
    eprintln!(
      "[finder][commit_rename] target exists: {}",
      target.display()
    );
    return false;
  }
  let ok = std::fs::rename(base.join(&entry.name), &target).is_ok();
  eprintln!(
    "[finder][commit_rename] {} -> {} ok={} in {}ms",
    entry.name,
    new_name,
    ok,
    t0.elapsed().as_millis()
  );
  ok
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
    // Cheap path first: tiny cached PNG with corners already baked
    // in, no decode on the main thread. Falls through to in-memory
    // decoding only when the cache cannot be built.
    if let Some(cached) = icons::photo_preview(path) {
      let image = gtk::Image::from_file(&cached);
      image.set_pixel_size(64);
      image.set_size_request(64, 64);
      image.set_halign(gtk::Align::Center);
      return image.upcast();
    }
    let t0 = std::time::Instant::now();
    let decoded = image::open(path);
    eprintln!(
      "[finder][preview] decode {} ok={} in {}ms",
      path.display(),
      decoded.is_ok(),
      t0.elapsed().as_millis()
    );
    if let Ok(img) = decoded {
      // Exact 64px backing: GtkImage pixel-size does not scale
      // paintables, so a larger texture would blow up the grid.
      let mut square = icons::cover_square(&img, 64);
      icons::round_corners(&mut square, 10);
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
      preview.set_size_request(64, 64);
      preview.set_halign(gtk::Align::Center);
      return preview.upcast();
    }
  }
  let image = gtk::Image::from_file(path);
  image.set_pixel_size(64);
  image.set_size_request(64, 64);
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

/// Tag dot colors for the file menu Tags row (red, orange, yellow,
/// green, blue, purple, gray).
const TAG_DOT_COLORS: [(u8, u8, u8); 7] = [
  (255, 59, 48),
  (255, 149, 0),
  (255, 204, 0),
  (52, 199, 89),
  (0, 122, 255),
  (175, 82, 222),
  (142, 142, 147),
];

/// File context menu entries for one entry. Rename opens inline
/// rename; the rest logs for now. Open With and Share carry a
/// trailing disclosure icon.
pub(crate) fn file_menu_entries(
  base: &std::path::Path,
  entry: &model::DirEntry,
  session: &SharedSession,
  refresh: &Refresh,
) -> Vec<MenuEntry> {
  let name = model::display_name(&entry.name, entry.is_dir);
  let rename_path = base.join(&entry.name);
  let rename_session = session.clone();
  let rename_refresh = refresh.clone();
  vec![
    MenuEntry::Item(
      MenuItem::new(lang::t("context.open")).on_activate(|| println!("Finder open")),
    ),
    MenuEntry::Item(MenuItem::new(lang::t("context.open_with")).trailing_icon("arrowtriangle.forward.fill")),
    MenuEntry::Divider,
    MenuEntry::Item(
      MenuItem::new(lang::t("context.move_to_trash"))
        .on_activate(|| println!("Finder move to trash")),
    ),
    MenuEntry::Divider,
    MenuEntry::Item(
      MenuItem::new(lang::t("context.get_info")).on_activate(|| println!("Finder get info")),
    ),
    MenuEntry::Item(
      MenuItem::new(lang::t("context.rename")).on_activate(move || {
        eprintln!("[finder][rename] menu clicked for {}", rename_path.display());
        if let Ok(mut guard) = rename_session.lock() {
          *guard = Some(EditSession { path: rename_path.clone() });
        }
        eprintln!("[finder][rename] session set, sending refresh signal");
        rename_refresh();
        println!("Finder rename");
      }),
    ),
    MenuEntry::Item(
      MenuItem::new(lang::t("context.compress")).on_activate(|| println!("Finder compress")),
    ),
    MenuEntry::Item(
      MenuItem::new(lang::t("context.duplicate")).on_activate(|| println!("Finder duplicate")),
    ),
    MenuEntry::Item(
      MenuItem::new(lang::t("context.share"))
        .trailing_icon("arrowtriangle.forward.fill")
        .on_activate(|| println!("Finder share")),
    ),
    MenuEntry::Divider,
    MenuEntry::Item(
      MenuItem::new(lang::t("context.copy").replace("{name}", &name))
        .on_activate(|| println!("Finder copy")),
    ),
    MenuEntry::Divider,
    MenuEntry::TagDots {
      title: lang::t("context.tags"),
      colors: TAG_DOT_COLORS.to_vec(),
    },
  ]
}

/// Apply or clear the selection highlight on one `FlowBoxChild`
/// (cell background plus white label). The cell box carries
/// `fd-cell`; the menu wrapper sits between child and cell.
pub(crate) fn set_cell_selected(flow_child: &gtk::FlowBoxChild, selected: bool) {
  let mut cell_opt = None;
  let mut cursor = flow_child.first_child();
  while let Some(widget) = cursor {
    cursor = widget.next_sibling();
    if widget.has_css_class("fd-cell") {
      cell_opt = Some(widget);
      break;
    }
    if let Some(inner) = widget.first_child() {
      if inner.has_css_class("fd-cell") {
        cell_opt = Some(inner);
        break;
      }
    }
  }
  let Some(cell) = cell_opt else {
    return;
  };
  if selected {
    cell.add_css_class("fd-sel");
  } else {
    cell.remove_css_class("fd-sel");
  }
  let mut label_cursor = cell.first_child();
  while let Some(widget) = label_cursor {
    label_cursor = widget.next_sibling();
    if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
      if selected {
        label.add_css_class("fd-lbl-sel");
      } else {
        label.remove_css_class("fd-lbl-sel");
      }
    }
  }
}

fn folder_cell(
  base: &std::path::Path,
  entry: &model::DirEntry,
  pal: &Palette,
  session: &SharedSession,
  refresh: &Refresh,
  grid: &gtk::FlowBox,
  status: &gtk::Label,
) -> gtk::Widget {
  let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
  cell.set_size_request(112, -1);
  cell.set_halign(gtk::Align::Center);
  cell.set_valign(gtk::Align::Start);
  cell.add_css_class("fd-cell");

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
  let editing = session
    .lock()
    .ok()
    .and_then(|guard| guard.clone())
    .map(|edit| edit.path == base.join(&entry.name))
    .unwrap_or(false);
  if editing {
    cell.append(&edit_field(base, entry, &shown, session, grid, status, pal, refresh));
  } else {
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
  }

  // Per-file context menu. The inner gesture claims the press first,
  // so the empty-space menu on the scroll area stays hidden over files.
  let menu = ContextMenu::new(GtkWrap::wrap(cell)).entries(file_menu_entries(
    base,
    entry,
    session,
    refresh,
  ));
  menu.to_gtk()
}

/// Inline rename field prefilled with the display name. Enter commits
/// (stays open on invalid names), Escape cancels. Rebuilds the grid
/// immediately through the passed widgets.
fn edit_field(
  base: &std::path::Path,
  entry: &model::DirEntry,
  initial: &str,
  session: &SharedSession,
  grid: &gtk::FlowBox,
  status: &gtk::Label,
  pal: &Palette,
  refresh: &Refresh,
) -> gtk::Entry {
  let field = gtk::Entry::new();
  field.set_text(initial);
  field.set_halign(gtk::Align::Center);
  field.set_width_chars(14);
  field.set_max_width_chars(24);
  let css = format!(
    ".fd-edit {{ font-family: '{}'; font-size: 12px; color: {}; }}",
    SF_PRO, "#1d1d1d"
  );
  crate::UIKit::widget::apply_css(&field, &css);
  field.add_css_class("fd-edit");
  field.connect_realize(|entry| {
    entry.grab_focus();
    entry.select_region(0, -1);
  });

  let disk_name = entry.name.clone();
  let is_dir = entry.is_dir;
  let is_app = entry.is_app;
  let ext = entry.ext.clone();
  let commit_base = base.to_path_buf();
  let commit_session = session.clone();
  let commit_grid = grid.clone();
  let commit_status = status.clone();
  let commit_pal = *pal;
  let commit_session2 = session.clone();
  let commit_refresh = refresh.clone();
  field.connect_activate(move |entry| {
    eprintln!("[finder][rename] enter pressed, typed={:?}", entry.text());
    let t0 = std::time::Instant::now();
    let disk = model::DirEntry {
      name: disk_name.clone(),
      is_dir,
      is_app,
      ext: ext.clone(),
    };
    if commit_rename(&commit_base, &disk, &entry.text()) {
      if let Ok(mut guard) = commit_session.lock() {
        *guard = None;
      }
      refresh_grid(
        &commit_grid,
        &commit_status,
        &commit_pal,
        &commit_base,
        &commit_session2,
        &commit_refresh,
      );
      eprintln!(
        "[finder][rename] commit + rebuild done in {}ms",
        t0.elapsed().as_millis()
      );
    } else {
      eprintln!(
        "[finder][rename] commit failed after {}ms, field stays open",
        t0.elapsed().as_millis()
      );
    }
  });

  let cancel_session = session.clone();
  let cancel_grid = grid.clone();
  let cancel_status = status.clone();
  let cancel_pal = *pal;
  let cancel_base = base.to_path_buf();
  let cancel_session2 = session.clone();
  let cancel_refresh = refresh.clone();
  let keys = gtk::EventControllerKey::new();
  keys.connect_key_pressed(move |_, key, _, _| {
    if key == gdk4::Key::Escape {
      if let Ok(mut guard) = cancel_session.lock() {
        *guard = None;
      }
      refresh_grid(
        &cancel_grid,
        &cancel_status,
        &cancel_pal,
        &cancel_base,
        &cancel_session2,
        &cancel_refresh,
      );
      glib::Propagation::Stop
    } else {
      glib::Propagation::Proceed
    }
  });
  field.add_controller(keys);

  field
}

/// Empty-space context menu entries: New Folder, divider, Get Info,
/// divider, New File submenu with Text File. Actions log for now,
/// except New Folder which creates a folder and opens inline rename.
pub(crate) fn empty_space_menu(session: &SharedSession, refresh: &Refresh) -> Vec<MenuEntry> {
  let new_base = model::downloads_dir();
  let new_session = session.clone();
  let new_refresh = refresh.clone();
  vec![
    MenuEntry::Item(MenuItem::new(lang::t("context.new_folder")).on_activate(move || {
      eprintln!("[finder][new_folder] menu clicked");
      if let Some(path) = create_folder(&new_base) {
        eprintln!("[finder][new_folder] session set for {}, sending refresh", path.display());
        if let Ok(mut guard) = new_session.lock() {
          *guard = Some(EditSession { path });
        }
        new_refresh();
      }
      println!("Finder new folder");
    })),
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
      .item(ToolbarItem::new("square.and.arrow.up").on_click(|| println!("Finder share")))
      .item(ToolbarItem::new("magnifyingglass").on_click(|| println!("Finder search")));
    toolbar_row.append(&actions.to_gtk());

    detail.append(&toolbar_row);

    let base = model::downloads_dir();
    let entries = model::list_downloads();
    let session: SharedSession = Arc::new(Mutex::new(None));

    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::Single);
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
    // Single-click selection: highlight the cell and whiten its label.
    crate::UIKit::widget::apply_css(
      &grid,
      ".fd-sel { background-color: rgba(10,132,255,0.30); border-radius: 8px; } \
       .fd-label.fd-lbl-sel { color: #ffffff; }",
    );
    grid.connect_selected_children_changed(|flow| {
      let mut cursor = flow.first_child();
      while let Some(widget) = cursor {
        cursor = widget.next_sibling();
        if let Ok(child) = widget.clone().downcast::<gtk::FlowBoxChild>() {
          set_cell_selected(&child, child.is_selected());
        }
      }
    });
    let status_label = gtk::Label::new(None);
    status_label.set_use_markup(true);
    status_label.set_halign(gtk::Align::Center);
    // Menu callbacks are Send-bound and must not touch GTK widgets:
    // they send a refresh signal that the 400ms tick below picks up.
    let (refresh_tx, refresh_rx) = std::sync::mpsc::channel::<()>();
    let refresh: Refresh = Arc::new(move || {
      let _ = refresh_tx.send(());
    });
    let pal_c = pal;
    let do_refresh = {
      let grid_c = grid.clone();
      let status_c = status_label.clone();
      let base_c = base.clone();
      let session_c = session.clone();
      let refresh_c = refresh.clone();
      move || {
        refresh_grid(&grid_c, &status_c, &pal_c, &base_c, &session_c, &refresh_c);
      }
    };
    refresh_grid(&grid, &status_label, &pal, &base, &session, &refresh);

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

    // Right-click on empty space shows the context menu; the inner
    // cell menus claim the press first, so it stays hidden over files.
    let menu = ContextMenu::new(GtkWrap::wrap(scroll.clone()))
      .entries(empty_space_menu(&session, &refresh));
    let menu_gtk = menu.to_gtk();
    menu_gtk.set_hexpand(true);
    menu_gtk.set_vexpand(true);
    detail.append(&menu_gtk);

    let status = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status.set_margin_top(6);
    status.set_margin_bottom(8);
    status.append(&status_label);
    detail.append(&status);

    // Live updates: the notify watcher and the menu/edit refresh
    // signal share one 400ms main-thread tick that drains bursts and
    // rebuilds the grid once per real change. The watcher also fires
    // on plain file opens (every rebuild opens files), so a metadata
    // snapshot gates the rebuild: without it the refresh retriggers
    // itself and starves the UI.
    let watch_rx = crate::watch::watch_dir(&base).map(|(watcher, rx)| {
      FOLDER_WATCHER.with(|slot| *slot.borrow_mut() = Some(watcher));
      rx
    });
    let session_tick = session.clone();
    let mut last_snapshot = model::snapshot(&base);
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
      let mut watch_signaled = false;
      if let Some(rx) = &watch_rx {
        while rx.try_recv().is_ok() {
          watch_signaled = true;
        }
      }
      let mut menu_signaled = false;
      while refresh_rx.try_recv().is_ok() {
        menu_signaled = true;
      }
      if !watch_signaled && !menu_signaled {
        return glib::ControlFlow::Continue;
      }
      let editing = !watch_refresh_allowed(&session_tick);
      eprintln!(
        "[finder][tick] watch={watch_signaled} menu={menu_signaled} editing={editing}"
      );
      let t0 = std::time::Instant::now();
      if watch_refresh_allowed(&session_tick) {
        if watch_signaled || menu_signaled {
          let current = model::snapshot(&base);
          if current != last_snapshot {
            eprintln!(
              "[finder][tick] snapshot changed ({} entries), rebuilding",
              current.len()
            );
            last_snapshot = current;
            do_refresh();
            eprintln!("[finder][tick] rebuild took {}ms", t0.elapsed().as_millis());
          } else {
            eprintln!("[finder][tick] snapshot identical, skip rebuild");
          }
        }
      } else if tick_should_rebuild(true, watch_signaled, menu_signaled) {
        // Inline rename open: drop watcher bursts (every rebuild opens
        // files, which the watcher reports, which would rebuild again
        // every 400ms and steal focus). Explicit menu/edit signals
        // still rebuild; the snapshot is synced so no stale rebuild
        // fires after the edit commits.
        last_snapshot = model::snapshot(&base);
        do_refresh();
        eprintln!(
          "[finder][tick] edit rebuild took {}ms",
          t0.elapsed().as_millis()
        );
      } else {
        eprintln!("[finder][tick] watcher burst dropped (editing)");
      }
      glib::ControlFlow::Continue
    });

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

/// Whether a watcher event may rebuild the grid. Paused while an
/// inline rename is open, so event bursts never destroy typed text
/// or steal focus.
fn watch_refresh_allowed(session: &SharedSession) -> bool {
  session.lock().map(|guard| guard.is_none()).unwrap_or(false)
}

/// Whether this tick rebuilds the grid. While an inline rename is
/// open (`editing`), only explicit menu/edit signals rebuild;
/// watcher bursts are dropped (each rebuild opens files, which the
/// watcher reports, which would otherwise rebuild every tick and
/// steal focus). Outside edits, either signal rebuilds (gated by
/// the snapshot diff at the call site).
fn tick_should_rebuild(editing: bool, watch_signaled: bool, menu_signaled: bool) -> bool {
  if editing {
    menu_signaled
  } else {
    watch_signaled || menu_signaled
  }
}

/// Rebuild the grid and status line from the live directory.
fn refresh_grid(  grid: &gtk::FlowBox,
  status: &gtk::Label,
  pal: &Palette,
  base: &std::path::Path,
  session: &SharedSession,
  refresh: &Refresh,
) {
  let t0 = std::time::Instant::now();
  let mut child = grid.first_child();
  while let Some(widget) = child {
    child = widget.next_sibling();
    grid.remove(&widget);
  }
  let t_list = std::time::Instant::now();
  let entries = model::list_dir(base);
  eprintln!(
    "[finder][refresh] list_dir {}: {} entries in {}ms",
    base.display(),
    entries.len(),
    t_list.elapsed().as_millis()
  );
  for entry in &entries {
    let t_cell = std::time::Instant::now();
    grid.insert(&folder_cell(base, entry, pal, session, refresh, grid, status), -1);
    let ms = t_cell.elapsed().as_millis();
    if ms > 20 {
      eprintln!("[finder][refresh] slow cell: {} ({}ms)", entry.name, ms);
    }
  }
  status.set_markup(&status_markup(&entries, pal));
  eprintln!(
    "[finder][refresh] rebuilt {} cells in {}ms",
    entries.len(),
    t0.elapsed().as_millis()
  );
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

  fn test_context() -> (SharedSession, Refresh, std::path::PathBuf) {
    let session: SharedSession = Arc::new(Mutex::new(None));
    let refresh: Refresh = Arc::new(|| {});
    (session, refresh, std::env::temp_dir())
  }

  #[test]
  fn empty_space_menu_structure() {
    let (session, refresh, _) = test_context();
    let entries = empty_space_menu(&session, &refresh);
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

  #[test]
  fn file_menu_structure() {
    let (session, refresh, base) = test_context();
    let entry = model::DirEntry {
      name: "wallpaper.jpg".to_string(),
      is_dir: false,
      is_app: false,
      ext: "jpg".to_string(),
    };
    let entries = file_menu_entries(&base, &entry, &session, &refresh);
    assert_eq!(entries.len(), 14);
    assert_eq!(item_label(&entries[0]), Some(lang::t("context.open").as_str()));
    match &entries[1] {
      MenuEntry::Item(item) => {
        assert_eq!(item.label, lang::t("context.open_with"));
        assert_eq!(
          item.trailing_icon.as_deref(),
          Some("arrowtriangle.forward.fill")
        );
      }
      _ => panic!("second entry must be Open With"),
    }
    assert!(matches!(entries[2], MenuEntry::Divider));
    assert!(matches!(entries[4], MenuEntry::Divider));
    assert!(matches!(entries[10], MenuEntry::Divider));
    assert_eq!(
      item_label(&entries[11]),
      Some(lang::t("context.copy").replace("{name}", "wallpaper").as_str())
    );
    assert!(matches!(entries[12], MenuEntry::Divider));
    match &entries[13] {
      MenuEntry::TagDots { title, colors } => {
        assert_eq!(title, &lang::t("context.tags"));
        assert_eq!(colors.len(), 7);
      }
      _ => panic!("last entry must be the Tags row"),
    }
  }

  /// Watcher-driven rebuilds pause while an inline rename is open,
  /// so event bursts never destroy typed text or steal focus. Menu
  /// and edit refresh signals always rebuild.
  #[test]
  fn watch_refresh_pauses_during_edit() {
    let session: SharedSession = Arc::new(Mutex::new(None));
    assert!(watch_refresh_allowed(&session));
    *session.lock().unwrap() = Some(EditSession {
      path: std::path::PathBuf::from("/tmp/x"),
    });
    assert!(!watch_refresh_allowed(&session));
  }

  #[test]
  fn tick_rebuild_signal_matrix() {
    // Idle: either signal rebuilds (snapshot gate at call site).
    assert!(tick_should_rebuild(false, true, false));
    assert!(tick_should_rebuild(false, false, true));
    assert!(tick_should_rebuild(false, true, true));
    assert!(!tick_should_rebuild(false, false, false));
    // Editing: watcher bursts dropped, menu signals rebuild.
    assert!(!tick_should_rebuild(true, true, false));
    assert!(tick_should_rebuild(true, false, true));
    assert!(tick_should_rebuild(true, true, true));
    assert!(!tick_should_rebuild(true, false, false));
  }
}
