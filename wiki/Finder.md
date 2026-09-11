# Finder

TontooOS file manager basis: a 1080x720 TontooUI window with a
Tahoe-style sidebar on the left (Favourites, Locations, Tags), a toolbar
row with navigation plus view and action controls plus search, a folder
icon grid in the middle and a status line at the bottom. The grid shows
static folders from `src/model.rs`; real filesystem listing and sidebar
selection wiring are later steps.

## Layout

From left to right the window contains:

1. Sidebar (`Sidebar`, 240px, traffic lights, search, sections)
2. Detail column: toolbar row, folder grid (`FlowBox`), status line

```rust
let mut app = App::with_delegate(lang::t("app.title"), 1080, 720, FinderDelegate);
// No extra window bar: the sidebar draws the only traffic lights.
app.no_window_bar();
app.auto_color_scheme(); // live Dark/Light follow
app.run();
```

## Sidebar

`TontooUI::Sidebar` with the `coreicon` feature (default). Three
sections mirror the macOS Finder: Favourites, Locations and Tags. Tag
rows use the SF Symbol `circle.fill` in the tag color.

| Section | Rows |
|---|---|
| `sidebar.favourites` | Recents (`clock.fill`), Applications (`square.stack.3d.up.fill`, blue), Downloads (`arrow.down.circle.fill`, blue), Documents (`doc.fill`), Desktop (`desktopcomputer`) |
| `sidebar.locations` | OSX (`internaldrive.fill`), Network (`network`) |
| `sidebar.tags` | Professional (blue), Urgent (red), Active (orange), Reference (yellow), Personal (green), Creative (purple), Archive (gray), all `circle.fill` |

```rust
let sidebar = Sidebar::new()
  .section(lang::t("sidebar.favourites"))
  .item(lang::t("sidebar.recents"), SidebarIcon::sf("clock.fill", gray()))
  .section(lang::t("sidebar.locations"))
  .item(lang::t("sidebar.osx"), SidebarIcon::sf("internaldrive.fill", gray()))
  .section(lang::t("sidebar.tags"))
  .item(lang::t("tags.personal"), SidebarIcon::sf("circle.fill", green()))
  .selected(0)
  .search_placeholder(lang::t("sidebar.search"))
  .width(240.0)
  .on_select(|i| println!("Finder selected: {}", i));
```

Rules:

- Selection currently only logs the index; swapping the detail content
  per row is a later step.
- Grey icons use `Color::from_rgb(142, 142, 147)`, blue icons use
  `Color::from_rgb(0, 122, 255)`.

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

## Folder grid

A `gtk::FlowBox` (4-8 columns, homogeneous) shows the 17 static folders
from `src/model.rs::folders()`. Each cell is a vertical box with the
themed folder icon from `Resources/foldericons/scalable/` (full-color
SVG rendered by GTK through librsvg, 64px) and a two-line centered
`SF Pro Display` label (`.fd-label`, 12px). When an icon file is
missing the cell falls back to Tahoe CSS artwork (`.fd-tab` plus
`.fd-body` in Finder blue `#7fbeec` with edge `#5ea3d8`), so the grid
never renders an empty cell.

`src/icons.rs` resolves `Resources/foldericons/<size>/<file>` across
dev checkouts (`Resources/`), `.app` bundles and installed files
(`/usr/share/finder/`). The sidebar keeps CoreIcon SF Symbols:
`SidebarIcon::file` relies on the `image` crate, which cannot decode
SVG.

| Folder | Icon |
|---|---|
| Applications | `blue-folder.svg` |
| Applications (Parallels), Parallels | `folder-vbox.svg` |
| Books | `folder-book.svg` |
| Business | `folder-chart.svg` |
| Desktop | `blue-user-desktop.svg` |
| Documents | `blue-folder-documents.svg` |
| Downloads | `blue-folder-download.svg` |
| Movies | `blue-folder-videos.svg` |
| Music | `blue-folder-music.svg` |
| News | `folder-notes.svg` |
| Pictures | `blue-folder-images.svg` |
| Projects | `folder-projects.svg` |
| Public | `blue-folder-public.svg` |
| Scripts | `folder-script.svg` |
| Simulations | `folder-calculate.svg` |
| Software | `folder-appimage.svg` |

### `folders`

```rust
pub fn folders() -> Vec<Folder>
```

Returns the static folders shown in the grid. `Folder` holds `name`
(proper noun, untranslated) and `icon` (SVG file in `scalable/`).

### `folder_icon`

```rust
pub fn folder_icon(file: &str) -> Option<PathBuf>
```

Resolves a `scalable/` icon file. Returns `None` when no layout holds
the file.

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
| `detail.title` | `Home` | `Home` |

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
