//! Linux: the Markd views run WebKitGTK on a thread of their own, which runs GTK's main loop.
//! Each view is an X11 child window of the suite's window (wry's `build_as_child`); `send` hands
//! the thread its work as idle callbacks on GLib's main context, which GTK's loop runs.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::os::raw::c_ulong;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock, mpsc};

use gtk::glib;
use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use webkit2gtk::{PrintOperationExt, WebViewExt};
use wry::WebViewExtUnix;
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};

use super::{Cmd, PxRect, Shared};

/// GTK started (on its thread), or why it couldn't.
fn started() -> &'static Result<(), String> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED.get_or_init(|| {
        if std::env::var_os("DISPLAY").is_none() {
            return Err("Markd tabs need an X11 display (XWayland on a Wayland desktop).".into());
        }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("markd-gtk".into())
            .spawn(move || {
                // The views are children of the suite's X11 window, so GTK must use X11 too.
                gtk::gdk::set_allowed_backends("x11");
                match gtk::init() {
                    Ok(()) => {
                        let _ = tx.send(Ok(()));
                        gtk::main();
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("Markd tabs couldn't start GTK: {e}")));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        rx.recv().unwrap_or_else(|_| Err("Markd tabs couldn't start GTK".into()))
    })
}

pub fn available() -> Result<(), String> {
    started().clone()
}

pub fn send(cmd: Cmd) {
    if started().is_ok() {
        // Always queued for GTK's thread, never run here (as `MainContext::invoke` may be while
        // GTK's loop isn't running yet).
        glib::idle_add_once(move || apply(cmd));
    }
}

/// The suite's X11 window, which the views go in.
pub fn parent_of(frame: &eframe::Frame) -> Result<u64, String> {
    match frame.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(h)) => Ok(h.window as u64),
        Ok(RawWindowHandle::Xcb(h)) => Ok(h.window.get() as u64),
        Ok(_) => Err("Markd tabs need the X11 window: start Craft Suite without CRAFT_SUITE_WAYLAND.".into()),
        Err(e) => Err(format!("Markd tabs: no window ({e})")),
    }
}

/// WebKitGTK writes the PDF itself (`print_pdf`), with no print dialog.
pub const PDF_DIRECT: bool = true;

/// Nothing to do on the window's thread here: GTK has its own.
pub fn pump(_frame: &eframe::Frame) {}

struct Parent(u64);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Xlib(XlibWindowHandle::new(self.0 as c_ulong));
        // SAFETY: the suite's window outlives its tabs' views (they're destroyed with the tabs,
        // and the window is the last to go).
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

struct View {
    webview: wry::WebView,
    /// The suite's window, which gets the keyboard back.
    parent: u64,
}

thread_local! {
    static VIEWS: RefCell<HashMap<u64, View>> = RefCell::new(HashMap::new());
    static CONTEXT: RefCell<Option<wry::WebContext>> = const { RefCell::new(None) };
    /// The `markd:` scheme is registered once per web context, by the first view.
    static PROTOCOL: Cell<bool> = const { Cell::new(false) };
}

/// Where the pages keep their storage (Markd's settings, recent files): one place for all tabs.
fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("craft-suite/markd"))
}

fn bounds(r: PxRect) -> wry::Rect {
    wry::Rect { position: wry::dpi::PhysicalPosition::new(r.x, r.y).into(), size: wry::dpi::PhysicalSize::new(r.w, r.h).into() }
}

fn apply(cmd: Cmd) {
    match cmd {
        Cmd::Create { tab, parent, rect } => {
            if let Err(e) = create(tab, parent, rect) {
                log::warn!("markd view: {e}");
            }
        }
        Cmd::Bounds(id, r) => with_view(id, |v| {
            let _ = v.webview.set_bounds(bounds(r));
        }),
        Cmd::Visible(id, visible) => with_view(id, |v| {
            let _ = v.webview.set_visible(visible);
        }),
        Cmd::Focus(id) => with_view(id, |v| {
            if let Some(xid) = container_of(&v.webview) {
                x_focus_page(xid);
            }
            let _ = v.webview.focus();
        }),
        Cmd::Unfocus(id) => with_view(id, |v| x_focus(v.parent)),
        Cmd::Eval(id, js) => with_view(id, |v| {
            let _ = v.webview.evaluate_script(&js);
        }),
        Cmd::Destroy(id) => {
            VIEWS.with(|views| views.borrow_mut().remove(&id));
        }
    }
}

