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
  Archive,
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

const ARCHIVE_EXTS: &[&str] = &[
  "zip", "rar", "7z", "tar", "gz", "gzip", "bz2",
];

/// Classify a lowercase extension.
pub fn file_kind(ext: &str) -> FileKind {
  if IMAGE_EXTS.contains(&ext) {
    FileKind::Image
  } else if VIDEO_EXTS.contains(&ext) {
    FileKind::Video
  } else if AUDIO_EXTS.contains(&ext) {
    FileKind::Audio
  } else if ARCHIVE_EXTS.contains(&ext) {
    FileKind::Archive
  } else {
    FileKind::Other
  }
}

/// Document icon file in `Resources/extensionicons/` for an extension,
/// or `None` when no specific icon exists (caller uses `basis.png`).
pub fn document_icon(ext: &str) -> Option<&'static str> {
  Some(match ext {
    "css" | "scss" | "sass" | "less" => "css.png",
    "exe" | "msi" => "exe.png",
    "doc" | "dot" | "odt" | "rtf" => "doc.png",
    "docx" | "docm" | "dotx" => "docx.png",
    "html" | "htm" | "xhtml" | "mhtml" => "html.png",
    "java" | "class" | "jar" => "java.png",
    "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "json" | "jsonc" => "javascript.png",
    "md" | "markdown" | "mdown" | "mkd" => "md.png",
    "pdf" => "pdf.png",
    "ppt" | "pptx" | "pps" | "ppsx" | "odp" => "ppt.png",
    "py" | "pyw" | "pyc" => "python.png",
    "rs" => "rust.png",
    "sh" | "bash" | "zsh" | "fish" | "bat" | "cmd" | "ps1" => "sh.png",
    "txt" | "text" | "log" | "ini" | "cfg" | "conf" | "toml" | "yaml" | "yml" | "xml" => {
      "txt.png"
    }
    "xls" | "xlsx" | "xlsm" | "xlsb" | "csv" | "tsv" | "ods" => "xls.png",
    _ => return None,
  })
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

/// On-disk name for an inline rename. Rejects empty names and path
/// separators. Directories keep the typed text (`.app` bundles keep
/// their suffix); files keep their extension unless the typed text
/// already carries one.
pub fn resolve_new_name(entry: &DirEntry, typed: &str) -> Option<String> {
  let text = typed.trim();
  if text.is_empty() || text.contains('/') || text.contains('\\') || text.contains('\0') {
    return None;
  }
  if entry.is_dir {
    if entry.is_app && !text.to_lowercase().ends_with(".app") {
      return Some(format!("{text}.app"));
    }
    return Some(text.to_string());
  }
  if text.contains('.') || entry.ext.is_empty() {
    return Some(text.to_string());
  }
  Some(format!("{text}.{}", entry.ext))
}

/// List a directory, directories first then files, each alphabetical
/// (case-insensitive). Windows `Zone.Identifier` marker files are
/// skipped (download metadata, not user files; WSL exposes them as
/// `name:Zone.Identifier`). Returns an empty list when unreadable.
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
    .filter(|(_, name)| {
      let lower = name.to_lowercase();
      !lower.ends_with(".zone.identifier") && !lower.contains(":zone.identifier")
    })
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

/// Free variant of a desired on-disk name: returns `desired` when
/// unused, else `stem (2)`, `stem (3)`, ... The counter goes before
/// the file extension (`wallpaper (2).png`), at the very end for
/// directories (`Docs (2)`), and before the `.app` suffix for
/// bundles (`Demo (2).app`). Leading-dot names (`.gitignore`) never
/// split an extension.
pub fn unique_name(base: &Path, desired: &str, is_dir: bool) -> String {
  if !base.join(desired).exists() {
    return desired.to_string();
  }
  let lower = desired.to_lowercase();
  let (stem, ext) = if is_dir && lower.ends_with(".app") {
    let cut = desired.len() - 4;
    (desired[..cut].to_string(), desired[cut..].to_string())
  } else if !is_dir {
    match desired.rfind('.') {
      Some(i) if i > 0 => (desired[..i].to_string(), desired[i..].to_string()),
      _ => (desired.to_string(), String::new()),
    }
  } else {
    (desired.to_string(), String::new())
  };
  let mut counter = 2;
  loop {
    let candidate = if ext.is_empty() {
      format!("{stem} ({counter})")
    } else {
      format!("{stem} ({counter}){ext}")
    };
    if !base.join(&candidate).exists() {
      return candidate;
    }
    counter += 1;
  }
}

