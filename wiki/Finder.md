# Finder

TontooOS file manager basis: a 1080x720 TontooUI window with a
Tahoe-style sidebar on the left (currently Favourites with Downloads),
a toolbar row with navigation plus view and action controls plus search,
a live `~/Downloads/` grid in the middle and a status line at the
bottom. Locations and Tags sidebar sections are later steps.

## Layout

From left to right the window contains:

1. Sidebar (`Sidebar`, 240px, traffic lights, search, one section)
2. Detail column: toolbar row, Downloads grid (`FlowBox`), status line

```rust
let mut app = App::with_delegate(lang::t("app.title"), 1080, 720, FinderDelegate);
// No extra window bar: the sidebar draws the only traffic lights.
app.no_window_bar();
app.auto_color_scheme(); // live Dark/Light follow
app.run();
```

## Sidebar

`TontooUI::Sidebar` with the `coreicon` feature (default). Currently a
single Favourites section with the Downloads row, without the
search field (`.no_search()`); Locations and Tags
sections are later steps.

| Section | Rows |
|---|---|
| `sidebar.favourites` | Downloads (`arrow.down.circle.fill`, blue) |

```rust
let sidebar = Sidebar::new()
  .section(lang::t("sidebar.favourites"))
  .item(lang::t("sidebar.downloads"), SidebarIcon::sf("arrow.down.circle.fill", blue()))
  .selected(0)
  .no_search()
  .width(240.0)
  .on_select(|i| println!("Finder selected: {}", i));
```

Rules:

- Selection currently only logs the index; swapping the detail content
  per row is a later step.

## Toolbar row

Three `TontooUI::Toolbar` groups in a `gtk::Box` row:

| Group | Items |
|---|---|
| Navigation | `chevron.backward` (back), `chevron.forward` (forward) |
| View switch | `square.grid.2x2` (icons), `list.bullet` (list), with select indicator, see [ListView.md](ListView.md) |
| Actions | `square.and.arrow.up` (share), `magnifyingglass` (search: expands a live name filter) |

The share button currently logs its action; search filtering is
described below.

## Navigation

Double-click (or Enter) on a folder opens it in both views;
`.app` bundles launch through LaunchPad instead (separate
process); single-click only selects (`activate-on-single-click` is
off). `chevron.backward` and
`chevron.forward` walk the back/forward history (empty stacks are
a no-op). The title shows the current folder name, the status line
its item count, and the `notify` watcher follows along
(`rewatch`). An in-progress inline rename is cancelled on
navigation. Opening plain files is a later step (logs for now).

### `launch_app`

```rust
pub fn launch_app(path: &Path)
```

Asks the LaunchPad daemon to start a `.app` bundle as a separate
process (`start_app`), on a throwaway thread so Finder never
blocks. Success and failure only log (`[finder][launch]`);
without a daemon (dev checkouts) it logs the error. Found in
`src/launch.rs`.

## Search

The magnifying glass swaps itself with a search field in place,
so the actions glass group grows (`detail.search` placeholder).
Every keystroke filters the current folder live by name in both
views (`.txt` matches all text files); only names are searched,
never contents. Clicking away collapses back to the icon and keeps
the filter; Escape clears it. Swaps run as idle callbacks (never
mutate the toolbar mid-emission). The field slides in/out through
a `GtkRevealer` (200ms); the button returns after the slide-out.
The open state is tracked locally
(never a GTK parent query). No match shows `search.empty`
(`Keine Treffer`) instead of the folder-empty text, and the status
count follows the filtered entries.

## Downloads grid

