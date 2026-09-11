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
  /// True for `.app` bundles (directory or file): the grid shows the
  /// app icon rendered once through CoreIcon.
  pub is_app: bool,
  /// Lowercase extension without dot (`jpg`, `mp4`, `mp3`), empty for
  /// directories and extensionless files.
  pub ext: String,
}

/// File kind driving the grid preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
  Image,
  Video,
  Audio,
  Other,
}

const IMAGE_EXTS: &[&str] = &[
  "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "tif", "tiff", "avif", "heic", "ico",
];

const VIDEO_EXTS: &[&str] = &[
  "mp4", "mkv", "avi", "mov", "webm", "m4v", "wmv", "flv", "mpg", "mpeg", "3gp", "ogv",
];

const AUDIO_EXTS: &[&str] = &[
  "mp3", "wav", "flac", "ogg", "oga", "m4a", "opus", "aac", "wma",
];

/// Classify a lowercase extension.
pub fn file_kind(ext: &str) -> FileKind {
  if IMAGE_EXTS.contains(&ext) {
    FileKind::Image
  } else if VIDEO_EXTS.contains(&ext) {
    FileKind::Video
  } else if AUDIO_EXTS.contains(&ext) {
    FileKind::Audio
  } else {
    FileKind::Other
  }
}

/// The listed directory: `~/Downloads/`.
pub fn downloads_dir() -> PathBuf {
  let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
  PathBuf::from(home).join("Downloads")
}

/// Display name: directories keep their name, `.app` bundles lose the
/// suffix, files lose the extension (`archive.tar.gz` shows as
/// `archive.tar`).
pub fn display_name(name: &str, is_dir: bool) -> String {
  if is_dir {
    if let Some(stripped) = name.strip_suffix(".app").or_else(|| name.strip_suffix(".APP")) {
      return stripped.to_string();
    }
    return name.to_string();
  }
  Path::new(name)
    .file_stem()
    .and_then(|stem| stem.to_str())
    .unwrap_or(name)
    .to_string()
}

/// List a directory, directories first then files, each alphabetical
/// (case-insensitive). Windows `Zone.Identifier` marker files are
/// skipped (download metadata, not user files). Returns an empty list
/// when unreadable.
pub fn list_dir(path: &Path) -> Vec<DirEntry> {
  let Ok(read) = std::fs::read_dir(path) else {
    return Vec::new();
  };
  let mut entries: Vec<DirEntry> = read
    .filter_map(|entry| entry.ok())
    .map(|entry| {
      let name = entry.file_name().to_string_lossy().into_owned();
      (entry, name)
    })
    .filter(|(_, name)| !name.to_lowercase().ends_with(".zone.identifier"))
    .map(|(entry, name)| {
      let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
      let is_app = name.to_lowercase().ends_with(".app");
      let ext = if is_dir {
        String::new()
      } else {
        Path::new(&name)
          .extension()
          .and_then(|ext| ext.to_str())
          .unwrap_or("")
          .to_lowercase()
      };
      DirEntry { name, is_dir, is_app, ext }
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
  fn app_display_name_strips_suffix() {
    assert_eq!(display_name("Demo.app", true), "Demo");
    assert_eq!(display_name("Demo.app", false), "Demo");
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

  #[test]
  fn file_kind_classifies_extensions() {
    assert_eq!(file_kind("jpg"), FileKind::Image);
    assert_eq!(file_kind("png"), FileKind::Image);
    assert_eq!(file_kind("mp4"), FileKind::Video);
    assert_eq!(file_kind("mkv"), FileKind::Video);
    assert_eq!(file_kind("mp3"), FileKind::Audio);
    assert_eq!(file_kind("flac"), FileKind::Audio);
    assert_eq!(file_kind("pdf"), FileKind::Other);
    assert_eq!(file_kind(""), FileKind::Other);
  }

  #[test]
  fn listing_skips_zone_identifier_markers() {
    let base = std::env::temp_dir().join(format!("finder-test-zone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("wallpaper.jpg"), b"x").unwrap();
    std::fs::write(base.join("wallpaper.jpg.Zone.Identifier"), b"x").unwrap();

    let entries = list_dir(&base);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "wallpaper.jpg");
    assert_eq!(entries[0].ext, "jpg");

    let _ = std::fs::remove_dir_all(&base);
  }
}
