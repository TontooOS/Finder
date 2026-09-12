//! Launch `.app` bundles through the LaunchPad daemon.
//!
//! The daemon starts the app as a separate process, so Finder never
//! blocks and never hosts app code. The request runs on a throwaway
//! thread (short socket roundtrip, but never on the GTK main
//! thread). Without a daemon (dev checkouts) the error only logs.

use std::path::Path;

/// Launch a `.app` bundle (directory or archive path). Fire and
/// forget: success and failure only log (`[finder][launch]`).
pub fn launch_app(path: &Path) {
  let path = path.to_path_buf();
  std::thread::spawn(move || match launch_app_blocking(&path) {
    Ok(info) => eprintln!("[finder][launch] started {} ({})", info.name, info.state),
    Err(err) => eprintln!("[finder][launch] failed for {}: {err}", path.display()),
  });
}

#[cfg(target_os = "linux")]
fn launch_app_blocking(path: &Path) -> Result<launchpad_lib::types::ServiceInfo, String> {
  let client = launchpad_lib::client::LaunchpadClient::new()?;
  client.start_app(&path.to_string_lossy())
}

#[cfg(not(target_os = "linux"))]
fn launch_app_blocking(_path: &Path) -> Result<launchpad_lib::types::ServiceInfo, String> {
  Err("app launching is only supported on Linux".to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(target_os = "linux")]
  #[test]
  fn launch_without_daemon_fails_cleanly() {
    let client =
      launchpad_lib::client::LaunchpadClient::with_socket("/nonexistent-finder-launchpad.sock");
    assert!(client.start_app("/tmp/Demo.app").is_err());
  }

  #[test]
  fn launch_app_never_panics() {
    // No daemon here: the spawned thread logs the error and exits.
    launch_app(Path::new("/tmp/Demo.app"));
  }
}