A `gtk::FlowBox` (4-8 columns, homogeneous) shows the live entries of
`~/Downloads/` from `src/model.rs::list_downloads()`. Windows
`Zone.Identifier` marker files are skipped. `.app` bundles (directory
or ZIP archive) show the app icon rendered once through CoreIcon
(`AppIcon`, original colors, cached 192px PNG in the temp dir keyed by
size plus mtime). Directories render the
default `folder.svg` icon from `Resources/foldericons/scalable/`
(full-color SVG rendered by GTK through librsvg, 64px). Files render
by kind: images show the picture itself (rounded corners baked into the alpha channel at an exact
64px backing with a 64px size request, since GTK `pixel-size` does
not scale paintables and GTK CSS `border-radius` does not clip image
content), videos show the cached
first-second frame (`video_thumb`, temp dir `finder-thumbs/`, keyed by
size plus mtime) with the themed `blue-folder-videos.svg` fallback
while `ffmpeg` is missing, audio files (mp3, wav, flac, ogg, oga,
m4a, opus, aac, wma) show `Resources/extensionicons/audio.png`,
archives (zip, rar, 7z, tar, gz, gzip, bz2) show
`Resources/extensionicons/zip.png`, and other files show their
document icon from `Resources/extensionicons/` (`basis.png` when no
specific icon exists).
Entries sort directories-first, then alphabetically
(case-insensitive). A `notify` watcher (`src/watch.rs`) rebuilds the
grid live when the folder changes. An empty or unreadable directory shows a
`ContentUnavailableView` (`detail.empty`, `detail.empty.hint`). When
an icon file is missing the cell falls back to Tahoe CSS artwork
(`.fd-tab` plus `.fd-body` in Finder blue `#7fbeec` with edge
`#5ea3d8`), so the grid never renders an empty cell. Every cell is
a fixed 84x84 square (`CELL`): artwork plus the label, identical
in all four directions. Artwork always renders through
`gtk::Picture` with `Contain` fit, so every source (192px app
icons, large PNGs, SVGs, cached photo previews) scales into
its square instead of blowing up its cell (48px, `ARTWORK`; folders
render at 54px, `FOLDER_ARTWORK`, because the folder glyph carries
transparent padding while app icons are full-bleed). The `FlowBox`
reflows its column count (4-8) dynamically when the window is
resized in any direction. Idle context-menu popovers are hidden
(`hide_idle_popover`), so the 180px menu content never drives the
column width. Static icons
(`folder.svg`, document icons, placeholders, cached previews) are
decoded once per process into a shared `gdk4::Paintable`
(`shared_paintable`); every cell gets a cheap
`gtk::Image::from_paintable` view instead of paying ~30ms of SVG
rasterization per entry per rebuild.

`src/icons.rs` resolves `Resources/foldericons/<size>/<file>` across
dev checkouts (`Resources/`), `.app` bundles and installed files
(`/usr/share/finder/`). All files in
`Resources/extensionicons/` sit on a square canvas (192x192,
transparent padding), so `Contain` fit renders every document icon
at the same width and height. The sidebar keeps CoreIcon SF Symbols:
`SidebarIcon::file` relies on the `image` crate, which cannot decode
SVG.

### `downloads_dir`

```rust
pub fn downloads_dir() -> PathBuf
```

Returns `~/Downloads/` (`HOME` env, `/tmp` fallback).

### `display_name`

```rust
pub fn display_name(name: &str, is_dir: bool) -> String
```

Directories keep their name (`.app` bundles lose the suffix);
files lose the extension (`archive.tar.gz` shows as `archive.tar`).

### `resolve_new_name`

```rust
pub fn resolve_new_name(entry: &DirEntry, typed: &str) -> Option<String>
```

On-disk name for an inline rename. Rejects empty names and path
separators (`None` keeps the field open). Directories keep the typed
text (`.app` bundles keep their suffix); files keep their extension
unless the typed text already carries one.

### `create_folder`

```rust
pub fn create_folder(base: &Path) -> Option<PathBuf>
```

Creates a uniquely named folder (`folder.untitled`,
`folder.untitled (2)`, ... via `unique_name`). Found in
`src/views/finder.rs`; the caller opens inline rename on the
result.

### `unique_name`

```rust
pub fn unique_name(base: &Path, desired: &str, is_dir: bool) -> String
```

Free variant of a desired on-disk name: `desired` when unused,
else `stem (2)`, `stem (3)`, ... The counter goes before the file
extension (`wallpaper (2).png`), at the very end for directories
(`Docs (2)`), and before the `.app` suffix for bundles
(`Demo (2).app`). Leading-dot names never split an extension.
Used by both rename commit and folder creation, so taken names
count up instead of failing.

