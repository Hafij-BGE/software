//! The desktop side of Markd's `native.ts`: what its Android app's `Bridge.java` does with the
//! Storage Access Framework, done with files and the system's file dialogs. A document's "uri"
//! is its path.
//!
//! Every call runs on a worker thread (dialogs block) and answers `Ok(result)` or
//! `Err("CODE: message")`; `CANCELLED` when a dialog was dismissed.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};

use super::Shared;

type Reply = Result<Value, String>;

fn cancelled() -> String {
    "CANCELLED: Cancelled".into()
}

fn io(e: impl std::fmt::Display) -> String {
    format!("IO: {e}")
}

fn arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args.get(name).and_then(Value::as_str).ok_or_else(|| format!("ARGS: missing {name}"))
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

const MARKDOWN: &[&str] = &["md", "markdown", "mdown", "mkd", "txt"];

fn markdown_filter(d: rfd::FileDialog) -> rfd::FileDialog {
    d.add_filter("Markdown", MARKDOWN).add_filter("All files", &["*"])
}

/// The folder a dialog starts in: the open document's.
fn start_dir(tab: &Shared) -> Option<PathBuf> {
    tab.lock().path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

/// The largest document or picture the page is handed whole.
const READ_LIMIT: u64 = 64 << 20;

fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(io)?;
    if meta.len() > READ_LIMIT {
        return Err(format!("IO: {} is too large to open here", name_of(path)));
    }
    std::fs::read(path).map_err(io)
}

/// A plain absolute path: no `.` or `..` in it, and no hidden folder or file (`~/.ssh`,
/// `~/.config`…), which a document never needs.
fn plain(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|c| match c {
            Component::Normal(n) => !n.to_string_lossy().starts_with('.'),
            Component::RootDir | Component::Prefix(_) => true,
            Component::CurDir | Component::ParentDir => false,
        })
}

/// Whether the page may read or write `path`: one the user chose (or a file in a folder they
/// chose), or else a Markdown file outside hidden folders, as the page's recent files are. So
/// script that got into the page still can't reach the user's other files.
fn allowed(tab: &Shared, path: &Path) -> Result<(), String> {
    let chosen = !path.components().any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        && tab.lock().granted.iter().any(|g| path.starts_with(g));
    let markdown = path.extension().and_then(|e| e.to_str()).is_some_and(|e| MARKDOWN.contains(&e.to_ascii_lowercase().as_str()));
    if chosen || (plain(path) && markdown) {
        Ok(())
    } else {
        Err(format!("BLOCKED: {} wasn't opened in Markd", path.display()))
    }
}

/// A folder the page may list or add a document to: one the user chose, or one outside hidden
/// folders.
fn allowed_dir(tab: &Shared, dir: &Path) -> Result<(), String> {
    let chosen = !dir.components().any(|c| matches!(c, Component::CurDir | Component::ParentDir)) && tab.lock().granted.iter().any(|g| dir.starts_with(g));
    if chosen || plain(dir) { Ok(()) } else { Err(format!("BLOCKED: {} isn't a folder Markd opened", dir.display())) }
}

fn grant(tab: &Shared, path: &Path) {
    tab.lock().granted.insert(path.to_path_buf());
}

fn read_document(path: &Path) -> Reply {
    let bytes = read_limited(path)?;
    Ok(json!({
        "uri": path.to_string_lossy(),
        "name": name_of(path),
        "text": String::from_utf8_lossy(&bytes),
        "hasFolder": true,
    }))
}

fn decode(args: &Value) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(arg(args, "base64")?.as_bytes()).map_err(|e| format!("ARGS: {e}"))
}

/// `name` in `dir`, or `name-2`, `name-3`… when that's taken.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem}-{n}{ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

