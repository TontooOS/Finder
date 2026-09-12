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
single Favourites section with the Downloads row; Locations and Tags
sections are later steps.

| Section | Rows |
|---|---|
| `sidebar.favourites` | Downloads (`arrow.down.circle.fill`, blue) |

```rust
let sidebar = Sidebar::new()
  .section(lang::t("sidebar.favourites"))
  .item(lang::t("sidebar.downloads"), SidebarIcon::sf("arrow.down.circle.fill", blue()))
  .selected(0)
  .search_placeholder(lang::t("sidebar.search"))
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
| View switch | `square.grid.2x2` (icons), `list.bullet` (list), no actions yet |
| Actions | `square.and.arrow.up` (share), `magnifyingglass` (search) |

All buttons currently log their action; view switching and search
filtering are later steps.

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
`#5ea3d8`), so the grid never renders an empty cell.

`src/icons.rs` resolves `Resources/foldericons/<size>/<file>` across
dev checkouts (`Resources/`), `.app` bundles and installed files
(`/usr/share/finder/`). The sidebar keeps CoreIcon SF Symbols:
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

Creates a uniquely named folder (`folder.untitled`, numbered when
taken). Found in `src/views/finder.rs`; the caller opens inline
rename on the result.

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

### `video_thumb`

```rust
pub fn video_thumb(source: &Path) -> Option<PathBuf>
```

First-second frame of a video as a cached PNG (`ffmpeg -ss 1`,
`scale=320:-1`). Returns a fresh extraction or the cached file.
Returns `None` when `ffmpeg` is missing or extraction fails (caller
shows the themed video icon). Audio files resolve
`Resources/extensionicons/audio.png` via `audio_icon()`.

### `cover_square`

```rust
pub fn cover_square(img: &DynamicImage, size: u32) -> RgbaImage
```

Aspect-fill resize plus center crop to an exact square for photo and
video previews.

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
in sync with the folder. Watcher rebuilds pause while an inline
rename is open (`watch_refresh_allowed`); menu and edit signals
always rebuild. Returns `None` when watching fails.

### `snapshot`

```rust
pub fn snapshot(path: &Path) -> Vec<SnapshotEntry>
```

Sorted metadata snapshot (name, kind, size, mtime seconds) for
change detection. The watcher also fires on plain file opens, and
every grid rebuild opens files (image decodes, icon cache checks):
rebuilding on those would retrigger itself forever and starve the
main thread, so the tick only rebuilds when this snapshot differs.

## Empty-space context menu

Right-clicking empty grid space shows a TontooUI `ContextMenu`
(`empty_space_menu()` in `src/views/finder.rs`):

| Order | Entry |
|---|---|
| 1 | `context.new_folder` (New Folder: creates `folder.untitled`, numbered when taken, then opens inline rename) |
| 2 | Divider |
| 3 | `context.get_info` (Get Info, logs for now) |
| 4 | Divider |
| 5 | `context.new_file` submenu (New File) with `context.text_file` (Text File, no action yet) |

Each file cell carries its own `ContextMenu` (see below); its inner
gesture claims the press first, so this menu stays hidden over files.
All actions log for now; creating folders/files and the info panel
are later steps.

## File context menu

Right-clicking a file or folder shows a per-file `ContextMenu`
(`file_menu_entries(name)` in `src/views/finder.rs`). The inner cell
gesture claims the press first, so the empty-space menu stays hidden
over files.

| Order | Entry |
|---|---|
| 1 | `context.open` (Open, logs for now) |
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
`status.line` with `{count}` and `{free}` replaced:

| Key | en_us | de_de |
|---|---|---|
| `status.line` | `{count} items, {free} available` | `{count} Objekte, {free} verfügbar` |
| `status.free` | `1.06 TB` | `1,06 TB` |

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
