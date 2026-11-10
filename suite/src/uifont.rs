//! The interface font every app's menus, ribbons and panels use (Settings › Window ›
//! Interface): Inter as bundled, the desktop's own interface font, or any installed font.
//!
//! Installed fonts come from fontconfig (`fc-match`, `fc-list`), which every Linux desktop has.
//! Where it's missing, or a font can't be read, the choice falls back to Inter.

use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use egui::FontData;

use crate::settings::UiFont;

/// One interface font: a regular and a bold face, under a key naming the family.
#[derive(Clone)]
pub struct Face {
    pub key: String,
    pub regular: Arc<FontData>,
    pub bold: Arc<FontData>,
}

/// The faces for `font`, or `None` when the apps draw their interfaces in their own fonts.
pub fn face(font: &UiFont) -> Option<Face> {
    match font {
        UiFont::Own => None,
        UiFont::Inter => inter(),
        UiFont::System => system_family().and_then(|f| installed(&f)).or_else(inter),
        UiFont::Family(family) => installed(family).or_else(inter),
    }
}

/// The desktop's interface font family: GNOME's `font-name`, else fontconfig's sans-serif.
pub fn system_family() -> Option<String> {
    static FAMILY: OnceLock<Option<String>> = OnceLock::new();
    FAMILY
        .get_or_init(|| {
            let gnome = run("gsettings", &["get", "org.gnome.desktop.interface", "font-name"])
                .map(|s| s.trim().trim_matches('\'').to_string())
                .and_then(|name| {
                    // "Inter 10", "Noto Sans Bold 11": the family is what's left before the size
                    // and style words.
                    let mut words: Vec<&str> = name.split_whitespace().collect();
                    while words.last().is_some_and(|w| w.parse::<f32>().is_ok() || STYLE_WORDS.contains(w)) {
                        words.pop();
                    }
                    (!words.is_empty()).then(|| words.join(" "))
                });
            gnome.or_else(|| run("fc-match", &["-f", "%{family[0]}", "sans-serif"]).filter(|s| !s.is_empty()))
        })
        .clone()
}

const STYLE_WORDS: [&str; 8] = ["Regular", "Bold", "Italic", "Oblique", "Medium", "Light", "Semi-Bold", "SemiBold"];

/// The installed font families an interface can use (TrueType and OpenType), sorted.
pub fn families() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut out: Vec<String> = ["TrueType", "CFF"]
            .iter()
            .filter_map(|format| run("fc-list", &["--format", "%{family[0]}\n", &format!(":fontformat={format}")]))
            .flat_map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('.')).map(String::from).collect::<Vec<_>>())
            .collect();
        out.sort_by_key(|f| f.to_lowercase());
        out.dedup();
        out
    })
}

/// Inter as the apps embed it (Regular and Bold, or one Variable face), else an installed copy.
/// A build without the craft fonts has no Inter of its own.
fn inter() -> Option<Face> {
    let get = |styles: &[&str]| {
        styles.iter().find_map(|style| {
            deckcraft_fonts::CRAFT_FONTS
                .iter()
                .find(|f| f.family == "Inter" && f.style == *style)
                .map(|f| Arc::new(FontData::from_static(f.bytes)))
        })
    };
    match get(&["Regular", "Variable"]) {
        Some(regular) => {
            let bold = get(&["SemiBold", "Bold"]).unwrap_or_else(|| regular.clone());
            Some(Face { key: "Inter".into(), regular, bold })
        }
        None => installed("Inter"),
    }
}

/// An installed family's regular and bold faces, read once.
fn installed(family: &str) -> Option<Face> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Face>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(family) {
        return hit.clone();
    }
    let read = |style: &str| -> Option<Arc<FontData>> {
        let pattern = format!("{family}:{style}");
        let out = run("fc-match", &["-f", "%{file}\n%{index}", &pattern])?;
        let mut lines = out.lines();
        let file = lines.next()?;
        // Variable fonts give a named instance in the upper bits; egui reads the face index only.
        let index = lines.next().and_then(|i| i.trim().parse::<u32>().ok()).unwrap_or(0) & 0xffff;
        let bytes = std::fs::read(file).ok()?;
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        Some(Arc::new(data))
    };
    let face = read("weight=regular").map(|regular| {
        let bold = read("weight=demibold").unwrap_or_else(|| regular.clone());
        Face { key: family.to_string(), regular, bold }
    });
    if let Ok(mut c) = cache.lock() {
        c.insert(family.to_string(), face.clone());
    }
    face
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_font_has_no_face() {
        assert!(face(&UiFont::Own).is_none());
    }

    #[test]
    fn inter_is_bundled_when_the_craft_fonts_are() {
        if deckcraft_fonts::CRAFT_FONTS.iter().any(|f| f.family == "Inter") {
            assert_eq!(face(&UiFont::Inter).map(|f| f.key).as_deref(), Some("Inter"));
        }
    }

    #[test]
    fn a_missing_family_falls_back_to_inter_or_a_substitute() {
        // fontconfig substitutes a font it has; without fontconfig, Inter.
        assert!(face(&UiFont::Family("No Such Font 123".into())).is_some());
    }
}
