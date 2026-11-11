//! Where each Design Idea puts the slide's shapes, and the shapes it adds. Coordinates are in
//! points on the slide; `m` is the slide margin.

use deckcraft_geom::Xfrm;
use deckcraft_model::text::{Align, Anchor, TextBody};
use deckcraft_model::{ColorRef, ColorTransform, Fill, Geom, PhType, SchemeSlot, Shape};

use super::content::{self, Item};
use super::decor::{accent, badge, block, clip, filled_item, item_shape, label, outline, polygon};

/// What an idea does to the slide.
#[derive(Default)]
pub(super) struct Plan {
    /// New boxes for the slide's own shapes (pictures are cropped to fill theirs).
    pub place: Vec<(usize, Xfrm)>,
    /// Pictures cropped to another shape than their own.
    pub clip: Vec<(usize, Geom)>,
    /// Text shapes split into the cards in `front`, hidden behind them.
    pub hide: Vec<usize>,
    /// The title gets the light Full-bleed band.
    pub band: bool,
    /// Added shapes: behind everything, between the pictures and the text, over everything.
    pub back: Vec<Shape>,
    pub mid: Vec<Shape>,
    pub front: Vec<Shape>,
}

/// The slide an idea lays out.
pub(super) struct Layout<'a> {
    pub sw: f64,
    pub sh: f64,
    pub m: f64,
    pub shapes: &'a [Shape],
    pub title: Option<usize>,
    /// Visible pictures, largest first.
    pub pictures: Vec<usize>,
    /// Text shapes other than the title.
    pub text: Vec<usize>,
    /// The accent colour of the chosen variant.
    pub slot: SchemeSlot,
}

/// A slide's text split into list items: the one text shape (by index), its text and items.
pub(super) struct List<'a> {
    pub index: usize,
    pub body: &'a TextBody,
    pub items: Vec<Item>,
}

impl Layout<'_> {
    /// The one text shape's list items, when there is exactly one text shape.
    pub fn list(&self) -> Option<List<'_>> {
        let [index] = self.text[..] else { return None };
        let body = self.shapes.get(index)?.text.as_ref()?;
        Some(List { index, body, items: content::items(body) })
    }

    /// The one text shape's text.
    pub fn body_text(&self) -> Option<String> {
        self.list().map(|l| l.body.text())
    }

    /// Whether the title has text.
    pub fn has_title(&self) -> bool {
        self.title.and_then(|t| self.shapes.get(t)).is_some_and(|t| t.text.as_ref().is_some_and(|x| !x.text().trim().is_empty()))
    }

    /// Whether every text shape is a subtitle placeholder (a title slide).
    pub fn title_slide(&self) -> bool {
        !self.text.is_empty() && self.text.iter().all(|&i| self.shapes.get(i).and_then(|s| s.ph.as_ref()).is_some_and(|p| p.kind == PhType::SubTitle))
    }

    fn title_at(&self, p: &mut Plan, x: Xfrm) {
        if let Some(t) = self.title {
            p.place.push((t, x));
        }
    }

    fn pic(&self, p: &mut Plan, k: usize, x: Xfrm) {
        if let Some(&i) = self.pictures.get(k) {
            p.place.push((i, x));
        }
    }

    /// Text shapes stacked in a box.
    fn stack(&self, p: &mut Plan, b: Xfrm) {
        let n = self.text.len().max(1) as f64;
        let gap = self.m * 0.4;
        let h = ((b.h - gap * (n - 1.0)) / n).max(1.0);
        for (k, i) in self.text.iter().enumerate() {
            p.place.push((*i, Xfrm::new(b.x, b.y + k as f64 * (h + gap), b.w, h)));
        }
    }

    fn tint(&self, alpha: f64) -> Fill {
        accent(self.slot, alpha)
    }

    fn solid(&self) -> Fill {
        accent(self.slot, 1.0)
    }

    /// A see-through card in the slide's background colour, so the theme's text reads on it
    /// over a picture.
    fn card_fill() -> Fill {
        Fill::solid(ColorRef::scheme(SchemeSlot::Bg1).with(ColorTransform::Alpha(88_000)))
    }

    /// Split the list's items into shapes at `boxes` (one each) and hide the source.
    fn split(&self, p: &mut Plan, list: &List, boxes: &[Xfrm], anchor: Anchor, align: Align) -> Vec<Shape> {
        let Some(src) = self.shapes.get(list.index) else { return vec![] };
        p.hide.push(list.index);
        list.items
            .iter()
            .zip(boxes)
            .enumerate()
            .map(|(k, (it, b))| item_shape(&format!("Item {}", k + 1), *b, src.id, list.body, it.start, it.len, anchor, align))
            .collect()
    }
}

