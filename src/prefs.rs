//! Persisted Finder preferences (CoreData).
//!
//! Stores the selected view mode per user. CoreData isolates storage
//! per app (`com.tontoo.finder`) and per user (under
//! `/Users/<user>/Library/Preferences/`), so every user keeps their
//! own grid/list choice across restarts. Every failure falls back to
//! the grid and only logs: Finder never crashes over preferences.

use crate::CoreData::{ManagedObject, PersistentContainer, StoreType};

/// Grid or list view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
  Grid,
  List,
}

impl ViewMode {
  /// Stored string (`grid` / `list`).
  pub fn as_str(self) -> &'static str {
    match self {
      ViewMode::Grid => "grid",
      ViewMode::List => "list",
    }
  }

  /// Parse a stored string. Unknown values fall back to grid.
  pub fn from_str(raw: &str) -> Self {
    match raw {
      "list" => ViewMode::List,
      _ => ViewMode::Grid,
    }
  }
}

const BUNDLE_ID: &str = "com.tontoo.finder";
const ENTITY: &str = "FinderPrefs";
const OBJECT_ID: &str = "view";
const KEY_MODE: &str = "mode";

fn open_container() -> Option<PersistentContainer> {
  match PersistentContainer::new_with_bundle(BUNDLE_ID.to_string(), StoreType::Fico) {
    Ok(container) => Some(container),
    Err(err) => {
      eprintln!("[finder][prefs] coredata unavailable: {err}; using defaults");
      None
    }
  }
}

/// Load the persisted view mode (default: grid).
pub fn load_view_mode() -> ViewMode {
  let Some(mut container) = open_container() else {
    return ViewMode::Grid;
  };
  let mode = container
    .view_context()
    .object(ENTITY, OBJECT_ID)
    .ok()
    .and_then(|obj| obj.get_str(KEY_MODE).map(str::to_string))
    .map(|raw| ViewMode::from_str(&raw))
    .unwrap_or(ViewMode::Grid);
  eprintln!("[finder][prefs] loaded view mode: {}", mode.as_str());
  mode
}

/// Persist the view mode. Failures only log.
pub fn save_view_mode(mode: ViewMode) {
  let Some(mut container) = open_container() else {
    return;
  };
  let mut ctx = container.view_context();
  let mut obj = ManagedObject::with_id(ENTITY, OBJECT_ID);
  obj.set(KEY_MODE, mode.as_str());
  if let Err(err) = ctx.save_object(obj).and_then(|_| ctx.save()) {
    eprintln!("[finder][prefs] save failed: {err}");
    return;
  }
  eprintln!("[finder][prefs] saved view mode: {}", mode.as_str());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn view_mode_roundtrip_with_default() {
    // Isolated prefs root on foreign systems (mirrors the CoreData
    // quickstart: temp preferences dir, foreign access allowed,
    // throwaway key file, explicit bundle id).
    let root = std::env::temp_dir().join(format!("finder-prefs-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("TONTOO_PREFERENCES_ROOT", root.to_string_lossy().to_string());
    std::env::set_var("TONTOO_COREDATA_ALLOW_FOREIGN", "1");
    std::env::set_var(
      "TONTOO_COREDATA_KEY_FILE",
      root.join("keyfile").to_string_lossy().to_string(),
    );
    std::env::set_var("TONTOO_APP_BUNDLE_ID", BUNDLE_ID);

    assert_eq!(load_view_mode(), ViewMode::Grid);
    save_view_mode(ViewMode::List);
    assert_eq!(load_view_mode(), ViewMode::List);
    save_view_mode(ViewMode::Grid);
    assert_eq!(load_view_mode(), ViewMode::Grid);
    assert_eq!(ViewMode::from_str("bogus"), ViewMode::Grid);

    let _ = std::fs::remove_dir_all(&root);
  }
}