/// Size plus mtime of one entry for the list view columns.
pub struct FileMeta {
  /// File size in bytes. Always 0 for directories (the list shows
  /// no size for folders) and for unreadable entries.
  pub len: u64,
  /// Modification time as unix seconds (0 when unknown).
  pub mtime_secs: u64,
}

/// Size plus mtime of one entry with a single metadata call.
/// Missing or unreadable entries yield zeros.
pub fn file_meta(base: &Path, name: &str) -> FileMeta {
  let meta = std::fs::metadata(base.join(name)).ok();
  match meta {
    Some(m) => FileMeta {
      len: if m.is_dir() { 0 } else { m.len() },
      mtime_secs: m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0),
    },
    None => FileMeta { len: 0, mtime_secs: 0 },
  }
}

/// Format decimals and strip trailing zeros (`9.53`, `10`, `1.2`).
fn trim_float(value: f64, decimals: usize) -> String {
  let text = format!("{value:.decimals$}");
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_string()
  } else {
    trimmed.to_string()
  }
}

/// Human file size like the list view shows: `1 KB`, `23 KB`,
/// `1.5 MB`, `9.53 GB`, `10 GB`. Kilobytes round up with a 1 KB
/// minimum; larger units carry trimmed decimals.
pub fn format_size(bytes: u64) -> String {
  const KB: f64 = 1024.0;
  const MB: f64 = 1024.0 * 1024.0;
  const GB: f64 = 1024.0 * 1024.0 * 1024.0;
  let value = bytes as f64;
  if value < MB {
    format!("{} KB", (value / KB).ceil().max(1.0) as u64)
  } else if value < GB {
    format!("{} MB", trim_float(value / MB, 1))
  } else {
    format!("{} GB", trim_float(value / GB, 2))
  }
}

/// Modification date like the list view shows (`12.09.2026 10:05`
/// in German, `09/12/2026 10:05` in English, local timezone).
/// Unknown times yield `--`.
pub fn format_mtime(secs: u64, german: bool) -> String {
  if secs == 0 {
    return "--".to_string();
  }
  use chrono::TimeZone;
  match chrono::Local.timestamp_opt(secs as i64, 0).single() {
    Some(local) if german => local.format("%d.%m.%Y %H:%M").to_string(),
    Some(local) => local.format("%m/%d/%Y %H:%M").to_string(),
    None => "--".to_string(),
  }
}

/// Cheap metadata snapshot of a directory for change detection.
/// Used by the refresh tick: the watcher also fires on plain file
/// opens/closes (notify watches `OPEN`), and every grid rebuild opens
/// files (image decodes, icon cache checks). Rebuilding on those
/// would retrigger itself forever and starve the main thread, so the
/// UI only rebuilds when this snapshot actually differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotEntry {
  pub name: String,
  pub is_dir: bool,
  pub len: u64,
  pub mtime_secs: u64,
}

