//! One tab's app: a WordCraft, DeckCraft or GridCraft instance, set up the way the upstream app's
//! own `main.rs` sets it up (services, preferences), but living in a tab of the suite's window;
//! or a Markd page (src/markd).
//!
//! The host-side helpers the upstream binaries use (native file dialogs, the clipboard, printing,
//! audio) are compiled straight from `upstream/*/apps/*/src` (see `main.rs`), so they follow
//! upstream updates. What's here mirrors the parts of the upstream `main.rs` files that can't be
//! shared that way; when an upstream `main.rs` changes how it builds its app, check this file.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;

use crate::kind::Kind;
use crate::settings::no_prefs;
use crate::{deck_audio, grid_print, word_animations, word_clipboard, word_file_dialogs, word_print, word_region};

pub enum Doc {
    Word(Box<WordTab>),
    Deck(Box<DeckTab>),
    Grid(Box<GridTab>),
    #[cfg(feature = "markd")]
    Mark(Box<crate::markd::MarkTab>),
}

pub struct WordTab {
    app: wordcraft_ui_egui::WordApp,
    dialogs: word_file_dialogs::Launcher,
}

pub struct DeckTab {
    app: deckcraft_ui_egui::SlideApp,
}

pub struct GridTab {
    app: gridcraft_ui_egui::SheetApp,
    /// `SheetApp::setup_context_for_language` ran for this instance.
    context_ready: bool,
}

/// What closing a tab needs.
pub enum CloseCheck {
    /// Nothing unsaved: close it.
    Now,
    /// The suite asks Save / Don't Save / Cancel.
    Ask,
    /// The app is asking in its own dialog; close when [`Doc::close_confirmed`] says so.
    AppAsks,
    /// Not now (a print job is still being submitted); the message says why.
    Busy(String),
}

impl Doc {
    /// A new, blank document, presentation or workbook.
    pub fn new_blank(kind: Kind, ctx: &egui::Context) -> Option<Doc> {
        let doc = match kind {
            Kind::Word => Doc::Word(Box::new(WordTab::new(ctx))),
            Kind::Deck => {
                let mut tab = DeckTab::new();
                if let Err(e) = tab.app.run("file.new", json!({})) {
                    log::warn!("new presentation: {e}");
                }
                Doc::Deck(Box::new(tab))
            }
            Kind::Grid => Doc::Grid(Box::new(GridTab::new())),
            #[cfg(feature = "markd")]
            Kind::Mark => Doc::Mark(Box::new(crate::markd::MarkTab::new(ctx, None))),
            _ => return None,
        };
        Some(doc)
    }

    /// The app for `path`'s type, with `path` open in it.
    pub fn open(path: &str, ctx: &egui::Context) -> Result<Doc, String> {
        let kind = Kind::for_path(path).ok_or_else(|| format!("no app in the suite opens {path}"))?;
        let mut doc = match kind {
            Kind::Word => Doc::Word(Box::new(WordTab::new(ctx))),
            Kind::Deck => Doc::Deck(Box::new(DeckTab::new())),
            Kind::Grid => Doc::Grid(Box::new(GridTab::new())),
            #[cfg(feature = "markd")]
            Kind::Mark => return Ok(Doc::Mark(Box::new(crate::markd::MarkTab::new(ctx, Some(path))))),
            _ => unreachable_kind()?,
        };
        doc.open_here(path)?;
        Ok(doc)
    }

