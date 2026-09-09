# Finder – Wiki

Finder is the TontooOS file manager basis: a 1080x720 TontooUI window with
a Tahoe-style sidebar (Favourites, Locations, Tags with CoreIcon SF
Symbols), a toolbar row (navigation, view switch, actions, search), a
folder icon grid and a status line. It follows the live system color
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

## Quick Start

Run the Finder window from the repository root:

```bash
cargo run
```

The window follows the GNOME system theme live (Dark `#1d1d1d`, Light
`#ececec`) and picks German strings when `LANG` starts with `de`.

See [Finder.md](Finder.md) for details.

## Changelog

- 2026-09-09: Initial Finder basis (TontooUI Sidebar + toolbar + folder grid + status line, `lang/en_us.json` and `lang/de_de.json`).
