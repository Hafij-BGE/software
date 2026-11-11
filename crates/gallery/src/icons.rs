//! The icon library.
//!
//! `data/icons.tsv` holds one icon per line: id, label, category, tags, kind and SVG path data.
//! Stroke icons (Tabler) are centre-lines on a 24-unit grid drawn 2 units wide with round caps;
//! fill icons (Health Icons) are silhouettes on a 48-unit grid. A path marked `~` is filled
//! even-odd; [`Icon::pieces`] turns those into nonzero-safe outlines, so a renderer or file format
//! that only knows the nonzero rule (a custom shape in a presentation, say) still shows the holes.

use std::collections::HashMap;
use std::sync::OnceLock;

use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Shape, Stroke};
use vello_cpu::peniko::{Color, Fill};
use vello_cpu::{PixmapMut, RenderContext, Resources};

const DATA: &str = include_str!("../data/icons.tsv");

/// How an icon is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Outlines stroked [`Icon::stroke_width`] units wide on a 24-unit grid.
    Stroke,
    /// Filled silhouettes on a 48-unit grid.
    Fill,
}

/// One icon in the library.
#[derive(Debug)]
pub struct Icon {
    /// Stable id, used by the apps' `insert.icon` commands (`dna`, `health-lungs`).
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    /// Search words, space separated.
    pub tags: &'static str,
    pub kind: Kind,
    data: &'static str,
}

/// Categories shown first, in this order; the rest follow alphabetically.
const LEAD: [&str; 8] = ["Biology & Science", "Medical", "People", "Communication", "Document", "Charts", "Arrows", "Nature & Animals"];

/// Every icon, in library order.
pub fn all() -> &'static [Icon] {
    static ALL: OnceLock<Vec<Icon>> = OnceLock::new();
    ALL.get_or_init(|| DATA.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).filter_map(parse_line).collect())
}

fn parse_line(line: &'static str) -> Option<Icon> {
    let mut f = line.split('\t');
    let (id, label, category, tags, kind, data) = (f.next()?, f.next()?, f.next()?, f.next()?, f.next()?, f.next()?);
    let kind = match kind {
        "s" => Kind::Stroke,
        "f" => Kind::Fill,
        _ => return None,
    };
    Some(Icon { id, label, category, tags, kind, data })
}

/// The categories, leading ones first.
pub fn categories() -> &'static [&'static str] {
    static CATS: OnceLock<Vec<&'static str>> = OnceLock::new();
    CATS.get_or_init(|| {
        let mut rest: Vec<&'static str> = Vec::new();
        for icon in all() {
            if !LEAD.contains(&icon.category) && !rest.contains(&icon.category) {
                rest.push(icon.category);
            }
        }
        rest.sort_unstable();
        LEAD.iter().copied().filter(|c| all().iter().any(|i| i.category == *c)).chain(rest).collect()
    })
}

/// The icon `name` names: its id (`arrow-right`, `arrow_right`, any case) or its label.
pub fn find(name: &str) -> Option<&'static Icon> {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| all().iter().enumerate().map(|(k, i)| (i.id, k)).collect());
    let raw = name.trim();
    if raw.is_empty() {
        return None;
    }
    let key = raw.to_ascii_lowercase().replace(['_', ' '], "-");
    index.get(key.as_str()).and_then(|&k| all().get(k)).or_else(|| all().iter().find(|i| i.label.eq_ignore_ascii_case(raw)))
}

/// Icons matching every word of `query` (in the id, label, tags or category), best matches first,
/// within `category` when given. An empty query lists the category (or everything).
pub fn search(query: &str, category: Option<&str>) -> Vec<&'static Icon> {
    let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
    let mut hits: Vec<(u8, &'static Icon)> =
        all().iter().filter(|i| category.is_none_or(|c| i.category == c)).filter_map(|i| score(i, &words).map(|s| (s, i))).collect();
    hits.sort_by_key(|(s, _)| *s);
    hits.into_iter().map(|(_, i)| i).collect()
}

/// Lower is better; `None` when a word doesn't match.
fn score(icon: &Icon, words: &[String]) -> Option<u8> {
    if words.is_empty() {
        return Some(0);
    }
    let label = icon.label.to_lowercase();
    let category = icon.category.to_lowercase();
    let mut best = 0u8;
    for w in words {
        let s = if icon.id == w || label == *w {
            0
        } else if icon.id.split('-').any(|p| p == w) {
            1
        } else if icon.id.starts_with(w.as_str()) {
            2
        } else if icon.id.contains(w.as_str()) || label.contains(w.as_str()) {
            3
        } else if icon.tags.split(' ').any(|t| t == w) {
            4
        } else if icon.tags.contains(w.as_str()) || category.contains(w.as_str()) {
            5
        } else {
            return None;
        };
        best = best.max(s);
    }
    // Plain icons before their variants (`heart` before `heart-broken`).
    Some(best * 2 + u8::from(icon.id.contains('-')))
}

