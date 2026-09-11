//! Icon lookup for Finder.
//!
//! Resolves files under `Resources/foldericons/` (SVG icon theme with
//! `scalable/`, `16/`, `22/`, `24/`, `colors/` and `symbolic/`). The grid
//! uses the full-color `scalable/` SVGs through GTK (librsvg); the
//! sidebar keeps CoreIcon SF Symbols because `SidebarIcon::file` relies
//! on the `image` crate, which cannot decode SVG.

use std::path::PathBuf;

/// Candidate directories holding the `Resources/` folder.
fn resources_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  if let Ok(cwd) = std::env::current_dir() {
    dirs.push(cwd.join("Resources"));
  }
  if let Ok(exe) = std::env::current_exe() {
    if let Some(parent) = exe.parent() {
      dirs.push(parent.join("Resources"));
      if let Some(grand) = parent.parent() {
        dirs.push(grand.join("Resources"));
        // .app bundle layout: <Name>.app/{App/binary, Resources/...}.
        dirs.push(grand.join("Resources"));
      }
    }
  }
  dirs.push(PathBuf::from("/usr/share/finder/Resources"));
  dirs
}

/// Resolve `Resources/foldericons/<size>/<file>` to an existing path.
/// Returns `None` when no candidate layout holds the file.
pub fn icon_path(size: &str, file: &str) -> Option<PathBuf> {
  for dir in resources_dirs() {
    let path = dir.join("foldericons").join(size).join(file);
    if path.is_file() {
      return Some(path);
    }
  }
  None
}

/// Resolve a full-color grid icon from `scalable/`.
pub fn folder_icon(file: &str) -> Option<PathBuf> {
  icon_path("scalable", file)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_icon_returns_none() {
    assert!(icon_path("scalable", "does-not-exist-finder.svg").is_none());
  }

  #[test]
  fn known_grid_icons_exist() {
    // Runs from the crate root in dev checkouts, so the default grid
    // icon must resolve. Skipped silently when run from another layout.
    if std::env::current_dir()
      .map(|cwd| cwd.join("Resources/foldericons/scalable/folder.svg"))
      .map(|path| path.is_file())
      .unwrap_or(false)
    {
      assert!(folder_icon("folder.svg").is_some());
    }
  }
}