fn with_view(id: u64, f: impl FnOnce(&View)) {
    VIEWS.with(|views| {
        if let Some(v) = views.borrow().get(&id) {
            f(v);
        }
    });
}

fn create(tab: Arc<Shared>, parent: u64, rect: PxRect) -> Result<(), String> {
    let id = tab.id;
    let view_id = super::view_id(id);
    let webview = CONTEXT.with(|context| -> Result<wry::WebView, String> {
        let mut context = context.borrow_mut();
        let context = context.get_or_insert_with(|| wry::WebContext::new(data_dir()));
        let messages = tab.clone();
        let mut builder = wry::WebViewBuilder::new_with_web_context(context)
            .with_id(&view_id)
            .with_initialization_script_for_main_only(super::HOST_JS, true)
            .with_ipc_handler(move |req| super::on_message(&messages, req.body()))
            .with_navigation_handler(allow_navigation)
            .with_new_window_req_handler(|url, _| {
                allow_navigation(url);
                wry::NewWindowResponse::Deny
            })
            .with_devtools(cfg!(debug_assertions))
            .with_bounds(bounds(rect))
            .with_url("markd://app/index.html");
        if !PROTOCOL.get() {
            builder = builder.with_asynchronous_custom_protocol("markd".into(), |view, request, responder| {
                let (status, mime, body) = super::serve(view, &request.uri().to_string());
                match wry::http::Response::builder().status(status).header(wry::http::header::CONTENT_TYPE, mime).body(body) {
                    Ok(r) => responder.respond(r),
                    Err(e) => log::warn!("markd page: {e}"),
                }
            });
            PROTOCOL.set(true);
        }
        builder.build_as_child(&Parent(parent)).map_err(|e| e.to_string())
    })?;
    // A click in the page gives it the keyboard: an X11 child window gets key presses only
    // while it has the input focus.
    let inner = webview.webview();

    let shared = tab.clone();
    inner.connect_button_press_event(move |w, _| {
        if let Some(xid) = w.toplevel().and_then(|t| t.window()).and_then(|g| g.downcast::<gdkx11::X11Window>().ok()).map(|x| x.xid()) {
            x_focus_page(xid as u64);
        }
        shared.lock().focused = true;
        glib::Propagation::Proceed
    });
    VIEWS.with(|views| views.borrow_mut().insert(id, View { webview, parent }));
    Ok(())
}

/// The X11 window wry made for the view inside the suite's window.
fn container_of(webview: &wry::WebView) -> Option<u64> {
    let window = webview.webview().toplevel()?.window()?;
    Some(window.downcast::<gdkx11::X11Window>().ok()?.xid() as u64)
}

/// The page's own files stay in the view; web and mail links open in the browser.
fn allow_navigation(url: String) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("markd://") || lower.starts_with("about:") || lower.starts_with("data:") || lower.starts_with("blob:") {
        return true;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        crate::system::open(&url);
    }
    false
}

/// Runs `f` with Xlib and GDK's own connection to the X server, ignoring X errors (a window that
/// is gone, or not mapped yet).
fn with_x(f: impl FnOnce(&x11_dl::xlib::Xlib, *mut x11_dl::xlib::Display)) {
    static XLIB: OnceLock<Option<x11_dl::xlib::Xlib>> = OnceLock::new();
    let Some(xlib) = XLIB.get_or_init(|| x11_dl::xlib::Xlib::open().ok()) else { return };
    let Some(display) = gtk::gdk::Display::default() else { return };
    let Ok(x11) = display.clone().downcast::<gdkx11::X11Display>() else { return };
    gtk::gdk::error_trap_push();
    // SAFETY: GDK's display, used on GDK's thread; it lives as long as GTK runs.
    let xdisplay = unsafe { gdkx11::ffi::gdk_x11_display_get_xdisplay(x11.to_glib_none().0) } as *mut x11_dl::xlib::Display;
    f(xlib, xdisplay);
    display.flush();
    gtk::gdk::error_trap_pop_ignored();
}

