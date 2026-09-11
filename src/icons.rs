//! Icon lookup for Finder.
//!
//! Resolves files under `Resources/foldericons/` (SVG icon theme with
//! `scalable/`, `16/`, `22/`, `24/`, `colors/` and `symbolic/`). The grid
//! uses the full-color `scalable/` SVGs through GTK (librsvg); the
//! sidebar keeps CoreIcon SF Symbols because `SidebarIcon::file` relies
//! on the `image` crate, which cannot decode SVG.

use std::path::{Path, PathBuf};

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

/// Resolve `Resources/<parts...>` to an existing path across dev
/// checkouts, `.app` bundles and installed files.
pub fn resources_file(parts: &[&str]) -> Option<PathBuf> {
  for dir in resources_dirs() {
    let mut path = dir.clone();
    for part in parts {
      path = path.join(part);
    }
    if path.is_file() {
      return Some(path);
    }
  }
  None
}

/// Themed icon for archive files (`Resources/extensionicons/zip.png`).
pub fn archive_icon() -> Option<PathBuf> {
  resources_file(&["extensionicons", "zip.png"])
}

/// Icon files inside a `.app` bundle, in lookup order (TBuild bundle
/// layout: the `tontoo.proj` icon lands in both `App/` and
/// `Resources/`).
const APP_ICON_CANDIDATES: &[&str] = &[
  "Resources/icon.png",
  "App/icon.png",
  "Resources/app_icon.png",
];

/// Raw icon file of a `.app` bundle directory: the first existing
/// candidate, else the `icon` field of `tontoo.proj` (relative to the
/// bundle dir).
fn bundle_icon_file(bundle: &Path) -> Option<PathBuf> {
  for candidate in APP_ICON_CANDIDATES {
    let path = bundle.join(candidate);
    if path.is_file() {
      return Some(path);
    }
  }
  let proj = std::fs::read_to_string(bundle.join("tontoo.proj")).ok()?;
  let value: serde_json::Value = serde_json::from_str(&proj).ok()?;
  let icon = value.get("icon")?.as_str()?;
  let path = bundle.join(icon);
  path.is_file().then_some(path)
}

/// Raw icon file of a `.app` ZIP archive: the first candidate entry,
/// extracted to a temp file keyed by the archive size plus mtime.
fn archive_icon_file(archive: &Path) -> Option<PathBuf> {
  let meta = std::fs::metadata(archive).ok()?;
  let mtime = meta
    .modified()
    .ok()?
    .duration_since(std::time::UNIX_EPOCH)
    .ok()?
    .as_secs();
  let stem = archive
    .file_stem()?
    .to_str()?
    .replace(['.', ' ', '-'], "_");
  let raw = std::env::temp_dir().join(format!("finder-approw_{}_{}_{mtime}.png", stem, meta.len()));
  if raw.is_file() {
    return Some(raw);
  }
  let file = std::fs::File::open(archive).ok()?;
  let mut zip = zip::ZipArchive::new(file).ok()?;
  for candidate in APP_ICON_CANDIDATES {
    if let Ok(mut entry) = zip.by_name(candidate) {
      let mut out = std::fs::File::create(&raw).ok()?;
      std::io::copy(&mut entry, &mut out).ok()?;
      return Some(raw);
    }
  }
  None
}

/// App icon for a `.app` entry (bundle directory or ZIP archive),
/// rendered once through CoreIcon (`AppIcon`, original colors) and
/// cached in the temp dir keyed by size plus mtime. The returned file
/// is 3x the 64px grid size. Returns `None` when no icon is found or
/// rendering fails (caller shows the default folder artwork).
pub fn app_icon(entry: &Path) -> Option<PathBuf> {
  let meta = std::fs::metadata(entry).ok()?;
  let mtime = meta
    .modified()
    .ok()?
    .duration_since(std::time::UNIX_EPOCH)
    .ok()?
    .as_secs();
  let stem = entry
    .file_stem()?
    .to_str()?
    .replace(['.', ' ', '-'], "_");
  let cached = std::env::temp_dir().join(format!("finder-appicon_{}_{}_{mtime}.png", stem, meta.len()));
  if cached.is_file() {
    return Some(cached);
  }
  let raw = if entry.is_dir() {
    bundle_icon_file(entry)
  } else {
    archive_icon_file(entry)
  }?;
  crate::CoreIcon::generator::AppIcon::from_file(&raw).save(&cached).ok()?;
  // Downscale the 1024px master once (Lanczos3 stays sharp on 1x and
  // covers HiDPI 2-3x at the 64px grid size).
  let master = image::open(&cached).ok()?;
  let small = image::imageops::resize(&master, 192, 192, image::imageops::FilterType::Lanczos3);
  small.save(&cached).ok()?;
  Some(cached)
}

/// Themed icon for audio files (`scalable/blue-folder-music.svg`).
pub fn audio_icon() -> Option<PathBuf> {
  folder_icon("blue-folder-music.svg")
}

/// Themed fallback icon for video files without a cached thumbnail
/// (`scalable/blue-folder-videos.svg`).
pub fn video_icon() -> Option<PathBuf> {
  folder_icon("blue-folder-videos.svg")
}

