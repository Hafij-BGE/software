//! One window: a second `craft-suite` (a double-click in the file manager, the desktop menu)
//! hands its files to the window already open, over a Unix socket in the runtime directory.
//!
//! The protocol is a few lines of text: `open <absolute path>`, `new <word|deck|grid|home>`, and
//! `focus` when there's nothing else to do.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use crate::Request;
use crate::kind::Kind;

pub enum Message {
    Open(String),
    New(Kind),
    Focus,
}

fn socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("craft-suite.sock");
    }
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    std::env::temp_dir().join(format!("craft-suite-{uid}.sock"))
}

fn kind_word(k: Kind) -> &'static str {
    match k {
        Kind::Word => "word",
        Kind::Deck => "deck",
        Kind::Grid => "grid",
        Kind::Home | Kind::Settings => "home",
    }
}

/// Sends `request` to a running window; true when one took it.
pub fn forward(request: &Request) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else { return false };
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
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
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
pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}
