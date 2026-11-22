//! Design Ideas: arrangements of what is on the current slide (picture full-bleed with the title
//! on a band, picture on one half, a circle crop, a grid or collage of several pictures, text
//! as cards, a timeline or process chevrons, title-slide designs). An idea moves and resizes the
//! slide's own shapes, crops pictures to their new box and may add accent shapes in one of the
//! theme's accent colours; it changes only that slide. Everything is computed here, offline.
//!
//! Shapes an idea adds are tagged (see `decor`): picking another idea takes them away again and
//! puts text it split into cards back together, so ideas can be tried one after another.

mod content;
mod decor;
mod layouts;
#[cfg(test)]
mod more_tests;

use std::borrow::Cow;

use deckcraft_geom::Xfrm;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::{Fill, Presentation, Rgba, SchemeSlot, Shape, ShapeId, ShapeKind};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, cmd, has_slide, opt_usize_param, str_param, xfrm_of};
use crate::{Result, Selection, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "design.ideas", "Design Ideas", [], None, "{slide?} → [{id, label, variants}] ideas for the slide", has_slide, ideas_cmd),
        cmd!("design.applyIdea", "Apply Design Idea", ["Home", "Designer"], None, "{id, variant?, slide?}", has_slide, apply_cmd),
    ]
}

/// An idea: id and label.
pub type Idea = (&'static str, &'static str);

/// Ideas for a slide with one picture (`pictureBullets` only when its text is a short list).
const PICTURE_IDEAS: [Idea; 12] = [
    ("fullBleed", "Full-bleed picture, title on a band"),
    ("pictureLeft", "Picture left, text right"),
    ("pictureRight", "Picture right, text left"),
    ("framed", "Framed picture beside the title"),
    ("pictureTop", "Picture across the top, text below"),
    ("pictureBottom", "Title and text above a picture strip"),
    ("captionBand", "Full-bleed picture with a caption band"),
    ("pictureCircle", "Picture in a circle beside the text"),
    ("diagonalSplit", "Picture cut on a diagonal"),
    ("pictureAccentPanel", "Picture over an accent panel"),
    ("pictureInset", "Text on a card over the picture"),
    ("pictureBullets", "Picture beside numbered points"),
];
/// Ideas for a slide with two to four pictures (`mosaic` only without body text).
const MULTI_PICTURE_IDEAS: [Idea; 4] = [
    ("grid", "Pictures in a grid"),
    ("collage", "Collage: one large picture and smaller ones"),
    ("filmstrip", "Pictures in a row on an accent strip"),
    ("mosaic", "Picture mosaic, title on a band"),
];
/// Ideas for a slide with a title and text.
const TEXT_IDEAS: [Idea; 8] = [
    ("titleColumn", "Title column beside the text"),
    ("centred", "Centred title over the text"),
    ("titleBottom", "Text above, title at the foot"),
    ("titleRight", "Text beside a title column on the right"),
    ("narrow", "Title and a narrow text column on the left"),
    ("accentBar", "Accent bar beside the title"),
    ("tintedPanel", "Title on a tinted panel"),
    ("underline", "Title with an accent underline"),
];
/// Ideas for a title and text that need a certain kind of text (a figure, a quotation, a list
/// of points, steps or dates, two sides to compare).
const CONTENT_IDEAS: [Idea; 8] = [
    ("bigNumber", "Key figure, large"),
    ("quote", "Quotation"),
    ("statement", "Statement on a band"),
    ("cards", "Points as cards"),
    ("agendaList", "Numbered agenda"),
    ("timeline", "Timeline"),
    ("chevrons", "Process chevrons"),
    ("comparison", "Two-column comparison"),
];
/// Ideas for a slide that has only a title, or a title and subtitle (a title, section or
/// closing slide).
const TITLE_IDEAS: [Idea; 8] = [
    ("titleMiddle", "Title in the middle of the slide"),
    ("titleLowerThird", "Title on the lower third"),
    ("titleTopLeft", "Large title at the top left"),
    ("titleBand", "Title on an accent band"),
    ("titleAccentLine", "Title over an accent line"),
    ("titleFrame", "Title in a frame"),
    ("titleSplit", "Split slide, title on a tinted half"),
    ("titleCorner", "Title with accent corners"),
];