    /// Opens `path` in this tab's app (replacing its blank document, as the app does).
    pub fn open_here(&mut self, path: &str) -> Result<(), String> {
        match self {
            Doc::Word(t) => t.app.run("file.open", json!({ "path": path })).map(|_| ()),
            Doc::Deck(t) => t.app.open_path(path),
            Doc::Grid(t) => {
                t.app.open_path(path);
                // GridCraft reports a failed open in its own message box, keeping Book1.
                match t.app.session.active().and_then(|d| d.path.clone()) {
                    Some(_) => Ok(()),
                    None => Err(format!("Spreadsheet could not open {path}")),
                }
            }
            #[cfg(feature = "markd")]
            Doc::Mark(t) => {
                t.open_here(path);
                Ok(())
            }
        }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Doc::Word(_) => Kind::Word,
            Doc::Deck(_) => Kind::Deck,
            Doc::Grid(_) => Kind::Grid,
            #[cfg(feature = "markd")]
            Doc::Mark(_) => Kind::Mark,
        }
    }

    /// The document's name, for the tab and the window title.
    pub fn title(&self) -> String {
        match self {
            Doc::Word(t) => t.app.title_stem(),
            Doc::Deck(t) => t.app.session.active().map(|d| d.title()).unwrap_or_else(|| "Slide".into()),
            Doc::Grid(t) => t.app.session.active().map(|d| d.display_title()).unwrap_or_else(|| "Spreadsheet".into()),
            #[cfg(feature = "markd")]
            Doc::Mark(t) => t.title(),
        }
    }

    pub fn dirty(&self) -> bool {
        match self {
            Doc::Word(t) => t.app.session.dirty,
            Doc::Deck(t) => t.app.session.documents().iter().any(|d| d.is_dirty()),
            Doc::Grid(t) => t.app.session.documents().iter().any(|d| d.is_dirty()),
            #[cfg(feature = "markd")]
            Doc::Mark(t) => t.dirty(),
        }
    }

    /// The file the tab shows, if it has been saved or was opened from one.
    pub fn path(&self) -> Option<String> {
        match self {
            Doc::Word(t) => t.app.session.path.as_ref().map(|p| p.to_string_lossy().to_string()),
            Doc::Deck(t) => t.app.session.active().and_then(|d| d.path.clone()),
            Doc::Grid(t) => t.app.session.active().and_then(|d| d.path.clone()),
            #[cfg(feature = "markd")]
            Doc::Mark(t) => t.path(),
        }
    }

    /// A blank document nobody has touched: opening a file of the same type may replace it.
    pub fn untouched(&self) -> bool {
        let single = match self {
            Doc::Word(_) => true,
            Doc::Deck(t) => t.app.session.documents().len() <= 1,
            Doc::Grid(t) => t.app.session.documents().len() <= 1,
            #[cfg(feature = "markd")]
            Doc::Mark(t) => return t.untouched(),
        };
        single && self.path().is_none() && !self.dirty()
    }

    /// Called when the tab becomes the active one, after the suite restored its look.
    pub fn activated(&mut self, ctx: &egui::Context) {
        #[cfg(feature = "markd")]
        if let Doc::Mark(t) = self {
            t.focus();
        }
        if let Doc::Grid(t) = self
            && !t.context_ready
        {
            gridcraft_ui_egui::SheetApp::setup_context_for_language(ctx, t.app.ui.dark, t.app.ui.language);
            t.context_ready = true;
        }
    }

    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        match self {
            Doc::Word(t) => t.app.raw_input_hook(raw),
            Doc::Deck(t) => t.app.raw_input_hook(raw),
            Doc::Grid(t) => t.app.raw_input_hook(raw),
            #[cfg(feature = "markd")]
            Doc::Mark(t) => {
                // A click on the window outside the page takes the keyboard back from it.
                if t.has_focus() && raw.events.iter().any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. })) {
                    t.unfocus();
                }
            }
        }
    }

    pub fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self {
            Doc::Word(t) => {
                t.app.logic(ctx);
                t.dialogs.show(frame);
            }
            Doc::Deck(t) => t.app.logic(ctx),
            Doc::Grid(t) => {
                t.app.logic(ctx);
                t.app.poll_printing(ctx);
            }
            #[cfg(feature = "markd")]
            Doc::Mark(_) => {}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        match self {
            Doc::Word(t) => {
                t.app.ui(ui);
                t.dialogs.show(frame);
            }
            Doc::Deck(t) => t.app.ui(ui),
            Doc::Grid(t) => t.app.ui(ui),
            #[cfg(feature = "markd")]
            Doc::Mark(t) => t.ui(ui, frame),
        }
    }

    /// Runs one of the app's commands on this tab, as its own automation does (Ask AI reads the
    /// document and applies the changes you accept this way).
    #[cfg(feature = "ai")]
    pub fn command(&mut self, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        match self {
            Doc::Word(t) => t.app.execute(id, params),
            Doc::Deck(t) => t.app.run(id, params),
            Doc::Grid(t) => t.app.run(id, params),
            #[cfg(feature = "markd")]
            Doc::Mark(_) => Err(format!("Markd doesn't take the command {id}")),
        }
    }

    /// Starts closing the tab.
    pub fn check_close(&mut self) -> CloseCheck {
        match self {
            Doc::Word(t) => {
                if t.app.printing.busy() {
                    return CloseCheck::Busy("Wait for the print job to finish submitting before closing.".into());
                }
                // WordCraft asks about unsaved changes itself (File › Close), and says when it's
                // done through `quit_requested`.
                if t.app.close_requested() { CloseCheck::Now } else { CloseCheck::AppAsks }
            }
            // Markd asks about unsaved changes in its page; its answer closes the tab.
            #[cfg(feature = "markd")]
            Doc::Mark(t) => {
                if t.check_close() { CloseCheck::Now } else { CloseCheck::AppAsks }
            }
            Doc::Grid(t) if t.app.printing.busy() => {
                CloseCheck::Busy("Wait for the print job to finish submitting before closing.".into())
            }
            _ if self.dirty() => CloseCheck::Ask,
            _ => CloseCheck::Now,
        }
    }

    /// The app wants its tab closed: WordCraft's Save prompt was answered, or File › Close was
    /// chosen with nothing unsaved (in WordCraft on its own, both close the window).
    pub fn close_confirmed(&mut self) -> bool {
        match self {
            Doc::Word(t) => t.app.quit_requested,
            #[cfg(feature = "markd")]
            Doc::Mark(t) => t.close_confirmed(),
            _ => false,
        }
    }

    /// Save for the suite's Save prompt; true when nothing is left unsaved.
    pub fn save_for_close(&mut self) -> Result<bool, String> {
        match self {
            Doc::Word(t) => t.app.run("file.save", json!({})).map(|_| !t.app.session.dirty),
            Doc::Deck(t) => Ok(t.app.save_all()),
            Doc::Grid(t) => {
                let has_path = t.app.session.active().is_some_and(|d| d.path.is_some());
                let result = if has_path {
                    t.app.session.execute("file.save", json!({}))
                } else {
                    let name = format!("{}.xlsx", t.app.session.active().map(|d| d.display_title()).unwrap_or_else(|| "Book1".into()));
                    match rfd::FileDialog::new().set_file_name(&name).add_filter("Excel Workbook (.xlsx)", &["xlsx"]).save_file() {
                        Some(p) => t.app.session.execute("file.saveAs", json!({ "path": p.to_string_lossy() })),
                        None => return Ok(false),
                    }
                };
                result.map_err(|e| e.to_string())?;
                Ok(!self.dirty())
            }
            // Not asked: Markd tabs ask in their page (`check_close`).
            #[cfg(feature = "markd")]
            Doc::Mark(t) => Ok(!t.dirty()),
        }
    }

    /// Don't Save: drop what's unsaved (and DeckCraft's AutoRecover copy of it).
    pub fn discard(&mut self) {
        if let Doc::Deck(t) = self {
            let _ = t.app.session.execute("file.recovery.discard", &json!({}));
            t.app.quit_confirmed = true;
        }
    }

    /// Lets DeckCraft close without its own quit prompt when the window closes.
    pub fn allow_window_close(&mut self) {
        if let Doc::Deck(t) = self {
            t.app.quit_confirmed = true;
        }
    }

    pub fn save_prefs(&self) {
        match self {
            Doc::Word(t) => word_save_prefs(&t.app),
            Doc::Deck(t) => deck_save_prefs(&t.app),
            Doc::Grid(t) => grid_save_prefs(&t.app),
            // The page keeps its own settings.
            #[cfg(feature = "markd")]
            Doc::Mark(_) => {}
        }
    }
}

