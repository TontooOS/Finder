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
use crate::prefs;
use crate::TontooUI::{
  ContentUnavailableView, ContextMenu, MenuEntry, MenuItem, Sidebar, SidebarIcon, Toolbar,
  ToolbarItem,
};
use crate::UIKit::prelude::*;
use crate::UIKit::widget::{WidgetId, next_widget_id};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
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

fn fitted_picture(paintable: &gdk4::Paintable) -> gtk::Picture {
  fitted_picture_sized(paintable, ARTWORK)
}

/// Fixed square artwork view for any icon file. `gtk::Picture` with
/// `Contain` scales every source (192px app icons, large PNGs,
/// 64px SVGs) into the same square, so all cells look identical.
/// `can_shrink` is required, otherwise large sources would force
/// their natural size onto the grid.
fn fitted_picture_sized(paintable: &gdk4::Paintable, size: i32) -> gtk::Picture {
  let picture = gtk::Picture::for_paintable(paintable);
  picture.set_content_fit(gtk::ContentFit::Contain);
  picture.set_can_shrink(true);
  picture.set_size_request(size, size);
  picture.set_halign(gtk::Align::Center);
  picture
}

/// Fixed artwork size (square) shared by every grid cell.
/// Mirrors `icons::PREVIEW_SIZE` so cached previews match the screen.
const ARTWORK: i32 = icons::PREVIEW_SIZE as i32;
/// Folder artwork renders slightly larger: the folder glyph carries
/// transparent padding while app and document icons are full-bleed,
/// so equal boxes made folders look smaller. 54px keeps visual
/// parity within the 84px cell.
const FOLDER_ARTWORK: i32 = 54;
/// Fixed square cell size: artwork plus two label lines fit inside,
/// so every cell measures the same in all four directions.
const CELL: i32 = 84;

/// Light artwork viewing a shared icon texture. The file is decoded
/// and rasterized once per process (`icons::shared_paintable`);
/// every cell gets a cheap view scaled into the fixed square
/// instead of paying ~30ms of SVG rasterization per rebuild.
fn icon_image(path: &std::path::Path) -> gtk::Picture {
  icon_image_sized(path, ARTWORK)
}

/// Same as `icon_image` with an explicit square size (folders render
/// slightly larger for visual parity, see `FOLDER_ARTWORK`).
fn icon_image_sized(path: &std::path::Path, size: i32) -> gtk::Picture {
  match icons::shared_paintable(path) {
    Some(paintable) => fitted_picture_sized(&paintable, size),
    None => {
      let picture = gtk::Picture::for_filename(path);
      picture.set_content_fit(gtk::ContentFit::Contain);
      picture.set_can_shrink(true);
      picture.set_size_request(size, size);
      picture.set_halign(gtk::Align::Center);
      picture
    }
  }
}

/// Default grid folder icon (`scalable/folder.svg`). Falls back to CSS
/// artwork when the file is missing so the cell never renders empty.
fn folder_icon_art() -> gtk::Box {
  match icons::folder_icon("folder.svg") {
    Some(path) => {
      let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
      holder.set_halign(gtk::Align::Center);
      holder.append(&icon_image_sized(&path, FOLDER_ARTWORK));
      holder
    }
    None => folder_art(),
  }
}

/// Hide the idle popover inside a `ContextMenu` wrapper. The popup
/// menu content carries `min-width: 180px`; left visible it drives
/// the wrapped widget's minimum width (FlowBox columns grew to
/// ~230px, showing 5 instead of 8). `popup()` re-shows it on
/// right-click, so hiding only affects the idle state.
fn hide_idle_popover(wrapper: &gtk::Widget) {
  let mut cursor = wrapper.first_child();
  while let Some(widget) = cursor {
    cursor = widget.next_sibling();
    if let Ok(pop) = widget.clone().downcast::<gtk::Popover>() {
      pop.set_visible(false);
    }
  }
}