/// `n` cells of a `cols`-column grid in `b`, `gap` apart.
fn grid(b: Xfrm, n: usize, cols: usize, gap: f64) -> Vec<Xfrm> {
    let cols = cols.max(1);
    let rows = n.div_ceil(cols).max(1);
    let cw = ((b.w - gap * (cols - 1) as f64) / cols as f64).max(1.0);
    let ch = ((b.h - gap * (rows - 1) as f64) / rows as f64).max(1.0);
    (0..n).map(|k| Xfrm::new(b.x + (k % cols) as f64 * (cw + gap), b.y + (k / cols) as f64 * (ch + gap), cw, ch)).collect()
}

/// Whether idea `id` suits the slide's content (ideas that need a list, a figure or a quote).
pub(super) fn fits(id: &str, l: &Layout) -> bool {
    let list = l.list();
    let items = list.as_ref().map(|x| x.items.as_slice()).unwrap_or_default();
    let flat = items.iter().all(|i| i.subs == 0);
    let chars = |max: usize| items.iter().all(|i| i.head.chars().count() <= max);
    match id {
        "pictureBullets" => (2..=6).contains(&items.len()) && chars(160),
        "mosaic" => l.text.is_empty(),
        "bigNumber" => l.body_text().is_some_and(|t| t.chars().count() <= 300 && content::statistic(&t).is_some()),
        "quote" => l.body_text().is_some_and(|t| content::is_quote(&t)),
        "statement" => l.body_text().is_some_and(|t| t.chars().count() <= 160) && items.len() == 1 && flat,
        "cards" => (3..=6).contains(&items.len()) && chars(140),
        "agendaList" => (3..=7).contains(&items.len()) && flat && chars(90),
        "timeline" => (3..=6).contains(&items.len()) && list.as_ref().is_some_and(|x| content::timeline(items) || content::steps(x.body, items)),
        "chevrons" => {
            (3..=5).contains(&items.len()) && flat && chars(40) && (list.as_ref().is_some_and(|x| content::steps(x.body, items)) || chars(28))
        }
        "comparison" => l.text.len() == 2 || (items.len() == 2 && items.iter().all(|i| i.subs > 0)),
        _ => true,
    }
}

/// The plan for idea `id`, or `None` for an unknown id.
pub(super) fn plan(id: &str, l: &Layout) -> Option<Plan> {
    plan_picture(id, l).or_else(|| plan_text(id, l)).or_else(|| plan_title(id, l))
}