fn unreachable_kind() -> Result<Doc, String> {
    Err("not an app".into())
}

/// Things every WordCraft tab shares, done once per run (as `main.rs` does before the window).
pub fn init_once() {
    if no_prefs() || std::env::var_os("WORDCRAFT_NO_PREFS").is_some() {
        return;
    }
    // Fonts the user added in WordCraft (#582) live in `fonts` in its settings folder.
    if let Some(dir) = wordcraft_control_key::settings_dir().map(|d| d.join("fonts")) {
        let (loaded, skipped) = wordcraft_fonts::FontDb::global().load_user_fonts(&dir);
        if !loaded.is_empty() || !skipped.is_empty() {
            log::info!("user fonts: {} loaded, {} skipped from {}", loaded.len(), skipped.len(), dir.display());
        }
    }
}

pub fn finish_print_jobs() {
    word_print::finish_pending_jobs();
    grid_print::finish_pending_jobs();
}

// ---- WordCraft (upstream/wordcraft/apps/wordcraft/src/main.rs) -------------------------------

impl WordTab {
    fn new(ctx: &egui::Context) -> WordTab {
        // The system region picks the new document's paper, the units and the decimal separator.
        let region = word_region::system();
        let (file_dialog, dialogs) = word_file_dialogs::hook(ctx);
        let mut session = wordcraft_engine::Session::new(wordcraft_engine::blank_document(&region));
        session.system_region = region;
        let services = wordcraft_ui_egui::Services {
            file_dialog: Some(file_dialog),
            printers: Some(word_print::discover()),
            print_native: Some(word_print::submit()),
            clipboard_picture: Some(Box::new(word_clipboard::picture)),
            animations_enabled: Some(word_animations::hook(ctx)),
            clipboard_text: Some(Box::new(word_clipboard::text)),
            clipboard_html: Some(Box::new(word_clipboard::html)),
            clipboard_rtf: Some(Box::new(word_clipboard::rtf)),
            clipboard_formats: Some(Box::new(word_clipboard::formats)),
            ..Default::default()
        };
        let mut app = wordcraft_ui_egui::WordApp::new(session, services);
        if word_prefs_enabled() {
            if let Some(ui) = word_prefs_path().and_then(|p| std::fs::read(p).ok()).and_then(|b| wordcraft_ui_egui::UiState::from_json(&b)) {
                app.apply_prefs(ui);
            }
            if let Some(p) = wordcraft_control_key::settings_dir().map(|b| b.join(wordcraft_engine::Session::USER_DICT_FILE)) {
                wordcraft_proof::load_user_dictionary(&p);
                app.session.user_dict_path = Some(p);
            }
        }
        // A region chosen in File › Options replaces the system's for the blank document too.
        if app.session.prefs.region.is_some() {
            let doc = app.session.blank_document();
            app.session.set_document(doc);
        }
        app.watch_desktop_theme(ctx);
        // The suite's window keeps the system title bar; the apps don't draw their own.
        app.integrated_titlebar = false;
        // The suite remembers the window, not each tab.
        app.ui.window = None;
        WordTab { app, dialogs }
    }
}