/// Icons for the words of `text` (a slide title, say): for each distinct word of 4+ letters, the
/// best whole-word match by id, label or tag. At most `limit`, without repeats.
pub fn suggest(text: &str, limit: usize) -> Vec<&'static Icon> {
    let mut out: Vec<&'static Icon> = Vec::new();
    let mut seen_words: Vec<String> = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase) {
        if out.len() >= limit {
            break;
        }
        if word.chars().count() < 4 || STOP_WORDS.contains(&word.as_str()) || seen_words.contains(&word) {
            continue;
        }
        let stem = word.strip_suffix('s').filter(|s| s.len() >= 4).unwrap_or(&word).to_string();
        seen_words.push(word.clone());
        let hit = all().iter().filter_map(|i| whole_word(i, &word, &stem).map(|s| (s, i))).min_by_key(|(s, i)| (*s, i.id.len()));
        if let Some((_, icon)) = hit
            && !out.iter().any(|o| o.id == icon.id)
        {
            out.push(icon);
        }
    }
    out
}

fn whole_word(icon: &Icon, word: &str, stem: &str) -> Option<u8> {
    let parts = || icon.id.split('-');
    if icon.id == word || icon.id == stem {
        Some(0)
    } else if parts().any(|p| p == word || p == stem) {
        Some(1)
    } else if icon.tags.split(' ').any(|t| t == word || t == stem) {
        Some(2)
    } else {
        None
    }
}

const STOP_WORDS: [&str; 24] = [
    "this", "that", "with", "from", "your", "have", "will", "what", "when", "where", "which", "their", "there", "about", "into", "more", "than",
    "them", "they", "were", "been", "over", "also", "some",
];

impl Icon {
    /// Size of the design grid in path units.
    pub fn view(&self) -> f64 {
        match self.kind {
            Kind::Stroke => 24.0,
            Kind::Fill => 48.0,
        }
    }

    /// Design line width in grid units (stroke icons).
    pub fn stroke_width(&self) -> f64 {
        2.0
    }

    /// The raw SVG path data of each piece and whether it fills even-odd.
    pub fn raw_paths(&self) -> impl Iterator<Item = (&'static str, bool)> {
        self.data.split('|').map(|p| match p.strip_prefix('~') {
            Some(d) => (d, true),
            None => (p, false),
        })
    }

    /// The icon's pieces in grid units. Each piece fills correctly with the nonzero rule (even-odd
    /// pieces have their contours re-wound); stroke icons are meant to be stroked, not filled.
    pub fn pieces(&self) -> Vec<BezPath> {
        self.raw_paths()
            .filter_map(|(d, even_odd)| {
                let path = BezPath::from_svg(d).ok()?;
                Some(if even_odd { nonzero(&path) } else { path })
            })
            .collect()
    }

    /// [`Icon::pieces`] as SVG path data, for apps with their own path types (or kurbo version).
    pub fn piece_data(&self) -> Vec<String> {
        self.pieces().iter().map(BezPath::to_svg).collect()
    }

