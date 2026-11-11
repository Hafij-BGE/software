use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use deckcraft_engine::cmd::ideas;
use deckcraft_model::{Presentation, ShapeKind};

use super::{Color32, CornerRadius, Rect, Sense, SlideApp, Stroke, Tokens, Ui, json, pos2, run, vec2};

/// Thumbnails rendered per frame at most, so opening the pane on a slide with many ideas stays
/// responsive (one render costs a few milliseconds); the rest show a placeholder until their turn.
const RENDERS_PER_FRAME: usize = 3;

/// Rendered thumbnails by name (`layout:{id}:{variant}`, `variant:{i}`, `theme:{name}`), valid for
/// one deck revision, slide and pane width. Only thumbnails scrolled into view are rendered.
#[derive(Clone, Default)]
struct IdeaThumbs {
    key: (u64, u64, usize, u32),
    tex: Arc<Mutex<HashMap<String, (egui::TextureHandle, u32, u32)>>>,
    /// The slide as it is, rendered at thumbnail size, to tell which whole-presentation
    /// previews change nothing.
    base: Arc<Mutex<Option<Vec<u8>>>>,
    /// Whole-presentation previews that look exactly like the slide as it is: a slide with its
    /// own fixed colours and fonts ignores theme colours and fonts, so these are left out.
    same: Arc<Mutex<HashSet<String>>>,
}

/// What the pane remembers between frames: the colour chip under the pointer (its idea is
/// previewed in that colour) and why the pane opened itself, if it did.
#[derive(Clone, Default)]
struct PaneState {
    hover: Option<(String, usize)>,
    opened_for: Option<&'static str>,
}

fn state_id() -> egui::Id {
    egui::Id::unique("design-ideas-pane")
}