pub fn call(tab: &Arc<Shared>, method: &str, args: &Value) -> Reply {
    match method {
        "getInfo" => Ok(json!({
            "sdkInt": 0,
            "release": std::env::consts::OS,
            "model": "Craft Suite",
            "version": crate::update::installed_version(),
            "handwritingSupported": false,
        })),
        "getLaunchDocument" => {
            let launch = tab.lock().launch.take();
            Ok(json!({ "launchUri": launch.map(|p| p.to_string_lossy().to_string()) }))
        }
        "openDocument" => {
            let mut d = markdown_filter(rfd::FileDialog::new().set_title("Open"));
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let path = d.pick_file().ok_or_else(cancelled)?;
            grant(tab, &path);
            read_document(&path)
        }
        "readDocument" => {
            let path = Path::new(arg(args, "uri")?);
            allowed(tab, path)?;
            read_document(path)
        }
        "writeDocument" => {
            let path = PathBuf::from(arg(args, "uri")?);
            allowed(tab, &path)?;
            write_file(&path, arg(args, "text")?.as_bytes())?;
            Ok(json!(true))
        }
        "createDocument" => {
            let mut d = markdown_filter(rfd::FileDialog::new().set_title("Save As").set_file_name(args.get("name").and_then(Value::as_str).unwrap_or("Untitled.md")));
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let path = d.save_file().ok_or_else(cancelled)?;
            grant(tab, &path);
            Ok(json!({ "uri": path.to_string_lossy(), "name": name_of(&path), "hasFolder": true }))
        }
        "saveFileAs" => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or("export");
            let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            let mut d = rfd::FileDialog::new().set_title("Export").set_file_name(name);
            if !ext.is_empty() {
                d = d.add_filter(ext.to_uppercase(), &[ext.as_str()]);
            }
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let bytes = decode(args)?;
            let path = d.save_file().ok_or_else(cancelled)?;
            write_file(&path, &bytes)?;
            Ok(json!({ "name": name_of(&path) }))
        }
        "openFolder" => {
            let mut d = rfd::FileDialog::new().set_title("Open folder");
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let dir = d.pick_folder().ok_or_else(cancelled)?;
            grant(tab, &dir);
            let s = dir.to_string_lossy();
            Ok(json!({ "treeUri": s, "rootId": s, "name": name_of(&dir) }))
        }
        "releaseFolder" | "setChrome" | "setHandwriting" => Ok(json!(true)),
        "listFolder" => {
            let dir = PathBuf::from(arg(args, "docId")?);
            allowed_dir(tab, &dir)?;
            let mut out = Vec::new();
            for entry in std::fs::read_dir(&dir).map_err(io)?.flatten() {
                let path = entry.path();
                let meta = entry.metadata().ok();
                let modified = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let s = path.to_string_lossy();
                out.push(json!({
                    "name": name_of(&path),
                    "uri": s,
                    "docId": s,
                    "isDir": meta.is_some_and(|m| m.is_dir()),
                    "modified": modified,
                }));
            }
            Ok(Value::Array(out))
        }
        "createInFolder" => {
            let dir = PathBuf::from(arg(args, "docId")?);
            allowed_dir(tab, &dir)?;
            let name = arg(args, "name")?;
            let mut parts = Path::new(name).components();
            if !matches!((parts.next(), parts.next()), (Some(Component::Normal(_)), None)) || name.starts_with('.') {
                return Err("ARGS: not a file name".into());
            }
            let path = free_name(&dir, name);
            allowed(tab, &path)?;
            std::fs::write(&path, b"").map_err(io)?;
            Ok(json!({ "uri": path.to_string_lossy() }))
        }
        "grantFolderAccess" => Ok(json!({ "hasFolder": true })),
        "setCurrentDocument" => {
            let uri = args.get("uri").and_then(Value::as_str).unwrap_or("");
            if !uri.is_empty() {
                allowed(tab, Path::new(uri))?;
            }
            tab.lock().path = (!uri.is_empty()).then(|| PathBuf::from(uri));
            Ok(json!(true))
        }
        "saveAsset" => {
            // Only beside the open document, and only pictures and media.
            let doc = PathBuf::from(arg(args, "docUri")?);
            if tab.lock().path.as_deref() != Some(doc.as_path()) {
                return Err("BLOCKED: not the open document".into());
            }
            let folder = doc.parent().ok_or_else(|| "IO: the document has no folder".to_string())?;
            let relative = arg(args, "path")?;
            if Path::new(relative).components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err("ARGS: not a path inside the document's folder".into());
            }
            let target = folder.join(relative);
            if !super::is_media(&target) {
                return Err("BLOCKED: only pictures and media are saved beside the document".into());
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
                // Not out through a link in the document's folder.
                let inside = parent.canonicalize().ok().zip(folder.canonicalize().ok()).is_some_and(|(p, f)| p.starts_with(f));
                if !inside {
                    return Err("ARGS: not a path inside the document's folder".into());
                }
            }
            let target = match (target.parent(), target.file_name()) {
                (Some(dir), Some(name)) => free_name(dir, &name.to_string_lossy()),
                _ => target,
            };
            std::fs::write(&target, decode(args)?).map_err(io)?;
            let rel = target.strip_prefix(folder).unwrap_or(&target).to_string_lossy().replace('\\', "/");
            Ok(json!({ "path": rel }))
        }
        "pickImage" => {
            let mut d = rfd::FileDialog::new().set_title("Insert picture").add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"]);
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let path = d.pick_file().ok_or_else(cancelled)?;
            let bytes = read_limited(&path)?;
            Ok(json!({ "mime": super::mime_of(&path), "base64": base64::engine::general_purpose::STANDARD.encode(bytes) }))
        }
        "printHtml" => {
            let job = args.get("jobName").and_then(Value::as_str).unwrap_or("Markd");
            if !super::host::PDF_DIRECT {
                // Printed from the browser instead ("Save as PDF" there).
                let safe: String = job.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
                let file = std::env::temp_dir().join(format!("markd-{safe}.html"));
                std::fs::write(&file, arg(args, "html")?).map_err(io)?;
                crate::system::open(&file);
                return Ok(json!(true));
            }
            let mut d = rfd::FileDialog::new().set_title("Export PDF").set_file_name(format!("{job}.pdf")).add_filter("PDF", &["pdf"]);
            if let Some(dir) = start_dir(tab) {
                d = d.set_directory(dir);
            }
            let path = d.save_file().ok_or_else(cancelled)?;
            let page = args.get("pageSize").and_then(Value::as_str).unwrap_or("A4");
            super::host::print_pdf(arg(args, "html")?.to_string(), page, &path)?;
            Ok(json!(true))
        }
        "openExternal" => {
            let url = arg(args, "url")?;
            let lower = url.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")) {
                return Err("BLOCKED: Only web and mail links can be opened".into());
            }
            crate::system::open(url);
            Ok(json!(true))
        }
        // Android's Share sheet; on the desktop, the file's folder.
        "share" => {
            let path = PathBuf::from(arg(args, "uri")?);
            allowed(tab, &path)?;
            let Some(dir) = path.parent().filter(|d| d.is_dir()) else { return Err("IO: the document has no folder".into()) };
            crate::system::open(dir);
            Ok(json!(true))
        }
        _ => Err(format!("UNKNOWN: Unknown method {method}")),
    }
}

/// Writes the file in place, as Markd's Android app does (a link or a shared file stays one).
fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths() {
        assert!(plain(Path::new("/home/u/notes/a.md")));
        assert!(!plain(Path::new("/home/u/.ssh/id_ed25519")));
        assert!(!plain(Path::new("/home/u/notes/../.bashrc")));
        assert!(!plain(Path::new("notes/a.md")));
    }
}
