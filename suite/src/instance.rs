//! One window: a second `craft-suite` (a double-click in the file manager, the desktop menu)
//! hands its files to the window already open, over a Unix socket in the runtime directory.
//!
//! The protocol is a few lines of text: `open <absolute path>`, `new <word|deck|grid|mark|home>`, and
//! `focus` when there's nothing else to do.
//!
//! On Windows there's no such socket yet: each start opens a window of its own.

#[cfg(unix)]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
#[cfg(unix)]
use std::sync::mpsc::channel;

use crate::Request;
use crate::kind::Kind;

#[cfg_attr(not(unix), allow(dead_code))]
pub enum Message {
    Open(String),
    New(Kind),
    Focus,
}

#[cfg(unix)]
fn socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("craft-suite.sock");
    }
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    std::env::temp_dir().join(format!("craft-suite-{uid}.sock"))
}

#[cfg(unix)]
fn kind_word(k: Kind) -> &'static str {
    match k {
        Kind::Word => "word",
        Kind::Deck => "deck",
        Kind::Grid => "grid",
        Kind::Mark => "mark",
        Kind::Home | Kind::Settings => "home",
    }
}

/// Sends `request` to a running window; true when one took it.
#[cfg(not(unix))]
pub fn forward(_request: &Request) -> bool {
    false
}

#[cfg(not(unix))]
pub fn serve(_ctx: egui::Context) -> Option<Receiver<Message>> {
    None
}

#[cfg(not(unix))]
pub fn cleanup() {}

/// Sends `request` to a running window; true when one took it.
#[cfg(unix)]
pub fn forward(request: &Request) -> bool {
    let path = socket_path();
    // Only to a window of this user's (the shared temporary folder could hold anyone's socket).
    use std::os::unix::fs::MetadataExt;
    let me = std::fs::metadata("/proc/self").map(|m| m.uid()).ok();
    if std::fs::symlink_metadata(&path).map(|m| m.uid()).ok() != me {
        return false;
    }
    let Ok(mut stream) = UnixStream::connect(&path) else { return false };
    let mut text = String::new();
    for f in &request.files {
        text.push_str(&format!("open {f}\n"));
    }
    for k in &request.new {
        text.push_str(&format!("new {}\n", kind_word(*k)));
    }
    text.push_str("focus\n");
    stream.write_all(text.as_bytes()).is_ok()
}

/// Listens for other `craft-suite` processes; None when another window already listens (or the
/// socket can't be made), in which case this window runs on its own.
#[cfg(unix)]
pub fn serve(ctx: egui::Context) -> Option<Receiver<Message>> {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return None;
    }
    // Left by a window that didn't exit cleanly.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("{}: {e}; files opened elsewhere will start another window", path.display());
            return None;
        }
    };
    // Only this user may connect.
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            // A client that never finishes can't hold the listener up, nor send without end.
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            for line in BufReader::new(stream.take(1 << 20)).lines().map_while(Result::ok) {
                let message = match line.split_once(' ') {
                    Some(("open", p)) => Message::Open(p.to_string()),
                    Some(("new", k)) => Kind::from_cli(k).map(Message::New).unwrap_or(Message::Focus),
                    _ => Message::Focus,
                };
                if tx.send(message).is_err() {
                    return;
                }
            }
            ctx.request_repaint();
        }
    });
    Some(rx)
}

/// Removes the socket when the window closes.
#[cfg(unix)]
pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}
