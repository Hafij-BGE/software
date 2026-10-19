//! Craft Suite: Word, Slide, Spreadsheet (ArtCraft's WordCraft, DeckCraft and GridCraft) and Markd
//! in one window, each document in its own tab.
//!
//! Usage: `craft-suite [--new word|slide|sheet|mark|home] [--new-window] [FILE...]`
//!
//! Files open in tabs of the app for their type. With "Open files in the running window" on (the
//! default), a second `craft-suite` hands its files to the window already open and exits.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// No console window behind the suite on Windows (release builds).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

// Ask AI (the `ai` feature; see src/ai/mod.rs).
#[cfg(feature = "ai")]
mod ai;
mod app;
mod hosts;
mod fontbar;
mod instance;
mod kind;
mod look;
// Markd tabs (the `markd` feature; see src/markd/mod.rs).
#[cfg(feature = "markd")]
mod markd;
mod pages;
mod settings;
mod system;
mod tabbar;
mod templates;
mod update;

// The upstream binaries' host helpers, compiled from their source so upstream fixes carry over.
// Each group is an inline module pointing at the app's `src` folder, so the files' own child
// modules (`animations/desktop.rs`) resolve as they do upstream.
#[allow(dead_code)]
#[path = "../../upstream/wordcraft/apps/wordcraft/src"]
mod word_src {
    pub mod animations;
    pub mod clipboard;
    pub mod file_dialogs;
    pub mod print;
    pub mod region;
}
#[allow(dead_code)]
#[path = "../../upstream/deckcraft/apps/deckcraft/src"]
mod deck_src {
    pub mod audio;
}
#[allow(dead_code)]
#[path = "../../upstream/gridcraft/apps/gridcraft/src"]
mod grid_src {
    pub mod print;
}
use deck_src::audio as deck_audio;
use grid_src::print as grid_print;
use word_src::{animations as word_animations, clipboard as word_clipboard, file_dialogs as word_file_dialogs, print as word_print, region as word_region};

use kind::Kind;

/// What the command line asked for.
#[derive(Debug, Default)]
pub struct Request {
    pub files: Vec<String>,
    pub new: Vec<Kind>,
}

/// Who made the suite, and where its apps come from (the About card, --help, the packages).
pub const CREATOR: &str = "Md. Hafijur Rahman";
pub const SOURCE: &str = "ArtCraft";

const USAGE: &str = "usage: craft-suite [--new word|slide|sheet|mark|home] [--new-window] [FILE...]

Opens documents (Word), presentations (Slide), spreadsheets (Spreadsheet) and Markdown files
(Markd) as tabs of one window. Each FILE opens in the app for its type; --new opens a blank
one (repeatable). --new-window starts a separate window even when one is already open.

Craft Suite, created by Md. Hafijur Rahman. Source: WordCraft, DeckCraft and GridCraft by
ArtCraft (https://github.com/storytold), as Word, Slide and Spreadsheet; Markd
(https://github.com/Hafij-BGE/markd), based on MarkText.";

fn main() -> std::process::ExitCode {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(log_level())).ok();
    let mut request = Request::default();
    let mut new_window = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return std::process::ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("craft-suite {}", update::installed_version());
                return std::process::ExitCode::SUCCESS;
            }
            "--new" => match args.next().as_deref().and_then(Kind::from_cli) {
                Some(k) => request.new.push(k),
                None => {
                    eprintln!("craft-suite: --new takes word, slide, sheet, mark or home\n\n{USAGE}");
                    return std::process::ExitCode::from(2);
                }
            },
            "--new-window" => new_window = true,
            // Desktop launchers pass these through when there's nothing to fill them with.
            "%F" | "%f" | "%U" | "%u" => {}
            a if a.starts_with("--") => {
                eprintln!("craft-suite: unknown option {a}\n\n{USAGE}");
                return std::process::ExitCode::from(2);
            }
            a => request.files.push(absolute(a)),
        }
    }
    let settings = settings::Settings::load();
    if settings.single_window && !new_window && instance::forward(&request) {
        return std::process::ExitCode::SUCCESS;
    }
    hosts::init_once();

    let viewport = egui::ViewportBuilder::default()
        .with_title("Craft Suite")
        .with_app_id("craft-suite")
        .with_inner_size([1440.0, 920.0])
        .with_min_inner_size([760.0, 480.0])
        .with_drag_and_drop(true)
        // Tabs in the title bar: the suite draws the window buttons (see `tabbar`).
        .with_decorations(settings.title_bar == settings::TitleBar::System)
        .with_icon(app::icon_data());
    #[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(unused_mut))]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    // As WordCraft and DeckCraft do: winit has no file drag-and-drop on Wayland, so run through
    // XWayland when it's there. CRAFT_SUITE_WAYLAND=1 keeps the native Wayland backend.
    #[cfg(all(unix, not(target_os = "macos")))]
    if std::env::var_os("DISPLAY").is_some() && std::env::var_os("CRAFT_SUITE_WAYLAND").is_none() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }));
    }
    let result = eframe::run_native(
        "Craft Suite",
        options,
        Box::new(move |cc| {
            let incoming = settings.single_window.then(|| instance::serve(cc.egui_ctx.clone())).flatten();
            Ok(Box::new(app::SuiteApp::new(&cc.egui_ctx, settings, request, incoming)))
        }),
    );
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("craft-suite: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn absolute(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    std::env::current_dir().map(|d| d.join(p).to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string())
}

/// Records go to standard error; `RUST_LOG=debug|info|warn|error|off` picks the level (warn).
struct Logger;
static LOGGER: Logger = Logger;

fn log_level() -> log::LevelFilter {
    match std::env::var("RUST_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "off" => log::LevelFilter::Off,
        "error" => log::LevelFilter::Error,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // Never panics on a closed standard error, unlike `eprintln!`.
            let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("[{} {}] {}\n", record.level(), record.target(), record.args()));
        }
    }
    fn flush(&self) {}
}