/// Every idea, in catalogue order.
pub fn catalogue() -> impl Iterator<Item = Idea> {
    PICTURE_IDEAS.into_iter().chain(MULTI_PICTURE_IDEAS).chain(TEXT_IDEAS).chain(CONTENT_IDEAS).chain(TITLE_IDEAS)
}

/// The colour variants of idea `id`: the accent each one is drawn in. Ideas that only move
/// the slide's shapes (and add none) have one.
pub fn variants_of(id: &str) -> &'static [SchemeSlot] {
    const ACCENTS: [SchemeSlot; 3] = [SchemeSlot::Accent1, SchemeSlot::Accent2, SchemeSlot::Accent3];
    const ONE: [SchemeSlot; 1] = [SchemeSlot::Accent1];
    match id {
        "captionBand" | "pictureCircle" | "diagonalSplit" | "pictureAccentPanel" | "pictureInset" | "pictureBullets" | "filmstrip" | "accentBar"
        | "tintedPanel" | "underline" | "bigNumber" | "quote" | "statement" | "cards" | "agendaList" | "timeline" | "chevrons" | "comparison"
        | "titleBand" | "titleAccentLine" | "titleFrame" | "titleSplit" | "titleCorner" => &ACCENTS,
        _ => &ONE,
    }
}

/// The roles of a slide's top-level shapes, by index.
struct Roles {
    title: Option<usize>,
    /// Visible pictures, largest first.
    pictures: Vec<usize>,
    text: Vec<usize>,
}

fn roles(shapes: &[Shape]) -> Roles {
    let mut r = Roles { title: None, pictures: vec![], text: vec![] };
    for (i, s) in shapes.iter().enumerate() {
        if s.hidden {
            continue;
        }
        if s.ph.as_ref().is_some_and(|p| p.kind.is_title()) && r.title.is_none() {
            r.title = Some(i);
        } else if matches!(s.kind, ShapeKind::Picture { .. }) {
            r.pictures.push(i);
        } else if s.text.as_ref().is_some_and(|t| !t.text().trim().is_empty()) {
            r.text.push(i);
        }
    }
    // The largest picture leads.
    let area = |j: usize| shapes.get(j).and_then(|s| s.xfrm).map(|x| x.w * x.h).unwrap_or(0.0);
    r.pictures.sort_by(|a, b| area(*b).total_cmp(&area(*a)));
    r
}

/// Whether `s` paints a fill or an outline once inherited and style-referenced values are
/// resolved, as the renderer does (`fill: None` alone means inherited, not invisible).
fn shows(ctx: Option<&Ctx>, s: &Shape) -> bool {
    let visible = |f: &Option<Fill>| f.as_ref().is_some_and(|f| !f.is_none());
    match ctx {
        Some(ctx) => visible(&resolve::fill(ctx, s).0) || visible(&resolve::line(ctx, s).0.fill),
        None => visible(&s.fill) || visible(&s.line.as_ref().and_then(|l| l.fill.clone())),
    }
}

/// Whether an idea may move this shape: a picture, a placeholder or a plain text box. Diagram
/// boxes, connectors, groups, tables, charts and media are laid out by their own geometry, which
/// the ideas do not know, so a slide holding one gets no ideas.
fn arrangeable(ctx: Option<&Ctx>, s: &Shape) -> bool {
    match s.kind {
        ShapeKind::Picture { .. } => true,
        ShapeKind::Shape => {
            let is_title = s.ph.as_ref().is_some_and(|p| p.kind.is_title());
            let has_text = s.text.as_ref().is_some_and(|t| !t.text().trim().is_empty());
            // Only titles and shapes with text are moved (see `roles`): an empty one that shows
            // (filled or outlined) would stay where it is, over the rearranged slide.
            (s.ph.is_some() || s.text_box) && (is_title || has_text || !shows(ctx, s))
        }
        _ => false,
    }
}