/// The picture ideas' plans.
fn plan_picture(id: &str, l: &Layout) -> Option<Plan> {
    let (sw, sh, m) = (l.sw, l.sh, l.m);
    let mut p = Plan::default();
    let has_text = !l.text.is_empty();
    match id {
        // ---- One picture -------------------------------------------------------------
        "fullBleed" => {
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw, sh));
            if l.title.is_some() {
                l.title_at(&mut p, Xfrm::new(m, sh * 0.68, sw - 2.0 * m, sh * 0.2));
                p.band = true;
            }
            l.stack(&mut p, Xfrm::new(m, sh * 0.06, sw * 0.45, sh * 0.5));
        }
        "pictureLeft" | "pictureRight" => {
            let left = id == "pictureLeft";
            let half = sw * 0.5;
            let (px, tx) = if left { (0.0, half + m) } else { (half, m) };
            l.pic(&mut p, 0, Xfrm::new(px, 0.0, half, sh));
            let tw = half - 2.0 * m;
            l.title_at(&mut p, Xfrm::new(tx, m, tw, sh * 0.24));
            l.stack(&mut p, Xfrm::new(tx, m + sh * 0.28, tw, sh - 2.0 * m - sh * 0.28));
        }
        "framed" => {
            l.pic(&mut p, 0, Xfrm::new(sw * 0.42, m * 1.5, sw * 0.58 - m * 1.5, sh - 3.0 * m));
            let tw = sw * 0.42 - 2.0 * m;
            l.title_at(&mut p, Xfrm::new(m, m * 1.5, tw, sh * 0.3));
            l.stack(&mut p, Xfrm::new(m, m * 1.5 + sh * 0.34, tw, sh - 3.0 * m - sh * 0.34));
        }
        "pictureTop" => {
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw, sh * 0.5));
            l.title_at(&mut p, Xfrm::new(m, sh * 0.5 + m * 0.6, sw - 2.0 * m, sh * 0.16));
            let top = sh * 0.5 + m * 0.6 + sh * 0.18;
            l.stack(&mut p, Xfrm::new(m, top, sw - 2.0 * m, (sh - m - top).max(1.0)));
        }
        "pictureBottom" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.16));
            l.stack(&mut p, Xfrm::new(m, m + sh * 0.18, sw - 2.0 * m, sh * 0.62 - m - sh * 0.18 - m * 0.4));
            l.pic(&mut p, 0, Xfrm::new(0.0, sh * 0.62, sw, sh * 0.38));
        }
        "captionBand" => {
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw, sh));
            let (y, h) = (sh * 0.68, sh * 0.32);
            p.mid.push(block("Caption Band", Xfrm::new(0.0, y, sw, h), "rect", Layout::card_fill()));
            p.mid.push(block("Caption Accent", Xfrm::new(0.0, y, sw, 6.0), "rect", l.solid()));
            let tw = if has_text { sw * 0.55 - m } else { sw - 2.0 * m };
            l.title_at(&mut p, Xfrm::new(m, y + 12.0, tw, h - 12.0 - m * 0.6));
            l.stack(&mut p, Xfrm::new(sw * 0.58, y + 12.0, sw * 0.42 - m, h - 12.0 - m * 0.6));
        }
        "pictureCircle" => {
            let d = sh - 3.0 * m;
            let (px, py) = (sw - 1.5 * m - d, (sh - d) / 2.0);
            l.pic(&mut p, 0, Xfrm::new(px, py, d, d));
            if let Some(&i) = l.pictures.first() {
                p.clip.push((i, Geom::preset("ellipse")));
            }
            p.front.push(outline("Picture Ring", Xfrm::new(px - 8.0, py - 8.0, d + 16.0, d + 16.0), "ellipse", l.slot, 3.0));
            p.front.push(block("Accent Dot", Xfrm::new(px + d * 0.06, py + d * 0.8, 44.0, 44.0), "ellipse", l.solid()));
            let tw = px - 2.5 * m;
            l.title_at(&mut p, Xfrm::new(m, 1.5 * m, tw, sh * 0.28));
            l.stack(&mut p, Xfrm::new(m, 1.5 * m + sh * 0.32, tw, sh - 3.0 * m - sh * 0.32));
        }
        "diagonalSplit" => {
            let bx = sw * 0.38;
            l.pic(&mut p, 0, Xfrm::new(bx, 0.0, sw - bx, sh));
            if let Some(&i) = l.pictures.first() {
                p.clip.push((i, clip(&[(0.22, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])));
            }
            let top = bx + 0.22 * (sw - bx);
            p.front.push(polygon("Diagonal Stripe", &[(top - 18.0, 0.0), (top - 8.0, 0.0), (bx - 8.0, sh), (bx - 18.0, sh)], l.solid()));
            let tw = bx - 18.0 - 2.0 * m;
            l.title_at(&mut p, Xfrm::new(m, 1.5 * m, tw, sh * 0.3));
            l.stack(&mut p, Xfrm::new(m, 1.5 * m + sh * 0.34, tw, sh - 3.0 * m - sh * 0.34));
        }
        "pictureAccentPanel" => {
            p.back.push(block("Accent Panel", Xfrm::new(0.0, 0.0, sw * 0.56, sh), "rect", l.tint(0.18)));
            l.pic(&mut p, 0, Xfrm::new(sw * 0.44, 1.5 * m, sw * 0.56 - 1.5 * m, sh - 3.0 * m));
            let tw = sw * 0.44 - 2.0 * m;
            l.title_at(&mut p, Xfrm::new(m, 1.5 * m, tw, sh * 0.26));
            p.front.push(block("Accent Rule", Xfrm::new(m, 1.5 * m + sh * 0.28, 64.0, 5.0), "rect", l.solid()));
            l.stack(&mut p, Xfrm::new(m, 1.5 * m + sh * 0.32, tw, sh - 3.0 * m - sh * 0.32));
        }
        "pictureInset" => {
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw, sh));
            let c = Xfrm::new(1.5 * m, 1.5 * m, sw * 0.42, sh - 3.0 * m);
            p.mid.push(block("Inset Card", c, "rect", Layout::card_fill()));
            p.mid.push(block("Inset Accent", Xfrm::new(c.x, c.y, 8.0, c.h), "rect", l.solid()));
            l.title_at(&mut p, Xfrm::new(c.x + 24.0, c.y + 16.0, c.w - 40.0, c.h * 0.34));
            l.stack(&mut p, Xfrm::new(c.x + 24.0, c.y + 16.0 + c.h * 0.38, c.w - 40.0, c.h - 32.0 - c.h * 0.38));
        }
        "pictureBullets" => {
            let list = l.list()?;
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw * 0.4, sh));
            let (x0, w) = (sw * 0.4 + m, sw * 0.6 - 2.0 * m);
            l.title_at(&mut p, Xfrm::new(x0, m, w, sh * 0.2));
            numbered_rows(l, &mut p, &list, Xfrm::new(x0, m + sh * 0.24, w, sh - 2.0 * m - sh * 0.24));
        }
        // ---- Two to four pictures ----------------------------------------------------
        "grid" => {
            let n = l.pictures.len();
            let top = m + sh * 0.2;
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.16));
            let gx = if has_text {
                l.stack(&mut p, Xfrm::new(m, top, sw * 0.3, sh - top - m));
                m + sw * 0.3 + m * 0.6
            } else {
                m
            };
            let cells = grid(Xfrm::new(gx, top, sw - m - gx, sh - top - m), n, if n == 4 { 2 } else { n }, 8.0);
            for (k, c) in cells.into_iter().enumerate() {
                l.pic(&mut p, k, c);
            }
        }
        "collage" => {
            let n = l.pictures.len();
            l.pic(&mut p, 0, Xfrm::new(0.0, 0.0, sw * 0.5, sh));
            let row = Xfrm::new(sw * 0.5 + 8.0, sh * 0.52, sw * 0.5 - 8.0, sh * 0.48);
            for (k, c) in grid(row, n.saturating_sub(1), n.saturating_sub(1), 8.0).into_iter().enumerate() {
                l.pic(&mut p, k + 1, c);
            }
            let (x, w, h) = (sw * 0.5 + m, sw * 0.5 - 2.0 * m, sh * 0.52 - 1.5 * m);
            if has_text {
                l.title_at(&mut p, Xfrm::new(x, m, w, h * 0.42));
                l.stack(&mut p, Xfrm::new(x, m + h * 0.46, w, h * 0.54));
            } else {
                l.title_at(&mut p, Xfrm::new(x, m, w, h));
            }
        }
        "filmstrip" => {
            let n = l.pictures.len();
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.16));
            l.stack(&mut p, Xfrm::new(m, m + sh * 0.18, sw - 2.0 * m, sh * 0.46 - m - sh * 0.18));
            let strip = Xfrm::new(0.0, sh * 0.5, sw, sh * 0.42);
            p.back.push(block("Film Strip", strip, "rect", l.solid()));
            let row = Xfrm::new(m, strip.y + 12.0, sw - 2.0 * m, strip.h - 24.0);
            for (k, c) in grid(row, n, n, 12.0).into_iter().enumerate() {
                l.pic(&mut p, k, c);
            }
        }
        "mosaic" => {
            let n = l.pictures.len();
            let g = 4.0;
            let cells: Vec<Xfrm> = match n {
                2 => grid(Xfrm::new(0.0, 0.0, sw, sh), 2, 2, g),
                3 => {
                    let lw = sw * 0.6 - g / 2.0;
                    let mut v = vec![Xfrm::new(0.0, 0.0, lw, sh)];
                    v.extend(grid(Xfrm::new(lw + g, 0.0, sw - lw - g, sh), 2, 1, g));
                    v
                }
                _ => grid(Xfrm::new(0.0, 0.0, sw, sh), n, 2, g),
            };
            for (k, c) in cells.into_iter().enumerate() {
                l.pic(&mut p, k, c);
            }
            if l.title.is_some() {
                l.title_at(&mut p, Xfrm::new(m, sh * 0.7, sw * 0.6, sh * 0.18));
                p.band = true;
            }
        }
        _ => return None,
    }
    Some(p)
}