### `list_dir`

```rust
pub fn list_dir(path: &Path) -> Vec<DirEntry>
```

Lists a directory, directories first then files, each alphabetical
(case-insensitive). Skips `*.Zone.Identifier` marker files. Returns
an empty list when unreadable.

### `file_kind`

```rust
pub fn file_kind(ext: &str) -> FileKind
```

Classifies a lowercase extension as `Image`, `Video`, `Audio`,
`Archive` or `Other`, driving the grid preview.

### `document_icon`

```rust
pub fn document_icon(ext: &str) -> Option<&'static str>
```

Document icon file in `Resources/extensionicons/` for an extension.
Families share one icon: spreadsheets (`xls`, `xlsx`, `xlsm`,
`xlsb`, `csv`, `tsv`, `ods`) use `xls.png`, scripts (`js`, `jsx`,
`mjs`, `cjs`, `ts`, `tsx`, `json`, `jsonc`) use `javascript.png`,
slides (`ppt`, `pptx`, `pps`, `ppsx`, `odp`) use `ppt.png`, Word
(`docx`, `docm`, `dotx`) uses `docx.png`, text-like (`txt`, `text`,
`log`, `ini`, `cfg`, `conf`, `toml`, `yaml`, `yml`, `xml`) uses
`txt.png`, shells (`sh`, `bash`, `zsh`, `fish`, `bat`, `cmd`, `ps1`)
use `sh.png`, Windows executables (`exe`, `msi`) use `exe.png`, plus
`css`, `doc`, `html`, `java`, `md`, `pdf`, `py`,
`rs` icons. Returns `None` when no specific icon exists (caller uses
`basis.png`).

### `list_downloads`

```rust
pub fn list_downloads() -> Vec<DirEntry>
```

Live entries of `~/Downloads/`. `DirEntry` holds `name` (on disk)
and `is_dir`.

### `folder_icon`

```rust
pub fn folder_icon(file: &str) -> Option<PathBuf>
```

Resolves a `scalable/` icon file. Returns `None` when no layout holds
the file.

### `image_placeholder`

```rust
pub fn image_placeholder() -> Option<PathBuf>
```

Placeholder for photos that cannot be decoded
(`Resources/extensionicons/image.png`). Shown instead of a broken
image.

### `video_placeholder`

```rust
pub fn video_placeholder() -> Option<PathBuf>
```

Placeholder for video files without a cached thumbnail
(`Resources/extensionicons/video.png`). Falls back to the themed
`blue-folder-videos.svg` when missing.

### `shared_paintable`

```rust
pub fn shared_paintable(path: &Path) -> Option<gdk4::Paintable>
```

Shared texture for an icon file, decoded and rasterized once per
process (thread-local cache on the GTK main thread). Every grid cell
gets a cheap `gtk::Image::from_paintable` view of it instead of
decoding the file again (~30ms per folder cell for the SVG
rasterizer). Returns `None` when the file cannot be loaded (caller
falls back to `GtkImage::from_file`).

### `video_thumb`

```rust
pub fn video_thumb(source: &Path) -> Option<PathBuf>
```

First-second frame of a video as a cached PNG (`ffmpeg -ss 1`,
`scale=320:-1`). Returns a fresh extraction or the cached file.
Returns `None` when `ffmpeg` is missing or extraction fails (caller
shows the themed video icon). Audio files resolve
`Resources/extensionicons/audio.png` via `audio_icon()`.

### `ffmpeg_available`

```rust
pub fn ffmpeg_available() -> bool
```

True when `ffmpeg` is on PATH and can extract video frames. The
result is cached in a `OnceCell`: the grid calls this once per video
per rebuild on the GTK main thread, and spawning `ffmpeg -version`
every time froze the UI for seconds in video-heavy folders.

### `photo_preview`

```rust
pub fn photo_preview(source: &Path) -> Option<PathBuf>
```