/// Render per-slide Design Ideas (with colour variants) and whole-presentation theme thumbnails;
/// a click runs `design.applyIdea`, `design.variant` or `design.theme`.
pub(super) fn designer(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let mut pane: PaneState = ui.ctx().data_mut(|d| d.get_temp(state_id())).unwrap_or_default();
    if let Some(why) = pane.opened_for {
        let mut close = false;
        egui::Frame::NONE.fill(t.hover).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Designer opened because {why}."));
                close = ui.small_button("Dismiss").clicked();
            });
        });
        if close {
            pane.opened_for = None;
        }
        ui.add_space(6.0);
    }
    ui.label(egui::RichText::new("Pick a design for this slide. Ideas are generated on your device.").color(t.text_dim));
    ui.checkbox(&mut app.ui.design_ideas_auto, "Show me design ideas automatically");
    ui.add_space(6.0);
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let idx = st.selection.slide;
    let sel = st.selection.clone();
    let w = ui.available_width() - 8.0;
    let ppp = ui.ctx().pixels_per_point();
    let px = (w * ppp).round().max(16.0) as u32;
    let key = (st.uid, st.revision, idx, px);
    let found = ideas::ideas_for(&doc, idx);
    let themes = deckcraft_model::theme::builtin_themes();
    // The slide's own master, which Design › Variants and Themes change (not always the first).
    let mi = master_of(&doc, idx);
    // The current theme in its other colours, as Design › Variants offers them.
    let current_colors = doc.masters.get(mi).map(|m| m.theme.colors.clone());
    let all_variants = doc.masters.get(mi).map(|m| deckcraft_model::theme::theme_variants(&m.theme)).unwrap_or_default();
    let variants: Vec<_> = all_variants.iter().filter(|v| Some(&v.colors) != current_colors.as_ref().map(|c| &c.colors)).cloned().collect();
    let scheme = super::scheme(app);
    let cache_id = egui::Id::unique("design-idea-thumbs");
    let mut cache: IdeaThumbs = ui.ctx().data_mut(|d| d.get_temp(cache_id)).unwrap_or_default();
    if cache.key != key {
        cache = IdeaThumbs { key, ..Default::default() };
        ui.ctx().data_mut(|d| d.insert_temp(cache_id, cache.clone()));
    }
    let mut budget = RENDERS_PER_FRAME;
    let mut pending = false;
    // The slide as it would look under `name`, for its thumbnail.
    let build = |name: &str| -> Option<Presentation> {
        let mut d = (*doc).clone();
        if let Some(rest) = name.strip_prefix("layout:") {
            let (id, v) = rest.rsplit_once(':')?;
            ideas::apply_idea_variant(&mut d, &sel, idx, id, v.parse().ok()?).ok()?;
        } else if let Some(i) = name.strip_prefix("variant:") {
            let colors = variants.get(i.parse::<usize>().ok()?)?;
            Arc::make_mut(d.masters.get_mut(mi)?).theme.colors = colors.clone();
        } else {
            // As `design.theme` applies it.
            let th = themes.iter().find(|th| Some(th.name.as_str()) == name.strip_prefix("theme:"))?;
            let m = Arc::make_mut(d.masters.get_mut(mi)?);
            m.theme = th.clone();
            m.name = th.name.clone();
            m.color_map = vec![];
        }
        Some(d)
    };
    // One thumbnail, full pane width: rendered when first scrolled into view, a placeholder
    // until then.
    let mut thumb = |ui: &mut Ui, name: &str, tip: &str| -> egui::Response {
        let aspect = (doc.slide_size.height / doc.slide_size.width.max(1.0)) as f32;
        let (r, resp) = ui.allocate_exact_size(vec2(w, w * aspect), Sense::click());
        let visible = ui.is_rect_visible(r);
        let mut have = cache.tex.lock().ok().and_then(|m| m.get(name).cloned());
        if have.is_none() && visible {
            if budget == 0 {
                pending = true;
            } else {
                budget -= 1;
                have = build(name).map(|d| {
                    let (tw, th) = crate::textures::fit_size(
                        (px, (px as f64 * d.slide_size.height / d.slide_size.width.max(1.0)).clamp(1.0, 1e9) as u32),
                        crate::textures::max_side(ui.ctx()),
                    );
                    let opts =
                        deckcraft_render::RenderOpts { scale: tw as f64 / d.slide_size.width.max(1.0), size: Some((tw, th)), ..Default::default() };
                    let img = deckcraft_render::render_slide(&d, idx, &opts);
                    if !name.starts_with("layout:") {
                        let mut base = cache.base.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        let base = base.get_or_insert_with(|| deckcraft_render::render_slide(&doc, idx, &opts).pixels);
                        if *base == img.pixels {
                            cache.same.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(name.to_string());
                            // Drawn this frame; gone next frame.
                            pending = true;
                        }
                    }
                    let tex = crate::textures::upload_editor(ui.ctx(), format!("idea-{name}"), crate::textures::to_color_image(&img, false));
                    let e = (tex, tw, th);
                    if let Ok(mut m) = cache.tex.lock() {
                        m.insert(name.to_string(), e.clone());
                    }
                    e
                });
            }
        }
        let at = match &have {
            Some((tex, tw, th)) => {
                // On the physical pixel grid, one texel per device pixel (#184).
                let at = crate::textures::device_pixel_rect(r, ppp);
                let at = Rect::from_min_size(at.min, vec2(*tw as f32 / ppp, *th as f32 / ppp));
                ui.painter().image(tex.id(), at, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                at
            }
            None => {
                ui.painter().rect_filled(r, CornerRadius::same(2), t.hover);
                r
            }
        };
        ui.painter().rect_stroke(
            at,
            CornerRadius::same(2),
            Stroke::new(if resp.hovered() { 2.5 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border }),
            egui::StrokeKind::Outside,
        );
        resp.on_hover_text(tip)
    };
    if found.is_empty() {
        ui.label(egui::RichText::new("Add a title with a picture or some text to get layout ideas.").color(t.text_dim));
    }
    let mut hover = None;
    let mut apply: Option<(&str, usize)> = None;
    for (id, label) in &found {
        let slots = ideas::variants_of(id);
        // A hovered colour chip previews its variant in the thumbnail above it.
        let shown = pane.hover.as_ref().filter(|(h, _)| h == id).map_or(0, |h| h.1);
        if thumb(ui, &format!("layout:{id}:{shown}"), label).clicked() {
            apply = Some((id, shown));
        }
        if slots.len() > 1 {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (v, slot) in slots.iter().enumerate() {
                    let c = scheme.get(*slot);
                    let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 7.0, Color32::from_rgb(c.r, c.g, c.b));
                    let (width, color) = if resp.hovered() || shown == v { (1.5, t.accent) } else { (1.0, t.border) };
                    ui.painter().circle_stroke(r.center(), 7.5, Stroke::new(width, color));
                    if resp.hovered() {
                        hover = Some((id.to_string(), v));
                    }
                    if resp.on_hover_text(format!("{label}, accent colour {}", v + 1)).clicked() {
                        apply = Some((id, v));
                    }
                }
            });
        }
        ui.add_space(8.0);
    }
    // Whole-presentation previews that would look like the slide as it is are left out.
    let same = cache.same.lock().map(|s| s.clone()).unwrap_or_default();
    let shown_variants: Vec<_> = (0..variants.len()).filter(|i| !same.contains(&format!("variant:{i}"))).collect();
    let shown_themes: Vec<_> = themes.iter().filter(|th| !same.contains(&format!("theme:{}", th.name))).collect();
    if shown_variants.is_empty() && shown_themes.is_empty() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "This slide uses its own fixed colours and fonts, so the presentation's colour variants and themes don't change how it looks.",
            )
            .color(t.text_dim),
        );
    }
    if !shown_variants.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Colours (whole presentation)").color(t.text_dim));
        ui.add_space(4.0);
    }
    let mut variant = None;
    for i in shown_variants {
        let v = &variants[i];
        if thumb(ui, &format!("variant:{i}"), &v.name).clicked() {
            variant = all_variants.iter().position(|a| a.colors == v.colors);
        }
        ui.add_space(8.0);
    }
    if !shown_themes.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Themes (whole presentation)").color(t.text_dim));
        ui.add_space(4.0);
    }
    let mut theme = None;
    for th in shown_themes {
        if thumb(ui, &format!("theme:{}", th.name), &th.name).clicked() {
            theme = Some(th.name.clone());
        }
        ui.add_space(8.0);
    }
    if let Some((id, variant)) = apply {
        run(app, "design.applyIdea", json!({"id": id, "variant": variant}));
        pane.opened_for = None;
    }
    if let Some(index) = variant {
        run(app, "design.variant", json!({"index": index}));
    }
    if let Some(name) = theme {
        run(app, "design.theme", json!({"name": name}));
    }
    if pending || hover != pane.hover {
        ui.ctx().request_repaint();
    }
    pane.hover = hover;
    ui.ctx().data_mut(|d| d.insert_temp(state_id(), pane));
}