fn word_prefs_enabled() -> bool {
    !no_prefs() && std::env::var_os("WORDCRAFT_NO_PREFS").is_none()
}

fn word_prefs_path() -> Option<std::path::PathBuf> {
    wordcraft_control_key::settings_dir().map(|b| b.join("ui.json"))
}

fn word_save_prefs(app: &wordcraft_ui_egui::WordApp) {
    if !word_prefs_enabled() {
        return;
    }
    let Some(p) = word_prefs_path() else { return };
    // Keep the window geometry WordCraft saved when it last ran on its own.
    let mut prefs = app.prefs();
    if let Some(old) = std::fs::read(&p).ok().and_then(|b| wordcraft_ui_egui::UiState::from_json(&b)) {
        prefs.window = old.window;
    }
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&prefs) {
        let _ = std::fs::write(&p, bytes);
    }
}

// ---- DeckCraft (upstream/deckcraft/apps/deckcraft/src/main.rs) -------------------------------

impl DeckTab {
    fn new() -> DeckTab {
        let mut app = deckcraft_ui_egui::SlideApp::new(deckcraft_engine::Session::new(), deck_services());
        if deck_prefs_enabled() {
            if let Some(p) = deck_dir().map(|d| d.join("ui.json"))
                && let Ok(bytes) = std::fs::read(&p)
                && let Ok(ui) = serde_json::from_slice::<deckcraft_ui_egui::UiState>(&bytes)
            {
                app.ui = ui;
            }
            if let Some(p) = deck_dir().map(|d| d.join("prefs.json"))
                && let Ok(bytes) = std::fs::read(&p)
                && let Ok(prefs) = serde_json::from_slice::<deckcraft_engine::Prefs>(&bytes)
            {
                app.session.prefs = prefs;
            }
        }
        // AutoRecover stays off: DeckCraft keeps one recovery folder per user, and several
        // DeckCraft tabs reading and clearing it would recover (or discard) each other's files.
        app.session.recovery_dir = None;
        app.integrated_titlebar = false;
        DeckTab { app }
    }
}