/// The title-and-text ideas' plans.
fn plan_text(id: &str, l: &Layout) -> Option<Plan> {
    let (sw, sh, m) = (l.sw, l.sh, l.m);
    let mut p = Plan::default();
    match id {
        // ---- Title and text ----------------------------------------------------------
        "titleColumn" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw * 0.32, sh - 2.0 * m));
            l.stack(&mut p, Xfrm::new(sw * 0.36, m, sw * 0.64 - m, sh - 2.0 * m));
        }
        "centred" => {
            l.title_at(&mut p, Xfrm::new(sw * 0.1, m, sw * 0.8, sh * 0.26));
            l.stack(&mut p, Xfrm::new(sw * 0.18, m + sh * 0.3, sw * 0.64, sh - 2.0 * m - sh * 0.3));
        }
        "titleBottom" => {
            l.stack(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.62));
            l.title_at(&mut p, Xfrm::new(m, sh * 0.72, sw - 2.0 * m, sh * 0.2));
        }
        "titleRight" => {
            l.stack(&mut p, Xfrm::new(m, m, sw * 0.64 - 2.0 * m, sh - 2.0 * m));
            l.title_at(&mut p, Xfrm::new(sw * 0.66, m, sw * 0.34 - m, sh - 2.0 * m));
        }
        "narrow" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw * 0.6, sh * 0.22));
            l.stack(&mut p, Xfrm::new(m, m + sh * 0.26, sw * 0.5, sh - 2.0 * m - sh * 0.26));
        }
        "accentBar" => {
            let x = m + 22.0;
            p.back.push(block("Accent Bar", Xfrm::new(m, m + 6.0, 8.0, sh * 0.2 - 12.0), "rect", l.solid()));
            l.title_at(&mut p, Xfrm::new(x, m, sw - m - x, sh * 0.2));
            l.stack(&mut p, Xfrm::new(x, m + sh * 0.26, sw - m - x, sh - 2.0 * m - sh * 0.26));
        }
        "tintedPanel" => {
            p.back.push(block("Tinted Panel", Xfrm::new(0.0, 0.0, sw * 0.36, sh), "rect", l.tint(0.18)));
            l.title_at(&mut p, Xfrm::new(m, m, sw * 0.36 - 2.0 * m, sh - 2.0 * m));
            l.stack(&mut p, Xfrm::new(sw * 0.36 + m, m, sw * 0.64 - 2.0 * m, sh - 2.0 * m));
        }
        "underline" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.18));
            p.back.push(block("Title Underline", Xfrm::new(m, m + sh * 0.19, sw * 0.16, 5.0), "rect", l.solid()));
            l.stack(&mut p, Xfrm::new(m, m + sh * 0.25, sw - 2.0 * m, sh - 2.0 * m - sh * 0.25));
        }
        "bigNumber" => {
            let stat = content::statistic(&l.body_text()?)?;
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.18));
            let r = Xfrm::new(m, m + sh * 0.24, sw * 0.38, sh - 2.0 * m - sh * 0.24);
            let d = r.w.min(r.h);
            p.back.push(block("Number Circle", Xfrm::new(r.x + (r.w - d) / 2.0, r.y + (r.h - d) / 2.0, d, d), "ellipse", l.tint(0.16)));
            let size = (r.h * 0.5).min(d * 0.9 / (0.62 * stat.chars().count().max(1) as f64)).clamp(24.0, 110.0).floor();
            p.front.push(label("Big Number", r, &stat, size, Some(ColorRef::scheme(l.slot)), true));
            let x = r.x + r.w + m;
            l.stack(&mut p, Xfrm::new(x, r.y, sw - m - x, r.h));
        }
        "quote" => {
            let x = m * 2.5;
            p.front.push(label("Quotation Mark", Xfrm::new(m * 0.8, m * 0.4, 100.0, 120.0), "\u{201C}", 120.0, Some(ColorRef::scheme(l.slot)), true));
            p.back.push(block("Quote Bar", Xfrm::new(x - 18.0, sh * 0.2, 6.0, sh * 0.5), "rect", l.solid()));
            l.stack(&mut p, Xfrm::new(x, sh * 0.2, sw - 2.0 * x, sh * 0.5));
            l.title_at(&mut p, Xfrm::new(x, sh * 0.74, sw - 2.0 * x, sh * 0.16));
        }
        "statement" => {
            l.title_at(&mut p, Xfrm::new(m, m * 0.6, sw - 2.0 * m, sh * 0.18));
            let band = Xfrm::new(0.0, sh * 0.26, sw, sh * 0.5);
            p.back.push(block("Statement Band", band, "rect", l.tint(0.16)));
            p.back.push(block("Statement Rule", Xfrm::new(sw * 0.42, band.y + band.h - 14.0, sw * 0.16, 4.0), "rect", l.solid()));
            l.stack(&mut p, Xfrm::new(2.0 * m, band.y + 12.0, sw - 4.0 * m, band.h - 30.0));
        }
        "cards" => {
            let list = l.list()?;
            let n = list.items.len();
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.18));
            let y0 = m + sh * 0.24;
            let cells = grid(Xfrm::new(m, y0, sw - 2.0 * m, sh - m - y0), n, if n <= 4 { n } else { 3 }, 14.0);
            for (k, c) in cells.iter().enumerate() {
                p.back.push(block(&format!("Card {}", k + 1), *c, "rect", l.tint(0.14)));
                p.back.push(block(&format!("Card Accent {}", k + 1), Xfrm::new(c.x, c.y, c.w, 6.0), "rect", l.solid()));
            }
            let inner: Vec<Xfrm> = cells.iter().map(|c| Xfrm::new(c.x + 6.0, c.y + 12.0, c.w - 12.0, c.h - 18.0)).collect();
            let shapes = l.split(&mut p, &list, &inner, Anchor::Top, Align::Left);
            p.front.extend(shapes);
        }
        "agendaList" => {
            let list = l.list()?;
            l.title_at(&mut p, Xfrm::new(m, m, sw * 0.3, sh - 2.0 * m));
            p.back.push(block("Agenda Rule", Xfrm::new(sw * 0.34, m, 4.0, sh - 2.0 * m), "rect", l.solid()));
            let x0 = sw * 0.34 + m;
            numbered_rows(l, &mut p, &list, Xfrm::new(x0, m, sw - m - x0, sh - 2.0 * m));
        }
        "timeline" => {
            let list = l.list()?;
            let n = list.items.len().max(1);
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.18));
            let ly = sh * 0.58;
            p.back.push(block("Timeline", Xfrm::new(m, ly - 2.0, sw - 2.0 * m, 4.0), "rect", l.solid()));
            let colw = (sw - 2.0 * m) / n as f64;
            let top = m + sh * 0.22;
            let mut boxes = vec![];
            let mut anchors = vec![];
            for k in 0..n {
                let cx = m + colw * (k as f64 + 0.5);
                p.front.push(block(&format!("Milestone {}", k + 1), Xfrm::new(cx - 9.0, ly - 9.0, 18.0, 18.0), "ellipse", l.solid()));
                let x = m + colw * k as f64 + 4.0;
                if k % 2 == 0 {
                    boxes.push(Xfrm::new(x, top, colw - 8.0, ly - 16.0 - top));
                    anchors.push(Anchor::Bottom);
                } else {
                    boxes.push(Xfrm::new(x, ly + 16.0, colw - 8.0, sh - m - ly - 16.0));
                    anchors.push(Anchor::Top);
                }
            }
            let mut shapes = l.split(&mut p, &list, &boxes, Anchor::Top, Align::Center);
            for (s, a) in shapes.iter_mut().zip(anchors) {
                if let Some(t) = s.text.as_mut() {
                    t.body.anchor = Some(a);
                }
            }
            p.front.extend(shapes);
        }
        "chevrons" => {
            let list = l.list()?;
            let n = list.items.len().max(1);
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.2));
            let h = sh * 0.3;
            let (pt, gap) = (h * 0.5, 6.0);
            let cw = (sw - 2.0 * m + (n - 1) as f64 * (pt - gap)) / n as f64;
            let boxes: Vec<Xfrm> = (0..n).map(|k| Xfrm::new(m + k as f64 * (cw - pt + gap), sh * 0.42, cw, h)).collect();
            let shapes = l.split(&mut p, &list, &boxes, Anchor::Middle, Align::Center);
            for (k, s) in shapes.into_iter().enumerate() {
                p.front.push(filled_item(s, if k == 0 { "homePlate" } else { "chevron" }, l.slot));
            }
        }
        "comparison" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw - 2.0 * m, sh * 0.18));
            let y0 = m + sh * 0.24;
            let cols = grid(Xfrm::new(m, y0, sw - 2.0 * m, sh - m - y0), 2, 2, m);
            for (k, c) in cols.iter().enumerate() {
                p.back.push(block(&format!("Column {}", k + 1), *c, "rect", l.tint(0.14)));
                p.back.push(block(&format!("Column Accent {}", k + 1), Xfrm::new(c.x, c.y, c.w, 6.0), "rect", l.solid()));
            }
            let inner: Vec<Xfrm> = cols.iter().map(|c| Xfrm::new(c.x + 12.0, c.y + 16.0, c.w - 24.0, c.h - 28.0)).collect();
            if let [a, b] = l.text[..] {
                p.place.push((a, inner[0]));
                p.place.push((b, inner[1]));
            } else {
                let list = l.list()?;
                let shapes = l.split(&mut p, &list, &inner, Anchor::Top, Align::Left);
                p.front.extend(shapes);
            }
        }
        _ => return None,
    }
    Some(p)
}