/// The slide's shapes without what earlier ideas added (see [`decor::normalize`]).
fn normalized(shapes: &[Shape]) -> Cow<'_, [Shape]> {
    if decor::touched(shapes) {
        let mut v = shapes.to_vec();
        decor::normalize(&mut v);
        Cow::Owned(v)
    } else {
        Cow::Borrowed(shapes)
    }
}

/// The layout input for `shapes`.
fn layout<'a>(doc: &Presentation, shapes: &'a [Shape], r: Roles, slot: SchemeSlot) -> layouts::Layout<'a> {
    let (sw, sh) = (doc.slide_size.width.max(1.0), doc.slide_size.height.max(1.0));
    layouts::Layout { sw, sh, m: sw.min(sh) * 0.06, shapes, title: r.title, pictures: r.pictures, text: r.text, slot }
}

/// The ideas for slide `index` (none when it has nothing to arrange, or other content in the way).
pub fn ideas_for(doc: &Presentation, index: usize) -> Vec<Idea> {
    let Some(slide) = doc.slides.get(index) else { return vec![] };
    let ctx = Ctx::for_slide(doc, slide);
    let shapes = normalized(&slide.shapes);
    if shapes.iter().any(|s| !s.hidden && !arrangeable(ctx.as_ref(), s)) {
        return vec![];
    }
    let l = layout(doc, &shapes, roles(&shapes), SchemeSlot::Accent1);
    let pick = |ideas: &[Idea]| ideas.iter().copied().filter(|(id, _)| layouts::fits(id, &l)).collect::<Vec<_>>();
    match l.pictures.len() {
        0 => {}
        1 => return pick(&PICTURE_IDEAS),
        // The ideas lay out up to four pictures; more would stay put, over the rearranged text.
        2..=4 => return pick(&MULTI_PICTURE_IDEAS),
        _ => return vec![],
    }
    if !l.has_title() && l.title.is_some() && l.text.is_empty() {
        // An empty title would look the same in every idea.
        return vec![];
    }
    match (l.title.is_some(), l.text.is_empty()) {
        (true, false) => {
            let mut v = if l.has_title() && l.title_slide() { TITLE_IDEAS.to_vec() } else { vec![] };
            v.extend(TEXT_IDEAS);
            v.extend(pick(&CONTENT_IDEAS));
            v
        }
        (true, true) => TITLE_IDEAS.to_vec(),
        _ => vec![],
    }
}

/// Crop a picture whose source has aspect `src` (w/h) to fill a box of aspect `dst`, centred.
fn cover_crop(src: f64, dst: f64) -> [f64; 4] {
    if !(src.is_finite() && dst.is_finite() && src > 0.0 && dst > 0.0) {
        return [0.0; 4];
    }
    if src > dst {
        let cut = (1.0 - dst / src) / 2.0;
        [cut, 0.0, cut, 0.0]
    } else {
        let cut = (1.0 - src / dst) / 2.0;
        [0.0, cut, 0.0, cut]
    }
}

/// The light fill behind the title in the full-bleed idea. Its opacity is an `alpha` transform
/// (about 220/255), as PPTX stores it, so the fill compares equal after a save and reopen.
fn band_fill() -> Fill {
    Fill::solid(deckcraft_model::ColorRef::rgb(Rgba::rgb(255, 255, 255)).with(deckcraft_model::ColorTransform::Alpha(86274)))
}

/// The band as released versions up to 0.10 stored it (opacity in the colour's own alpha).
fn legacy_band_fill() -> Fill {
    Fill::solid(deckcraft_model::ColorRef::rgb(Rgba::rgba(255, 255, 255, 220)))
}

/// Whether `f` is a Full-bleed band, in the current or the legacy form.
fn is_band(f: &Fill) -> bool {
    *f == band_fill() || *f == legacy_band_fill()
}

/// Apply idea `id` to slide `index` of `doc` in its first colour variant (`sel` resolves
/// inherited placeholder boxes).
pub fn apply_idea(doc: &mut Presentation, sel: &Selection, index: usize, id: &str) -> Result<()> {
    apply_idea_variant(doc, sel, index, id, 0)
}