Rounded 64px grid preview for a photo as a cached PNG file in the
temp dir (`finder-previews/`, keyed by size plus mtime). Generates
once on first view (decode plus `cover_square` plus
`round_corners`), then serves the tiny file, so full grid rebuilds
stay fast even with multi-megapixel photos (decoding a 4K photo in
debug builds blocked the main thread for seconds on every
rebuild). Returns `None` when the source cannot be decoded (caller
falls back to in-memory decoding or the themed icon).

### `cover_square`

```rust
pub fn cover_square(img: &DynamicImage, size: u32) -> RgbaImage
```

Aspect-fill resize plus center crop to an exact square for photo and
video previews. Uses Triangle resampling: Lanczos3 on a
multi-megapixel photo blocked the main thread for seconds, and at
64px thumbnails the difference is invisible.

### `round_corners`

```rust
pub fn round_corners(img: &mut RgbaImage, radius: u32)
```

Rounds image corners in place (transparent outside the radius). GTK
CSS `border-radius` does not clip image content, so previews bake
the mask into the alpha channel.

### `watch_dir`

```rust
pub fn watch_dir(path: &Path) -> Option<(RecommendedWatcher, Receiver<()>)>
```

Watches a directory (non-recursive) with `notify` and signals per
file system event. The UI drains the channel on a 400ms main-thread
tick and rebuilds the grid once per burst, so the Finder view stays
in sync with the folder. The tick tracks watcher and menu/edit
signals separately (`tick_should_rebuild`): while an inline rename is
open, watcher bursts are dropped and only explicit menu/edit signals
rebuild, with the snapshot synced so no stale rebuild fires after the
edit commits. Returns `None` when watching fails.

### `tick_should_rebuild`

```rust
fn tick_should_rebuild(editing: bool, watch_signaled: bool, menu_signaled: bool) -> bool
```

Whether the 400ms tick rebuilds the grid. While an inline rename is
open (`editing`), only explicit menu/edit signals rebuild; watcher
bursts are dropped, because every rebuild opens files (image decodes,
icon cache checks), the watcher reports those opens, and rebuilding
on them would loop every 400ms, steal focus and freeze the UI. When
idle, either signal rebuilds (still gated by the `snapshot` diff at
the call site).

- Returns `menu_signaled` when `editing` is true.
- Returns `watch_signaled || menu_signaled` otherwise.

### `snapshot`

```rust
pub fn snapshot(path: &Path) -> Vec<SnapshotEntry>
```

Sorted metadata snapshot (name, kind, size, mtime seconds) for
change detection. The watcher also fires on plain file opens, and
every grid rebuild opens files (image decodes, icon cache checks):
rebuilding on those would retrigger itself forever and starve the
main thread, so the tick only rebuilds when this snapshot differs.

### `refresh_grid`

```rust
fn refresh_grid(grid: &FlowBox, status: &Label, pal: &Palette, base: &Path, session: &SharedSession, refresh: &Refresh, listed: &Rc<RefCell<Vec<ListedEntry>>>, rebuild: &Rebuild)
```

Clears the grid and rebuilds it from `list_dir(base)` plus the status
line. Always lists `base` (never a hardcoded folder), so the shown
entries match the watched directory. Records the listing for
double-click lookup and wires right-click selection per cell.

### `set_cell_selected`

```rust
fn set_cell_selected(flow_child: &FlowBoxChild, selected: bool)
```

macOS-style selection: gray rounded background behind the icon
(`fd-art-sel`, scheme-aware) plus blue tightly around the label
text with white letters (`fd-lbl-sel`, never a full-width bar).
Walks the cell recursively (artwork and labels sit inside their
menu wrappers). Native `FlowBoxChild` selection backgrounds are
neutralized (transparent), so only the icon gray and the tight
label blue ever show.

## Empty-space context menu