/// Wrap one widget (icon or text) with the per-file context menu.
/// The menu hugs content: presses on cell padding, row gaps or the
/// date/size columns fall through to the empty-space menu below,
/// whose gesture then fires instead.
fn file_menu_wrap(
  inner: impl IsA<gtk::Widget>,
  base: &std::path::Path,
  entry: &model::DirEntry,
  session: &SharedSession,
  refresh: &Refresh,
) -> gtk::Widget {
  let menu = ContextMenu::new(GtkWrap::wrap(inner)).entries(file_menu_entries(
    base,
    entry,
    session,
    refresh,
  ));
  let wrapped = menu.to_gtk();
  hide_idle_popover(&wrapped);
  wrapped
}

/// Small type icon at the far left of a list row.
const LIST_ICON: i32 = 20;
/// Fixed width of the modified-date column (header and rows share it).
const DATE_WIDTH: i32 = 150;
/// Fixed width of the size column (header and rows share it).
const SIZE_WIDTH: i32 = 110;

/// The two view buttons in toolbar order (grid, list). A `Toolbar`
/// renders root[title-area, bar] with one glass group holding one
/// `GtkButton` per item; this collects those buttons so the active
/// view can carry a select indicator.
fn collect_view_buttons(toolbar: &gtk::Widget) -> Vec<gtk::Button> {
  fn push_buttons(node: &gtk::Widget, out: &mut Vec<gtk::Button>) {
    let mut cursor = node.first_child();
    while let Some(widget) = cursor {
      cursor = widget.next_sibling();
      if let Ok(btn) = widget.clone().downcast::<gtk::Button>() {
        out.push(btn);
      } else {
        push_buttons(&widget, out);
      }
    }
  }
  let mut buttons = Vec::new();
  push_buttons(toolbar, &mut buttons);
  eprintln!("[finder][view] found {} toolbar buttons", buttons.len());
  buttons
}

/// Select indicator on the active view button (grid first, list
/// second), mirroring the sidebar selection.
fn apply_view_indicator(buttons: &[gtk::Button], mode: prefs::ViewMode) {
  for (index, btn) in buttons.iter().enumerate() {
    let active = (index == 0) == (mode == prefs::ViewMode::Grid);
    if active {
      btn.add_css_class("fd-view-on");
    } else {
      btn.remove_css_class("fd-view-on");
    }
  }
}

/// Shared handles for rebuilding either view.
#[derive(Clone)]
struct ViewCtx {
  slot: gtk::Box,
  grid: gtk::FlowBox,
  grid_menu: gtk::Widget,
  status: gtk::Label,
  pal: Palette,
  base: std::path::PathBuf,
  session: SharedSession,
  refresh: Refresh,
  mode: Rc<RefCell<prefs::ViewMode>>,
}

/// Rebuild callback for the active view (main thread only).
type Rebuild = Rc<dyn Fn()>;

/// Placeholder for an empty or unreadable directory.
fn empty_view() -> gtk::Widget {
  let empty = ContentUnavailableView::new()
    .title(lang::t("detail.empty"))
    .message(lang::t("detail.empty.hint"));
  let empty_gtk = empty.to_gtk();
  empty_gtk.set_hexpand(true);
  empty_gtk.set_vexpand(true);
  empty_gtk
}

/// Rebuild the content area for the active view: icon grid or list.
fn refresh_content(ctx: &ViewCtx, rebuild: &Rebuild) {
  let mut child = ctx.slot.first_child();
  while let Some(widget) = child {
    child = widget.next_sibling();
    ctx.slot.remove(&widget);
  }
  match *ctx.mode.borrow() {
    prefs::ViewMode::Grid => {
      if model::list_dir(&ctx.base).is_empty() {
        ctx.slot.append(&empty_view());
        ctx
          .status
          .set_markup(&status_markup(&[], &ctx.pal));
      } else {
        refresh_grid(
          &ctx.grid,
          &ctx.status,
          &ctx.pal,
          &ctx.base,
          &ctx.session,
          &ctx.refresh,
        );
        ctx.slot.append(&ctx.grid_menu);
      }
    }
    prefs::ViewMode::List => refresh_list(ctx, rebuild),
  }
}

