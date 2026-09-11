//! Finder data model (basis).
//!
//! Static sidebar and folder content for the first Finder basis step.
//! Folder names are proper nouns and stay untranslated; `icon` names the
//! full-color SVG in `Resources/foldericons/scalable/` rendered in the
//! grid. All chrome text comes from `lang/`. Real filesystem listing
//! replaces `folders()` in a later step.

/// One folder in the icon grid.
pub struct Folder {
  /// Display name (proper noun, untranslated).
  pub name: &'static str,
  /// Icon file in `Resources/foldericons/scalable/`.
  pub icon: &'static str,
}

/// Folders shown in the icon grid (17 items, like the design reference).
pub fn folders() -> Vec<Folder> {
  vec![
    Folder { name: "Applications", icon: "blue-folder.svg" },
    Folder { name: "Applications (Parallels)", icon: "folder-vbox.svg" },
    Folder { name: "Books", icon: "folder-book.svg" },
    Folder { name: "Business", icon: "folder-chart.svg" },
    Folder { name: "Desktop", icon: "blue-user-desktop.svg" },
    Folder { name: "Documents", icon: "blue-folder-documents.svg" },
    Folder { name: "Downloads", icon: "blue-folder-download.svg" },
    Folder { name: "Movies", icon: "blue-folder-videos.svg" },
    Folder { name: "Music", icon: "blue-folder-music.svg" },
    Folder { name: "News", icon: "folder-notes.svg" },
    Folder { name: "Parallels", icon: "folder-vbox.svg" },
    Folder { name: "Pictures", icon: "blue-folder-images.svg" },
    Folder { name: "Projects", icon: "folder-projects.svg" },
    Folder { name: "Public", icon: "blue-folder-public.svg" },
    Folder { name: "Scripts", icon: "folder-script.svg" },
    Folder { name: "Simulations", icon: "folder-calculate.svg" },
    Folder { name: "Software", icon: "folder-appimage.svg" },
  ]
}

/// Number of items shown in the status line.
pub fn item_count() -> usize {
  folders().len()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashSet;

  #[test]
  fn grid_has_seventeen_items() {
    assert_eq!(item_count(), 17);
  }

  #[test]
  fn folder_names_are_unique() {
    let mut seen = HashSet::new();
    for folder in folders() {
      assert!(seen.insert(folder.name), "duplicate folder: {}", folder.name);
    }
  }

  #[test]
  fn folder_icons_end_with_svg() {
    for folder in folders() {
      assert!(
        folder.icon.ends_with(".svg"),
        "grid icon must be an SVG: {}",
        folder.icon
      );
    }
  }
}