fn deck_prefs_enabled() -> bool {
    !no_prefs() && std::env::var_os("DECKCRAFT_NO_PREFS").is_none()
}

fn deck_dir() -> Option<std::path::PathBuf> {
    crate::settings::config_dir().map(|c| c.join("deckcraft"))
}

fn deck_save_prefs(app: &deckcraft_ui_egui::SlideApp) {
    if !deck_prefs_enabled() {
        return;
    }
    let Some(dir) = deck_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(bytes) = serde_json::to_vec_pretty(&app.ui) {
        let _ = std::fs::write(dir.join("ui.json"), bytes);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&app.session.prefs) {
        let _ = std::fs::write(dir.join("prefs.json"), bytes);
    }
}

fn deck_services() -> deckcraft_ui_egui::Services {
    deckcraft_ui_egui::Services {
        pick_open: Some(Box::new(|purpose: &str| {
            let d = rfd::FileDialog::new();
            let d = match purpose {
                "picture" => d.add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"]),
                "audio" => d.add_filter(
                    "Audio",
                    &["wav", "mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "aif", "aiff", "aifc", "caf", "wma", "weba", "mka"],
                ),
                "video" => d.add_filter("Video", &["mp4", "m4v", "mov", "webm", "mkv", "wmv"]),
                _ => d
                    .add_filter("Presentations", &["deckcraft", "pptx", "potx", "ppsx"])
                    .add_filter("DeckCraft", &["deckcraft"])
                    .add_filter("PowerPoint", &["pptx", "potx", "ppsx"])
                    .add_filter("Outline", &["txt", "md"]),
            };
            d.pick_file().map(|p| p.to_string_lossy().to_string())
        })),
        pick_save: Some(Box::new(|name: &str| rfd::FileDialog::new().set_file_name(name).save_file().map(|p| p.to_string_lossy().to_string()))),
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
        clipboard_image: Some(Box::new(|| {
            let mut cb = arboard::Clipboard::new().ok()?;
            let img = cb.get_image().ok()?;
            let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())?;
            let mut out = Vec::new();
            rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
            Some(out)
        })),
        // `DECKCRAFT_NO_AUDIO` plays media silently, as in DeckCraft itself.
        audio_out: std::env::var_os("DECKCRAFT_NO_AUDIO")
            .is_none()
            .then(|| Box::new(deck_audio::CpalOut::default()) as Box<dyn deckcraft_media::AudioOut>),
        ..Default::default()
    }
}

// ---- GridCraft (upstream/gridcraft/apps/gridcraft/src/main.rs) -------------------------------

impl GridTab {
    fn new() -> GridTab {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        let mut app = gridcraft_ui_egui::SheetApp::new(session, grid_services());
        app.watch_desktop_theme();
        app.after_engine();
        grid_load_prefs(&mut app);
        GridTab { app, context_ready: false }
    }
}

fn grid_prefs_enabled() -> bool {
    !no_prefs() && std::env::var_os("GRIDCRAFT_NO_PREFS").is_none()
}

fn grid_dir() -> Option<std::path::PathBuf> {
    crate::settings::config_dir().map(|c| c.join("gridcraft"))
}

