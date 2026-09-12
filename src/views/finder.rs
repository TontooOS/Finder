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
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

thread_local! {
  /// Keeps the downloads watcher alive for the app lifetime.
  static FOLDER_WATCHER: RefCell<Option<notify::RecommendedWatcher>> = RefCell::new(None);
  /// Active search filter, already normalized (trimmed, lowercase).
  /// Empty means no filtering. Main thread only.
  static SEARCH_QUERY: RefCell<String> = RefCell::new(String::new());
}

/// Current search filter (normalized, may be empty).
fn search_query() -> String {
  SEARCH_QUERY.with(|slot| slot.borrow().clone())
}

/// Set the search filter from raw input.
fn set_search_query(raw: &str) {
  SEARCH_QUERY.with(|slot| *slot.borrow_mut() = model::normalize_query(raw));
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
/// Folder (2)`, ...) and return its path.
fn create_folder(base: &std::path::Path) -> Option<std::path::PathBuf> {
  let t0 = std::time::Instant::now();
  let stem = lang::t("folder.untitled");
  let candidate = base.join(model::unique_name(base, &stem, true));
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

/// Commit an inline rename. Taken names count up (`Docs (2)`,
/// `wallpaper (2).png`) instead of failing; returns true on
/// success (caller refreshes). On invalid input the session stays
/// active so the name can be fixed.
fn commit_rename(base: &std::path::Path, entry: &model::DirEntry, typed: &str) -> bool {
  let t0 = std::time::Instant::now();
  let Some(new_name) = model::resolve_new_name(entry, typed) else {
    eprintln!(
      "[finder][commit_rename] rejected typed text {typed:?} for {}",
      entry.name
    );
    return false;
  };
  let unique = model::unique_name(base, &new_name, entry.is_dir);
  let target = base.join(&unique);
  let ok = std::fs::rename(base.join(&entry.name), &target).is_ok();
  eprintln!(
    "[finder][commit_rename] {} -> {} ok={} in {}ms",
    entry.name,
    unique,
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

/// Find the popover child of a `ContextMenu` wrapper (the popup
/// menu content carries `min-width: 180px`; left parented it drives
/// the wrapped widget's minimum width, so FlowBox columns grew to
/// ~230px, showing 5 instead of 8).
fn find_menu_popover(wrapper: &gtk::Widget) -> Option<gtk::Popover> {
  let mut cursor = wrapper.first_child();
  while let Some(widget) = cursor {
    cursor = widget.next_sibling();
    if let Ok(pop) = widget.clone().downcast::<gtk::Popover>() {
      return Some(pop);
    }
  }
  None
}

thread_local! {
  /// Every context-menu popover: the popup, whether it may hang
  /// detached (cell/row menus; scroll menus stay parented), and
  /// whether WE currently have it parented. Never ask GTK about
  /// parenthood: `parent()` on a widget whose parent died trips
  /// `gtk_widget_get_parent` criticals, so the flag is the single
  /// source of truth (set on re-attach, cleared on dismiss).
  static OPEN_MENUS: RefCell<Vec<TrackedMenu>> = RefCell::new(Vec::new());
}

/// One registered popover (see `OPEN_MENUS`).
struct TrackedMenu {
  pop: gtk::Popover,
  detachable: bool,
  parented: Rc<Cell<bool>>,
}

/// Forget all registered popovers. Called on rebuild after
/// `popdown_all_menus`, so fresh wrappers register fresh popovers
/// and nothing stale survives.
fn forget_menus() {
  OPEN_MENUS.with(|slot| slot.borrow_mut().clear());
}

/// Dismiss every known context menu. Called on any press inside
/// our window, on selection changes, view switches and rebuilds,
/// so a menu never survives the action or selection that follows
/// it. Only logs when something was actually open (or a stray
/// toplevel exists); the quiet path runs on every click.
fn popdown_all_menus() {
  OPEN_MENUS.with(|slot| {
    let guard = slot.borrow();
    let mut open = 0;
    for entry in guard.iter() {
      if entry.pop.is_visible() {
        open += 1;
      }
      popdown_tree(&entry.pop);
      // Detached-again after dismiss (tracked flag only, never a
      // GTK parent query): the next measure must see the bare
      // cell/row, and teardown must never meet a mapped child.
      if entry.detachable && entry.parented.get() {
        entry.pop.unparent();
        entry.parented.set(false);
      }
    }
    let live = guard.len();
    drop(guard);
    // Toplevel census: an open menu is its own toplevel surface, so
    // a stuck-but-dead menu shows up here even when its widgets are
    // gone. Baseline is 1 (the main window).
    let tops = gtk::Window::list_toplevels();
    if open > 0 || tops.len() > 1 {
      eprintln!(
        "[finder][menu] popdown_all: registry_live={live} was_open={open} toplevels={}",
        tops.len()
      );
      for top in &tops {
        let title = top
          .clone()
          .downcast::<gtk::Window>()
          .ok()
          .and_then(|w| w.title().map(|s| s.to_string()));
        eprintln!(
          "[finder][menu]   top visible={} title={title:?} type={} name={}",
          top.is_visible(),
          top.type_().name(),
          top.widget_name()
        );
      }
    }
  });
}

/// Current active state of the application window, if present.
/// Scans `gtk::Window::list_toplevels()` for the first
/// `gtk::ApplicationWindow` (fallback: plain `gtk::Window`) and
/// returns `is_active()`. Returns `None` when no window exists
/// (for example in headless tests). Cheap enough for the 400ms
/// tick; callers stay quiet when the value is unchanged.
fn app_window_active() -> Option<bool> {
  let tops = gtk::Window::list_toplevels();
  for top in &tops {
    if let Ok(win) = top.clone().downcast::<gtk::ApplicationWindow>() {
      let win: gtk::Window = win.upcast();
      return Some(win.is_active());
    }
  }
  for top in &tops {
    if let Ok(win) = top.clone().downcast::<gtk::Window>() {
      return Some(win.is_active());
    }
  }
  None
}

/// True when a previous active state exists and differs from the
/// current one (flip in either direction). Returns false on the
/// first observation (`None`), so startup never dismisses menus.
fn active_flipped(prev: Option<bool>, current: bool) -> bool {
  matches!(prev, Some(p) if p != current)
}

/// Dismiss one popover plus nested submenu popovers (child first).
/// No visibility flags are touched: with textbook presentation
/// (correct parent at popup time) plain `popdown()` removes the
/// surface, and any visibility override risks the state machine.
fn popdown_tree(pop: &gtk::Popover) {
  fn walk(node: &gtk::Widget) {
    let mut cursor = node.first_child();
    while let Some(widget) = cursor {
      cursor = widget.next_sibling();
      if let Ok(child) = widget.clone().downcast::<gtk::Popover>() {
        walk(&child.clone().upcast());
        child.popdown();
      } else {
        walk(&widget);
      }
    }
  }
  let before = pop.is_visible();
  let root: gtk::Widget = pop.clone().upcast();
  walk(&root);
  pop.popdown();
  if before {
    eprintln!(
      "[finder][menu] dismissed open menu, visible_after={}",
      pop.is_visible()
    );
  }
}

/// Register a scroll/empty-space menu popover. It stays parented
/// (wide hosts do not care about the 180px content); only cell/row
/// popovers hang detached (see `file_menu_wrap`).
fn register_menu_popover(wrapper: &gtk::Widget) {
  if let Some(pop) = find_menu_popover(wrapper) {
    OPEN_MENUS.with(|slot| {
      slot.borrow_mut().push(TrackedMenu {
        pop,
        detachable: false,
        parented: Rc::new(Cell::new(true)),
      })
    });
  }
}

/// Dismiss on presses that hit no interactive child (dividers,
/// padding, tag dots): pick the deepest widget under the press; if
/// it is not inside a `GtkButton`, only the menus close and Finder
/// state stays untouched. Button presses pass through (the SDK pops
/// down after the action). This distinguishes "menu itself
/// unselected" from presses on Finder content (outer gestures).
fn attach_menu_background_dismiss(pop: &gtk::Popover) {
  for button in [1u32, 3u32] {
    let press = gtk::GestureClick::new();
    press.set_button(button);
    press.connect_pressed(|gesture, _, x, y| {
      let on_button = gesture.widget().map_or(false, |root| {
        let mut node = root.pick(x, y, gtk::PickFlags::DEFAULT);
        while let Some(widget) = node {
          if widget.clone().downcast::<gtk::Button>().is_ok() {
            return true;
          }
          node = widget.parent();
        }
        false
      });
      if !on_button {
        popdown_all_menus();
      }
    });
    pop.add_controller(press);
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
  if let Some(pop) = find_menu_popover(&wrapped) {
    // Idle popovers hang detached (zero size contribution, kept
    // alive by the registry). A capture gesture re-attaches right
    // before the SDK popup runs (capture beats bubble), so the
    // popup gets correct coordinates, grab and autohide. Guards on
    // both calls: double presses must neither warn nor misparent.
    // Dead-area presses inside the open menu dismiss it (attached
    // below); button presses pass through to their actions.
    attach_menu_background_dismiss(&pop);
    pop.unparent();
    let parented = Rc::new(Cell::new(false));
    OPEN_MENUS.with(|slot| {
      slot.borrow_mut().push(TrackedMenu {
        pop: pop.clone(),
        detachable: true,
        parented: parented.clone(),
      })
    });
    let pop_c = pop;
    let wrap_w = wrapped.downgrade();
    let reattach = gtk::GestureClick::new();
    reattach.set_button(3);
    reattach.set_propagation_phase(gtk::PropagationPhase::Capture);
    reattach.connect_pressed(move |_, _, _, _| {
      if let Some(container) = wrap_w.upgrade() {
        if !parented.get() {
          pop_c.set_parent(&container);
          parented.set(true);
        }
      }
    });
    wrapped.add_controller(reattach);
  }
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
  grid_scroll: gtk::ScrolledWindow,
  status: gtk::Label,
  title: gtk::Label,
  pal: Palette,
  nav: NavState,
  listed: Rc<RefCell<Vec<ListedEntry>>>,
  session: SharedSession,
  refresh: Refresh,
  mode: Rc<RefCell<prefs::ViewMode>>,
}

/// Rebuild callback for the active view (main thread only).
type Rebuild = Rc<dyn Fn()>;

/// Navigation stacks: current folder plus back/forward history.
/// Pure data (no widgets), so history moves are unit-testable; the
/// watcher, snapshot and rebuild happen in `after_navigate`.
#[derive(Debug, Default)]
struct NavStacks {
  current: std::path::PathBuf,
  back: Vec<std::path::PathBuf>,
  forward: Vec<std::path::PathBuf>,
}

/// Fresh navigation (double-clicked folder): push current to back,
/// clear forward. Returns false when already there.
fn nav_to(stacks: &mut NavStacks, target: std::path::PathBuf) -> bool {
  if stacks.current == target {
    return false;
  }
  stacks.back.push(std::mem::replace(&mut stacks.current, target));
  stacks.forward.clear();
  true
}

/// Back button: push current to forward, pop back. False when empty.
fn nav_back(stacks: &mut NavStacks) -> bool {
  let Some(previous) = stacks.back.pop() else {
    return false;
  };
  stacks
    .forward
    .push(std::mem::replace(&mut stacks.current, previous));
  true
}

/// Forward button: push current to back, pop forward. False when empty.
fn nav_forward(stacks: &mut NavStacks) -> bool {
  let Some(next) = stacks.forward.pop() else {
    return false;
  };
  stacks
    .back
    .push(std::mem::replace(&mut stacks.current, next));
  true
}

/// Shared navigation state (main thread only).
#[derive(Clone)]
struct NavState {
  stacks: Rc<RefCell<NavStacks>>,
  snapshot: Rc<RefCell<Vec<model::SnapshotEntry>>>,
  watch_rx: Rc<
    RefCell<Option<std::sync::mpsc::Receiver<()>>>,
  >,
}

/// Current folder (clone).
fn nav_current(nav: &NavState) -> std::path::PathBuf {
  nav.stacks.borrow().current.clone()
}

/// Display name of a folder for the title: file name, full path for roots.
fn folder_title(base: &std::path::Path) -> String {
  base
    .file_name()
    .and_then(|name| name.to_str())
    .map(str::to_string)
    .unwrap_or_else(|| base.to_string_lossy().into_owned())
}

/// One listed row for activation lookup (double-click by index).
struct ListedEntry {
  name: String,
  is_dir: bool,
}

/// Toolbar navigation request (Send-bound callbacks route through
/// the tick; double-click navigates directly).
#[derive(Debug, Clone)]
enum NavAction {
  Back,
  Forward,
}

/// Rewatch the current folder and reset its snapshot after
/// navigation (replaces the previous watcher).
fn rewatch(nav: &NavState) {
  let base = nav_current(nav);
  let fresh = crate::watch::watch_dir(&base).map(|(watcher, rx)| {
    FOLDER_WATCHER.with(|slot| *slot.borrow_mut() = Some(watcher));
    rx
  });
  *nav.watch_rx.borrow_mut() = fresh;
  *nav.snapshot.borrow_mut() = model::snapshot(&base);
  eprintln!("[finder][nav] now in {}", base.display());
}

/// Shared tail of every navigation: cancel rename, rewatch, rebuild.
fn after_navigate(nav: &NavState, session: &SharedSession, rebuild: &Rebuild) {
  if let Ok(mut guard) = session.lock() {
    *guard = None;
  }
  rewatch(nav);
  rebuild();
}

/// Open the activated entry when it is still a directory. Guards
/// with a live `is_dir` check in case the listing went stale.
fn activate_index(
  nav: &NavState,
  listed: &Rc<RefCell<Vec<ListedEntry>>>,
  session: &SharedSession,
  rebuild: &Rebuild,
  index: i32,
) {
  let hit = listed
    .borrow()
    .get(index as usize)
    .map(|entry| (entry.name.clone(), entry.is_dir));
  let Some((name, was_dir)) = hit else {
    return;
  };
  if !was_dir {
    eprintln!("[finder][nav] open file (later step): {name}");
    return;
  }
  let target = nav_current(nav).join(&name);
  if !target.is_dir() {
    return;
  }
  if nav_to(&mut nav.stacks.borrow_mut(), target.clone()) {
    eprintln!("[finder][nav] opened {}", target.display());
    after_navigate(nav, session, rebuild);
  }
}

/// Placeholder for an empty or unreadable directory.
fn empty_view() -> gtk::Widget {
  empty_view_for(&search_query())
}

/// Placeholder picked by state: no-match text while searching,
/// otherwise the folder-empty text.
fn empty_view_for(query: &str) -> gtk::Widget {
  let (title_key, hint_key) = if query.is_empty() {
    ("detail.empty", "detail.empty.hint")
  } else {
    ("search.empty", "search.empty.hint")
  };
  let empty = ContentUnavailableView::new()
    .title(lang::t(title_key))
    .message(lang::t(hint_key));
  let empty_gtk = empty.to_gtk();
  empty_gtk.set_hexpand(true);
  empty_gtk.set_vexpand(true);
  empty_gtk
}

/// Title markup for the current folder (15px bold primary).
fn title_markup(name: &str, pal: &Palette) -> String {
  format!(
    "<span font_desc=\"{} bold 15\" foreground=\"{}\">{}</span>",
    SF_PRO,
    pal.fg,
    glib::markup_escape_text(name),
  )
}

/// Rebuild the content area for the active view: icon grid or list.
fn refresh_content(ctx: &ViewCtx, rebuild: &Rebuild) {
  // Dismiss menus before tearing down their widgets, then forget
  // them: fresh wrappers register fresh popovers below.
  popdown_all_menus();
  forget_menus();
  let mut child = ctx.slot.first_child();
  while let Some(widget) = child {
    child = widget.next_sibling();
    ctx.slot.remove(&widget);
  }
  let base = nav_current(&ctx.nav);
  ctx.title.set_markup(&title_markup(&folder_title(&base), &ctx.pal));
  match *ctx.mode.borrow() {
    prefs::ViewMode::Grid => {
      if model::list_dir(&base).is_empty() {
        ctx.slot.append(&empty_view());
        ctx
          .status
          .set_markup(&status_markup(&[], &ctx.pal, &base));
      } else {
        refresh_grid(
          &ctx.grid,
          &ctx.status,
          &ctx.pal,
          &base,
          &ctx.session,
          &ctx.refresh,
          &ctx.listed,
          rebuild,
        );
        // Fresh empty-space menu each rebuild (captures the current base).
        let wrapped = ContextMenu::new(GtkWrap::wrap(ctx.grid_scroll.clone()))
          .entries(empty_space_menu(&base, &ctx.session, &ctx.refresh))
          .to_gtk();
        register_menu_popover(&wrapped);
        wrapped.set_hexpand(true);
        wrapped.set_vexpand(true);
        ctx.slot.append(&wrapped);
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
  let base = nav_current(&ctx.nav);
  let query = search_query();
  let mut entries = model::list_dir(&base);
  if !query.is_empty() {
    entries.retain(|entry| model::matches_query(&entry.name, &query));
  }
  *ctx.listed.borrow_mut() = entries
    .iter()
    .map(|entry| ListedEntry {
      name: entry.name.clone(),
      is_dir: entry.is_dir,
    })
    .collect();
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
    // A new selection dismisses any open menu first.
    list.connect_row_selected(|_, _| popdown_all_menus());
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
        &base,
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
    // Double-click (or Enter) opens folders.
    {
      let nav_c = ctx.nav.clone();
      let listed_c = ctx.listed.clone();
      let session_c = ctx.session.clone();
      let rebuild_c = rebuild.clone();
      list.connect_row_activated(move |_, row| {
        activate_index(&nav_c, &listed_c, &session_c, &rebuild_c, row.index());
      });
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
      .entries(empty_space_menu(&base, &ctx.session, &ctx.refresh));
    let menu_gtk = menu.to_gtk();
    register_menu_popover(&menu_gtk);
    menu_gtk.set_hexpand(true);
    menu_gtk.set_vexpand(true);
    content.append(&menu_gtk);
    ctx.slot.append(&content);
  }
  ctx
    .status
    .set_markup(&status_markup(&entries, &ctx.pal, &base));
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
  listed: &Rc<RefCell<Vec<ListedEntry>>>,
  rebuild: &Rebuild,
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
      edit_field(base, entry, &shown, session, rebuild),
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
  rebuild: &Rebuild,
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
  let commit_rebuild = rebuild.clone();
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
      commit_rebuild();
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

/// Empty-space context menu entries: New Folder, divider, Get Info,
/// divider, New File submenu with Text File. New Folder creates in
/// `base` (the current folder) and opens inline rename.
pub(crate) fn empty_space_menu(
  base: &std::path::Path,
  session: &SharedSession,
  refresh: &Refresh,
) -> Vec<MenuEntry> {
  let new_base = base.to_path_buf();
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

    // Any press inside our window dismisses open menus first (GTK
    // autohide does not engage for the parented popovers). Capture
    // phase runs before the menu gestures; presses inside an open
    // menu never reach us (separate popup surface), so menu use is
    // unaffected. Neither gesture claims, so selection and menus
    // keep working normally afterwards.
    for button in [1u32, 3u32] {
      let dismiss = gtk::GestureClick::new();
      dismiss.set_button(button);
      dismiss.set_propagation_phase(gtk::PropagationPhase::Capture);
      dismiss.connect_pressed(|_, _, _, _| popdown_all_menus());
      outer.add_controller(dismiss);
    }

    let sidebar = Sidebar::new()
      .section(lang::t("sidebar.favourites"))
      .item(
        lang::t("sidebar.downloads"),
        SidebarIcon::sf("arrow.down.circle.fill", blue()),
      )
      .selected(0)
      .no_search()
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

    let base = model::downloads_dir();
    let nav = NavState {
      stacks: Rc::new(RefCell::new(NavStacks {
        current: base.clone(),
        back: Vec::new(),
        forward: Vec::new(),
      })),
      snapshot: Rc::new(RefCell::new(Vec::new())),
      watch_rx: Rc::new(RefCell::new(None)),
    };
    rewatch(&nav);

    let (nav_tx, nav_rx) = std::sync::mpsc::channel::<NavAction>();
    let back_tx = nav_tx.clone();
    let forward_tx = nav_tx.clone();
    let nav_toolbar = Toolbar::new()
      .item(ToolbarItem::new("chevron.backward").on_click(move || {
        eprintln!("[finder][nav] back button clicked");
        let _ = back_tx.send(NavAction::Back);
      }))
      .item(ToolbarItem::new("chevron.forward").on_click(move || {
        eprintln!("[finder][nav] forward button clicked");
        let _ = forward_tx.send(NavAction::Forward);
      }));
    toolbar_row.append(&nav_toolbar.to_gtk());

    let title = gtk::Label::new(None);
    title.set_use_markup(true);
    title.set_halign(gtk::Align::Start);
    title.set_valign(gtk::Align::Center);
    title.set_hexpand(true);
    title.set_markup(&title_markup(&folder_title(&base), &pal));
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

    // Search field (built once, shown on demand): expands the
    // actions row, filters the current folder live by name, keeps
    // the filter when collapsed, clears on Escape.
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some(&lang::t("detail.search")));
    search_entry.set_width_chars(18);
    search_entry.set_valign(gtk::Align::Center);
    {
      let (field_bg, field_fg) = if dark {
        ("#3a3a3c", "#ececec")
      } else {
        ("#ffffff", "#1d1d1d")
      };
      crate::UIKit::widget::apply_css(
        &search_entry,
        &format!(
          ".fd-search {{ background-color: {field_bg}; color: {field_fg}; \
            border-radius: 14px; border: none; padding: 4px 10px; \
            font-family: '{SF_PRO}'; font-size: 13px; }}"
        ),
      );
    }
    search_entry.add_css_class("fd-search");
    // Animated wrapper: slide the field in/out instead of popping.
    // The entry lives in the revealer permanently (no reparenting
    // of the field itself); only the revealer swaps with the
    // button, after the slide-out finishes.
    let search_revealer = gtk::Revealer::new();
    search_revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    search_revealer.set_transition_duration(200);
    search_revealer.set_valign(gtk::Align::Center);
    search_revealer.set_child(Some(&search_entry));

    let actions = Toolbar::new()
      .item(ToolbarItem::new("square.and.arrow.up").on_click(|| println!("Finder share")))
      .item(ToolbarItem::new("magnifyingglass"));
    let actions_gtk = actions.to_gtk();
    // Handles for the in-place swap (last button in the group).
    // Fallbacks keep the app running if the SDK layout ever
    // changes; the button then simply stays inert.
    let mut action_buttons = collect_view_buttons(&actions_gtk);
    let actions_search_btn = action_buttons.pop().unwrap_or_else(|| {
      eprintln!("[finder][search] search button not found, search disabled");
      gtk::Button::new()
    });
    let actions_group = actions_search_btn
      .parent()
      .and_then(|parent| parent.downcast::<gtk::Box>().ok())
      .unwrap_or_else(|| {
        eprintln!("[finder][search] actions group not found, search disabled");
        gtk::Box::new(gtk::Orientation::Horizontal, 0)
      });
    // Search open state, tracked locally: never ask GTK about
    // widget parents (see popover registry for why).
    let search_open: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    // Expand swaps the search button with the field in place, so
    // the glass group grows (or collapse it back to the icon when
    // already shown). The button callback is wired directly (main
    // thread), so expanding feels instant. Structural swaps run as
    // idle callbacks: mutating the group mid-emission (clicked /
    // focus-leave) trips `gtk_widget_get_parent` criticals.
    let expand_search = {
      let group_c = actions_group.clone();
      let btn_c = actions_search_btn.clone();
      let reveal_c = search_revealer.clone();
      let entry_c = search_entry.clone();
      let open_c = search_open.clone();
      move || {
        let group_c = group_c.clone();
        let btn_c = btn_c.clone();
        let reveal_c = reveal_c.clone();
        let entry_c = entry_c.clone();
        let open_c = open_c.clone();
        glib::idle_add_local(move || {
          if !open_c.get() {
            entry_c.set_text(&search_query());
            // Button out (if still in), revealer in (if not yet):
            // group and button live forever, so membership reads
            // here are safe.
            if btn_c.parent().is_some() {
              group_c.remove(&btn_c);
            }
            if reveal_c.parent().is_none() {
              group_c.append(&reveal_c);
            }
            reveal_c.set_reveal_child(true);
            entry_c.grab_focus();
            open_c.set(true);
            eprintln!("[finder][search] expanded");
          } else {
            reveal_c.set_reveal_child(false);
            open_c.set(false);
            eprintln!("[finder][search] collapsed");
            // Swap back to the button after the slide-out.
            let group_c2 = group_c.clone();
            let btn_c2 = btn_c.clone();
            let reveal_c2 = reveal_c.clone();
            let open_c2 = open_c.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(220), move || {
              if !open_c2.get() {
                if reveal_c2.parent().is_some() {
                  group_c2.remove(&reveal_c2);
                }
                if btn_c2.parent().is_none() {
                  group_c2.append(&btn_c2);
                }
              }
              glib::ControlFlow::Break
            });
          }
          glib::ControlFlow::Break
        });
      }
    };
    // Collapse without clearing (clicking away keeps the filter).
    let collapse_search: Rc<dyn Fn()> = Rc::new({
      let group_c = actions_group.clone();
      let btn_c = actions_search_btn.clone();
      let reveal_c = search_revealer.clone();
      let open_c = search_open.clone();
      move || {
        let group_c = group_c.clone();
        let btn_c = btn_c.clone();
        let reveal_c = reveal_c.clone();
        let open_c = open_c.clone();
        glib::idle_add_local(move || {
          if open_c.get() {
            reveal_c.set_reveal_child(false);
            open_c.set(false);
            eprintln!("[finder][search] collapsed");
            let group_c2 = group_c.clone();
            let btn_c2 = btn_c.clone();
            let reveal_c2 = reveal_c.clone();
            let open_c2 = open_c.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(220), move || {
              if !open_c2.get() {
                if reveal_c2.parent().is_some() {
                  group_c2.remove(&reveal_c2);
                }
                if btn_c2.parent().is_none() {
                  group_c2.append(&btn_c2);
                }
              }
              glib::ControlFlow::Break
            });
          }
          glib::ControlFlow::Break
        });
      }
    });
    // Wire the search button directly (its `on_click` is Send-bound
    // and must not touch widgets).
    {
      let expand_c = expand_search;
      actions_search_btn.connect_clicked(move |_| expand_c());
    }
    toolbar_row.append(&actions_gtk);
    // Escape clears the query and collapses; focus loss collapses
    // and keeps the filter.
    {
      let entry_c = search_entry.clone();
      let collapse_c = collapse_search.clone();
      let keys = gtk::EventControllerKey::new();
      keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk4::Key::Escape {
          entry_c.set_text("");
          collapse_c();
          glib::Propagation::Stop
        } else {
          glib::Propagation::Proceed
        }
      });
      search_entry.add_controller(keys);
      let focus = gtk::EventControllerFocus::new();
      focus.connect_leave(move |_| collapse_search());
      search_entry.add_controller(focus);
    }

    detail.append(&toolbar_row);

    let base = model::downloads_dir();
    let session: SharedSession = Arc::new(Mutex::new(None));

    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::Single);
    // Single click only selects; folders open on double-click (or Enter).
    grid.set_activate_on_single_click(false);
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
      // A new selection dismisses any open menu first.
      popdown_all_menus();
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

    // Grid scroll holding the persistent grid; the empty-space
    // menu around it is rebuilt per refresh (current folder).
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

    let ctx = ViewCtx {
      slot: content_slot.clone(),
      grid: grid.clone(),
      grid_scroll: grid_scroll.clone(),
      status: status_label.clone(),
      title: title.clone(),
      pal,
      nav: nav.clone(),
      listed: Rc::new(RefCell::new(Vec::new())),
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
    // signal, the view-switch signal and the navigation signal
    // share one 400ms main-thread tick that drains bursts and
    // rebuilds the active view once per real change. The watcher
    // also fires on plain file opens (every rebuild opens files),
    // so a metadata snapshot gates the rebuild: without it the
    // refresh retriggers itself and starves the UI. The watcher
    // follows navigation (`rewatch`).
    let session_tick = session.clone();
    let nav_tick = nav.clone();
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
    // Live search: every keystroke filters the current folder by
    // name and rebuilds immediately (main-thread signal, no tick).
    {
      let rebuild_c = rebuild_all.clone();
      search_entry.connect_changed(move |entry| {
        set_search_query(&entry.text());
        eprintln!("[finder][search] query={:?}", entry.text());
        rebuild_c();
      });
    }
    refresh_content(&ctx, &rebuild_all);
    let rebuild_tick = rebuild_all.clone();
    // Double-click (or Enter) on a grid cell opens folders.
    {
      let nav_c = nav.clone();
      let listed_c = ctx.listed.clone();
      let session_c = session.clone();
      let rebuild_c = rebuild_all.clone();
      grid.connect_child_activated(move |_, child| {
        activate_index(&nav_c, &listed_c, &session_c, &rebuild_c, child.index());
      });
    }
    let mode_tick = mode_state.clone();
    let mut last_active: Option<bool> = None;
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
      // Focus change dismisses menus: presses outside our widget
      // hierarchy never reach our gestures and GTK autohide does not
      // engage for the parented popovers, so a flip of the main
      // window active state in either direction pops everything
      // down. Runs before the early return so a lone focus change
      // (no watcher/menu/view signal) still closes menus.
      if let Some(current) = app_window_active() {
        if active_flipped(last_active, current) {
          eprintln!("[finder][menu] window active changed -> {current}");
          popdown_all_menus();
        }
        last_active = Some(current);
      }
      let mut watch_signaled = false;
      {
        let guard = nav_tick.watch_rx.borrow();
        if let Some(rx) = guard.as_ref() {
          while rx.try_recv().is_ok() {
            watch_signaled = true;
          }
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
      let mut nav_moved = false;
      while let Ok(action) = nav_rx.try_recv() {
        eprintln!("[finder][nav] action {action:?}");
        let moved = match action {
          NavAction::Back => nav_back(&mut nav_tick.stacks.borrow_mut()),
          NavAction::Forward => nav_forward(&mut nav_tick.stacks.borrow_mut()),
        };
        nav_moved |= moved;
      }
      if !watch_signaled && !menu_signaled && view_switch.is_none() && !nav_moved {
        return glib::ControlFlow::Continue;
      }
      if let Some(mode) = view_switch {
        if mode != *mode_tick.borrow() {
          *mode_tick.borrow_mut() = mode;
          prefs::save_view_mode(mode);
          apply_view_indicator(&view_buttons, mode);
          eprintln!("[finder][view] switched to {}", mode.as_str());
        }
        *nav_tick.snapshot.borrow_mut() = model::snapshot(&nav_current(&nav_tick));
        rebuild_tick();
        eprintln!("[finder][view] rebuilt after switch");
        return glib::ControlFlow::Continue;
      }
      if nav_moved {
        after_navigate(&nav_tick, &session_tick, &rebuild_tick);
        return glib::ControlFlow::Continue;
      }
      let editing = !watch_refresh_allowed(&session_tick);
      eprintln!(
        "[finder][tick] watch={watch_signaled} menu={menu_signaled} editing={editing}"
      );
      let t0 = std::time::Instant::now();
      if watch_refresh_allowed(&session_tick) {
        if watch_signaled || menu_signaled {
          let current = model::snapshot(&nav_current(&nav_tick));
          if current != *nav_tick.snapshot.borrow() {
            eprintln!(
              "[finder][tick] snapshot changed ({} entries), rebuilding",
              current.len()
            );
            *nav_tick.snapshot.borrow_mut() = current;
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
        *nav_tick.snapshot.borrow_mut() = model::snapshot(&nav_current(&nav_tick));
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
/// Status line markup for the current entries in `base`. The free
/// space is live (`statvfs`); German uses a decimal comma. Falls
/// back to the `status.free` placeholder when unknown.
fn status_markup(entries: &[model::DirEntry], pal: &Palette, base: &std::path::Path) -> String {
  let free = match model::free_bytes(base) {
    Some(bytes) => {
      let text = model::format_size(bytes);
      if crate::lang::locale() == "de_de" {
        text.replace('.', ",")
      } else {
        text
      }
    }
    None => lang::t("status.free"),
  };
  let line = lang::t("status.line")
    .replace("{count}", &model::item_count(entries).to_string())
    .replace("{free}", &free);
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
  listed: &Rc<RefCell<Vec<ListedEntry>>>,
  rebuild: &Rebuild,
) {
  let t0 = std::time::Instant::now();
  let mut child = grid.first_child();
  while let Some(widget) = child {
    child = widget.next_sibling();
    grid.remove(&widget);
  }
  let t_list = std::time::Instant::now();
  let query = search_query();
  let mut entries = model::list_dir(base);
  if !query.is_empty() {
    entries.retain(|entry| model::matches_query(&entry.name, &query));
  }
  *listed.borrow_mut() = entries
    .iter()
    .map(|entry| ListedEntry {
      name: entry.name.clone(),
      is_dir: entry.is_dir,
    })
    .collect();
  eprintln!(
    "[finder][refresh] list_dir {}: {} entries in {}ms",
    base.display(),
    entries.len(),
    t_list.elapsed().as_millis()
  );
  for (index, entry) in entries.iter().enumerate() {
    let t_cell = std::time::Instant::now();
    grid.insert(&folder_cell(base, entry, pal, session, refresh, listed, rebuild), -1);
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
  status.set_markup(&status_markup(&entries, pal, base));
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
  fn nav_history_moves() {
    use std::path::PathBuf;
    let mut stacks = NavStacks {
      current: PathBuf::from("/a"),
      back: Vec::new(),
      forward: Vec::new(),
    };
    // Empty stacks do nothing.
    assert!(!nav_back(&mut stacks));
    assert!(!nav_forward(&mut stacks));
    assert!(!nav_to(&mut stacks, PathBuf::from("/a")));
    // Fresh navigation pushes back and clears forward.
    assert!(nav_to(&mut stacks, PathBuf::from("/b")));
    assert_eq!(stacks.current, PathBuf::from("/b"));
    assert_eq!(stacks.back, vec![PathBuf::from("/a")]);
    assert!(stacks.forward.is_empty());
    assert!(nav_to(&mut stacks, PathBuf::from("/c")));
    // Back and forward shuttle current between stacks.
    assert!(nav_back(&mut stacks));
    assert_eq!(stacks.current, PathBuf::from("/b"));
    assert!(nav_back(&mut stacks));
    assert_eq!(stacks.current, PathBuf::from("/a"));
    assert!(!nav_back(&mut stacks));
    assert!(nav_forward(&mut stacks));
    assert_eq!(stacks.current, PathBuf::from("/b"));
    // A fresh navigation drops the forward trail.
    assert!(nav_to(&mut stacks, PathBuf::from("/d")));
    assert_eq!(stacks.current, PathBuf::from("/d"));
    assert!(stacks.forward.is_empty());
    assert!(!nav_forward(&mut stacks));
  }

  #[test]
  fn empty_space_menu_structure() {
    let (session, refresh, base) = test_context();
    let entries = empty_space_menu(&base, &session, &refresh);
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

  #[test]
  fn active_flip_matrix() {
    // First observation never counts as a flip (no dismiss on startup).
    assert!(!active_flipped(None, true));
    assert!(!active_flipped(None, false));
    // Either direction flips (focus lost or regained).
    assert!(active_flipped(Some(true), false));
    assert!(active_flipped(Some(false), true));
    // Unchanged states stay quiet.
    assert!(!active_flipped(Some(true), true));
    assert!(!active_flipped(Some(false), false));
  }
}