/// Gives the keyboard to the X11 window `xid`.
fn x_focus(xid: u64) {
    // SAFETY: plain Xlib calls on a live display.
    with_x(|xlib, d| unsafe {
        (xlib.XSetInputFocus)(d, xid as c_ulong, x11_dl::xlib::RevertToParent, x11_dl::xlib::CurrentTime);
    });
}

/// Gives the keyboard to the page: the window GTK made inside wry's container (`xid`), which is
/// the one GDK listens to for keys. X reports key presses to the focus window itself while the
/// pointer is elsewhere in the suite's window, so it must be that one, not the container.
fn x_focus_page(xid: u64) {
    let mut target = xid as c_ulong;
    // SAFETY: plain Xlib calls on a live display; XQueryTree's list is freed with XFree.
    with_x(|xlib, d| unsafe {
        for _ in 0..4 {
            let (mut root, mut parent) = (0, 0);
            let mut children: *mut c_ulong = std::ptr::null_mut();
            let mut n: std::os::raw::c_uint = 0;
            if (xlib.XQueryTree)(d, target, &mut root, &mut parent, &mut children, &mut n) == 0 {
                break;
            }
            let first = if n > 0 && !children.is_null() { Some(*children) } else { None };
            if !children.is_null() {
                (xlib.XFree)(children.cast());
            }
            match first {
                Some(child) => target = child,
                None => break,
            }
        }
        (xlib.XSetInputFocus)(d, target, x11_dl::xlib::RevertToParent, x11_dl::xlib::CurrentTime);
    });
}

/// Prints `html` to a PDF file at `path` on `page` paper (A4, Letter or Legal), without a dialog:
/// Markd's "Export PDF".
pub fn print_pdf(html: String, page: &str, path: &Path) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let uri = glib::filename_to_uri(path, None).map_err(|e| format!("IO: {e}"))?.to_string();
    let paper = match page {
        "Letter" => gtk::PAPER_NAME_LETTER,
        "Legal" => gtk::PAPER_NAME_LEGAL,
        _ => gtk::PAPER_NAME_A4,
    };
    let paper = paper.to_string();
    if let Err(e) = started() {
        return Err(format!("PRINT: {e}"));
    }
    glib::idle_add_once(move || {
        let window = gtk::OffscreenWindow::new();
        let view = webkit2gtk::WebView::new();
        window.add(&view);
        window.set_default_size(800, 1100);
        window.show_all();
        let done = Rc::new(Cell::new(false));
        let tx = Rc::new(RefCell::new(Some(tx)));
        let keep = Rc::new(RefCell::new(Some(window)));
        view.connect_load_changed(move |view, event| {
            if event != webkit2gtk::LoadEvent::Finished || done.replace(true) {
                return;
            }
            let op = webkit2gtk::PrintOperation::new(view);
            let settings = gtk::PrintSettings::new();
            settings.set_printer("Print to File");
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&uri));
            let setup = gtk::PageSetup::new();
            let size = gtk::PaperSize::new(Some(&paper));
            setup.set_paper_size(&size);
            settings.set_paper_size(&size);
            op.set_print_settings(&settings);
            op.set_page_setup(&setup);
            let finished = tx.clone();
            let failed = tx.clone();
            let keep_finished = keep.clone();
            op.connect_finished(move |_| {
                if let Some(tx) = finished.borrow_mut().take() {
                    let _ = tx.send(Ok(()));
                }
                keep_finished.borrow_mut().take();
            });
            op.connect_failed(move |_, e| {
                if let Some(tx) = failed.borrow_mut().take() {
                    let _ = tx.send(Err(format!("PRINT: {e}")));
                }
            });
            op.print();
        });
        view.load_html(&html, None);
    });
    rx.recv_timeout(std::time::Duration::from_secs(180)).unwrap_or_else(|_| Err("PRINT: the PDF wasn't written in time".into()))
}