Right-clicking empty space shows a TontooUI `ContextMenu`
(`empty_space_menu()` in `src/views/finder.rs`): empty grid area,
cell padding around icon and text, and (in the list view) the gap
plus the date and size columns. Left-click selection is unaffected.
Every wrapper popover registers in a thread-local list
(`OPEN_MENUS`); any press inside the window (capture gestures on
the root, both mouse buttons), selection changes, view switches
and rebuilds dismiss all menus explicitly
(`popdown_all_menus`, then `forget_menus` on rebuild). Cell and
row popovers hang detached while idle (zero size contribution,
kept alive by the registry) and a capture gesture re-attaches
them right before the SDK popup runs, so coordinates, grab,
autohide and popdown behave like a textbook popover; scroll
popovers stay parented. Never hide popovers with `set_visible`
(breaks the grab state machine) or exclude them with
`set_child_visible` (breaks mapping entirely).
Presses outside the window never reach these gestures either, so
the 400ms main-thread tick also polls the application window
active state (`app_window_active` via `list_toplevels` plus
`ApplicationWindow::is_active`); any flip in either direction
dismisses all menus (`popdown_all_menus`) and logs
`[finder][menu] window active changed -> {bool}`. The check is
cheap and quiet (no log when unchanged). Presses on dead menu
areas (dividers, padding, tag dots) dismiss only the menu:
`attach_menu_background_dismiss` hit-tests with `pick` and lets
button presses through to their actions, so menu-unselect and
Finder presses stay distinguished.

| Order | Entry |
|---|---|
| 1 | `context.new_folder` (New Folder: creates `folder.untitled`, numbered when taken, then opens inline rename) |
| 2 | Divider |
| 3 | `context.get_info` (Get Info, logs for now) |
| 4 | Divider |
| 5 | `context.new_file` submenu (New File) with `context.text_file` (Text File, no action yet) |

Each icon and each name carries its own `ContextMenu` (see
below); their inner gestures claim the press first, so this menu
stays hidden over icons and text. All actions log for now;
creating folders/files and the info panel are later steps.

### `popdown_all_menus`

```rust
fn popdown_all_menus()
```

Dismisses every registered context-menu popover (plus nested
submenu popovers via `popdown_tree`). Called on presses inside
the window, selection changes, view switches, rebuilds, and
window active flips. Logs a toplevel census only when a menu was
open or a stray toplevel exists.

### `app_window_active`

```rust
fn app_window_active() -> Option<bool>
```

Current active state of the application window (`true` when
focused). Scans `gtk::Window::list_toplevels()` for the first
`gtk::ApplicationWindow` (fallback: plain `gtk::Window`) and
returns `is_active()`. Returns `None` when no window exists (for
example in headless tests).

### `active_flipped`

```rust
fn active_flipped(prev: Option<bool>, current: bool) -> bool
```

True when a previous active state exists and differs from the
current one (flip in either direction). Returns `false` on the
first observation (`None`), so startup never dismisses menus.

### `attach_menu_background_dismiss`

```rust
fn attach_menu_background_dismiss(pop: &gtk::Popover)
```

Dismisses the menu on presses that hit no interactive child
(dividers, padding, tag dots): picks the deepest widget under the
press and only pops down when it is not inside a `GtkButton`.
Button presses pass through to their actions. Attached to every
registered popover, so menu-unselect never touches Finder state.

## File context menu

Right-clicking an icon or a name shows a per-file `ContextMenu`
(`file_menu_entries(name)` in `src/views/finder.rs`,
`file_menu_wrap()` hugs one widget) and selects the item first
(capture gesture on the cell selects it before the menu opens, like
Finder). Grid cells wrap artwork and
label separately, list rows wrap icon plus name; presses anywhere
else (padding, gaps, date/size columns) fall through to the
empty-space menu above.

| Order | Entry |
|---|---|
| 1 | `context.open` (Open: launches `.app` bundles via LaunchPad, other files log for now) |
| 2 | `context.open_with` (Open With) with trailing `arrowtriangle.forward.fill`, no action |
| 3 | Divider |
| 4 | `context.move_to_trash` (Move to Trash, logs for now) |
| 5 | Divider |
| 6 | `context.get_info` (Get Info, logs for now), `context.rename` (Rename: inline edit below the icon, Enter commits, Escape cancels) |
| 7 | `context.compress`, `context.duplicate` (both log for now) |
| 8 | `context.share` (Share) with trailing `arrowtriangle.forward.fill`, logs for now |
| 8 | Divider |
| 9 | `context.copy` (`Copy "{name}"` with the file name, logs for now) |
| 10 | Divider |
| 11 | `context.tags` (Tags...) with the 7 tag dots, no action |