fn grid_load_prefs(app: &mut gridcraft_ui_egui::SheetApp) {
    if !grid_prefs_enabled() {
        return;
    }
    // First run (or a ui.json without a usable language): follow the desktop's language and
    // light or dark appearance, until they're chosen and saved in ui.json.
    app.ui.language = gridcraft_ui_egui::i18n::Language::system();
    app.ui.system_theme = true;
    let Some(dir) = grid_dir() else { return };
    if let Ok(b) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<gridcraft_ui_egui::UiState>(&b)
    {
        let saved = serde_json::from_slice(&b).ok().and_then(|v| gridcraft_ui_egui::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui;
        app.ui.language = language;
    }
    if let Ok(b) = std::fs::read(dir.join("prefs.json"))
        && let Ok(p) = serde_json::from_slice(&b)
    {
        app.session.prefs = p;
    }
}

fn grid_save_prefs(app: &gridcraft_ui_egui::SheetApp) {
    if !grid_prefs_enabled() {
        return;
    }
    let Some(dir) = grid_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec_pretty(&app.ui) {
        let _ = std::fs::write(dir.join("ui.json"), b);
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.session.prefs) {
        let _ = std::fs::write(dir.join("prefs.json"), b);
    }
}

fn read_clipboard<T>(
    handle: &RefCell<Option<arboard::Clipboard>>,
    read: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    let mut clipboard = handle.try_borrow_mut().ok()?;
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    read(clipboard.as_mut()?).ok()
}

fn grid_services() -> gridcraft_ui_egui::Services {
    // Keep the owner alive: X11/Wayland serve clipboard data from this handle.
    let clipboard: Rc<RefCell<Option<arboard::Clipboard>>> = Rc::default();
    let (html_clipboard, text_clipboard) = (clipboard.clone(), clipboard.clone());
    gridcraft_ui_egui::Services {
        copy_html: Some(Box::new(move |html, text| {
            let mut clipboard = clipboard.borrow_mut();
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
            }
            clipboard.as_mut().ok_or("clipboard unavailable")?.set_html(html, Some(text)).map_err(|e| e.to_string())
        })),
        paste_html: Some(Box::new(move || read_clipboard(&html_clipboard, |clipboard| clipboard.get().html()))),
        paste_text: Some(Box::new(move || read_clipboard(&text_clipboard, |clipboard| clipboard.get_text()))),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Spreadsheets", &["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "json"])
                .add_filter("Excel Workbook", &["xlsx", "xlsm"])
                .add_filter("Excel 97-2003 Workbook (import)", &["xls"])
                .add_filter("Excel Binary Workbook (data import)", &["xlsb"])
                .add_filter("OpenDocument Spreadsheet (data import)", &["ods"])
                .add_filter("CSV", &["csv"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Pictures (PNG, JPEG)", &["png", "jpg", "jpeg"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let path = std::path::Path::new(suggested);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Book1").to_string();
            // The suggested name's type (an export asks for .pdf, .csv or .html) comes first.
            const TYPES: [(&str, &str); 4] =
                [("Excel Workbook (.xlsx)", "xlsx"), ("CSV UTF-8 (.csv)", "csv"), ("Web Page (.html)", "html"), ("PDF (.pdf)", "pdf")];
            let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).filter(|e| TYPES.iter().any(|(_, x)| x == e));
            let ext = ext.unwrap_or_else(|| "xlsx".to_string());
            let mut dialog = rfd::FileDialog::new().set_file_name(format!("{stem}.{ext}"));
            for (label, x) in TYPES.iter().filter(|(_, x)| *x == ext).chain(TYPES.iter().filter(|(_, x)| *x != ext)) {
                dialog = dialog.add_filter(*label, &[*x]);
            }
            dialog.save_file().and_then(|p| p.to_str().map(str::to_string))
        })),
        open_url: Some(Box::new(|url: &str| grid_open_with_system(url))),
        printers: Some(grid_print::discover()),
        print_native: Some(grid_print::submit()),
        ..Default::default()
    }
}

/// Opens a link with the system's default application: http, https and mailto only.
fn grid_open_with_system(target: &str) {
    let gridcraft_engine::links::LinkKind::External(target) = gridcraft_engine::links::classify(target) else {
        log::warn!("refused to open {target:?}: only http, https and mailto links are opened");
        return;
    };
    crate::system::open(target.as_str());
}