/// Apply colour variant `variant` (see [`variants_of`]) of idea `id` to slide `index` of `doc`.
pub fn apply_idea_variant(doc: &mut Presentation, sel: &Selection, index: usize, id: &str, variant: usize) -> Result<()> {
    if !ideas_for(doc, index).iter().any(|i| i.0 == id) {
        return Err(bad("design.applyIdea", format!("no idea `{id}` for this slide (see design.ideas)")));
    }
    let slots = variants_of(id);
    let Some(&slot) = slots.get(variant) else {
        return Err(bad("design.applyIdea", format!("idea `{id}` has {} colour variant(s); `variant` must be below that", slots.len())));
    };
    let sel = Selection { slide: index, ..sel.clone() };
    let mut shapes = doc.slides.get(index).map(|s| s.shapes.clone()).ok_or_else(|| bad("design.applyIdea", "no slide"))?;
    decor::normalize(&mut shapes);
    let boxes: Vec<Xfrm> = shapes.iter().map(|s| xfrm_of(doc, &sel, s)).collect();
    let r = roles(&shapes);
    let (mut plan, pictures) = {
        let l = layout(doc, &shapes, r, slot);
        let plan = layouts::plan(id, &l).ok_or_else(|| bad("design.applyIdea", format!("no idea `{id}`")))?;
        // Pictures go to the back (largest first), so text over them stays readable.
        let mut pictures = l.pictures.clone();
        pictures.sort_unstable();
        (plan, pictures)
    };
    for s in plan.back.iter_mut().chain(plan.mid.iter_mut()).chain(plan.front.iter_mut()) {
        s.id = ShapeId(doc.alloc_id().ok_or_else(|| bad("design.applyIdea", NO_ID_SPACE))?);
    }
    for (i, x) in &plan.place {
        let (Some(s), Some(old)) = (shapes.get_mut(*i), boxes.get(*i)) else { continue };
        if let ShapeKind::Picture { fill } = &mut s.kind {
            // The source's aspect, from the current box and crop.
            let c = fill.crop;
            let (fw, fh) = ((1.0 - c[0] - c[2]).max(0.01), (1.0 - c[1] - c[3]).max(0.01));
            let src = (old.w / fw) / (old.h / fh).max(1e-6);
            fill.crop = cover_crop(src, x.w / x.h.max(1e-6));
        }
        s.xfrm = Some(Xfrm { rot: 0.0, ..*x });
    }
    for (i, g) in plan.clip.drain(..) {
        if let Some(s) = shapes.get_mut(i) {
            decor::set_geom(s, g);
        }
    }
    for &i in &plan.hide {
        if let Some(s) = shapes.get_mut(i) {
            decor::hide_as_source(s);
        }
    }
    if let Some(t) = shapes.iter_mut().find(|s| !s.hidden && s.ph.as_ref().is_some_and(|p| p.kind.is_title())) {
        if plan.band {
            t.fill = Some(band_fill());
        } else if t.fill.as_ref().is_some_and(is_band) {
            // The band belongs to full-bleed and mosaic only: another idea takes it away again.
            t.fill = None;
        }
    }
    // Back shapes, pictures, shapes between, the rest, front shapes.
    let mut out = Vec::with_capacity(shapes.len() + plan.back.len() + plan.mid.len() + plan.front.len());
    out.append(&mut plan.back);
    let mut rest = vec![];
    for (i, s) in shapes.into_iter().enumerate() {
        if pictures.binary_search(&i).is_ok() { out.push(s) } else { rest.push(s) }
    }
    out.append(&mut plan.mid);
    out.append(&mut rest);
    out.append(&mut plan.front);
    let slide = doc.slides.get_mut(index).ok_or_else(|| bad("design.applyIdea", "no slide"))?;
    std::sync::Arc::make_mut(slide).shapes = out;
    Ok(())
}

fn ideas_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let i = opt_usize_param(p, "slide", "design.ideas")?.unwrap_or(st.selection.slide);
    Ok(json!(
        ideas_for(&st.doc, i).iter().map(|(id, label)| json!({"id": id, "label": label, "variants": variants_of(id).len()})).collect::<Vec<_>>()
    ))
}

