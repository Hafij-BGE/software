//! Suite updates, through `craft-suite-update` (packaging/craft-suite-update): it compares the
//! installed package with the newest release of the repository this build came from, and
//! downloads and installs that release's .deb.
//!
//! On Windows there's no updater yet: the Home page links to the Releases page instead, from
//! `CRAFT_SUITE_RELEASES_URL` at build time.

use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

const SHARE_DIR: &str = "/usr/share/craft-suite";
const UPDATER: &str = "craft-suite-update";

/// Where the build's VERSION and upstream.txt are: /usr/share/craft-suite from the .deb, the
/// program's own folder on Windows.
fn share_dir() -> std::path::PathBuf {
    if cfg!(windows) {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default()
    } else {
        std::path::PathBuf::from(SHARE_DIR)
    }
}

pub fn installed_version() -> String {
    std::fs::read_to_string(share_dir().join("VERSION"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| format!("{} (development build)", env!("CARGO_PKG_VERSION")))
}

/// The upstream apps this build was made from, one per line (written by build-deb.sh).
pub fn upstream_summary() -> String {
    std::fs::read_to_string(share_dir().join("upstream.txt")).unwrap_or_default()
}

#[derive(Clone, Debug)]
pub enum Status {
    Idle,
    Checking,
    UpToDate(String),
    Available { current: String, latest: String },
    Installing,
    Installed(String),
    Failed(String),
}

impl Status {
    pub fn busy(&self) -> bool {
        matches!(self, Status::Checking | Status::Installing)
    }
}

pub struct Updater {
    pub status: Status,
    pending: Option<Receiver<Status>>,
}

impl Updater {
    pub fn new() -> Updater {
        Updater { status: Status::Idle, pending: None }
    }

    /// Picks up a finished check or install.
    pub fn poll(&mut self) {
        if let Some(status) = self.pending.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.status = status;
            self.pending = None;
        }
    }

    /// Checks (or, when an update is known to be available, installs) on a thread.
    pub fn start(&mut self, ctx: &egui::Context, install: bool) {
        if self.status.busy() || cfg!(windows) {
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.status = if install { Status::Installing } else { Status::Checking };
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let args: &[&str] = if install { &["--install", "--yes"] } else { &["--check"] };
            let status = match Command::new(UPDATER).args(args).stdin(Stdio::null()).output() {
                Err(e) => Status::Failed(format!("could not run {UPDATER}: {e}")),
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let words: Vec<&str> = stdout.lines().last().unwrap_or("").split_whitespace().collect();
                    match (out.status.code(), words.as_slice()) {
                        (Some(0), ["up-to-date", v, ..]) => Status::UpToDate(v.to_string()),
                        (Some(0), ["installed", v, ..]) => Status::Installed(v.to_string()),
                        (Some(10), ["available", current, latest, ..]) => {
                            Status::Available { current: current.to_string(), latest: latest.to_string() }
                        }
                        _ => {
                            let err = String::from_utf8_lossy(&out.stderr);
                            let msg = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the updater failed");
                            Status::Failed(msg.trim().to_string())
                        }
                    }
                }
            };
            let _ = tx.send(status);
            ctx.request_repaint();
        });
    }

    /// The status line and the Check / Install button.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if cfg!(windows) {
            if let Some(url) = option_env!("CRAFT_SUITE_RELEASES_URL")
                && ui.button("Get updates").on_hover_text("Open the Releases page: download and run the newest installer").clicked()
            {
                crate::system::open(url);
            }
            return;
        }
        self.poll();
        ui.horizontal_wrapped(|ui| {
            match &self.status {
                Status::Idle => {}
                Status::Checking => {
                    ui.spinner();
                    ui.label("Checking for updates…");
                }
                Status::Installing => {
                    ui.spinner();
                    ui.label("Installing the update…");
                }
                Status::UpToDate(v) => {
                    ui.label(format!("Up to date ({v})"));
                }
                Status::Available { current, latest } => {
                    ui.label(format!("Update available: {current} → {latest}"));
                }
                Status::Installed(v) => {
                    ui.label(format!("Updated to {v}. Restart Craft Suite to use it."));
                }
                Status::Failed(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Update: {e}"));
                }
            }
            let install = matches!(self.status, Status::Available { .. });
            let label = if install { "Install update" } else { "Check for updates" };
            if ui.add_enabled(!self.status.busy(), egui::Button::new(label)).clicked() {
                self.start(ui.ctx(), install);
            }
        });
    }
}
