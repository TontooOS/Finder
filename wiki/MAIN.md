# Finder – Wiki

Finder is the TontooOS file manager basis: a 1080x720 TontooUI window with
a Tahoe-style sidebar (currently Favourites with Downloads), a toolbar
row (navigation, view switch, actions, search), a live `~/Downloads/`
grid and a status line. It follows the live system color
scheme and loads `en_us`/`de_de` strings from `lang/`.

- Repository: https://github.com/TontooOS/TontooOS
- License: TCL v26.1
- Version: 0.1.0

## Feature Index

| Feature | File | Description |
|---|---|---|
| Main index | [MAIN.md](MAIN.md) | This page |
| Rules | [RULE.md](RULE.md) | Development and usage rules |
| Finder | [Finder.md](Finder.md) | Sidebar layout, toolbar, folder grid, status line and localization |
| ListView | [ListView.md](ListView.md) | List view with date/size columns, view switch with indicator, per-user CoreData persistence |

## Quick Start

Run the Finder window from the repository root:

```bash
cargo run
```

The window follows the GNOME system theme live (Dark `#1d1d1d`, Light
`#ececec`) and picks German strings when `LANG` starts with `de`.

See [Finder.md](Finder.md) for details.

## Changelog

- 2026-09-12: macOS-style grid selection (gray behind icon, blue tightly around label text).
- 2026-09-12: Search field slides in/out with a revealer animation.
- 2026-09-12: Search swaps run as idle callbacks (no toolbar mutation mid-emission, no `get_parent` criticals).
- 2026-09-12: Status line shows live free space (`statvfs`) with German decimal comma, `status.free` as fallback.
- 2026-09-12: Track popover parents with local flags (never GTK queries); rename commit/cancel rebuild via the shared handle.
- 2026-09-12: Toolbar search with live name filtering, collapse-keeps-filter, Escape clears, no-match empty state.
- 2026-09-12: Folders open on double-click only (single-click selects).
- 2026-09-12: Dead menu areas (dividers, padding, tags) dismiss only the menu via hit-testing; button presses run actions.
- 2026-09-12: Detach idle cell/row popovers and re-attach before popup (narrow columns with textbook grab/autohide/popdown).
- 2026-09-12: Folder navigation (double-click/Enter opens folders, back/forward history, title follows, watcher re-armed per folder).
- 2026-09-12: Keep popovers out of measurement with `set_child_visible` instead of `set_visible` (fixes broken grab/autohide/popdown).
- 2026-09-12: Dismiss context menus on window focus change (400ms tick polls `ApplicationWindow::is_active`).
- 2026-09-12: Taken names count up (`Docs (2)`, `wallpaper (2).png`) on rename and create instead of failing.
- 2026-09-12: Dismiss menus on any press inside the window (capture gestures); census logging only when a menu was open.
- 2026-09-12: Dismiss all menus explicitly on selection change, view switch and rebuild (`popdown_all_menus` registry).
- 2026-09-12: Removed the sidebar search field (`.no_search()`).
- 2026-09-12: Right-click selects the item first in both views (capture gesture, Finder behavior).
- 2026-09-12: File menus hug icon and text in both views; padding, gaps and date/size columns open the empty-space menu.
- 2026-09-12: List view with Name/Date/Size columns, toolbar view switch with select indicator, per-user CoreData persistence (`com.tontoo.finder`).
- 2026-09-12: Padded all `extensionicons` to a square 192x192 canvas so every document icon renders at equal width and height.
- 2026-09-12: Scaled the whole grid down 25% (84px cells, 48px artwork, 54px folders) with one shared `PREVIEW_SIZE`.
- 2026-09-12: Hide idle menu popovers so columns fit 8 across, folders render at 72px for visual parity, and the column width is logged.
- 2026-09-12: Fixed square 112x112 cells with uniform 64x64 artwork (`gtk::Picture`, `Contain` fit) and dynamic FlowBox reflow on window resize.
- 2026-09-12: Share icon textures across cells (`shared_paintable`) and use the `image.png`/`video.png` placeholders, cutting ~30ms of SVG rasterization per entry per rebuild.
- 2026-09-12: Cache photo grid previews on disk (`photo_preview`) and use Triangle resampling, fixing the ~19s rebuild stall on 4K photos.
- 2026-09-12: Added stderr timing logs (`[finder]` prefix) across the rename/new-folder path to trace the ~10s UI stall.
- 2026-09-12: Fixed Rename/New Folder UI freeze (tick drops watcher bursts while editing, cached `ffmpeg` check, `refresh_grid` lists `base`).
- 2026-09-09: Initial Finder basis (TontooUI Sidebar + toolbar + folder grid + status line, `lang/en_us.json` and `lang/de_de.json`).