fn apply_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let id = str_param(p, "id").ok_or_else(|| bad("design.applyIdea", "missing `id`"))?.to_string();
    let slide = opt_usize_param(p, "slide", "design.applyIdea")?;
    let variant = opt_usize_param(p, "variant", "design.applyIdea")?.unwrap_or(0);
    s.edit(|doc, sel| {
        let i = slide.unwrap_or(sel.slide);
        apply_idea_variant(doc, sel, i, &id, variant)?;
        if i == sel.slide {
            // Shapes the idea hid (split into cards) or took away leave the selection.
            let shown = |sid: ShapeId| doc.slides.get(i).is_some_and(|s| s.shapes.iter().any(|x| x.id == sid && !x.hidden));
            sel.shapes.retain(|sid| shown(*sid));
            if sel.text.as_ref().is_some_and(|t| !shown(t.shape)) {
                sel.text = None;
            }
        }
        Ok(json!({"slide": i, "id": id, "variant": variant}))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;

    use deckcraft_model::style::PictureFill;
    use deckcraft_model::{PhType, Placeholder, ShapeId};

    use super::*;

    fn deck(picture: bool) -> Session {
        let mut s = Session::with_new();
        let st = s.doc_mut().unwrap();
        let doc = Arc::make_mut(&mut st.doc);
        let slide = Arc::make_mut(&mut doc.slides[0]);
        slide.shapes.clear();
        let title = Shape {
            id: ShapeId(500),
            ph: Some(Placeholder { kind: PhType::Title, ..Default::default() }),
            xfrm: Some(Xfrm::new(50.0, 30.0, 860.0, 90.0)),
            text: Some(deckcraft_model::text::TextBody::from_text("Harbour at dawn")),
            ..Default::default()
        };
        let body = Shape {
            id: ShapeId(501),
            text_box: true,
            xfrm: Some(Xfrm::new(50.0, 140.0, 860.0, 350.0)),
            text: Some(deckcraft_model::text::TextBody::from_text("one\ntwo")),
            ..Default::default()
        };
        slide.shapes.push(title);
        slide.shapes.push(body);
        if picture {
            // A 2:1 picture covering most of the slide, over the title.
            let pic = Shape {
                id: ShapeId(502),
                kind: ShapeKind::Picture { fill: PictureFill::default() },
                xfrm: Some(Xfrm::new(0.0, 0.0, 900.0, 450.0)),
                ..Default::default()
            };
            slide.shapes.push(pic);
        }
        s
    }

    fn shape(s: &Session, id: u32) -> Shape {
        s.doc().unwrap().doc.slides[0].shapes.iter().find(|x| x.id == ShapeId(id)).cloned().unwrap()
    }

    fn overlaps(a: Xfrm, b: Xfrm) -> bool {
        a.x < b.x + b.w - 0.5 && b.x < a.x + a.w - 0.5 && a.y < b.y + b.h - 0.5 && b.y < a.y + a.h - 0.5
    }

    #[test]
    fn picture_slides_get_layout_ideas_and_text_slides_get_text_ideas() {
        let s = deck(true);
        let ids: Vec<&str> = ideas_for(&s.doc().unwrap().doc, 0).iter().map(|i| i.0).collect();
        assert_eq!(ids[..6], ["fullBleed", "pictureLeft", "pictureRight", "framed", "pictureTop", "pictureBottom"]);
        assert_eq!(ids.len(), PICTURE_IDEAS.len(), "a two-point text also gets Picture beside numbered points");
        let s = deck(false);
        assert_eq!(ideas_for(&s.doc().unwrap().doc, 0).len(), TEXT_IDEAS.len());
        assert!(ideas_for(&s.doc().unwrap().doc, 9).is_empty());
    }

    #[test]
    fn picture_left_puts_text_beside_the_picture_cropped_to_fill() {
        let mut s = deck(true);
        let theme_before = s.doc().unwrap().doc.masters[0].theme.name.clone();
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        let (pic, title, body) = (shape(&s, 502), shape(&s, 500), shape(&s, 501));
        let px = pic.xfrm.unwrap();
        assert_eq!((px.x, px.y, px.w, px.h), (0.0, 0.0, 480.0, 540.0));
        assert!(!overlaps(px, title.xfrm.unwrap()) && !overlaps(px, body.xfrm.unwrap()));
        assert!(!overlaps(title.xfrm.unwrap(), body.xfrm.unwrap()));
        // 2:1 source into an 8:9 box: cut equally from left and right.
        let ShapeKind::Picture { fill } = &pic.kind else { panic!() };
        assert!(fill.crop[0] > 0.25 && (fill.crop[0] - fill.crop[2]).abs() < 1e-9 && fill.crop[1] == 0.0, "{:?}", fill.crop);
        // Only this slide's arrangement changed, not the theme; undo restores it.
        assert_eq!(s.doc().unwrap().doc.masters[0].theme.name, theme_before);
        assert_eq!(s.doc().unwrap().doc.slides[0].shapes[0].id, ShapeId(502), "picture at the back");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(shape(&s, 502).xfrm.unwrap().w, 900.0);
    }

    #[test]
    fn full_bleed_covers_the_slide_and_puts_the_title_on_a_band() {
        let mut s = deck(true);
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        let px = shape(&s, 502).xfrm.unwrap();
        assert_eq!((px.w, px.h), (960.0, 540.0));
        let t = shape(&s, 500);
        assert!(t.fill.is_some());
        assert!(t.xfrm.unwrap().y > 300.0);
    }

    #[test]
    fn a_title_only_slide_gets_title_ideas() {
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.retain(|x| x.id == ShapeId(500));
        let ids: Vec<&str> = ideas_for(&s.doc().unwrap().doc, 0).iter().map(|i| i.0).collect();
        assert_eq!(ids[..3], ["titleMiddle", "titleLowerThird", "titleTopLeft"]);
        assert_eq!(ids.len(), TITLE_IDEAS.len());
        s.execute("design.applyIdea", &json!({"id": "titleLowerThird"})).unwrap();
        let x = shape(&s, 500).xfrm.unwrap();
        assert!(x.y > 300.0 && x.y + x.h <= 540.01, "{x:?}");
        assert!(s.execute("design.applyIdea", &json!({"id": "centred"})).is_err(), "text ideas need text");
    }

    #[test]
    fn every_idea_keeps_shapes_on_the_slide_and_unknown_ideas_are_refused() {
        for pic in [true, false] {
            let probe = deck(pic);
            for (id, _) in ideas_for(&probe.doc().unwrap().doc, 0) {
                let mut s = deck(pic);
                s.execute("design.applyIdea", &json!({"id": id})).unwrap();
                for sh in &s.doc().unwrap().doc.slides[0].shapes {
                    let x = sh.xfrm.unwrap();
                    assert!(x.x >= -0.01 && x.y >= -0.01 && x.x + x.w <= 960.01 && x.y + x.h <= 540.01 && x.w > 0.0 && x.h > 0.0, "{id}: {x:?}");
                }
            }
        }
        let mut s = deck(false);
        assert!(s.execute("design.applyIdea", &json!({"id": "fullBleed"})).is_err());
        assert!(s.execute("design.applyIdea", &json!({"id": "nope"})).is_err());
    }

    #[test]
    fn an_invalid_slide_index_is_refused_not_replaced_by_the_selection() {
        let mut s = deck(false);
        let before = s.doc().unwrap().doc.clone();
        for bad_slide in [json!(-1), json!(1.5), json!("0")] {
            assert!(s.execute("design.applyIdea", &json!({"id": "centred", "slide": bad_slide})).is_err(), "{bad_slide}");
            assert!(s.execute("design.ideas", &json!({"slide": bad_slide})).is_err(), "{bad_slide}");
        }
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().doc) || *before == *s.doc().unwrap().doc);
        assert!(s.execute("design.applyIdea", &json!({"id": "centred", "slide": 0})).is_ok());
        assert!(s.execute("design.ideas", &json!({})).is_ok());
    }

    #[test]
    fn switching_from_full_bleed_removes_the_title_band() {
        let mut s = deck(true);
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        assert_eq!(shape(&s, 500).fill, Some(band_fill()));
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        assert!(shape(&s, 500).fill.is_none(), "the band belongs to full-bleed only");
    }

    #[test]
    fn the_band_goes_on_the_visible_title_not_a_hidden_one_before_it() {
        let mut s = deck(true);
        let st = s.doc_mut().unwrap();
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut st.doc).slides[0]);
        let hidden = Shape {
            id: ShapeId(503),
            hidden: true,
            ph: Some(Placeholder { kind: PhType::Title, ..Default::default() }),
            xfrm: Some(Xfrm::new(50.0, 30.0, 860.0, 90.0)),
            text: Some(deckcraft_model::text::TextBody::from_text("Old title")),
            ..Default::default()
        };
        slide.shapes.insert(0, hidden);
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        assert_eq!(shape(&s, 500).fill, Some(band_fill()), "visible title gets the band");
        assert!(shape(&s, 503).fill.is_none(), "hidden title is untouched");
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        assert!(shape(&s, 500).fill.is_none(), "switching clears the visible title's band");
        assert!(shape(&s, 503).fill.is_none());
    }

    #[test]
    fn switching_ideas_removes_the_band_after_a_pptx_round_trip() {
        let mut s = deck(true);
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        let bytes = deckcraft_pptx::export(&s.doc().unwrap().doc).unwrap();
        let back = deckcraft_pptx::import(&bytes).unwrap();
        s.doc_mut().unwrap().doc = Arc::new(back);
        let title = s.doc().unwrap().doc.slides[0].shapes.iter().position(|x| x.ph.as_ref().is_some_and(|p| p.kind.is_title())).unwrap();
        let id = s.doc().unwrap().doc.slides[0].shapes[title].id.0;
        assert_eq!(shape(&s, id).fill, Some(band_fill()), "band survives the save");
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        assert!(shape(&s, id).fill.is_none(), "the band is removed after reopening");
    }

    #[test]
    fn switching_ideas_removes_a_band_saved_by_earlier_versions() {
        let mut s = deck(true);
        let legacy = Fill::solid(deckcraft_model::ColorRef::rgb(Rgba::rgba(255, 255, 255, 220)));
        let st = s.doc_mut().unwrap();
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut st.doc).slides[0]);
        slide.shapes.iter_mut().find(|x| x.id == ShapeId(500)).unwrap().fill = Some(legacy);
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        assert!(shape(&s, 500).fill.is_none());
    }

    #[test]
    fn a_title_fill_set_by_the_user_survives_other_ideas() {
        // Set through the command, and set to exactly the band's own fill.
        let mut s = deck(true);
        s.execute("shape.fill", &json!({"id": 500, "color": "#ffffff", "transparency": 0.14})).unwrap();
        let user = shape(&s, 500).fill;
        assert!(user.is_some());
        s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
        assert_eq!(shape(&s, 500).fill, user);
    }

    #[test]
    fn full_bleed_twice_keeps_the_band_and_undo_after_switching_brings_it_back() {
        let mut s = deck(true);
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        s.execute("design.applyIdea", &json!({"id": "fullBleed"})).unwrap();
        assert_eq!(shape(&s, 500).fill, Some(band_fill()));
        s.execute("design.applyIdea", &json!({"id": "framed"})).unwrap();
        assert!(shape(&s, 500).fill.is_none());
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(shape(&s, 500).fill, Some(band_fill()));
    }

    #[test]
    fn slides_with_diagrams_or_other_graphics_get_no_ideas() {
        // A filled autoshape with text (a diagram box) is not text to restack.
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.push(Shape {
            id: ShapeId(510),
            xfrm: Some(Xfrm::new(300.0, 200.0, 200.0, 80.0)),
            fill: Some(Fill::solid(deckcraft_model::ColorRef::rgb(Rgba::rgba(200, 80, 20, 255)))),
            text: Some(deckcraft_model::text::TextBody::from_text("Solar wind")),
            ..Default::default()
        });
        assert!(ideas_for(&s.doc().unwrap().doc, 0).is_empty());
        assert!(s.execute("design.applyIdea", &json!({"id": "titleColumn"})).is_err());
        // A connector too, and a hidden one does not count.
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.push(Shape {
            id: ShapeId(511),
            kind: ShapeKind::Connector { start: None, end: None },
            xfrm: Some(Xfrm::new(300.0, 200.0, 100.0, 0.0)),
            ..Default::default()
        });
        assert!(ideas_for(&s.doc().unwrap().doc, 0).is_empty());
        Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]).shapes[2].hidden = true;
        assert_eq!(ideas_for(&s.doc().unwrap().doc, 0).len(), TEXT_IDEAS.len());
    }

    #[test]
    fn two_visible_pictures_get_multi_picture_ideas_but_a_hidden_second_one_does_not_count() {
        let mut s = deck(true);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.push(Shape {
            id: ShapeId(503),
            kind: ShapeKind::Picture { fill: PictureFill::default() },
            xfrm: Some(Xfrm::new(600.0, 300.0, 200.0, 100.0)),
            ..Default::default()
        });
        let ids: Vec<&str> = ideas_for(&s.doc().unwrap().doc, 0).iter().map(|i| i.0).collect();
        assert_eq!(ids, ["grid", "collage", "filmstrip"], "no mosaic: the slide has body text");
        assert!(s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).is_err());
        Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]).shapes[3].hidden = true;
        assert_eq!(ideas_for(&s.doc().unwrap().doc, 0).len(), PICTURE_IDEAS.len());
    }

    #[test]
    fn an_empty_text_box_blocks_ideas_only_when_it_shows() {
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.push(Shape { id: ShapeId(512), text_box: true, xfrm: Some(Xfrm::new(10.0, 10.0, 100.0, 40.0)), ..Default::default() });
        assert_eq!(ideas_for(&s.doc().unwrap().doc, 0).len(), TEXT_IDEAS.len(), "invisible and empty: nothing in the way");
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes[2].fill = Some(Fill::solid(deckcraft_model::ColorRef::rgb(Rgba::rgba(200, 80, 20, 255))));
        assert!(ideas_for(&s.doc().unwrap().doc, 0).is_empty(), "a filled empty box would stay on top");
    }

    #[test]
    fn an_empty_box_with_an_explicit_no_fill_does_not_block_ideas() {
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        slide.shapes.push(Shape {
            id: ShapeId(512),
            text_box: true,
            xfrm: Some(Xfrm::new(10.0, 10.0, 100.0, 40.0)),
            fill: Some(Fill::None),
            ..Default::default()
        });
        assert_eq!(ideas_for(&s.doc().unwrap().doc, 0).len(), TEXT_IDEAS.len(), "an explicit no fill shows nothing");
    }

    #[test]
    fn an_empty_box_filled_through_its_style_blocks_ideas() {
        let mut s = deck(false);
        let slide = Arc::make_mut(&mut Arc::make_mut(&mut s.doc_mut().unwrap().doc).slides[0]);
        let mut style = deckcraft_model::style::ShapeStyle::accent(deckcraft_model::SchemeSlot::Accent1);
        style.line_ref.0 = 0;
        slide.shapes.push(Shape {
            id: ShapeId(512),
            text_box: true,
            xfrm: Some(Xfrm::new(10.0, 10.0, 100.0, 40.0)),
            style: Some(style),
            ..Default::default()
        });
        assert!(ideas_for(&s.doc().unwrap().doc, 0).is_empty(), "the theme fill shows, so the box would stay on top");
    }

    #[test]
    fn cover_crop_matches_the_box_aspect() {
        let c = cover_crop(2.0, 1.0);
        assert!((c[0] - 0.25).abs() < 1e-12 && c[1] == 0.0);
        let c = cover_crop(1.0, 2.0);
        assert!((c[1] - 0.25).abs() < 1e-12 && c[0] == 0.0);
        assert_eq!(cover_crop(f64::NAN, 1.0), [0.0; 4]);
    }
}