/// The master whose layout slide `idx` uses, as `design.variant` and `design.theme` pick it.
fn master_of(doc: &Presentation, idx: usize) -> usize {
    doc.slides.get(idx).and_then(|s| doc.masters.iter().position(|m| m.layout(s.layout).is_some())).unwrap_or(0)
}

/// What auto-suggest has seen of a slide: its picture count, whether it was seen without both a
/// title and content (so filling it in is news), and whether that was suggested already.
#[derive(Clone, Copy)]
struct Seen {
    pictures: usize,
    was_empty: bool,
    suggested: bool,
}

#[derive(Clone, Default)]
struct AutoState {
    last: (u64, u64, usize),
    slides: HashMap<(u64, u32), Seen>,
}

/// Open the Designer by itself, as PowerPoint does, when a picture lands on the current slide or a
/// new slide gets both a title and content: if "Show me design ideas automatically" is on, no
/// other pane is open, and there are ideas for the slide. Checked once per deck revision.
pub(crate) fn auto_suggest(app: &mut SlideApp, ctx: &egui::Context) {
    let Some(st) = app.session.active() else { return };
    let idx = st.selection.slide;
    let key = (st.uid, st.revision, idx);
    let id = egui::Id::unique("design-ideas-auto");
    let mut auto: AutoState = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    if auto.last == key {
        return;
    }
    let Some(slide) = st.doc.slides.get(idx) else { return };
    let visible = || slide.shapes.iter().filter(|s| !s.hidden && !s.decorative);
    let pictures = visible().filter(|s| matches!(s.kind, ShapeKind::Picture { .. })).count();
    let filled = |title: bool| {
        visible().any(|s| s.ph.as_ref().is_some_and(|p| p.kind.is_title()) == title && s.text.as_ref().is_some_and(|t| !t.text().trim().is_empty()))
    };
    let full = filled(true) && filled(false);
    let editing = st.selection.text.is_some();
    let slot = (st.uid, slide.id.0);
    let mut why = None;
    match auto.slides.get_mut(&slot) {
        // The first look at a slide is the baseline: opening a deck suggests nothing.
        None => {
            auto.slides.insert(slot, Seen { pictures, was_empty: !full, suggested: false });
        }
        Some(seen) => {
            if pictures > seen.pictures {
                why = Some("a picture was added");
            } else if full && seen.was_empty && !seen.suggested && !editing {
                why = Some("the slide has a title and content");
                seen.suggested = true;
            }
            seen.pictures = pictures;
            seen.was_empty |= !full;
        }
    }
    // While text is being typed, look again at every revision until it is done.
    if !editing {
        auto.last = key;
    }
    if let Some(why) = why
        && app.ui.design_ideas_auto
        && app.ui.pane.is_none()
        && !ideas::ideas_for(&st.doc, idx).is_empty()
    {
        app.ui.pane = Some("designer".into());
        let mut pane: PaneState = ctx.data_mut(|d| d.get_temp(state_id())).unwrap_or_default();
        pane.opened_for = Some(why);
        ctx.data_mut(|d| d.insert_temp(state_id(), pane));
    }
    ctx.data_mut(|d| d.insert_temp(id, auto));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use deckcraft_engine::Session;
    use serde_json::json;

    use super::auto_suggest;
    use crate::{Services, SlideApp};

    /// Run auto-suggest with no pane open; the pane it opened, if any.
    fn opened(app: &mut SlideApp, ctx: &egui::Context) -> Option<String> {
        app.ui.pane = None;
        auto_suggest(app, ctx);
        app.ui.pane.clone()
    }

    fn picture(app: &mut SlideApp) {
        let png = deckcraft_render::Image { width: 4, height: 2, pixels: [255, 0, 0, 255].repeat(8) }.to_png();
        let data = deckcraft_engine::cmd::base64_encode(&png);
        app.session.execute("insert.picture", &json!({"data": data, "name": "red.png", "rect": [500, 150, 300, 150]})).unwrap();
    }

    #[test]
    fn a_new_picture_opens_the_designer_once_and_only_when_wanted() {
        let ctx = egui::Context::default();
        let mut s = Session::with_new();
        {
            let doc = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc);
            let slide = std::sync::Arc::make_mut(&mut doc.slides[0]);
            slide.shapes.retain(|x| x.ph.as_ref().is_some_and(|p| p.kind.is_title()));
            slide.shapes[0].text = Some(deckcraft_model::text::TextBody::from_text("Harbour"));
        }
        let mut app = SlideApp::new(s, Services::default());
        assert_eq!(opened(&mut app, &ctx), None, "the first look at a slide is a baseline");
        picture(&mut app);
        assert_eq!(opened(&mut app, &ctx).as_deref(), Some("designer"));
        assert_eq!(opened(&mut app, &ctx), None, "same revision, no second open");
        // Another pane open: left alone.
        app.ui.pane = Some("format".into());
        picture(&mut app);
        auto_suggest(&mut app, &ctx);
        assert_eq!(app.ui.pane.as_deref(), Some("format"));
        // Turned off: nothing opens.
        app.ui.design_ideas_auto = false;
        picture(&mut app);
        assert_eq!(opened(&mut app, &ctx), None);
    }

    /// The whole-presentation previews the Designer left out on the current slide, after enough
    /// frames to render every thumbnail.
    fn unchanged_previews(app: &mut SlideApp) -> std::collections::HashSet<String> {
        let ctx = egui::Context::default();
        let input =
            || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 20000.0))), ..Default::default() };
        for _ in 0..60 {
            ctx.run_ui(input(), |ui| super::designer(app, ui)).drop_without_applying_deltas();
        }
        let cache: super::IdeaThumbs = ctx.data_mut(|d| d.get_temp(egui::Id::unique("design-idea-thumbs"))).unwrap();
        cache.same.lock().unwrap().clone()
    }

    #[test]
    fn previews_that_change_nothing_are_left_out() {
        // A shape in the theme's accent colour: every variant recolours it.
        let mut app = SlideApp::new(Session::with_new(), Services::default());
        app.session.execute("shape.insert", &json!({"preset": "rect", "rect": [100, 100, 300, 100]})).unwrap();
        let same = unchanged_previews(&mut app);
        assert!(!same.iter().any(|n| n.starts_with("variant:")), "{same:?}");
        // A slide in its own fixed colours, with nothing from the master: every variant and
        // theme looks the same, so none is offered.
        {
            let doc = std::sync::Arc::make_mut(&mut app.session.doc_mut().unwrap().doc);
            std::sync::Arc::make_mut(&mut doc.slides[0]).shapes.clear();
        }
        app.session.execute("design.background", &json!({"color": "#FFFFFF"})).unwrap();
        app.session.execute("design.hideBackgroundGraphics", &json!({"hide": true})).unwrap();
        let same = unchanged_previews(&mut app);
        let themes = deckcraft_model::theme::builtin_themes().len();
        assert_eq!(same.iter().filter(|n| n.starts_with("theme:")).count(), themes, "{same:?}");
        assert_eq!(same.iter().filter(|n| n.starts_with("variant:")).count(), 3, "{same:?}");
    }

    #[test]
    fn the_preference_defaults_on_and_round_trips() {
        let ui = crate::state::UiState::default();
        assert!(ui.design_ideas_auto);
        let mut v = serde_json::to_value(&ui).unwrap();
        v["designIdeasAuto"] = json!(false);
        let back: crate::state::UiState = serde_json::from_value(v.clone()).unwrap();
        assert!(!back.design_ideas_auto);
        v.as_object_mut().unwrap().remove("designIdeasAuto");
        let old: crate::state::UiState = serde_json::from_value(v).unwrap();
        assert!(old.design_ideas_auto, "older settings files turn it on");
    }
}