/// The title-slide ideas' plans.
fn plan_title(id: &str, l: &Layout) -> Option<Plan> {
    let (sw, sh, m) = (l.sw, l.sh, l.m);
    let mut p = Plan::default();
    let has_text = !l.text.is_empty();
    match id {
        // ---- Title (and subtitle) only -----------------------------------------------
        "titleMiddle" => {
            if has_text {
                l.title_at(&mut p, Xfrm::new(sw * 0.1, sh * 0.3, sw * 0.8, sh * 0.28));
                l.stack(&mut p, Xfrm::new(sw * 0.1, sh * 0.6, sw * 0.8, sh * 0.16));
            } else {
                l.title_at(&mut p, Xfrm::new(sw * 0.1, sh * 0.36, sw * 0.8, sh * 0.28));
            }
        }
        "titleLowerThird" => {
            if has_text {
                l.title_at(&mut p, Xfrm::new(m, sh * 0.52, sw - 2.0 * m, sh * 0.24));
                l.stack(&mut p, Xfrm::new(m, sh * 0.78, sw - 2.0 * m, sh * 0.22 - m * 0.6));
            } else {
                l.title_at(&mut p, Xfrm::new(m, sh * 0.62, sw - 2.0 * m, sh * 0.24));
            }
        }
        "titleTopLeft" => {
            l.title_at(&mut p, Xfrm::new(m, m, sw * 0.7, sh * 0.4));
            l.stack(&mut p, Xfrm::new(m, m + sh * 0.44, sw * 0.6, sh * 0.2));
        }
        "titleBand" => {
            let b = Xfrm::new(0.0, sh * 0.3, sw, sh * 0.34);
            p.back.push(block("Title Band", b, "rect", l.tint(0.2)));
            p.back.push(block("Title Band Edge", Xfrm::new(0.0, b.y, sw, 5.0), "rect", l.solid()));
            l.title_at(&mut p, Xfrm::new(1.5 * m, b.y + 8.0, sw - 3.0 * m, b.h - 16.0));
            l.stack(&mut p, Xfrm::new(1.5 * m, sh * 0.68, sw - 3.0 * m, sh * 0.18));
        }
        "titleAccentLine" => {
            l.title_at(&mut p, Xfrm::new(2.0 * m, sh * 0.26, sw * 0.72, sh * 0.32));
            p.back.push(block("Accent Line", Xfrm::new(2.0 * m, sh * 0.6, sw * 0.22, 6.0), "rect", l.solid()));
            l.stack(&mut p, Xfrm::new(2.0 * m, sh * 0.64, sw * 0.6, sh * 0.2));
        }
        "titleFrame" => {
            p.back.push(outline("Title Frame", Xfrm::new(1.2 * m, 1.2 * m, sw - 2.4 * m, sh - 2.4 * m), "rect", l.slot, 3.0));
            l.title_at(&mut p, Xfrm::new(2.5 * m, sh * 0.28, sw - 5.0 * m, sh * 0.3));
            l.stack(&mut p, Xfrm::new(2.5 * m, sh * 0.6, sw - 5.0 * m, sh * 0.16));
        }
        "titleSplit" => {
            p.back.push(block("Split Panel", Xfrm::new(0.0, 0.0, sw * 0.5, sh), "rect", l.tint(0.2)));
            p.back.push(block("Split Edge", Xfrm::new(sw * 0.5 - 3.0, 0.0, 6.0, sh), "rect", l.solid()));
            l.title_at(&mut p, Xfrm::new(m, sh * 0.3, sw * 0.5 - 2.0 * m, sh * 0.4));
            l.stack(&mut p, Xfrm::new(sw * 0.5 + m, sh * 0.3, sw * 0.5 - 2.0 * m, sh * 0.4));
        }
        "titleCorner" => {
            p.back.push(block("Corner", Xfrm::new(0.0, sh * 0.5, sw * 0.32, sh * 0.5), "rtTriangle", l.solid()));
            let corner = Xfrm { flip_h: true, flip_v: true, ..Xfrm::new(sw * 0.78, 0.0, sw * 0.22, sh * 0.36) };
            p.back.push(block("Corner Tint", corner, "rtTriangle", l.tint(0.35)));
            l.title_at(&mut p, Xfrm::new(sw * 0.18, sh * 0.3, sw * 0.7, sh * 0.28));
            l.stack(&mut p, Xfrm::new(sw * 0.18, sh * 0.6, sw * 0.6, sh * 0.16));
        }
        _ => return None,
    }
    Some(p)
}

/// The list's items as rows in `b`, each behind a numbered accent badge.
fn numbered_rows(l: &Layout, p: &mut Plan, list: &List, b: Xfrm) {
    let n = list.items.len().max(1);
    let gap = 8.0;
    let rh = ((b.h - gap * (n - 1) as f64) / n as f64).max(1.0);
    let bs = (rh * 0.8).min(36.0);
    let mut boxes = vec![];
    for k in 0..n {
        let y = b.y + k as f64 * (rh + gap);
        p.back.push(badge(
            &format!("Number {}", k + 1),
            Xfrm::new(b.x, y + (rh - bs) / 2.0, bs, bs),
            "ellipse",
            l.slot,
            &(k + 1).to_string(),
            (bs * 0.45).floor(),
        ));
        boxes.push(Xfrm::new(b.x + bs + 14.0, y, b.w - bs - 14.0, rh));
    }
    let shapes = l.split(p, list, &boxes, Anchor::Middle, Align::Left);
    p.front.extend(shapes);
}
