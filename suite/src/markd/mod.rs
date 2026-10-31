//! Markd tabs: the Markd markdown editor (upstream/markd), in the web version its Android app
//! runs (`packages/android`: the muya editor, ribbon, themes, page layout and exports), shown in
//! a web view placed over the tab's area of the window.
//!
//! The page talks to the suite as it talks to the Android app: `window.MarkdHost.postMessage`
//! with `{id, method, args}` calls that `bridge` answers (files, dialogs, PDF), and
//! `window.__markdNative.event(name)` for what the suite tells it (open a file, close). The
//! suite's own messages (`host.js`) report the document's name and unsaved state and pass the
//! suite's keyboard shortcuts back to the window.
//!
//! The web view is a native child window, so it lives outside egui: each frame the tab reports
//! its rectangle, and the view is hidden while it isn't the active tab or while something egui
//! draws (a menu, a prompt) covers it. Linux: WebKitGTK on a thread of its own running GTK, the
//! view a child of the window's X11 window (the suite runs through XWayland on Wayland desktops).
//! Windows: WebView2 on the window's own thread.

mod bridge;
#[cfg(all(unix, not(target_os = "macos")))]
#[path = "gtk_host.rs"]
mod host;
#[cfg(windows)]
#[path = "win_host.rs"]
mod host;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::app::Action;
use crate::kind::Kind;

/// What the page's file and title bar say, and what it asked of the suite; shared between the
/// tab, the web view's thread and the bridge's workers.
pub struct Shared {
    pub id: u64,
    /// The secret the page's messages carry (see host.js).
    token: String,
    state: Mutex<State>,
}

#[derive(Default)]
pub struct State {
    /// The document's name and unsaved changes, as Markd's title bar shows them.
    pub name: String,
    pub dirty: bool,
    /// The file the page has open (`setCurrentDocument`); relative images load from its folder.
    pub path: Option<PathBuf>,
    /// Files the user chose (a dialog, the file the tab opened with): the page may read and write
    /// these whatever they are, and other files only when they look like Markdown (`bridge`).
    pub granted: HashSet<PathBuf>,
    /// The file the tab opens with, handed over once (`getLaunchDocument`).
    pub launch: Option<PathBuf>,
    /// The page answered the suite's `close` event: true when the document may close.
    pub close: Option<bool>,
    /// Suite shortcuts pressed while the page had the keyboard.
    pub keys: Vec<String>,
    /// The page has the keyboard (it was clicked, or the tab was shown).
    pub focused: bool,
    /// The page has loaded and reported its state.
    pub ready: bool,
    /// The document's Markdown and selected text, as the page last reported them (for Ask AI).
    pub markdown: String,
    pub selection: String,
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A rectangle of the window in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// What the web view's thread is asked to do.
pub enum Cmd {
    /// A new view for `tab`, a child of the native window `parent` (an X11 window, an HWND).
    Create { tab: Arc<Shared>, parent: u64, rect: PxRect },
    Bounds(u64, PxRect),
    Visible(u64, bool),
    /// Give the view the keyboard, or give it back to the window.
    Focus(u64),
    Unfocus(u64),
    Eval(u64, String),
    Destroy(u64),
}

/// The tabs' shared states by id, for the page server and the bridge.
fn tabs() -> MutexGuard<'static, HashMap<u64, Arc<Shared>>> {
    static TABS: OnceLock<Mutex<HashMap<u64, Arc<Shared>>>> = OnceLock::new();
    TABS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn tab_by_view_id(view: &str) -> Option<Arc<Shared>> {
    let id: u64 = view.strip_prefix("markd-")?.parse().ok()?;
    tabs().get(&id).cloned()
}

fn view_id(id: u64) -> String {
    format!("markd-{id}")
}

static CTX: OnceLock<egui::Context> = OnceLock::new();

fn repaint() {
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

/// Hands `cmd` to the views' thread.
fn send(cmd: Cmd) {
    host::send(cmd);
}

/// Runs `name` in the page (`window.__markdNative.event`); its answer comes back as an
/// `eventResult` message.
fn event(id: u64, name: &str, payload: serde_json::Value) {
    send(Cmd::Eval(id, format!("window.__craftSuiteEvent({}, {payload})", serde_json::Value::from(name))));
}

// ---- the page's files -----------------------------------------------------------------------

/// Where Markd's web files are: next to the program (Windows, a build folder), in the package's
/// share folder, or the build output of `packaging/build-markd-web.sh` in a source checkout.
pub fn web_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(d) = std::env::var_os("CRAFT_SUITE_MARKD_WEB") {
            candidates.push(PathBuf::from(d));
        }
        if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
            candidates.push(exe_dir.join("markd"));
            candidates.push(exe_dir.join("../share/craft-suite/markd"));
            candidates.push(exe_dir.join("../lib/craft-suite/markd"));
        }
        candidates.push(PathBuf::from("/usr/share/craft-suite/markd"));
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../build/markd/www"));
        candidates.into_iter().find(|d| d.join("index.html").is_file()).and_then(|d| d.canonicalize().ok())
    })
    .as_deref()
}

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("wasm") => "application/wasm",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// Files a document may show from beside it (`/doc/<token>/<path>`): pictures and media only, so
/// a document can't have the page read anything else from the disk.
fn is_media(path: &Path) -> bool {
    let m = mime_of(path);
    m.starts_with("image/") || m.starts_with("video/") || m.starts_with("audio/")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(Ok(b)) = s.get(i + 1..i + 3).map(|h| u8::from_str_radix(h, 16))
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The largest file the page server reads into memory (a video beside the document, say).
const SERVE_LIMIT: u64 = 512 << 20;

/// The page's content security policy: its own scripts only, so script written into a document
/// never runs (`'unsafe-eval'` is for Vega's expressions); inline styles for the diagrams; and
/// pictures, media and data from anywhere, as documents link them.
const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; \
    img-src * data: blob:; media-src * data: blob:; font-src 'self' data:; connect-src * data: blob:; \
    worker-src 'self' blob:; frame-src 'self' about: data: blob:; object-src 'none'; base-uri 'none'; form-action 'none'";

fn read_limited(file: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(file).ok()?;
    if !meta.is_file() || meta.len() > SERVE_LIMIT {
        return None;
    }
    std::fs::read(file).ok()
}

/// One of the page's own files under `root`, and its name. The text files are installed gzipped
/// (packaging/build-markd-web.sh) and unpacked here.
fn web_file(root: &Path, relative: &str) -> Option<(PathBuf, Vec<u8>)> {
    let inside = |p: PathBuf| p.canonicalize().ok().filter(|p| p.starts_with(root));
    if let Some(file) = inside(root.join(relative)) {
        let bytes = read_limited(&file)?;
        return Some((file, bytes));
    }
    let gz = inside(root.join(format!("{relative}.gz")))?;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(&gz).ok()?).take(SERVE_LIMIT).read_to_end(&mut bytes).ok()?;
    Some((gz.with_extension(""), bytes))
}

