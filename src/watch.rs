//! Live folder watching for Finder.
//!
//! A `notify` watcher observes the listed directory and signals the GTK
//! main thread through an `mpsc` channel. The UI drains the channel on
//! a short tick and rebuilds the grid once per burst, so copies with
//! many events cause a single refresh.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};

/// Start watching `path` (non-recursive). Returns the watcher (keep it
/// alive while watching) and a channel that receives `()` per file
/// system event batch. Returns `None` when watching fails.
pub fn watch_dir(path: &Path) -> Option<(RecommendedWatcher, Receiver<()>)> {
  let (tx, rx): (Sender<()>, Receiver<()>) = std::sync::mpsc::channel();
  let mut watcher = RecommendedWatcher::new(
    move |result: Result<Event, notify::Error>| {
      if result.is_ok() {
        let _ = tx.send(());
      }
    },
    notify::Config::default(),
  )
  .ok()?;
  watcher.watch(path, RecursiveMode::NonRecursive).ok()?;
  Some((watcher, rx))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn watcher_signals_new_file() {
    let base = std::env::temp_dir().join(format!("finder-test-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    let (watcher, rx) = watch_dir(&base).expect("watcher starts");
    let _keep = watcher;
    std::fs::write(base.join("fresh.txt"), b"x").unwrap();

    let signaled = rx.recv_timeout(Duration::from_secs(10)).is_ok();
    let _ = std::fs::remove_dir_all(&base);
    assert!(signaled, "watcher must signal a created file");
  }

  #[test]
  fn missing_dir_returns_none() {
    assert!(watch_dir(Path::new("/nonexistent-finder-watch-dir")).is_none());
  }
}
