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
}