/// The picture `relative` beside a document in `folder`: a path down from the folder (no `..`,
/// no absolute path, no link out of it) to a picture or media file.
fn doc_file(folder: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || Path::new(relative).components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let folder = folder.canonicalize().ok()?;
    let file = folder.join(relative).canonicalize().ok()?;
    (file.starts_with(&folder) && is_media(&file)).then_some(file)
}

/// Answers the page's requests: its own files, and pictures beside the open document.
fn serve(view: &str, uri: &str) -> (u16, &'static str, Vec<u8>) {
    let not_found = (404, "text/plain", b"not found".to_vec());
    // markd://app/<path> (Linux) or https://markd.app/<path> (Windows).
    let path = uri.split_once("://").map(|(_, rest)| rest).and_then(|rest| rest.split_once('/')).map(|(_, p)| p).unwrap_or("");
    let path = path.split(['?', '#']).next().unwrap_or("");
    let path = percent_decode(path);
    if let Some(rest) = path.strip_prefix("doc/") {
        let Some(tab) = tab_by_view_id(view) else { return not_found };
        let Some(folder) = tab.lock().path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)) else { return not_found };
        // `<cache-buster>/<path>`: the page picks the first part to reload changed pictures.
        let Some((_, relative)) = rest.split_once('/') else { return not_found };
        let Some(file) = doc_file(&folder, relative) else { return (403, "text/plain", b"not a picture beside the document".to_vec()) };
        return match read_limited(&file) {
            Some(bytes) => (200, mime_of(&file), bytes),
            None => not_found,
        };
    }
    let Some(root) = web_dir() else { return not_found };
    let relative = if path.is_empty() { "index.html" } else { path.as_str() };
    match web_file(root, relative) {
        Some((file, bytes)) => (200, mime_of(&file), bytes),
        None => not_found,
    }
}