### `app_icon`

```rust
pub fn app_icon(entry: &Path) -> Option<PathBuf>
```

App icon for a `.app` entry, rendered once through CoreIcon and
cached. Bundle directories resolve the raw icon from
`Resources/icon.png`, `App/icon.png`, `Resources/app_icon.png`, else
the `icon` field of `tontoo.proj`. `.app` ZIP archives (TBuild bundle
layout) extract the first matching entry to a temp file; entries may
sit behind a top-level `<Name>.app/` prefix, so candidates match by
path suffix. Rendering
uses `CoreIcon::generator::AppIcon::from_file` (original colors) plus
a Lanczos3 downscale to 192px. Returns `None` when no icon is found
or rendering fails (caller shows the default folder artwork).

### `item_count`

```rust
pub fn item_count(entries: &[DirEntry]) -> usize
```

Returns the entry count. Used by the status line.

## Status line

The bottom row shows one centered 11px secondary label built from
`status.line` with `{count}` and `{free}` replaced. The count
follows the active view (filtered entries while searching); the
free space is live from `statvfs` (`model::free_bytes`, available
blocks times block size), formatted like list sizes with a decimal
comma in German:

| Key | en_us | de_de |
|---|---|---|
| `status.line` | `{count} items, {free} available` | `{count} Objekte, {free} verfügbar` |
| `status.free` | fallback placeholder | fallback placeholder |

`status.free` only shows when the free space is unknown (non-Linux
or unreadable path).

### `free_bytes`

```rust
pub fn free_bytes(path: &Path) -> Option<u64>
```

Free space of the filesystem holding `path`, in bytes. Linux-only;
other systems yield `None`.

## Colors

All text uses the `SF Pro Display` family, resolved from the system font
paths (`/usr/share/fonts/OTF/SF-Pro-Display-Regular.otf`, etc.).

| Token | Dark | Light |
|---|---|---|
| Background | `#1d1d1d` | `#ececec` |
| Primary text | `#F5F5F7` | `#1E1E1E` |
| Secondary text | `#A1A1A6` | `#6E6E73` |
| Folder body | `#7fbeec` | `#7fbeec` |
| Folder edge | `#5ea3d8` | `#5ea3d8` |

The scheme is read from `uikit::app::current_color_scheme()` with a
`ColorScheme::detect_system()` fallback, so the window matches the live
system theme on every rebuild.

## Localization

Strings live in `lang/en_us.json` and `lang/de_de.json` (only these
two). `src/lang.rs` detects German from `LANGUAGE`, `LC_ALL`, `LANG`
or `/etc/locale.conf` and falls back to `en_us`.

`Resources/lang/` holds copies of both files: TBuild copies only
`Resources/` into the `.app` bundle (root `lang/` is used just for the
localized `name` in `Info.tontoo`). Keep both locations in sync.