/// Directory caching extracted video frames (one PNG per source file).
pub fn thumb_dir() -> PathBuf {
  std::env::temp_dir().join("finder-thumbs")
}

/// Cache path for a source file. The key carries size plus mtime, so
/// edited videos regenerate instead of serving a stale frame.
pub fn thumb_key(source: &Path) -> Option<PathBuf> {
  let meta = std::fs::metadata(source).ok()?;
  let mtime = meta
    .modified()
    .ok()?
    .duration_since(std::time::UNIX_EPOCH)
    .ok()?
    .as_secs();
  let stem = source
    .file_stem()?
    .to_str()?
    .replace(['.', ' ', '-'], "_");
  Some(thumb_dir().join(format!("thumb_{}_{}_{mtime}.png", stem, meta.len())))
}

/// True when `ffmpeg` is on PATH and can extract video frames.
pub fn ffmpeg_available() -> bool {
  std::process::Command::new("ffmpeg")
    .arg("-version")
    .output()
    .map(|out| out.status.success())
    .unwrap_or(false)
}

/// First-second frame of a video as a cached PNG. Returns a fresh
/// extraction or the cached file. Returns `None` when `ffmpeg` is
/// missing or extraction fails (caller shows the themed video icon).
pub fn video_thumb(source: &Path) -> Option<PathBuf> {
  let cached = thumb_key(source)?;
  if cached.is_file() {
    return Some(cached);
  }
  if !ffmpeg_available() {
    return None;
  }
  if let Some(parent) = cached.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  let status = std::process::Command::new("ffmpeg")
    .args([
      "-y",
      "-v",
      "error",
      "-ss",
      "1",
      "-i",
      &source.to_string_lossy(),
      "-vframes",
      "1",
      "-vf",
      "scale=320:-1",
    ])
    .arg(&cached)
    .status()
    .ok()?;
  if status.success() && cached.is_file() {
    Some(cached)
  } else {
    let _ = std::fs::remove_file(&cached);
    None
  }
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
      assert!(audio_icon().is_some());
      assert!(video_icon().is_some());
      assert!(archive_icon().is_some());
    }
  }

  #[test]
  fn thumb_key_is_stable_per_file() {
    let base = std::env::temp_dir().join(format!("finder-test-thumb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let file = base.join("clip.mp4");
    std::fs::write(&file, b"x").unwrap();
    let first = thumb_key(&file);
    let second = thumb_key(&file);
    assert!(first.is_some());
    assert_eq!(first, second);
    let _ = std::fs::remove_dir_all(&base);
  }

  #[test]
  fn video_thumb_without_ffmpeg_returns_none() {
    if ffmpeg_available() {
      return;
    }
    let base = std::env::temp_dir().join(format!("finder-test-noff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let file = base.join("clip.mp4");
    std::fs::write(&file, b"x").unwrap();
    assert!(video_thumb(&file).is_none());
    let _ = std::fs::remove_dir_all(&base);
  }

  fn write_test_png(path: &Path) {
    let img = image::RgbImage::new(16, 16);
    image::DynamicImage::ImageRgb8(img).save(path).unwrap();
  }

  #[test]
  fn app_icon_from_bundle_dir_renders_once() {
    let base = std::env::temp_dir().join(format!("finder-test-appdir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let resources = base.join("Demo.app/Resources");
    std::fs::create_dir_all(&resources).unwrap();
    write_test_png(&resources.join("icon.png"));

    let first = app_icon(&base.join("Demo.app"));
    let second = app_icon(&base.join("Demo.app"));
    assert!(first.is_some());
    assert_eq!(first, second);
    let rendered = first.unwrap();
    assert!(rendered.is_file());
    let img = image::open(&rendered).unwrap();
    assert_eq!((img.width(), img.height()), (192, 192));

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_file(&rendered);
  }

  #[test]
  fn app_icon_from_bundle_zip_renders_once() {
    let base = std::env::temp_dir().join(format!("finder-test-appzip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let raw = base.join("raw-icon.png");
    write_test_png(&raw);

    let archive = base.join("Demo.app");
    {
      let file = std::fs::File::create(&archive).unwrap();
      let mut zip = zip::ZipWriter::new(file);
      zip
        .start_file("Resources/icon.png", zip::write::SimpleFileOptions::default())
        .unwrap();
      let mut src = std::fs::File::open(&raw).unwrap();
      std::io::copy(&mut src, &mut zip).unwrap();
      zip.finish().unwrap();
    }

    let rendered = app_icon(&archive);
    assert!(rendered.is_some());
    assert!(rendered.unwrap().is_file());

    let _ = std::fs::remove_dir_all(&base);
  }

  #[test]
  fn app_icon_without_icon_returns_none() {
    let base = std::env::temp_dir().join(format!("finder-test-appno-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let bundle = base.join("Empty.app");
    std::fs::create_dir_all(&bundle).unwrap();
    assert!(app_icon(&bundle).is_none());
    let _ = std::fs::remove_dir_all(&base);
  }
}
