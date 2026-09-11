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

Three `TontooUI::Toolbar` groups plus a `TontooUI::TextInput` search
field in a `gtk::Box` row:

| Group | Items |
|---|---|
| Navigation | `chevron.left` (back), `chevron.right` (forward) |
| View switch | `square.grid.2x2` (icons), `list.bullet` (list), `rectangle.split.3x1` (columns), `rectangle.stack` (gallery) |
| Actions | `arrow.up.arrow.down` (sort), `square.and.arrow.up` (share), `tag` (tag) |
| Search | `TextInput` with `detail.search` placeholder, 170px wide |

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
by kind: images show the picture itself, videos show the cached
first-second frame (`video_thumb`, temp dir `finder-thumbs/`, keyed by
size plus mtime) with the themed `blue-folder-videos.svg` fallback
while `ffmpeg` is missing, audio files show `blue-folder-music.svg`,
archives (zip, rar, 7z, tar, gz, gzip, bz2) show
`Resources/extensionicons/zip.png`.
Other files show no icon, only the name with the extension stripped.
Entries sort directories-first, then alphabetically
(case-insensitive). An empty or unreadable directory shows a
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

Directories keep their name; files lose the extension
(`archive.tar.gz` shows as `archive.tar`).

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

Classifies a lowercase extension as `Image`, `Video`, `Audio` or
`Other`, driving the grid preview.

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
shows the themed video icon). No extra icon pack is needed for audio:
`audio_icon()` resolves the shipped `blue-folder-music.svg`.

### `app_icon`

```rust
pub fn app_icon(entry: &Path) -> Option<PathBuf>
```

App icon for a `.app` entry, rendered once through CoreIcon and
cached. Bundle directories resolve the raw icon from
`Resources/icon.png`, `App/icon.png`, `Resources/app_icon.png`, else
the `icon` field of `tontoo.proj`. `.app` ZIP archives (TBuild bundle
layout) extract the first matching entry to a temp file. Rendering
uses `CoreIcon::generator::AppIcon::from_file` (original colors) plus
a Lanczos3 downscale to 192px. Returns `None` when no icon is found
or rendering fails (caller shows the default folder artwork).

### `item_count`

```rust
pub fn item_count() -> usize
```

Returns `folders().len()` (currently 17). Used by the status line.

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
- TontooUI TextInput -- single-line search field
- CoreIcon SF Symbols -- sidebar and toolbar icon artwork