/// List header row: Name (expands) plus fixed Date Modified and Size
/// columns. Spacing, margins and widths mirror the rows so columns
/// align.
fn list_header(pal: &Palette) -> gtk::Box {
  let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
  row.set_margin_start(16);
  row.set_margin_end(16);
  row.set_margin_top(4);
  row.set_margin_bottom(4);
  let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  spacer.set_size_request(LIST_ICON, -1);
  row.append(&spacer);
  let name = markup_label(&lang::t("list.name"), 12, "bold", pal.secondary);
  name.set_halign(gtk::Align::Start);
  name.set_hexpand(true);
  row.append(&name);
  let date = markup_label(&lang::t("list.date"), 12, "bold", pal.secondary);
  date.set_size_request(DATE_WIDTH, -1);
  date.set_halign(gtk::Align::Center);
  row.append(&date);
  let size = markup_label(&lang::t("list.size"), 12, "bold", pal.secondary);
  size.set_size_request(SIZE_WIDTH, -1);
  size.set_halign(gtk::Align::End);
  row.append(&size);
  row
}

/// Small type icon at the far left of a list row. Photos and videos
/// show their placeholder (never decoded thumbnails), matching the
/// reference layout.
fn row_icon(base: &std::path::Path, entry: &model::DirEntry) -> gtk::Widget {
  if entry.is_app {
    let full = base.join(&entry.name);
    if let Some(icon) = icons::app_icon(&full) {
      return icon_image_sized(&icon, LIST_ICON).upcast();
    }
  } else if entry.is_dir {
    if let Some(folder) = icons::folder_icon("folder.svg") {
      return icon_image_sized(&folder, LIST_ICON).upcast();
    }
  } else {
    let icon = match model::file_kind(&entry.ext) {
      model::FileKind::Image => icons::image_placeholder(),
      model::FileKind::Video => icons::video_placeholder().or_else(icons::video_icon),
      model::FileKind::Audio => icons::audio_icon(),
      model::FileKind::Archive => icons::archive_icon(),
      model::FileKind::Other => model::document_icon(&entry.ext)
        .and_then(icons::extension_icon)
        .or_else(icons::generic_file_icon),
    };
    if let Some(path) = icon {
      return icon_image_sized(&path, LIST_ICON).upcast();
    }
  }
  match icons::generic_file_icon() {
    Some(path) => icon_image_sized(&path, LIST_ICON).upcast(),
    None => {
      let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
      gap.set_size_request(LIST_ICON, LIST_ICON);
      gap.upcast()
    }
  }
}

/// Inline rename field for a list row. Enter commits (stays open on
/// invalid names), Escape cancels; both rebuild the active view.
fn list_edit_field(
  base: &std::path::Path,
  entry: &model::DirEntry,
  initial: &str,
  session: &SharedSession,
  rebuild: &Rebuild,
) -> gtk::Entry {
  let field = gtk::Entry::new();
  field.set_text(initial);
  field.set_hexpand(true);
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
  let commit_rebuild = rebuild.clone();
  field.connect_activate(move |entry| {
    let disk = model::DirEntry {
      name: disk_name.clone(),
      is_dir,
      is_app,
      ext: ext.clone(),
    };
    eprintln!(
      "[finder][rename] enter pressed in list, typed={:?}",
      entry.text()
    );
    if commit_rename(&commit_base, &disk, &entry.text()) {
      if let Ok(mut guard) = commit_session.lock() {
        *guard = None;
      }
      commit_rebuild();
    }
  });

  let cancel_session = session.clone();
  let cancel_rebuild = rebuild.clone();
  let keys = gtk::EventControllerKey::new();
  keys.connect_key_pressed(move |_, key, _, _| {
    if key == gdk4::Key::Escape {
      if let Ok(mut guard) = cancel_session.lock() {
        *guard = None;
      }
      cancel_rebuild();
      glib::Propagation::Stop
    } else {
      glib::Propagation::Proceed
    }
  });
  field.add_controller(keys);

  field
}

