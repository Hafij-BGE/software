//! Handing things to the desktop: opening a folder, a web link or a file with the system's own
//! application.

use std::ffi::OsStr;

/// Opens `target` (a folder, a file or an http/https/mailto link) with the system's default
/// application.
pub fn open(target: impl AsRef<OsStr>) {
    let target = target.as_ref();
    let program = if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(program).arg(target).spawn() {
        log::warn!("{program} {}: {e}", target.to_string_lossy());
    }
}