| Key | en_us | de_de |
|---|---|---|
| `app.title` | `Finder` | `Finder` |
| `sidebar.search` | `Search` | `Suchen` |
| `sidebar.favourites` | `Favourites` | `Favoriten` |
| `sidebar.locations` | `Locations` | `Orte` |
| `sidebar.tags` | `Tags` | `Tags` |
| `sidebar.recents` | `Recents` | `Zuletzt` |
| `sidebar.applications` | `Applications` | `Programme` |
| `sidebar.downloads` | `Downloads` | `Downloads` |
| `sidebar.documents` | `Documents` | `Dokumente` |
| `sidebar.desktop` | `Desktop` | `Schreibtisch` |
| `sidebar.osx` | `OSX` | `OSX` |
| `sidebar.network` | `Network` | `Netzwerk` |
| `tags.professional` | `Professional` | `Professionell` |
| `tags.urgent` | `Urgent` | `Dringend` |
| `tags.active` | `Active` | `Aktiv` |
| `tags.reference` | `Reference` | `Referenz` |
| `tags.personal` | `Personal` | `Persönlich` |
| `tags.creative` | `Creative` | `Kreativ` |
| `tags.archive` | `Archive` | `Archiv` |
| `detail.search` | `Search` | `Suchen` |
| `detail.empty` | `Downloads is empty` | `Downloads ist leer` |
| `detail.empty.hint` | `Files you download appear here.` | `Heruntergeladene Dateien erscheinen hier.` |
| `folder.untitled` | `Untitled Folder` | `Unbenannter Ordner` |
| `context.new_folder` | `New Folder` | `Neuer Ordner` |
| `context.get_info` | `Get Info` | `Informationen` |
| `context.new_file` | `New File` | `Neue Datei` |
| `context.text_file` | `Text File` | `Textdatei` |
| `context.open` | `Open` | `Öffnen` |
| `context.open_with` | `Open With` | `Öffnen mit` |
| `context.move_to_trash` | `Move to Trash` | `In den Papierkorb legen` |
| `context.rename` | `Rename` | `Umbenennen` |
| `context.compress` | `Compress` | `Komprimieren` |
| `context.duplicate` | `Duplicate` | `Duplizieren` |
| `context.share` | `Share` | `Teilen` |
| `context.copy` | `Copy "{name}"` | `"{name}" kopieren` |
| `context.tags` | `Tags...` | `Tags …` |

### `t(key)`

```rust
pub fn t(key: &str) -> String
```

Returns the localized string for `key`. Returns the key itself when the
locale file or key is missing, so the UI never renders empty text.

## Usage / Example

```bash
cargo run
LANG=de_DE.UTF-8 cargo run
```

The first command shows English strings (`Favourites`), the second
German strings (`Favoriten`).

## Debug logging

Rename, New Folder, the 400ms tick, grid rebuilds and the icon
pipeline log timing info to stderr with a `[finder]` prefix:

| Prefix | Meaning |
|---|---|
| `[finder][rename]` | Menu click, Enter commit, commit result plus rebuild time |
| `[finder][new_folder]` | Menu click, created folder, refresh signal |
| `[finder][tick]` | Watcher/menu signals per tick, snapshot verdict, rebuild time |
| `[finder][refresh]` | `list_dir` time, cells slower than 20ms, total rebuild time |
| `[finder][preview]` | Image decode time per photo (in-memory fallback) |
| `[finder][photo]` | Preview cache generation time per photo (once) |
| `[finder][icons]` | Icon file load time (once per file, then shared) |
| `[finder][menu]` | Menu dismissals, toplevel census, window active flips |
| `[finder][view]` | View button clicks, mode switches and toolbar button count |
| `[finder][nav]` | Navigation button clicks, opened folders and file-open notes |
| `[finder][video]` | Frame extraction time per video |
| `[finder][app_icon]` | Cache hit or CoreIcon render time per `.app` |
| `[finder][ffmpeg]` | One-time `ffmpeg` probe time (result is cached) |
| `[finder][create_folder]` | Folder creation time |
| `[finder][commit_rename]` | Rename validation plus filesystem time |

Run with stderr visible to find the slow step, e.g. cells slower
than 20ms or a multi-second `[finder][refresh] rebuilt ...` line:

```bash
cargo run 2> finder.log
```

## Packaging

`tontoo.proj` (`bundle_id: com.tontoo.finder`) lets TBuild assemble the
`.app` bundle:

```bash
tbuild app /path/to/Finder
```

The bundle contains the release binary (`App/`), the icon
(`Resources/app_icon.png`) and `Resources/lang/` (`lang/`). The runtime
lookup covers the bundle layout
(`<Name>.app/Resources/lang`), dev checkouts (`lang/`,
`Resources/`) and installed files (`/usr/share/finder/`).

## Cross References

- [MAIN.md](MAIN.md) -- wiki entry point
- TontooUI Sidebar -- sidebar with traffic lights, sections and item list
- TontooUI Toolbar -- glass capsule toolbar with items
- CoreIcon SF Symbols -- sidebar and toolbar icon artwork
