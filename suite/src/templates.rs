//! Templates: ready-made documents, presentations and spreadsheets on the Home page. Picking one
//! opens a new, untitled copy of it (`SuiteApp::open_template`); the template itself is never
//! changed.
//!
//! They come from two places:
//! - the templates installed with the suite (`templates/` in the repository, installed to
//!   `/usr/share/craft-suite/templates`, or `templates\` beside `craft-suite.exe`), listed in its
//!   `index.json` with each one's title, category, thumbnail and licence (`ATTRIBUTION.md` there
//!   credits the open-licence ones);
//! - your own folder (`~/.local/share/craft-suite/templates`, Windows
//!   `%APPDATA%\craft-suite\templates`): every document, presentation or spreadsheet put there, in
//!   subfolders too, is listed under "My templates", with `<name>.png` beside it as its picture.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::kind::Kind;

pub struct Template {
    pub path: PathBuf,
    pub title: String,
    pub category: String,
    pub kind: Kind,
    pub thumbnail: Option<PathBuf>,
    /// Licence and author, for the tile's tooltip: "CC-BY-4.0 · Jane Doe".
    pub credit: Option<String>,
}

/// One row of `index.json`.
#[derive(Deserialize)]
struct Entry {
    file: String,
    title: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    author: Option<String>,
}

#[derive(Deserialize)]
struct Index {
    templates: Vec<Entry>,
}

/// The templates folder installed with the suite.
pub fn installed_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(d) = std::env::var_os("CRAFT_SUITE_TEMPLATES") {
            candidates.push(PathBuf::from(d));
        }
        if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
            candidates.push(exe_dir.join("templates"));
            candidates.push(exe_dir.join("../share/craft-suite/templates"));
        }
        candidates.push(PathBuf::from("/usr/share/craft-suite/templates"));
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../templates"));
        candidates.into_iter().find(|d| d.join("index.json").is_file()).and_then(|d| d.canonicalize().ok())
    })
    .as_deref()
}

/// Your own templates folder (made when you open it from the Home page).
pub fn user_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("craft-suite").join("templates"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|d| d.join("craft-suite").join("templates"))
    }
}

pub const MY_TEMPLATES: &str = "My templates";

/// Every template, the installed ones first (in `index.json`'s order), then your own.
pub fn scan() -> Vec<Template> {
    let mut out = Vec::new();
    if let Some(dir) = installed_dir()
        && let Ok(text) = std::fs::read_to_string(dir.join("index.json"))
    {
        match serde_json::from_str::<Index>(&text) {
            Ok(index) => {
                for e in index.templates {
                    let path = dir.join(&e.file);
                    let Some(kind) = path.to_str().and_then(Kind::for_path) else { continue };
                    if !path.is_file() {
                        continue;
                    }
                    let credit = match (e.license, e.author) {
                        (Some(l), Some(a)) => Some(format!("{l} · {a}")),
                        (Some(l), None) => Some(l),
                        (None, Some(a)) => Some(a),
                        (None, None) => None,
                    };
                    let thumbnail = e.thumbnail.map(|t| dir.join(t)).filter(|t| t.is_file());
                    out.push(Template { path, title: e.title, category: e.category, kind, thumbnail, credit });
                }
            }
            Err(e) => log::warn!("{}: {e}", dir.join("index.json").display()),
        }
    }
    if let Some(dir) = user_dir() {
        let mut own = Vec::new();
        collect(&dir, 0, &mut own);
        own.sort_by_key(|t| t.title.to_lowercase());
        out.extend(own);
    }
    out
}

/// Your own templates under `dir`, a few folders deep.
fn collect(dir: &Path, depth: usize, out: &mut Vec<Template>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 3 {
                collect(&path, depth + 1, out);
            }
            continue;
        }
        let Some(kind) = path.to_str().and_then(Kind::for_path) else { continue };
        let thumbnail = Some(path.with_extension("png")).filter(|t| t.is_file());
        out.push(Template { title: title_of(&path), path, category: MY_TEMPLATES.into(), kind, thumbnail, credit: None });
    }
}

/// "quarterly-report.docx" → "Quarterly report".
fn title_of(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().replace(['-', '_'], " ")).unwrap_or_default();
    let mut chars = stem.trim().chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => "Template".into(),
    }
}

/// The templates, and their pictures as textures once drawn.
#[derive(Default)]
pub struct Library {
    items: Option<Vec<Template>>,
    thumbnails: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl Library {
    pub fn items(&mut self) -> &[Template] {
        self.items.get_or_insert_with(scan)
    }

    /// Looks again (after you add templates to your folder).
    pub fn rescan(&mut self) {
        self.items = None;
        self.thumbnails.clear();
    }

    pub fn thumbnail(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
        self.thumbnails
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let bytes = std::fs::read(path).ok()?;
                let (w, h, rgba) = crate::app::decode_png(&bytes)?;
                let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                Some(ctx.load_texture(path.to_string_lossy(), image, egui::TextureOptions::LINEAR))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_file_names() {
        assert_eq!(title_of(Path::new("/x/quarterly-report.docx")), "Quarterly report");
        assert_eq!(title_of(Path::new("budget_2026.xlsx")), "Budget 2026");
        assert_eq!(title_of(Path::new(".pptx")), ".pptx");
    }

    #[test]
    fn the_repository_index_lists_existing_files() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../templates");
        let Ok(text) = std::fs::read_to_string(dir.join("index.json")) else { return };
        let index: Index = serde_json::from_str(&text).expect("index.json parses");
        for e in index.templates {
            assert!(dir.join(&e.file).is_file(), "{} is missing", e.file);
            assert!(Kind::for_path(&e.file).is_some(), "{} isn't a file the suite opens", e.file);
            assert!(e.license.is_some(), "{} has no licence", e.file);
        }
    }
}