    /// The icon as an SVG document in `rgb`, `size` units square.
    pub fn svg(&self, rgb: [u8; 3], size: f64) -> String {
        let v = self.view();
        let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {v} {v}">"#);
        match self.kind {
            Kind::Stroke => s.push_str(&format!(
                r#"<g fill="none" stroke="{hex}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"#,
                self.stroke_width()
            )),
            Kind::Fill => s.push_str(&format!(r#"<g fill="{hex}">"#)),
        }
        for (d, even_odd) in self.raw_paths() {
            let rule = if even_odd { r#" fill-rule="evenodd""# } else { "" };
            s.push_str(&format!(r#"<path{rule} d="{d}"/>"#));
        }
        s.push_str("</g></svg>");
        s
    }

    /// The icon `px` pixels square in `rgba`, as premultiplied RGBA bytes.
    pub fn render(&self, px: u16, rgba: [u8; 4]) -> Vec<u8> {
        let px = px.clamp(4, 4096);
        let mut ctx = RenderContext::new(px, px);
        ctx.set_transform(Affine::scale(f64::from(px) / self.view()));
        ctx.set_paint(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
        match self.kind {
            Kind::Stroke => {
                ctx.set_stroke(Stroke::new(self.stroke_width()).with_caps(Cap::Round).with_join(Join::Round));
                for piece in self.pieces() {
                    ctx.stroke_path(&piece);
                }
            }
            Kind::Fill => {
                ctx.set_fill_rule(Fill::NonZero);
                for piece in self.pieces() {
                    ctx.fill_path(&piece);
                }
            }
        }
        ctx.flush();
        let mut pixels = vec![0u8; usize::from(px) * usize::from(px) * 4];
        let mut res = Resources::new();
        if let Some(pm) = PixmapMut::new(px, px, &mut pixels) {
            ctx.render(pm, &mut res);
        }
        pixels
    }

    /// The icon `px` pixels square in `rgb` on transparency, as a PNG file.
    pub fn png(&self, px: u16, rgb: [u8; 3]) -> Option<Vec<u8>> {
        let px = px.clamp(4, 4096);
        let mut pixels = self.render(px, [rgb[0], rgb[1], rgb[2], 255]);
        unpremultiply(&mut pixels);
        let img = image::RgbaImage::from_raw(u32::from(px), u32::from(px), pixels)?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(out.into_inner())
    }
}

/// Premultiplied RGBA to straight RGBA, in place.
pub fn unpremultiply(pixels: &mut [u8]) {
    for p in pixels.as_chunks_mut::<4>().0 {
        let a = u16::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// `path` re-wound so the nonzero rule fills what even-odd does: a contour nested inside an even
/// number of others winds one way, inside an odd number the other.
fn nonzero(path: &BezPath) -> BezPath {
    let subs = subpaths(path);
    let mut out = BezPath::new();
    for (k, sub) in subs.iter().enumerate() {
        let Some(probe) = start(sub) else { continue };
        let depth = subs.iter().enumerate().filter(|(j, other)| *j != k && other.winding(probe) != 0).count();
        let want_positive = depth % 2 == 0;
        let fixed = if (sub.area() >= 0.0) == want_positive { sub.clone() } else { sub.reverse_subpaths() };
        out.extend(fixed);
    }
    out
}

fn subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) || out.is_empty() {
            out.push(BezPath::new());
        }
        if let Some(last) = out.last_mut() {
            last.push(*el);
        }
    }
    out.retain(|p| p.elements().len() > 1);
    out
}

/// A point on the contour just inside its start, to test nesting with.
fn start(sub: &BezPath) -> Option<Point> {
    match sub.elements().first() {
        Some(PathEl::MoveTo(p)) => Some(*p),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_has_thousands_of_unique_icons_in_both_kinds() {
        let n = all().len();
        assert!(n >= 5000, "{n} icons");
        let mut ids: Vec<&str> = all().iter().map(|i| i.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids repeat");
        assert!(all().iter().any(|i| i.kind == Kind::Fill));
        assert!(all().iter().any(|i| i.kind == Kind::Stroke));
    }

    #[test]
    fn biology_and_science_is_the_first_category_and_has_dna_and_lungs() {
        assert_eq!(categories().first(), Some(&"Biology & Science"));
        let bio = search("", Some("Biology & Science"));
        assert!(bio.len() >= 150, "{}", bio.len());
        assert!(bio.iter().any(|i| i.id == "dna"));
        assert!(search("lungs", None).iter().any(|i| i.id.starts_with("health-")));
    }

    #[test]
    fn every_icon_parses() {
        for icon in all() {
            assert!(!icon.pieces().is_empty(), "{} has no outline", icon.id);
        }
    }

    #[test]
    fn find_takes_ids_labels_and_underscores() {
        assert_eq!(find("arrow_right").map(|i| i.id), Some("arrow-right"));
        assert_eq!(find("Arrow Right").map(|i| i.id), Some("arrow-right"));
        assert!(find("no such icon").is_none());
    }

    #[test]
    fn search_puts_the_plain_icon_first() {
        assert_eq!(search("heart", None).first().map(|i| i.id), Some("heart"));
        assert!(search("zzzz qqq", None).is_empty());
    }

    #[test]
    fn suggest_finds_icons_for_slide_words() {
        let ids: Vec<&str> = suggest("Our cloud storage and the calendar for this year", 4).iter().map(|i| i.id).collect();
        assert!(ids.contains(&"cloud"), "{ids:?}");
        assert!(ids.contains(&"calendar"), "{ids:?}");
        assert!(ids.len() <= 4);
    }

    #[test]
    fn icons_render_and_encode() {
        for id in ["heart", "dna", "health-lungs"] {
            let Some(icon) = find(id).or_else(|| search(id.trim_start_matches("health-"), None).into_iter().next()) else {
                continue;
            };
            let px = icon.render(48, [0, 0, 0, 255]);
            assert!(px.as_chunks::<4>().0.iter().any(|p| p[3] > 0), "{id} draws nothing");
            assert!(icon.png(48, [0, 0, 0]).is_some_and(|p| p.starts_with(b"\x89PNG")));
            assert!(icon.svg([0, 0, 0], 72.0).starts_with("<svg"));
        }
    }

    #[test]
    fn even_odd_holes_survive_the_nonzero_rewind() {
        // A square with a square hole, both wound the same way: even-odd shows the hole.
        let path = BezPath::from_svg("M0 0H10V10H0Z M3 3H7V7H3Z").unwrap_or_default();
        let fixed = nonzero(&path);
        assert_eq!(fixed.winding(Point::new(5.0, 5.0)), 0, "hole filled");
        assert_ne!(fixed.winding(Point::new(1.0, 1.0)), 0, "ring empty");
    }
}
