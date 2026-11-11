//! The shapes a Design Idea adds (accent bars, panels, cards, badges) and how the next idea
//! takes them away again. Every added shape carries the [`TAG`] add-in tag, which survives a
//! PPTX save; a text shape an idea split into cards is hidden and tagged as their source, and
//! comes back (with any text edited in the cards) when another idea is picked.

use deckcraft_geom::Xfrm;
use deckcraft_model::style::ShapeStyle;
use deckcraft_model::text::{Align, Anchor, BodyProps, LevelStyle, ListStyle, Paragraph, Run, RunProps, TextBody};
use deckcraft_model::{ColorRef, ColorTransform, CustomPath, Fill, Geom, Line, SchemeSlot, Shape, ShapeId};

/// The add-in tag on every shape a Design Idea made or changed.
pub(crate) const TAG: &str = "DECKCRAFT_DESIGN_IDEA";
/// On a card or other shape holding list items: `{source id}:{first paragraph}:{paragraphs}`.
pub(crate) const ITEM_TAG: &str = "DECKCRAFT_DESIGN_IDEA_ITEM";
/// On a picture an idea cropped to a shape: the picture's own geometry (JSON).
pub(crate) const GEOM_TAG: &str = "DECKCRAFT_DESIGN_IDEA_GEOM";
const DECOR: &str = "decor";
const SOURCE: &str = "source";

fn tag<'a>(s: &'a Shape, key: &str) -> Option<&'a str> {
    s.tags.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Whether `s` was added by a Design Idea.
pub(crate) fn is_decor(s: &Shape) -> bool {
    tag(s, TAG) == Some(DECOR)
}

/// Whether `s` is a text shape an idea split into cards (kept hidden behind them).
pub(crate) fn is_source(s: &Shape) -> bool {
    tag(s, TAG) == Some(SOURCE)
}

/// Whether any shape on the slide carries an idea's mark.
pub(crate) fn touched(shapes: &[Shape]) -> bool {
    shapes.iter().any(|s| s.tags.iter().any(|(k, _)| k == TAG || k == GEOM_TAG))
}

/// Mark `s` as the hidden source of an idea's cards.
pub(crate) fn hide_as_source(s: &mut Shape) {
    s.hidden = true;
    s.tags.retain(|(k, _)| k != TAG);
    s.tags.push((TAG.into(), SOURCE.into()));
}

/// Crop picture `s` to `g`, remembering its own geometry.
pub(crate) fn set_geom(s: &mut Shape, g: Geom) {
    if tag(s, GEOM_TAG).is_none()
        && let Ok(json) = serde_json::to_string(&s.geom)
    {
        s.tags.push((GEOM_TAG.into(), json));
    }
    s.geom = g;
}

/// The slide's shapes as they were before any idea: added shapes removed, split text shapes
/// shown again with the text edited in their cards, cropped pictures back in their own shape.
/// A card whose source is gone is kept as an ordinary shape, so no text is lost.
pub(crate) fn normalize(shapes: &mut Vec<Shape>) {
    if !touched(shapes) {
        return;
    }
    // Text edited in the cards goes back into the source, last item first.
    let mut items: Vec<(ShapeId, usize, usize, Vec<Paragraph>)> = shapes
        .iter()
        .filter(|s| is_decor(s))
        .filter_map(|s| {
            let v = tag(s, ITEM_TAG)?;
            let mut it = v.split(':').map(|x| x.parse::<usize>().ok());
            let (Some(Some(id)), Some(Some(start)), Some(Some(len))) = (it.next(), it.next(), it.next()) else { return None };
            Some((ShapeId(u32::try_from(id).ok()?), start, len, s.text.as_ref().map(|t| t.paragraphs.clone()).unwrap_or_default()))
        })
        .collect();
    items.sort_by(|a, b| (a.0, b.1).cmp(&(b.0, a.1)));
    for (src, start, len, card) in &items {
        let Some(s) = shapes.iter_mut().find(|s| s.id == *src && is_source(s)) else { continue };
        let Some(body) = s.text.as_mut() else { continue };
        sync_back(body, *start, *len, card);
    }
    let sources: Vec<ShapeId> = shapes.iter().filter(|s| is_source(s)).map(|s| s.id).collect();
    shapes.retain_mut(|s| {
        if !is_decor(s) {
            return true;
        }
        let orphan = tag(s, ITEM_TAG).and_then(|v| v.split(':').next()?.parse::<u32>().ok()).is_some_and(|id| !sources.contains(&ShapeId(id)));
        if orphan {
            s.tags.retain(|(k, _)| k != TAG && k != ITEM_TAG);
        }
        orphan
    });
    for s in shapes.iter_mut() {
        if is_source(s) {
            s.hidden = false;
            s.tags.retain(|(k, _)| k != TAG);
        }
        if let Some(json) = tag(s, GEOM_TAG).map(str::to_string) {
            if let Ok(g) = serde_json::from_str::<Geom>(&json) {
                s.geom = g;
            }
            s.tags.retain(|(k, _)| k != GEOM_TAG);
        }
    }
}

