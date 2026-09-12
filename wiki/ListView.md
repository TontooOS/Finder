# ListView

List view for Finder plus the toolbar view switch: the two view
buttons (`square.grid.2x2`, `list.bullet`) toggle between the icon
grid and a column list (Name, Date Modified, Size) like the
reference layout. The active button carries a select indicator, and
the choice persists per user through CoreData.

## View switch

The toolbar view group holds both buttons in one glass capsule.
Clicking sends the mode through a channel; the 400ms main-thread
tick applies it (swap content, move the indicator, persist), so
`Send`-bound menu callbacks never touch GTK widgets directly.

| Button | Icon | Mode |
|---|---|---|
| 1 | `square.grid.2x2` | Grid (icon grid) |
| 2 | `list.bullet` | List (this page) |

### `collect_view_buttons`

```rust
fn collect_view_buttons(toolbar: &gtk::Widget) -> Vec<gtk::Button>
```

Collects the item buttons of a `Toolbar` in order (root,
title-area, bar, glass group, buttons). Used once at startup to
keep handles for the select indicator.

### `apply_view_indicator`

```rust
fn apply_view_indicator(buttons: &[gtk::Button], mode: prefs::ViewMode)
```

Moves the select indicator: the active button (grid first, list
second) gets the `fd-view-on` class (blue pill,
`rgba(10,132,255,0.85)`), mirroring the sidebar selection.

### `refresh_content`

```rust
fn refresh_content(ctx: &ViewCtx, rebuild: &Rebuild)
```

Rebuilds the content slot for the active view: icon grid (with the
empty view when the folder is empty) or the list below. Called by
the tick, the view switch and rename commit/cancel.

## List layout

A fixed header row plus one `GtkListBoxRow` per entry in a
`ScrolledWindow` (empty folders show the shared
`ContentUnavailableView`). Header and rows share spacing (8),
margins (16) and fixed column widths, so columns align:

| Column | Width | Align |
|---|---|---|
| Icon (20px) plus Name (expands) | flexible | Start, ellipsized |
| Date Modified | 150px (`DATE_WIDTH`) | Center |
| Size | 110px (`SIZE_WIDTH`) | End |

Single-click selection uses the native list selection styled like
the grid (`.fd-row:selected`, blue fill plus white labels);
right-clicking a row selects it first (capture gesture), then opens
its file menu. Double-click (or Enter) on a folder opens it, see
navigation in [Finder.md](Finder.md). The
file menu hugs icon plus name (`file_menu_wrap`); the expanding
gap and the date/size columns fall through to the empty-space
menu. Each row carries the same per-file context menu as grid
cells (Rename opens the inline field inside the row); the
empty-space menu wraps the scrolled list.

### `list_header`

```rust
fn list_header(pal: &Palette) -> gtk::Box
```

Header row with `list.name`, `list.date`, `list.size` (12px bold,
secondary). A 20px spacer aligns `list.name` with the row names.

### `list_row`

```rust
fn list_row(base: &Path, entry: &DirEntry, pal: &Palette, german: bool, session: &SharedSession, refresh: &Refresh, rebuild: &Rebuild) -> gtk::ListBoxRow
```

One row: 20px type icon, name (13px, expands, ellipsized) or the
inline rename field, date (12px, secondary), size (12px,
secondary, right). Folders show no size.

### `row_icon`

```rust
fn row_icon(base: &Path, entry: &DirEntry) -> gtk::Widget
```

Small type icon at the far left: folders use `folder.svg`,
`.app` bundles their rendered icon, photos and videos their
placeholder (`image.png`, `video.png`, never decoded thumbnails),
audio and archives their themed icon, other files their document
icon (`basis.png` fallback). Falls back to the generic icon, then
to an empty gap, so the name column never shifts.

### `refresh_list`

```rust
fn refresh_list(ctx: &ViewCtx, rebuild: &Rebuild)
```

Rebuilds header plus rows and the status line. Logs the row count
and time (`[finder][refresh] rebuilt ... list rows`).

### `file_menu_wrap`

```rust
fn file_menu_wrap(inner: impl IsA<gtk::Widget>, base: &Path, entry: &DirEntry, session: &SharedSession, refresh: &Refresh) -> gtk::Widget
```

Wraps one widget (grid artwork, grid label, list icon-plus-name)
with the per-file context menu. Menus hug content, so presses on
padding, gaps or date/size columns fall through to the
empty-space menu.

## Columns

### `file_meta`

```rust
pub fn file_meta(base: &Path, name: &str) -> FileMeta
```

Size plus mtime of one entry with a single metadata call.
`FileMeta` holds `len` (bytes, always 0 for directories) and
`mtime_secs` (unix seconds, 0 when unknown). Missing entries yield
zeros.

### `format_size`

```rust
pub fn format_size(bytes: u64) -> String
```

Human size: `1 KB`, `23 KB`, `1.5 MB`, `9.53 GB`, `10 GB`
(Kilobytes round up with a 1 KB minimum, larger units carry
trimmed decimals).

### `format_mtime`

```rust
pub fn format_mtime(secs: u64, german: bool) -> String
```

Local modification date: `12.09.2026 10:05` in German,
`09/12/2026 10:05` in English. Unknown times yield `--`.

## Persistence

### `ViewMode`

```rust
pub enum ViewMode { Grid, List }
```

```rust
pub fn as_str(self) -> &'static str
pub fn from_str(raw: &str) -> Self
```

Stored as `grid` / `list`; unknown values fall back to grid.

### `load_view_mode`

```rust
pub fn load_view_mode() -> ViewMode
```

Reads entity `FinderPrefs`, id `view`, key `mode` from CoreData
(bundle `com.tontoo.finder`, `Fico` store). CoreData isolates
storage per app and per user, so every user keeps their own
choice. Any failure returns grid and only logs
(`[finder][prefs]`).

### `save_view_mode`

```rust
pub fn save_view_mode(mode: ViewMode)
```

Upserts the mode object and saves. Failures only log; Finder
never crashes over preferences.

## Usage / Example

```bash
cargo run
```

Click the list button (right view button): the indicator moves,
rows appear with date and size, and the choice survives restarts
of the app (per user).

## Cross References

- [MAIN.md](MAIN.md) – wiki entry point
- [Finder.md](Finder.md) – grid, toolbar, status line and localization
- CoreData (TontooLibs) – `PersistentContainer` behind the per-user persistence