/// One list row: icon, name (or inline rename field), modified date,
/// size. Folders show no size. Carries the per-file context menu.
fn list_row(
  base: &std::path::Path,
  entry: &model::DirEntry,
  pal: &Palette,
  german: bool,
  session: &SharedSession,
  refresh: &Refresh,
  rebuild: &Rebuild,
) -> gtk::ListBoxRow {
  let row = gtk::ListBoxRow::new();
  row.add_css_class("fd-row");
  let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
  inner.set_margin_start(16);
  inner.set_margin_end(16);
  inner.set_margin_top(2);
  inner.set_margin_bottom(2);

  // Icon plus name hug content under one file menu; the expanding
  // gap, date and size stay outside it, so presses there fall
  // through to the empty-space menu. Left-click selection is
  // unaffected (handled by the ListBox itself).
  let namebox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
  namebox.append(&row_icon(base, entry));

  let shown = model::display_name(&entry.name, entry.is_dir);
  let editing = session
    .lock()
    .ok()
    .and_then(|guard| guard.clone())
    .map(|edit| edit.path == base.join(&entry.name))
    .unwrap_or(false);
  if editing {
    namebox.append(&list_edit_field(base, entry, &shown, session, rebuild));
  } else {
    let label = markup_label(&shown, 13, "normal", pal.fg);
    label.set_halign(gtk::Align::Start);
    label.set_max_width_chars(32);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.add_css_class("fd-label");
    namebox.append(&label);
  }
  inner.append(&file_menu_wrap(namebox, base, entry, session, refresh));

  let gap = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  gap.set_hexpand(true);
  inner.append(&gap);

  let meta = model::file_meta(base, &entry.name);
  let date = markup_label(&model::format_mtime(meta.mtime_secs, german), 12, "normal", pal.secondary);
  date.set_size_request(DATE_WIDTH, -1);
  date.set_halign(gtk::Align::Center);
  inner.append(&date);

  let size_text = if entry.is_dir {
    String::new()
  } else {
    model::format_size(meta.len)
  };
  let size = markup_label(&size_text, 12, "normal", pal.secondary);
  size.set_size_request(SIZE_WIDTH, -1);
  size.set_halign(gtk::Align::End);
  inner.append(&size);

  row.set_child(Some(&inner));
  row
}