/// Answers a request of the page's on a thread of its own, so reading and unpacking files doesn't
/// hold up the window (the responder hands the answer back to the web view's thread).
pub(super) fn respond(view: &str, request: &wry::http::Request<Vec<u8>>, responder: wry::RequestAsyncResponder) {
    let (view, uri) = (view.to_string(), request.uri().to_string());
    std::thread::spawn(move || {
        let (status, mime, body) = serve(&view, &uri);
        let mut response = wry::http::Response::builder().status(status).header(wry::http::header::CONTENT_TYPE, mime);
        if mime.starts_with("text/html") {
            response = response.header("Content-Security-Policy", CSP);
        }
        match response.body(body) {
            Ok(r) => responder.respond(r),
            Err(e) => log::warn!("markd page: {e}"),
        }
    });
}

/// The script every page gets before its own: the `MarkdHost` the page's `native.ts` looks for.
fn host_js(tab: &Shared) -> String {
    include_str!("host.js").replace("@TOKEN@", &tab.token)
}

/// A secret no page can guess: the std hasher's random keys, the time and the tab.
fn new_token(id: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    (0..4u64)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(id ^ i);
            h.write_u128(now);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Handles a message from the page: a bridge call, or one of the suite's own.
fn on_message(tab: &Arc<Shared>, text: &str) {
    let Ok(envelope) = serde_json::from_str::<serde_json::Value>(text) else { return };
    if envelope.get("t").and_then(|v| v.as_str()) != Some(tab.token.as_str()) {
        log::warn!("tab {}: a message without the page's secret, ignored", tab.id);
        return;
    }
    let Some(text) = envelope.get("m").and_then(|v| v.as_str()) else { return };
    log::debug!("tab {}: {}", tab.id, text.chars().take(200).collect::<String>());
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(text) else { return };
    if let Some(kind) = msg.get("suite").and_then(|v| v.as_str()) {
        let mut s = tab.lock();
        match kind {
            "state" => {
                s.name = msg.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                s.dirty = msg.get("dirty").and_then(|v| v.as_bool()).unwrap_or(false);
                s.ready = true;
            }
            "key" => {
                if let Some(a) = msg.get("action").and_then(|v| v.as_str()) {
                    s.keys.push(a.to_string());
                }
            }
            "focus" => s.focused = true,
            "eventResult" => match msg.get("name").and_then(|v| v.as_str()) {
                Some("close") => s.close = Some(msg.get("result").and_then(|v| v.as_bool()).unwrap_or(false)),
                Some("snapshot") => {
                    let result = msg.get("result");
                    let field = |k: &str| result.and_then(|r| r.get(k)).and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    s.markdown = field("markdown");
                    s.selection = field("selection");
                }
                Some("edit") => {
                    if let Some(error) = msg.get("result").and_then(|r| r.get("error")).and_then(|v| v.as_str()) {
                        log::warn!("tab {}: Ask AI's edit: {error}", tab.id);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        drop(s);
        repaint();
        return;
    }
    let (Some(call), Some(method)) = (msg.get("id").and_then(|v| v.as_str()), msg.get("method").and_then(|v| v.as_str())) else { return };
    let (call, method) = (call.to_string(), method.to_string());
    let args = msg.get("args").cloned().unwrap_or(serde_json::Value::Null);
    let tab = tab.clone();
    // Dialogs and file input block: each call runs on a thread of its own.
    std::thread::spawn(move || {
        let reply = match bridge::call(&tab, &method, &args) {
            Ok(result) => serde_json::json!({ "id": call, "ok": true, "result": result }),
            Err(error) => serde_json::json!({ "id": call, "ok": false, "error": error }),
        };
        let text = serde_json::Value::from(reply.to_string());
        send(Cmd::Eval(tab.id, format!("window.__craftSuiteReply({text})")));
        repaint();
    });
}

// ---- the tab --------------------------------------------------------------------------------

pub struct MarkTab {
    shared: Arc<Shared>,
    /// The view exists (the first frame the tab showed made it).
    created: bool,
    /// Why there's no view, shown in the tab.
    error: Option<String>,
    /// What the view was last told.
    rect_sent: Option<PxRect>,
    visible_sent: bool,
    /// The tab's area this frame, set by `ui` and used by `sync`.
    area: Option<egui::Rect>,
    /// The suite asked the page whether it may close.
    closing: bool,
}

impl MarkTab {
    pub fn new(ctx: &egui::Context, launch: Option<&str>) -> MarkTab {
        let _ = CTX.set(ctx.clone());
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let shared = Arc::new(Shared {
            id,
            token: new_token(id),
            state: Mutex::new(State {
                name: launch.and_then(|p| Path::new(p).file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled.md".into()),
                path: launch.map(PathBuf::from),
                launch: launch.map(PathBuf::from),
                granted: launch.map(PathBuf::from).into_iter().collect(),
                ..Default::default()
            }),
        });
        tabs().insert(id, shared.clone());
        let error = if web_dir().is_none() {
            Some("Markd's files aren't installed (build them with packaging/build-markd-web.sh).".to_string())
        } else {
            host::available().err()
        };
        MarkTab { shared, created: false, error, rect_sent: None, visible_sent: false, area: None, closing: false }
    }

    pub fn title(&self) -> String {
        self.shared.lock().name.clone()
    }

    pub fn dirty(&self) -> bool {
        self.shared.lock().dirty
    }

    pub fn path(&self) -> Option<String> {
        self.shared.lock().path.as_ref().map(|p| p.to_string_lossy().to_string())
    }

    /// Nothing typed and no file: a file opened next may take its place.
    pub fn untouched(&self) -> bool {
        let s = self.shared.lock();
        s.path.is_none() && !s.dirty
    }

    /// Opens `path` in this tab (the page asks about unsaved changes first).
    pub fn open_here(&mut self, path: &str) {
        let mut s = self.shared.lock();
        s.granted.insert(PathBuf::from(path));
        if !s.ready {
            s.launch = Some(PathBuf::from(path));
            s.path = Some(PathBuf::from(path));
            return;
        }
        drop(s);
        event(self.shared.id, "open", serde_json::json!({ "uri": path }));
    }

    /// Starts closing: true when it can close now; otherwise the page asks about unsaved changes
    /// and `close_confirmed` says when it may.
    pub fn check_close(&mut self) -> bool {
        let mut s = self.shared.lock();
        if !s.dirty || !self.created || !s.ready {
            return true;
        }
        s.close = None;
        drop(s);
        self.closing = true;
        event(self.shared.id, "close", serde_json::Value::Null);
        false
    }

    /// The page saved or discarded the changes it was asked about.
    pub fn close_confirmed(&mut self) -> bool {
        if !self.closing {
            return false;
        }
        match self.shared.lock().close {
            Some(true) => true,
            Some(false) => {
                self.closing = false;
                false
            }
            None => false,
        }
    }

    /// Suite shortcuts pressed in the page, as the suite's actions.
    pub fn take_actions(&mut self, settings: &crate::settings::Settings, active: u64, out: &mut Vec<Action>) {
        let keys = std::mem::take(&mut self.shared.lock().keys);
        for k in keys {
            let action = match k.as_str() {
                "new" if settings.ctrl_n_new_tab => Action::New { kind: Kind::Mark, replace_page: false },
                _ if !settings.shortcuts => continue,
                "next" => Action::Cycle(1),
                "prev" => Action::Cycle(-1),
                "moveNext" => Action::MoveBy(active, 1),
                "movePrev" => Action::MoveBy(active, -1),
                "newDefault" => Action::NewDefault,
                "close" => Action::Close(active),
                "home" => Action::Home,
                _ => continue,
            };
            out.push(action);
        }
    }

    /// Takes the tab's area; the view goes there (see `sync`).
    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let rect = ui.max_rect();
        if let Some(error) = &self.error {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() * 0.3);
                ui.heading("Markd");
                ui.add_space(8.0);
                ui.label(error.as_str());
            });
            return;
        }
        ui.allocate_rect(rect, egui::Sense::hover());
        self.area = Some(rect);
        if !self.created {
            match host::parent_of(frame) {
                Ok(parent) => {
                    let px = to_px(rect, ui.ctx().pixels_per_point());
                    send(Cmd::Create { tab: self.shared.clone(), parent, rect: px });
                    self.created = true;
                    self.rect_sent = Some(px);
                    self.visible_sent = true;
                    send(Cmd::Focus(self.shared.id));
                }
                Err(e) => self.error = Some(e),
            }
        }
        if !self.shared.lock().ready {
            ui.centered_and_justified(|ui| ui.spinner());
        }
    }

    /// Places or hides the view after the frame is drawn: `shown` when its tab is the active one
    /// and nothing egui drew covers it.
    fn sync(&mut self, ctx: &egui::Context, shown: bool) {
        if !self.created {
            return;
        }
        let area = self.area.take();
        let visible = shown && area.is_some_and(|a| !covered(ctx, a));
        if let Some(a) = area {
            let px = to_px(a, ctx.pixels_per_point());
            if self.rect_sent != Some(px) {
                send(Cmd::Bounds(self.shared.id, px));
                self.rect_sent = Some(px);
            }
        }
        if visible != self.visible_sent {
            send(Cmd::Visible(self.shared.id, visible));
            if !visible && self.shared.lock().focused {
                self.unfocus();
            }
            self.visible_sent = visible;
        }
    }

    pub fn focus(&self) {
        if self.created {
            send(Cmd::Focus(self.shared.id));
        }
    }

    pub fn unfocus(&self) {
        let mut s = self.shared.lock();
        if s.focused {
            s.focused = false;
            drop(s);
            send(Cmd::Unfocus(self.shared.id));
        }
    }

    /// Ask AI reads the document and makes the edits it may (`ai::context::allowed`).
    #[cfg(feature = "ai")]
    pub fn command(&mut self, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        match id {
            "document.text" => Ok(serde_json::json!({ "text": self.shared.lock().markdown })),
            "document.selection" => Ok(serde_json::json!({ "text": self.shared.lock().selection })),
            "text.insert" | "text.append" | "edit.replaceAll" | "document.replace" => {
                if !self.created {
                    return Err("Markd isn't open yet".into());
                }
                event(self.shared.id, "edit", serde_json::json!({ "command": id, "params": params }));
                Ok(serde_json::Value::Null)
            }
            _ => Err(format!("Markd doesn't take the command {id}")),
        }
    }

    pub fn has_focus(&self) -> bool {
        self.shared.lock().focused
    }
}

impl Drop for MarkTab {
    fn drop(&mut self) {
        tabs().remove(&self.shared.id);
        if self.created {
            send(Cmd::Destroy(self.shared.id));
        }
    }
}

fn to_px(r: egui::Rect, ppp: f32) -> PxRect {
    let x = (r.min.x * ppp).round() as i32;
    let y = (r.min.y * ppp).round() as i32;
    let w = ((r.max.x * ppp).round() as i32 - x).max(1) as u32;
    let h = ((r.max.y * ppp).round() as i32 - y).max(1) as u32;
    PxRect { x, y, w, h }
}

/// Something egui shows above the panels (a menu, a popup, a prompt) overlaps `area`: the native
/// view would hide it, so the view steps aside meanwhile.
fn covered(ctx: &egui::Context, area: egui::Rect) -> bool {
    let toast = egui::Id::new("craft_suite_toast");
    ctx.memory(|m| {
        m.areas().visible_layer_ids().into_iter().any(|layer| {
            matches!(layer.order, egui::Order::Middle | egui::Order::Foreground | egui::Order::Debug)
                && layer.id != toast
                && m.area_rect(layer.id).is_some_and(|r| r.intersects(area))
        })
    })
}

/// After each frame: places the views of `tabs` (the active one shown, the rest hidden) and
/// runs what needs the window's own thread (Windows).
pub fn sync<'a>(ctx: &egui::Context, frame: &eframe::Frame, active: Option<u64>, tabs: impl Iterator<Item = (u64, &'a mut MarkTab)>, skip: bool) {
    for (id, tab) in tabs {
        tab.sync(ctx, !skip && Some(id) == active);
    }
    host::pump(frame);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_files_stay_beside_the_document() {
        let dir = std::env::temp_dir().join(format!("markd-serve-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("pics")).unwrap();
        std::fs::write(dir.join("pics/a.png"), b"png").unwrap();
        std::fs::write(dir.join("notes.txt"), b"secret").unwrap();
        assert!(doc_file(&dir, "pics/a.png").is_some());
        assert!(doc_file(&dir, "notes.txt").is_none(), "not a picture");
        assert!(doc_file(&dir, "../markd-serve-test/pics/a.png").is_none());
        assert!(doc_file(&dir, "/etc/passwd").is_none());
        assert!(doc_file(&dir, "").is_none());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/usr/share/pixmaps", dir.join("out")).unwrap();
            let outside = std::fs::read_dir("/usr/share/pixmaps").ok().and_then(|mut d| d.find_map(|e| e.ok().map(|e| e.file_name())));
            if let Some(name) = outside {
                assert!(doc_file(&dir, &format!("out/{}", name.to_string_lossy())).is_none(), "through a link");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn web_files_are_unpacked() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("markd-web-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let root = dir.canonicalize().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(b"console.log(1)").unwrap();
        std::fs::write(root.join("assets/app.js.gz"), gz.finish().unwrap()).unwrap();
        std::fs::write(root.join("index.html"), b"<html>").unwrap();
        let (file, bytes) = web_file(&root, "assets/app.js").unwrap();
        assert_eq!(bytes, b"console.log(1)");
        assert_eq!(mime_of(&file), "text/javascript; charset=utf-8");
        assert_eq!(web_file(&root, "index.html").unwrap().1, b"<html>");
        assert!(web_file(&root, "../../etc/passwd").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