/// Put a card's paragraphs back over paragraphs `start..start + len` of the source when their
/// text changed (the source keeps its own formatting otherwise).
fn sync_back(body: &mut TextBody, start: usize, len: usize, card: &[Paragraph]) {
    let Some(range) = body.paragraphs.get(start..start + len) else { return };
    let base = range.iter().map(|p| p.level).min().unwrap_or(0);
    let texts = |ps: &[Paragraph]| ps.iter().map(Paragraph::text).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>();
    if texts(range) == texts(card) {
        return;
    }
    let new: Vec<Paragraph> = card
        .iter()
        .map(|p| {
            let mut p = p.clone();
            p.level = p.level.saturating_add(base).min(8);
            p
        })
        .collect();
    body.paragraphs.splice(start..start + len, new);
}

/// A plain added shape at `x`.
fn base(name: &str, x: Xfrm) -> Shape {
    Shape { name: format!("Design Idea {name}"), xfrm: Some(x), decorative: true, tags: vec![(TAG.into(), DECOR.into())], ..Default::default() }
}

/// An accent colour, or a see-through tint of it (`alpha` 0–1) that keeps the theme's text
/// readable on light and dark backgrounds alike.
pub(crate) fn accent(slot: SchemeSlot, alpha: f64) -> Fill {
    let c = ColorRef::scheme(slot);
    Fill::solid(if alpha < 1.0 { c.with(ColorTransform::Alpha((alpha.clamp(0.0, 1.0) * 100_000.0) as i32)) } else { c })
}

/// A filled shape with no outline.
pub(crate) fn block(name: &str, x: Xfrm, geom: &str, fill: Fill) -> Shape {
    Shape { geom: Geom::preset(geom), fill: Some(fill), line: Some(Line::none()), ..base(name, x) }
}

/// An outline with no fill.
pub(crate) fn outline(name: &str, x: Xfrm, geom: &str, slot: SchemeSlot, width: f64) -> Shape {
    Shape { geom: Geom::preset(geom), fill: Some(Fill::None), line: Some(Line::solid(ColorRef::scheme(slot), width)), ..base(name, x) }
}

/// A filled polygon through `pts` (slide coordinates), boxed at their bounds.
pub(crate) fn polygon(name: &str, pts: &[(f64, f64)], fill: Fill) -> Shape {
    let (x0, y0) = pts.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
    let (x1, y1) = pts.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        d.push_str(&format!("{} {:.2} {:.2} ", if i == 0 { "M" } else { "L" }, p.0 - x0, p.1 - y0));
    }
    d.push('Z');
    let geom = Geom::Custom { paths: vec![CustomPath { w, h, d, stroke: false, ..Default::default() }] };
    Shape { geom, fill: Some(fill), line: Some(Line::none()), ..base(name, Xfrm::new(x0, y0, w, h)) }
}

/// A picture crop through `pts`, fractions (0–1) of the picture's box.
pub(crate) fn clip(pts: &[(f64, f64)]) -> Geom {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        d.push_str(&format!("{} {:.0} {:.0} ", if i == 0 { "M" } else { "L" }, p.0 * 100_000.0, p.1 * 100_000.0));
    }
    d.push('Z');
    Geom::Custom { paths: vec![CustomPath { w: 100_000.0, h: 100_000.0, d, ..Default::default() }] }
}

/// Text body properties: wrapped, anchored, with a list style giving every level `size` points
/// (runs that set their own size keep it) and `align`.
fn body(anchor: Anchor, align: Align, size: f64, inset: f64) -> (BodyProps, ListStyle) {
    let level = LevelStyle {
        para: deckcraft_model::text::ParaProps { align: Some(align), ..Default::default() },
        run: RunProps { size: Some(size), ..Default::default() },
    };
    let props = BodyProps {
        wrap: Some(true),
        anchor: Some(anchor),
        inset_l: Some(inset),
        inset_r: Some(inset),
        inset_t: Some(inset),
        inset_b: Some(inset),
        ..Default::default()
    };
    (props, ListStyle { levels: vec![Some(level); 9] })
}

/// A text size (points) that fits `chars` characters in `paras` paragraphs into a `w` × `h` box,
/// between 10 and `max`.
pub(crate) fn fit_size(w: f64, h: f64, chars: usize, paras: usize, max: f64) -> f64 {
    let chars = chars.max(1) as f64;
    let by_area = (w.max(1.0) * h.max(1.0) / (0.62 * chars)).sqrt();
    let by_lines = h / (1.35 * paras.max(1) as f64);
    by_area.min(by_lines).min(max).max(10.0).floor()
}