/// Rebuild the list view: fixed header plus one row per entry.
fn refresh_list(ctx: &ViewCtx, rebuild: &Rebuild) {
  let t0 = std::time::Instant::now();
  let entries = model::list_dir(&ctx.base);
  let german = crate::lang::locale() == "de_de";
  if entries.is_empty() {
    ctx.slot.append(&empty_view());
  } else {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.append(&list_header(&ctx.pal));
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    crate::UIKit::widget::apply_css(
      &list,
      &format!(
        ".finder-list {{ background-color: {}; }} \
         .fd-row:selected {{ background-color: rgba(10,132,255,0.30); border-radius: 6px; }} \
         .fd-row:selected label {{ color: #ffffff; }}",
        ctx.pal.bg
      ),
    );
    list.add_css_class("finder-list");
    for entry in &entries {
      let row = list_row(
        &ctx.base,
        entry,
        &ctx.pal,
        german,
        &ctx.session,
        &ctx.refresh,
        rebuild,
      );
      list.append(&row);
      // Right-click also selects the row (Finder behavior): a capture
      // gesture runs before the menu gestures and only selects, so
      // the file menu still opens normally.
      let list_c = list.clone();
      let row_c = row.clone();
      let press = gtk::GestureClick::new();
      press.set_button(3);
      press.set_propagation_phase(gtk::PropagationPhase::Capture);
      press.connect_pressed(move |_, _, _, _| {
        list_c.select_row(Some(&row_c));
      });
      row.add_controller(press);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    crate::UIKit::widget::apply_css(
      &scroll,
      &format!(".finder-scroll {{ background-color: {}; }}", ctx.pal.bg),
    );
    scroll.add_css_class("finder-scroll");
    let menu = ContextMenu::new(GtkWrap::wrap(scroll.clone()))
      .entries(empty_space_menu(&ctx.session, &ctx.refresh));
    let menu_gtk = menu.to_gtk();
    hide_idle_popover(&menu_gtk);
    menu_gtk.set_hexpand(true);
    menu_gtk.set_vexpand(true);
    content.append(&menu_gtk);
    ctx.slot.append(&content);
  }
  ctx
    .status
    .set_markup(&status_markup(&entries, &ctx.pal));
  eprintln!(
    "[finder][refresh] rebuilt {} list rows in {}ms",
    entries.len(),
    t0.elapsed().as_millis()
  );
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
  holder.append(&icon_image(path));
  holder
}

/// Grid artwork for one file: shared icon texture for icons, or a
/// rounded-corner texture for photos and video frames (GTK CSS
/// `border-radius` does not clip image content). Undecodable photos
/// show the `image.png` placeholder instead of a broken image.
fn preview_art(path: &std::path::Path, round: bool) -> gtk::Widget {
  if round {
    // Cheap path first: tiny cached PNG with corners already baked
    // in, no decode on the main thread. Falls through to in-memory
    // decoding only when the cache cannot be built.
    if let Some(cached) = icons::photo_preview(path) {
      return icon_image(&cached).upcast();
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
      // Exact 64px backing: Picture scales any texture into the
      // fixed square, so decoded pixels never blow up the grid.
      let mut square = icons::cover_square(&img, ARTWORK as u32);
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
      return fitted_picture(&texture.upcast()).upcast();
    }
    // Undecodable photo: show the image placeholder, never a broken file.
    if let Some(placeholder) = icons::image_placeholder() {
      return icon_image(&placeholder).upcast();
    }
  }
  icon_image(path).upcast()
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
  // Fixed square cells: artwork (64) plus label always measure
  // CELL x CELL, so the grid looks identical in all four
  // directions. The FlowBox reflows the column count (4-8)
  // dynamically when the window is resized in any direction.
  let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
  cell.set_size_request(CELL, CELL);
  cell.set_halign(gtk::Align::Center);
  cell.set_valign(gtk::Align::Start);
  cell.add_css_class("fd-cell");

  // Icon and text carry their own file menu each, so padding
  // presses fall through to the empty-space menu. Left-click
  // selection is unaffected (handled by the FlowBox itself).
  cell.append(&file_menu_wrap(
    folder_artwork(base, entry),
    base,
    entry,
    session,
    refresh,
  ));

  let shown = model::display_name(&entry.name, entry.is_dir);
  let editing = session
    .lock()
    .ok()
    .and_then(|guard| guard.clone())
    .map(|edit| edit.path == base.join(&entry.name))
    .unwrap_or(false);
  if editing {
    cell.append(&file_menu_wrap(
      edit_field(base, entry, &shown, session, grid, status, pal, refresh),
      base,
      entry,
      session,
      refresh,
    ));
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
    cell.append(&file_menu_wrap(label, base, entry, session, refresh));
  }

  cell.upcast()
}

/// Grid artwork for one entry (icon only, no label): `.app`
/// bundles show the app icon rendered once through CoreIcon,
/// directories the default folder icon, images the picture itself,
/// videos the cached first-second frame (placeholder while `ffmpeg`
/// is missing), audio files the music icon, archives the zip icon.
/// Other files show their document icon; files without any icon
/// render an empty holder so the name keeps its position.
fn folder_artwork(base: &std::path::Path, entry: &model::DirEntry) -> gtk::Widget {
  if entry.is_app {
    let full = base.join(&entry.name);
    match icons::app_icon(&full) {
      Some(icon) => preview_image(&icon).upcast(),
      None => folder_art().upcast(),
    }
  } else if entry.is_dir {
    folder_icon_art().upcast()
  } else {
    let full = base.join(&entry.name);
    match model::file_kind(&entry.ext) {
      model::FileKind::Image => rounded_preview(&full).upcast(),
      model::FileKind::Video => match icons::video_thumb(&full) {
        Some(thumb) => rounded_preview(&thumb).upcast(),
        None => match icons::video_placeholder().or_else(icons::video_icon) {
          Some(icon) => preview_image(&icon).upcast(),
          None => folder_art().upcast(),
        },
      },
      model::FileKind::Audio => match icons::audio_icon() {
        Some(icon) => preview_image(&icon).upcast(),
        None => folder_art().upcast(),
      },
      model::FileKind::Archive => match icons::archive_icon() {
        Some(icon) => preview_image(&icon).upcast(),
        None => folder_art().upcast(),
      },
      model::FileKind::Other => {
        let icon = model::document_icon(&entry.ext)
          .and_then(icons::extension_icon)
          .or_else(icons::generic_file_icon);
        match icon {
          Some(path) => preview_image(&path).upcast(),
          None => {
            let gap = gtk::Box::new(gtk::Orientation::Vertical, 0);
            gap.set_size_request(ARTWORK, ARTWORK);
            gap.upcast()
          }
        }
      }
    }
  }
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

    let mode_state: Rc<RefCell<prefs::ViewMode>> =
      Rc::new(RefCell::new(prefs::load_view_mode()));
    let (view_tx, view_rx) = std::sync::mpsc::channel::<prefs::ViewMode>();
    let grid_tx = view_tx.clone();
    let list_tx = view_tx.clone();
    let views = Toolbar::new()
      .item(ToolbarItem::new("square.grid.2x2").on_click(move || {
        eprintln!("[finder][view] grid button clicked");
        let _ = grid_tx.send(prefs::ViewMode::Grid);
      }))
      .item(ToolbarItem::new("list.bullet").on_click(move || {
        eprintln!("[finder][view] list button clicked");
        let _ = list_tx.send(prefs::ViewMode::List);
      }));
    let views_gtk = views.to_gtk();
    let view_buttons = collect_view_buttons(&views_gtk);
    for btn in &view_buttons {
      crate::UIKit::widget::apply_css(
        btn,
        ".fd-view-on { background-color: rgba(10,132,255,0.85); }",
      );
    }
    apply_view_indicator(&view_buttons, *mode_state.borrow());
    toolbar_row.append(&views_gtk);

    let actions = Toolbar::new()
      .item(ToolbarItem::new("square.and.arrow.up").on_click(|| println!("Finder share")))
      .item(ToolbarItem::new("magnifyingglass").on_click(|| println!("Finder search")));
    toolbar_row.append(&actions.to_gtk());

    detail.append(&toolbar_row);

    let base = model::downloads_dir();
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
    // Content slot holding the grid or the list for the active view.
    let content_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_slot.set_hexpand(true);
    content_slot.set_vexpand(true);

    // Grid scroll with the empty-space menu, built once and re-shown
    // on every switch back to the grid.
    let grid_scroll = gtk::ScrolledWindow::new();
    grid_scroll.set_child(Some(&grid));
    grid_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    grid_scroll.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    grid_scroll.set_hexpand(true);
    grid_scroll.set_vexpand(true);
    crate::UIKit::widget::apply_css(
      &grid_scroll,
      &format!(".finder-scroll {{ background-color: {}; }}", pal.bg),
    );
    grid_scroll.add_css_class("finder-scroll");
    let grid_menu = {
      let wrapped = ContextMenu::new(GtkWrap::wrap(grid_scroll.clone()))
        .entries(empty_space_menu(&session, &refresh))
        .to_gtk();
      hide_idle_popover(&wrapped);
      wrapped.set_hexpand(true);
      wrapped.set_vexpand(true);
      wrapped
    };

    let ctx = ViewCtx {
      slot: content_slot.clone(),
      grid: grid.clone(),
      grid_menu: grid_menu.clone(),
      status: status_label.clone(),
      pal,
      base: base.clone(),
      session: session.clone(),
      refresh: refresh.clone(),
      mode: mode_state.clone(),
    };
    detail.append(&content_slot);

    let status = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status.set_margin_top(6);
    status.set_margin_bottom(8);
    status.append(&status_label);
    detail.append(&status);

    // Live updates: the notify watcher, the menu/edit refresh
    // signal and the view-switch signal share one 400ms main-thread
    // tick that drains bursts and rebuilds the active view once per
    // real change. The watcher also fires on plain file opens (every
    // rebuild opens files), so a metadata snapshot gates the
    // rebuild: without it the refresh retriggers itself and starves
    // the UI.
    let watch_rx = crate::watch::watch_dir(&base).map(|(watcher, rx)| {
      FOLDER_WATCHER.with(|slot| *slot.borrow_mut() = Some(watcher));
      rx
    });
    let session_tick = session.clone();
    let mut last_snapshot = model::snapshot(&base);
    // Rebuilds the active view (main thread only). List rows embed
    // a rebuild handle for rename commit/cancel; the handle is
    // installed after creation (a self-reference cannot be built in
    // a single step).
    let rebuild_cell: Rc<RefCell<Option<Rebuild>>> = Rc::new(RefCell::new(None));
    let rebuild_all: Rebuild = {
      let cell_c = rebuild_cell.clone();
      let ctx_c = ctx.clone();
      Rc::new(move || {
        if let Some(rebuild) = cell_c.borrow().as_ref().cloned() {
          refresh_content(&ctx_c, &rebuild);
        }
      })
    };
    *rebuild_cell.borrow_mut() = Some(rebuild_all.clone());
    refresh_content(&ctx, &rebuild_all);
    let rebuild_tick = rebuild_all.clone();
    let mode_tick = mode_state.clone();
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
      let mut view_switch = None;
      while let Ok(mode) = view_rx.try_recv() {
        view_switch = Some(mode);
      }
      if !watch_signaled && !menu_signaled && view_switch.is_none() {
        return glib::ControlFlow::Continue;
      }
      if let Some(mode) = view_switch {
        if mode != *mode_tick.borrow() {
          *mode_tick.borrow_mut() = mode;
          prefs::save_view_mode(mode);
          apply_view_indicator(&view_buttons, mode);
          eprintln!("[finder][view] switched to {}", mode.as_str());
        }
        let current = model::snapshot(&base);
        last_snapshot = current;
        rebuild_tick();
        eprintln!("[finder][view] rebuilt after switch");
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
            rebuild_tick();
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
        rebuild_tick();
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
  for (index, entry) in entries.iter().enumerate() {
    let t_cell = std::time::Instant::now();
    grid.insert(&folder_cell(base, entry, pal, session, refresh, grid, status), -1);
    // Right-click also selects the cell (Finder behavior): a capture
    // gesture on the FlowBoxChild runs before the menu gestures and
    // only selects, so the file menu still opens normally.
    if let Some(flow_child) = grid.child_at_index(index as i32) {
      let grid_c = grid.clone();
      let child_c = flow_child.clone();
      let press = gtk::GestureClick::new();
      press.set_button(3);
      press.set_propagation_phase(gtk::PropagationPhase::Capture);
      press.connect_pressed(move |_, _, _, _| {
        grid_c.select_child(&child_c);
      });
      flow_child.add_controller(press);
    }
    let ms = t_cell.elapsed().as_millis();
    if ms > 20 {
      eprintln!("[finder][refresh] slow cell: {} ({}ms)", entry.name, ms);
    }
  }
  status.set_markup(&status_markup(&entries, pal));
  // Column width driver check: FlowBox columns follow the widest
  // child minimum. Expect ~CELL; ~230px means the idle popover (or
  // another wrapper child) still drives the width.
  if let Some(first) = grid.child_at_index(0) {
    let (cmin, cnat, _, _) = first.measure(gtk::Orientation::Horizontal, -1);
    eprintln!("[finder][refresh] first column min={cmin}px nat={cnat}px");
  }
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