/// Sorted metadata snapshot (Zone.Identifier markers excluded like in
/// the listing). Missing or unreadable directories yield an empty
/// snapshot.
pub fn snapshot(path: &Path) -> Vec<SnapshotEntry> {
  let Ok(read) = std::fs::read_dir(path) else {
    return Vec::new();
  };
  let mut entries: Vec<SnapshotEntry> = read
    .filter_map(|entry| entry.ok())
    .map(|entry| {
      let name = entry.file_name().to_string_lossy().into_owned();
      (entry, name)
    })
    .filter(|(_, name)| {
      let lower = name.to_lowercase();
      !lower.ends_with(".zone.identifier") && !lower.contains(":zone.identifier")
    })
    .map(|(entry, name)| {
      let meta = entry.metadata().ok();
      SnapshotEntry {
        is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
        len: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        mtime_secs: meta
          .and_then(|m| m.modified().ok())
          .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
          .map(|d| d.as_secs())
          .unwrap_or(0),
        name,
      }
    })
    .collect();
  entries.sort_by(|a, b| a.name.cmp(&b.name));
  entries
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

  fn test_entry(name: &str, is_dir: bool, is_app: bool, ext: &str) -> DirEntry {
    DirEntry {
      name: name.to_string(),
      is_dir,
      is_app,
      ext: ext.to_string(),
    }
  }

  #[test]
  fn resolve_new_name_rejects_bad_input() {
    let file = test_entry("photo.png", false, false, "png");
    assert!(resolve_new_name(&file, "").is_none());
    assert!(resolve_new_name(&file, "   ").is_none());
    assert!(resolve_new_name(&file, "a/b").is_none());
    assert!(resolve_new_name(&file, "a\\b").is_none());
  }

  #[test]
  fn resolve_new_name_keeps_file_extension() {
    let file = test_entry("photo.png", false, false, "png");
    assert_eq!(resolve_new_name(&file, "wallpaper"), Some("wallpaper.png".to_string()));
    assert_eq!(resolve_new_name(&file, "icon.svg"), Some("icon.svg".to_string()));
    let plain = test_entry("README", false, false, "");
    assert_eq!(resolve_new_name(&plain, "NOTES"), Some("NOTES".to_string()));
  }

  #[test]
  fn resolve_new_name_keeps_app_suffix() {
    let bundle = test_entry("Demo.app", true, true, "");
    assert_eq!(resolve_new_name(&bundle, "Other"), Some("Other.app".to_string()));
    assert_eq!(resolve_new_name(&bundle, "Other.app"), Some("Other.app".to_string()));
    let dir = test_entry("Docs", true, false, "");
    assert_eq!(resolve_new_name(&dir, "Files"), Some("Files".to_string()));
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
    assert_eq!(file_kind("zip"), FileKind::Archive);
    assert_eq!(file_kind("rar"), FileKind::Archive);
    assert_eq!(file_kind("7z"), FileKind::Archive);
    assert_eq!(file_kind("tar"), FileKind::Archive);
    assert_eq!(file_kind("gz"), FileKind::Archive);
    assert_eq!(file_kind("gzip"), FileKind::Archive);
    assert_eq!(file_kind("bz2"), FileKind::Archive);
    assert_eq!(file_kind("pdf"), FileKind::Other);
    assert_eq!(file_kind(""), FileKind::Other);
  }

  #[test]
  fn document_icon_maps_families() {
    assert_eq!(document_icon("xlsx"), Some("xls.png"));
    assert_eq!(document_icon("csv"), Some("xls.png"));
    assert_eq!(document_icon("ts"), Some("javascript.png"));
    assert_eq!(document_icon("json"), Some("javascript.png"));
    assert_eq!(document_icon("rs"), Some("rust.png"));
    assert_eq!(document_icon("py"), Some("python.png"));
    assert_eq!(document_icon("md"), Some("md.png"));
    assert_eq!(document_icon("pdf"), Some("pdf.png"));
    assert_eq!(document_icon("docx"), Some("docx.png"));
    assert_eq!(document_icon("pptx"), Some("ppt.png"));
    assert_eq!(document_icon("sh"), Some("sh.png"));
    assert_eq!(document_icon("txt"), Some("txt.png"));
    assert_eq!(document_icon("html"), Some("html.png"));
    assert_eq!(document_icon("css"), Some("css.png"));
    assert_eq!(document_icon("java"), Some("java.png"));
    assert_eq!(document_icon("exe"), Some("exe.png"));
    assert_eq!(document_icon("msi"), Some("exe.png"));
    assert_eq!(document_icon("dll"), None);
  }

  #[test]
  fn listing_skips_zone_identifier_markers() {
    let base = std::env::temp_dir().join(format!("finder-test-zone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("wallpaper.jpg"), b"x").unwrap();
    std::fs::write(base.join("wallpaper.jpg.Zone.Identifier"), b"x").unwrap();
    std::fs::write(base.join("notes.txt:Zone.Identifier"), b"x").unwrap();

    let entries = list_dir(&base);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "wallpaper.jpg");
    assert_eq!(entries[0].ext, "jpg");

    let _ = std::fs::remove_dir_all(&base);
  }

  #[test]
  fn snapshot_tracks_real_changes_only() {
    let base = std::env::temp_dir().join(format!("finder-test-snap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("a.txt"), b"x").unwrap();

    let first = snapshot(&base);
    // Repeat reads (like a grid rebuild) must not change it.
    let _ = std::fs::read(base.join("a.txt")).unwrap();
    assert_eq!(snapshot(&base), first);

    std::fs::write(base.join("b.txt"), b"y").unwrap();
    assert_ne!(snapshot(&base), first);
    std::fs::remove_file(base.join("b.txt")).unwrap();
    assert_eq!(snapshot(&base), first);

    let _ = std::fs::remove_dir_all(&base);
  }

  #[test]
  fn format_size_uses_trimmed_units() {
    assert_eq!(format_size(0), "1 KB");
    assert_eq!(format_size(512), "1 KB");
    assert_eq!(format_size(1024), "1 KB");
    assert_eq!(format_size(23 * 1024), "23 KB");
    assert_eq!(format_size(1536 * 1024), "1.5 MB");
    assert_eq!(format_size(1258291), "1.2 MB");
    assert_eq!(format_size(10 * 1024 * 1024 * 1024), "10 GB");
    assert_eq!(format_size(10233714893), "9.53 GB");
  }

  #[test]
  fn format_mtime_locale_shapes() {
    assert_eq!(format_mtime(0, true), "--");
    assert_eq!(format_mtime(0, false), "--");
    // Fixed shape in both locales (exact wall time is tz-dependent).
    let german = format_mtime(1789026305, true);
    assert_eq!(german.len(), 16);
    assert_eq!(&german[2..3], ".");
    let english = format_mtime(1789026305, false);
    assert_eq!(english.len(), 16);
    assert_eq!(&english[2..3], "/");
  }

  #[test]
  fn file_meta_reports_len_and_dir_blank() {
    let base = std::env::temp_dir().join(format!("finder-test-meta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("sub")).unwrap();
    std::fs::write(base.join("data.bin"), vec![7u8; 3000]).unwrap();

    let file = file_meta(&base, "data.bin");
    assert_eq!(file.len, 3000);
    assert!(file.mtime_secs > 0);
    let dir = file_meta(&base, "sub");
    assert_eq!(dir.len, 0);
    let missing = file_meta(&base, "nope.txt");
    assert_eq!((missing.len, missing.mtime_secs), (0, 0));

    let _ = std::fs::remove_dir_all(&base);
  }

  #[test]
  fn unique_name_numbers_collisions() {
    let base = std::env::temp_dir().join(format!("finder-test-unique-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    // Free names pass through untouched.
    assert_eq!(unique_name(&base, "Docs", true), "Docs");
    assert_eq!(unique_name(&base, "wallpaper.png", false), "wallpaper.png");

    // Directories count up at the end.
    std::fs::create_dir_all(base.join("Docs")).unwrap();
    assert_eq!(unique_name(&base, "Docs", true), "Docs (2)");
    std::fs::create_dir_all(base.join("Docs (2)")).unwrap();
    assert_eq!(unique_name(&base, "Docs", true), "Docs (3)");

    // Files keep their extension.
    std::fs::write(base.join("wallpaper.png"), b"x").unwrap();
    assert_eq!(unique_name(&base, "wallpaper.png", false), "wallpaper (2).png");
    std::fs::write(base.join("wallpaper (2).png"), b"x").unwrap();
    assert_eq!(unique_name(&base, "wallpaper.png", false), "wallpaper (3).png");

    // Bundles keep their suffix.
    std::fs::create_dir_all(base.join("Demo.app")).unwrap();
    assert_eq!(unique_name(&base, "Demo.app", true), "Demo (2).app");

    // Leading-dot names never split an extension.
    std::fs::write(base.join(".gitignore"), b"x").unwrap();
    assert_eq!(unique_name(&base, ".gitignore", false), ".gitignore (2)");

    let _ = std::fs::remove_dir_all(&base);
  }
}
