//! Finder data model (basis).
//!
//! Live listing of `~/Downloads/` for the Downloads sidebar row.
//! Directories render the default `folder.svg` icon, files render as a
//! plain text label with the extension stripped. Entries sort
//! directories-first, then alphabetically (case-insensitive). An
//! unreadable or missing directory yields an empty list; all chrome
//! text comes from `lang/`.

use std::path::{Path, PathBuf};

/// One entry in the icon grid.
pub struct DirEntry {
  /// File name on disk (with extension for files).
  pub name: String,
  /// True for directories, false for files.
  pub is_dir: bool,
}

/// The listed directory: `~/Downloads/`.
pub fn downloads_dir() -> PathBuf {
  let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
  PathBuf::from(home).join("Downloads")
}

/// Display name: directories keep their name, files lose the extension
/// (`archive.tar.gz` shows as `archive.tar`).
pub fn display_name(name: &str, is_dir: bool) -> String {
  if is_dir {
    name.to_string()
  } else {
    Path::new(name)
      .file_stem()
      .and_then(|stem| stem.to_str())
      .unwrap_or(name)
      .to_string()
  }
}

/// List a directory, directories first then files, each alphabetical
/// (case-insensitive). Returns an empty list when unreadable.
pub fn list_dir(path: &Path) -> Vec<DirEntry> {
  let Ok(read) = std::fs::read_dir(path) else {
    return Vec::new();
  };
  let mut entries: Vec<DirEntry> = read
    .filter_map(|entry| entry.ok())
    .map(|entry| DirEntry {
      name: entry.file_name().to_string_lossy().into_owned(),
      is_dir: entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false),
    })
    .collect();
  entries.sort_by(|a, b| {
    b.is_dir.cmp(&a.is_dir).then_with(|| {
      display_name(&a.name, a.is_dir)
        .to_lowercase()
        .cmp(&display_name(&b.name, b.is_dir).to_lowercase())
    })
  });
  entries
}

/// Live entries of `~/Downloads/`.
pub fn list_downloads() -> Vec<DirEntry> {
  list_dir(&downloads_dir())
}

/// Number of items shown in the status line.
pub fn item_count(entries: &[DirEntry]) -> usize {
  entries.len()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn file_display_name_strips_extension() {
    assert_eq!(display_name("photo.png", false), "photo");
    assert_eq!(display_name("archive.tar.gz", false), "archive.tar");
    assert_eq!(display_name("README", false), "README");
  }

  #[test]
  fn dir_display_name_keeps_name() {
    assert_eq!(display_name("Projects.d", true), "Projects.d");
  }

  #[test]
  fn unreadable_dir_yields_empty_list() {
    let empty = list_dir(Path::new("/nonexistent-finder-test-dir"));
    assert!(empty.is_empty());
  }

  #[test]
  fn listing_sorts_dirs_first_then_alpha() {
    let base = std::env::temp_dir().join(format!("finder-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("docs")).unwrap();
    std::fs::write(base.join("beta.txt"), b"x").unwrap();
    std::fs::write(base.join("Alpha.TXT"), b"x").unwrap();
    std::fs::write(base.join("README"), b"x").unwrap();

    let entries = list_dir(&base);
    let shown: Vec<String> = entries
      .iter()
      .map(|entry| display_name(&entry.name, entry.is_dir))
      .collect();
    assert_eq!(shown, vec!["docs", "Alpha", "beta", "README"]);

    let _ = std::fs::remove_dir_all(&base);
  }
}