/// A shape holding list item paragraphs (copied from `src` paragraphs `start..start + len`, one
/// level up so the item's head is at the top level).
#[allow(clippy::too_many_arguments)]
pub(crate) fn item_shape(name: &str, x: Xfrm, src: ShapeId, body_of: &TextBody, start: usize, len: usize, anchor: Anchor, align: Align) -> Shape {
    let ps: Vec<Paragraph> = body_of.paragraphs.get(start..start + len).map(<[Paragraph]>::to_vec).unwrap_or_default();
    let base_level = ps.iter().filter(|p| !p.text().trim().is_empty()).map(|p| p.level).min().unwrap_or(0);
    let paragraphs: Vec<Paragraph> = ps
        .into_iter()
        .map(|mut p| {
            p.level = p.level.saturating_sub(base_level);
            p
        })
        .collect();
    let chars: usize = paragraphs.iter().map(Paragraph::char_len).sum();
    let paras = paragraphs.iter().filter(|p| !p.is_empty()).count();
    let inset = 7.2;
    let size = fit_size(x.w - 2.0 * inset, x.h - 2.0 * inset, chars, paras, 24.0);
    let (bp, ls) = body(anchor, align, size, inset);
    let mut s = base(name, x);
    s.text_box = true;
    s.fill = Some(Fill::None);
    s.line = Some(Line::none());
    s.text = Some(TextBody { body: bp, list_style: ls, paragraphs });
    s.tags.push((ITEM_TAG.into(), format!("{}:{start}:{len}", src.0)));
    s
}

/// A text label: `text` in one run of `size` points, coloured `color` (theme text when `None`).
pub(crate) fn label(name: &str, x: Xfrm, text: &str, size: f64, color: Option<ColorRef>, bold: bool) -> Shape {
    let (bp, ls) = body(Anchor::Middle, Align::Center, size, 0.0);
    let props = RunProps { size: Some(size), bold: bold.then_some(true), fill: color.map(Fill::solid), ..Default::default() };
    let p = Paragraph { runs: vec![Run::with(text, props)], ..Default::default() };
    let mut s = base(name, x);
    s.text_box = true;
    s.fill = Some(Fill::None);
    s.line = Some(Line::none());
    s.text = Some(TextBody { body: BodyProps { wrap: Some(false), ..bp }, list_style: ls, paragraphs: vec![p] });
    s
}

/// A filled theme shape (accent fill, light text) holding `text`, as Insert ▸ Shapes draws one.
pub(crate) fn badge(name: &str, x: Xfrm, geom: &str, slot: SchemeSlot, text: &str, size: f64) -> Shape {
    let mut style = ShapeStyle::accent(slot);
    style.line_ref.0 = 0;
    let (bp, ls) = body(Anchor::Middle, Align::Center, size, 0.0);
    let text = (!text.is_empty()).then(|| TextBody {
        body: bp,
        list_style: ls,
        paragraphs: vec![Paragraph {
            runs: vec![Run::with(text, RunProps { size: Some(size), bold: Some(true), ..Default::default() })],
            ..Default::default()
        }],
    });
    Shape { geom: Geom::preset(geom), style: Some(style), line: Some(Line::none()), text, ..base(name, x) }
}

/// Turn an item shape into a filled theme shape (accent fill, light text), for chevrons.
pub(crate) fn filled_item(mut s: Shape, geom: &str, slot: SchemeSlot) -> Shape {
    let mut style = ShapeStyle::accent(slot);
    style.line_ref.0 = 0;
    s.geom = Geom::preset(geom);
    s.style = Some(style);
    s.fill = None;
    s.text_box = false;
    s
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn normalize_removes_added_shapes_and_restores_sources_with_edited_text() {
        let mut src = Shape { id: ShapeId(7), text: Some(TextBody::from_text("one\ntwo\nthree")), text_box: true, ..Default::default() };
        let card = item_shape("card", Xfrm::new(0.0, 0.0, 100.0, 50.0), ShapeId(7), src.text.as_ref().unwrap(), 1, 1, Anchor::Top, Align::Left);
        let mut edited = card.clone();
        edited.text = Some(TextBody::from_text("TWO"));
        hide_as_source(&mut src);
        let bar = block("bar", Xfrm::new(0.0, 0.0, 10.0, 10.0), "rect", accent(SchemeSlot::Accent1, 1.0));
        let mut shapes = vec![bar, src, edited];
        normalize(&mut shapes);
        assert_eq!(shapes.len(), 1);
        assert!(!shapes[0].hidden && shapes[0].tags.is_empty());
        assert_eq!(shapes[0].text.as_ref().unwrap().text(), "one\nTWO\nthree");
        // Without its source a card stays, as an ordinary shape.
        let mut shapes = vec![card];
        normalize(&mut shapes);
        assert_eq!(shapes.len(), 1);
        assert!(!is_decor(&shapes[0]));
    }

    #[test]
    fn a_cropped_picture_gets_its_own_shape_back() {
        let mut p = Shape { geom: Geom::preset("roundRect"), ..Default::default() };
        set_geom(&mut p, Geom::preset("ellipse"));
        set_geom(&mut p, Geom::preset("diamond"));
        let mut shapes = vec![p];
        normalize(&mut shapes);
        assert_eq!(shapes[0].geom, Geom::preset("roundRect"));
        assert!(shapes[0].tags.is_empty());
    }

    #[test]
    fn fit_size_shrinks_long_text() {
        assert!(fit_size(200.0, 100.0, 10, 1, 24.0) >= 20.0);
        assert!(fit_size(200.0, 100.0, 400, 4, 24.0) < 14.0);
        assert_eq!(fit_size(1.0, 1.0, 1000, 10, 24.0), 10.0);
    }
}
