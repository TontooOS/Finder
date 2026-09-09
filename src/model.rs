//! Finder data model (basis).
//!
//! Static sidebar and folder content for the first Finder basis step.
//! Folder names are proper nouns and stay untranslated; all chrome text
//! comes from `lang/`. Real filesystem listing replaces `folders()` in a
//! later step.

/// Folders shown in the icon grid (17 items, like the design reference).
pub fn folders() -> Vec<&'static str> {
  vec![
    "Applications",
    "Applications (Parallels)",
    "Books",
    "Business",
    "Desktop",
    "Documents",
    "Downloads",
    "Movies",
    "Music",
    "News",
    "Parallels",
    "Pictures",
    "Projects",
    "Public",
    "Scripts",
    "Simulations",
    "Software",
  ]
}

/// Number of items shown in the status line.
pub fn item_count() -> usize {
  folders().len()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn grid_has_seventeen_items() {
    assert_eq!(item_count(), 17);
  }

  #[test]
  fn folder_names_are_unique() {
    let all = folders();
    let mut seen = std::collections::HashSet::new();
    for name in all {
      assert!(seen.insert(name), "duplicate folder: {name}");
    }
  }
}
