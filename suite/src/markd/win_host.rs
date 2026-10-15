//! Windows: the Markd views are WebView2 child windows of the suite's window, made and driven on
//! the window's own thread: `send` queues the work and `pump` (after each frame) runs it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroIsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wry::WebViewBuilderExtWindows;
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};

use super::{Cmd, PxRect, Shared};

static QUEUE: Mutex<Vec<Cmd>> = Mutex::new(Vec::new());

pub fn available() -> Result<(), String> {
    Ok(())
}

pub fn send(cmd: Cmd) {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(cmd);
    super::repaint();
}

pub fn parent_of(frame: &eframe::Frame) -> Result<u64, String> {
    match frame.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => Ok(h.hwnd.get() as u64),
        Ok(_) => Err("Markd tabs: not a Win32 window".into()),
        Err(e) => Err(format!("Markd tabs: no window ({e})")),
    }
}

/// WebView2 prints only through its dialog: Export PDF opens the page in the browser instead,
/// to print from there.
pub const PDF_DIRECT: bool = false;

pub fn print_pdf(_html: String, _page: &str, _path: &Path) -> Result<(), String> {
    Err("UNAVAILABLE: PDF export isn't available here".into())
}

pub fn pump(_frame: &eframe::Frame) {
    let work: Vec<Cmd> = std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()));
    for cmd in work {
        apply(cmd);
    }
}

struct Parent(u64);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.0 as isize).ok_or(HandleError::Unavailable)?;
        // SAFETY: the suite's window outlives its tabs' views.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(hwnd))) })
    }
}

thread_local! {
    static VIEWS: RefCell<HashMap<u64, wry::WebView>> = RefCell::new(HashMap::new());
    static CONTEXT: RefCell<Option<wry::WebContext>> = const { RefCell::new(None) };
}

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")).map(|d| PathBuf::from(d).join("craft-suite").join("markd"))
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
            let _ = v.set_bounds(bounds(r));
        }),
        Cmd::Visible(id, visible) => with_view(id, |v| {
            let _ = v.set_visible(visible);
        }),
        Cmd::Focus(id) => with_view(id, |v| {
            let _ = v.focus();
        }),
        Cmd::Unfocus(id) => with_view(id, |v| {
            let _ = v.focus_parent();
        }),
        Cmd::Eval(id, js) => with_view(id, |v| {
            let _ = v.evaluate_script(&js);
        }),
        Cmd::Destroy(id) => {
            VIEWS.with(|views| views.borrow_mut().remove(&id));
        }
    }
}

fn with_view(id: u64, f: impl FnOnce(&wry::WebView)) {
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
        wry::WebViewBuilder::new_with_web_context(context)
            .with_id(&view_id)
            .with_https_scheme(true)
            .with_initialization_script_for_main_only(super::host_js(&tab), true)
            .with_ipc_handler(move |req| super::on_message(&messages, req.body()))
            .with_navigation_handler(allow_navigation)
            .with_new_window_req_handler(|url, _| {
                allow_navigation(url);
                wry::NewWindowResponse::Deny
            })
            .with_devtools(cfg!(debug_assertions))
            .with_bounds(bounds(rect))
            .with_asynchronous_custom_protocol("markd".into(), |view, request, responder| {
                let (status, mime, body) = super::serve(view, &request.uri().to_string());
                match wry::http::Response::builder().status(status).header(wry::http::header::CONTENT_TYPE, mime).body(body) {
                    Ok(r) => responder.respond(r),
                    Err(e) => log::warn!("markd page: {e}"),
                }
            })
            .with_url("https://markd.app/index.html")
            .build_as_child(&Parent(parent))
            .map_err(|e| e.to_string())
    })?;
    VIEWS.with(|views| views.borrow_mut().insert(id, webview));
    Ok(())
}

fn allow_navigation(url: String) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://markd.app/") || lower.starts_with("about:") || lower.starts_with("data:") || lower.starts_with("blob:") {
        return true;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        crate::system::open(&url);
    }
    false
}
