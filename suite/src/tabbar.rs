//! The tab bar: along the top or bottom (one row, or a row per app) or down a side, with app
//! icons and colours, drag to reorder, pinning, per-tab colours and a context menu.
//!
//! It draws with its own light or dark visuals (the suite's theme) and only the Proportional
//! font family, which every app's fonts define, so it looks the same whichever app is shown.
//!
//! With tabs in the title bar (Settings › Window) the bar also moves the window (drag its empty
//! space), maximizes it (double-click) and has the minimize, maximize and close buttons.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::app::{Action, SuiteApp};
use crate::kind::Kind;
use crate::settings::{CloseButtons, ColorStyle, Mixing, Rgb, TabPosition};

#[derive(Default)]
pub struct TabBarState {
    /// The tab being dragged.
    drag: Option<u64>,
}

/// A row of tabs along the top or bottom: the tabs, and a gap above them (below them at the
/// bottom) that moves the window.
const ROW_H: f32 = 40.0;
const GAP: f32 = 6.0;
const SIDE_TAB_H: f32 = 34.0;
/// The + and menu buttons.
const BUTTON_W: f32 = 32.0;
/// A window button (minimize, maximize, close).
const CONTROL_W: f32 = 46.0;
/// Empty bar kept free to drag the window by, beside the window buttons.
const DRAG_MIN: f32 = 56.0;

/// The suite's colours: the tab bar, the active tab (the same as the Home and Settings pages
/// under it), and the pages' side bar and cards.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bar: Color32,
    pub page: Color32,
    pub hover: Color32,
    pub separator: Color32,
    pub side: Color32,
    pub card: Color32,
    pub border: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bar: Color32::from_gray(22),
            page: Color32::from_gray(34),
            hover: Color32::from_gray(30),
            separator: Color32::from_gray(62),
            side: Color32::from_gray(28),
            card: Color32::from_gray(42),
            border: Color32::from_gray(56),
        }
    } else {
        Palette {
            bar: Color32::from_rgb(222, 225, 230),
            page: Color32::from_rgb(250, 250, 251),
            hover: Color32::from_rgb(234, 236, 240),
            separator: Color32::from_rgb(184, 188, 196),
            side: Color32::from_rgb(242, 243, 246),
            card: Color32::WHITE,
            border: Color32::from_rgb(222, 225, 231),
        }
    }
}

pub fn is_dark(app: &SuiteApp, ctx: &egui::Context) -> bool {
    match app.settings.theme {
        crate::settings::SuiteTheme::Light => false,
        crate::settings::SuiteTheme::Dark => true,
        crate::settings::SuiteTheme::System => ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(false),
    }
}

/// Colours offered for a tab, besides its app's.
const SWATCHES: [[u8; 3]; 10] = [
    [0xe0, 0x31, 0x31],
    [0xf0, 0x8c, 0x00],
    [0xf5, 0xc5, 0x18],
    [0x2f, 0x9e, 0x44],
    [0x0c, 0xa6, 0x78],
    [0x10, 0x98, 0xad],
    [0x1c, 0x7e, 0xd6],
    [0x70, 0x48, 0xe8],
    [0xd6, 0x33, 0x6c],
    [0x86, 0x8e, 0x96],
];

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Black or white, whichever reads better on `bg`.
fn contrast(bg: Color32) -> Color32 {
    let lum = 0.2126 * bg.r() as f32 + 0.7152 * bg.g() as f32 + 0.0722 * bg.b() as f32;
    if lum > 150.0 { Color32::from_gray(20) } else { Color32::WHITE }
}

fn bar_visuals(app: &SuiteApp, ctx: &egui::Context) -> egui::Visuals {
    if is_dark(app, ctx) { egui::Visuals::dark() } else { egui::Visuals::light() }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Glyph {
    Plus,
    Close,
    Menu,
    Minimize,
    Maximize,
    Restore,
}

pub fn paint_glyph(painter: &egui::Painter, rect: Rect, glyph: Glyph, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.28;
    let stroke = Stroke::new(1.6, color);
    let thin = Stroke::new(1.0, color);
    match glyph {
        Glyph::Plus => {
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
            painter.line_segment([pos2(c.x, c.y - r), pos2(c.x, c.y + r)], stroke);
        }
        Glyph::Close => {
            let r = r * 0.8;
            painter.line_segment([pos2(c.x - r, c.y - r), pos2(c.x + r, c.y + r)], stroke);
            painter.line_segment([pos2(c.x - r, c.y + r), pos2(c.x + r, c.y - r)], stroke);
        }
        Glyph::Menu => {
            for dy in [-r * 0.8, 0.0, r * 0.8] {
                painter.line_segment([pos2(c.x - r, c.y + dy), pos2(c.x + r, c.y + dy)], stroke);
            }
        }
        Glyph::Minimize => {
            painter.line_segment([pos2(c.x - 5.0, c.y + 0.5), pos2(c.x + 5.0, c.y + 0.5)], thin);
        }
        Glyph::Maximize => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0)), 1.0, thin, egui::StrokeKind::Inside);
        }
        Glyph::Restore => {
            let back = Rect::from_center_size(c + vec2(2.0, -2.0), Vec2::splat(8.0));
            painter.line_segment([back.left_top(), back.right_top()], thin);
            painter.line_segment([back.right_top(), back.right_bottom()], thin);
            painter.rect_stroke(Rect::from_center_size(c + vec2(-1.0, 1.0), Vec2::splat(8.0)), 1.0, thin, egui::StrokeKind::Inside);
        }
    }
}

/// Where a tab (or a + / menu button) is drawn in its slot: below the gap that moves the window
/// when tabs are along the top, above it at the bottom.
fn tab_area(app: &SuiteApp, slot: Rect) -> Rect {
    match app.settings.tab_position {
        TabPosition::Top => Rect::from_min_max(slot.min + vec2(0.0, GAP), slot.max),
        TabPosition::Bottom => Rect::from_min_max(slot.min, slot.max - vec2(0.0, GAP)),
        TabPosition::Left | TabPosition::Right => slot,
    }
}

/// Minimize, maximize (or restore) and close, flush with the window's top-right corner; laid
/// out right to left.
fn window_controls(ui: &mut Ui, height: f32) {
    let ctx = ui.ctx().clone();
    let maximized = SuiteApp::maximized(&ctx);
    let text = ui.visuals().text_color();
    let hover = ui.visuals().widgets.hovered.weak_bg_fill;
    ui.spacing_mut().item_spacing.x = 0.0;
    for glyph in [Glyph::Close, if maximized { Glyph::Restore } else { Glyph::Maximize }, Glyph::Minimize] {
        let (rect, resp) = ui.allocate_exact_size(vec2(CONTROL_W, height), Sense::click());
        let close = glyph == Glyph::Close;
        let mut color = text;
        if resp.hovered() {
            let bg = if close { Color32::from_rgb(0xc4, 0x2b, 0x1c) } else { hover };
            ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
            if close {
                color = Color32::WHITE;
            }
        }
        paint_glyph(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(20.0)), glyph, color);
        let tip = match glyph {
            Glyph::Close => "Close",
            Glyph::Minimize => "Minimize",
            Glyph::Restore => "Restore",
            _ => "Maximize",
        };
        if resp.on_hover_text(tip).clicked() {
            ctx.send_viewport_cmd(match glyph {
                Glyph::Close => egui::ViewportCommand::Close,
                Glyph::Minimize => egui::ViewportCommand::Minimized(true),
                _ => egui::ViewportCommand::Maximized(!maximized),
            });
        }
    }
}

/// Dragging `resp` moves the window; double-clicking it maximizes or restores the window.
fn window_drag(ui: &Ui, resp: &egui::Response) {
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let maximized = SuiteApp::maximized(ui.ctx());
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}

/// The title bar when the tabs aren't along the top: the suite's icon, the window title and
/// the window buttons.
fn title_row(app: &mut SuiteApp, ui: &mut Ui, visuals: &egui::Visuals, fill: Color32) {
    const H: f32 = 36.0;
    egui::Panel::top("craft_suite_title")
        .frame(egui::Frame::NONE.fill(fill))
        .resizable(false)
        .exact_size(H)
        .show(ui, |ui| {
            *ui.visuals_mut() = visuals.clone();
            let bg = ui.interact(ui.max_rect(), ui.id().with("title_bg"), Sense::click_and_drag());
            window_drag(ui, &bg);
            bg.context_menu(|ui| new_menu(app, ui));
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                if let Some(icon) = app.icon(Kind::Home) {
                    ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(18.0)));
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(app.title_sent.clone()).size(13.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| window_controls(ui, H));
            });
        });
}

/// A small button with a drawn glyph (no font glyphs: the apps' fonts may lack them), in a
/// `size` slot of the tab bar; it is drawn where the tabs are (see `tab_area`).
fn glyph_button(app: &SuiteApp, ui: &mut Ui, glyph: Glyph, size: Vec2, tip: &str) -> egui::Response {
    let (_, slot) = ui.allocate_space(size);
    let area = tab_area(app, slot);
    let rect = Rect::from_center_size(area.center(), Vec2::splat(area.height().min(area.width()) - 4.0));
    let resp = ui.interact(rect, ui.id().with(("craft_suite_glyph", glyph as u8, slot.min.x as i32, slot.min.y as i32)), Sense::click());
    let v = ui.visuals();
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), v.widgets.hovered.weak_bg_fill);
    }
    paint_glyph(ui.painter(), rect, glyph, v.text_color());
    resp.on_hover_text(tip)
}

/// The New menu: every kind of tab, and Open.
fn new_menu(app: &mut SuiteApp, ui: &mut Ui) {
    for k in app.settings.app_order.clone() {
        if ui.button(format!("New {}  ({})", k.noun(), k.app_name())).clicked() {
            app.actions.push(Action::New { kind: k, replace_page: false });
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Open…").clicked() {
        app.actions.push(Action::OpenDialog);
        ui.close();
    }
    if ui.button("Home").clicked() {
        app.actions.push(Action::Home);
        ui.close();
    }
    if ui.button("Settings").clicked() {
        app.actions.push(Action::Settings);
        ui.close();
    }
}

pub fn show(app: &mut SuiteApp, ui: &mut Ui) {
    let visuals = bar_visuals(app, ui.ctx());
    let mut fill = palette(visuals.dark_mode).bar;
    if app.settings.tint_bar_with_active
        && let Some(t) = app.tab(app.active)
    {
        fill = mix(fill, app.tab_color(t), 0.2);
    }
    let integrated = app.integrated();
    let top = app.settings.tab_position == TabPosition::Top;
    if integrated && !top {
        title_row(app, ui, &visuals, fill);
    }
    let frame = match app.settings.tab_position {
        // Along the top, the window buttons sit flush with the window's corner.
        TabPosition::Top => egui::Frame::NONE.fill(fill).inner_margin(egui::Margin { left: 8, right: if integrated { 0 } else { 6 }, top: 0, bottom: 0 }),
        TabPosition::Bottom => egui::Frame::NONE.fill(fill).inner_margin(egui::Margin::symmetric(8, 0)),
        TabPosition::Left | TabPosition::Right => egui::Frame::NONE.fill(fill).inner_margin(egui::Margin::same(8)),
    };
    let mut rects: Vec<(u64, Rect)> = Vec::new();
    let vertical = app.settings.tab_position.vertical();
    let id = egui::Id::new("craft_suite_tabs");
    let body = |app: &mut SuiteApp, ui: &mut Ui, rects: &mut Vec<(u64, Rect)>| {
        *ui.visuals_mut() = visuals.clone();
        ui.style_mut().override_font_id = Some(egui::FontId::proportional(13.0));
        // The bar's empty space: with tabs in the title bar along the top it moves and
        // maximizes the window; otherwise double-clicking it opens a new tab. Added first, so
        // tabs and buttons (added after it) take their own clicks.
        let bg = ui.interact(ui.max_rect(), ui.id().with("bar_bg"), Sense::click_and_drag());
        if integrated && top {
            window_drag(ui, &bg);
        } else if bg.double_clicked() && app.settings.double_click_bar_new_tab {
            app.actions.push(Action::NewDefault);
        }
        bg.context_menu(|ui| new_menu(app, ui));
        if vertical { side_strip(app, ui, rects) } else { rows(app, ui, rects) }
    };
    match app.settings.tab_position {
        TabPosition::Top => {
            egui::Panel::top(id).frame(frame).resizable(false).show(ui, |ui| body(app, ui, &mut rects));
        }
        TabPosition::Bottom => {
            egui::Panel::bottom(id).frame(frame).resizable(false).show(ui, |ui| body(app, ui, &mut rects));
        }
        TabPosition::Left | TabPosition::Right => {
            let panel = if app.settings.tab_position == TabPosition::Left { egui::Panel::left(id) } else { egui::Panel::right(id) };
            // A fixed width (Settings › Side tab bar width): a resizable side panel grows to its
            // widest with full-width tabs in it.
            panel.frame(frame).resizable(false).exact_size(app.settings.side_width).show(ui, |ui| body(app, ui, &mut rects));
        }
    }
    finish_drag(app, ui, &rects, vertical);
}

/// Tabs along the top or bottom: one row, or one per app (Separate).
fn rows(app: &mut SuiteApp, ui: &mut Ui, rects: &mut Vec<(u64, Rect)>) {
    let groups = app.display_groups();
    let separate = app.settings.mixing == Mixing::Separate;
    let count = groups.len().max(1);
    let controls = app.integrated() && app.settings.tab_position == TabPosition::Top;
    ui.spacing_mut().item_spacing.y = 0.0;
    for (gi, (kind, ids)) in groups.into_iter().enumerate() {
        // Top-aligned: every slot is a full row high (tabs draw below or above the gap).
        ui.horizontal_top(|ui| {
            ui.set_min_height(ROW_H);
            ui.spacing_mut().item_spacing.x = 0.0;
            if separate {
                group_label(app, ui, kind, 96.0);
            }
            let last = gi + 1 == count;
            let buttons = if last || separate { 2.0 * BUTTON_W + 4.0 } else { 0.0 } + if last { ai_button_w(app) } else { 0.0 };
            let window = if controls && gi == 0 { 3.0 * CONTROL_W + DRAG_MIN } else { 0.0 };
            let avail = (ui.available_width() - buttons - window).max(60.0);
            let compact_w = SIDE_TAB_H + 8.0;
            let compact = ids.iter().filter(|id| app.tab(**id).is_some_and(|t| t.pinned && app.settings.compact_pinned)).count();
            let normal = ids.len() - compact;
            let space = (avail - compact as f32 * compact_w).max(0.0);
            let width = if normal > 0 { (space / normal as f32).clamp(app.settings.tab_min_width, app.settings.tab_max_width) } else { 0.0 };
            egui::ScrollArea::horizontal()
                .id_salt(("craft_suite_tab_row", gi))
                .max_width(avail)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (i, id) in ids.iter().enumerate() {
                            // A thin line between two tabs, neither of them the active one.
                            let next = ids.get(i + 1);
                            let line = next.is_some_and(|n| *n != app.active) && *id != app.active;
                            tab(app, ui, *id, width, ROW_H, line, rects);
                        }
                    });
                });
            if buttons > 0.0 {
                ui.add_space(4.0);
                if last || separate {
                    bar_buttons(app, ui, if separate { kind } else { None });
                }
                #[cfg(feature = "ai")]
                if last {
                    crate::ai::bar_button(app, ui, vec2(ai_button_w(app), ROW_H));
                }
            }
            if window > 0.0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| window_controls(ui, ROW_H));
            }
        });
    }
}

/// Room for the Ask AI button (when it's on).
fn ai_button_w(_app: &SuiteApp) -> f32 {
    #[cfg(feature = "ai")]
    if _app.ai.config.enabled {
        return crate::ai::BUTTON_W;
    }
    0.0
}

/// The + button (a new tab: of this row's app in a row per app) and the menu button.
fn bar_buttons(app: &mut SuiteApp, ui: &mut Ui, kind: Option<Kind>) {
    let tip = match kind {
        Some(k) => format!("New {}", k.noun()),
        None => "New tab (Ctrl+Alt+Insert)".to_string(),
    };
    let slot = vec2(BUTTON_W, ROW_H);
    if glyph_button(app, ui, Glyph::Plus, slot, &tip).clicked() {
        app.actions.push(match kind {
            Some(k) if k.is_app() => Action::New { kind: k, replace_page: false },
            _ => Action::NewDefault,
        });
    }
    let menu = glyph_button(app, ui, Glyph::Menu, slot, "New, Open, Home and Settings");
    egui::Popup::menu(&menu).show(|ui| new_menu(app, ui));
}

fn group_label(app: &SuiteApp, ui: &mut Ui, kind: Option<Kind>, width: f32) {
    let (text, color) = match kind {
        Some(k) => (k.app_name(), app.settings.color_of(k).color32()),
        None => ("Suite", app.settings.home_color.color32()),
    };
    let height = if app.settings.tab_position.vertical() { SIDE_TAB_H } else { ROW_H };
    let (_, slot) = ui.allocate_space(vec2(width, height));
    let rect = tab_area(app, slot);
    let p = ui.painter();
    p.rect_filled(Rect::from_min_size(rect.left_top() + vec2(0.0, 8.0), vec2(3.0, rect.height() - 16.0)), CornerRadius::same(1), color);
    if let Some(k) = kind
        && let Some(icon) = app.icon(k)
    {
        let r = Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(16.0));
        p.image(icon.id(), r, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
    }
    p.text(pos2(rect.left() + 30.0, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.0), ui.visuals().weak_text_color());
}

/// Tabs down the left or right side, with a header per app when tabs are in a row per app.
fn side_strip(app: &mut SuiteApp, ui: &mut Ui, rects: &mut Vec<(u64, Rect)>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let slot = vec2(BUTTON_W, SIDE_TAB_H);
        if glyph_button(app, ui, Glyph::Plus, slot, "New tab (Ctrl+Alt+Insert)").clicked() {
            app.actions.push(Action::NewDefault);
        }
        let menu = glyph_button(app, ui, Glyph::Menu, slot, "New, Open, Home and Settings");
        egui::Popup::menu(&menu).show(|ui| new_menu(app, ui));
        #[cfg(feature = "ai")]
        crate::ai::bar_button(app, ui, vec2(ai_button_w(app), SIDE_TAB_H));
    });
    ui.add_space(4.0);
    let separate = app.settings.mixing == Mixing::Separate;
    egui::ScrollArea::vertical().id_salt("craft_suite_side_tabs").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for (kind, ids) in app.display_groups() {
            if separate {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let w = ui.available_width() - BUTTON_W - 4.0;
                    group_label(app, ui, kind, w);
                    if let Some(k) = kind
                        && glyph_button(app, ui, Glyph::Plus, vec2(BUTTON_W, SIDE_TAB_H), &format!("New {}", k.noun())).clicked()
                    {
                        app.actions.push(Action::New { kind: k, replace_page: false });
                    }
                });
            }
            let width = ui.available_width();
            for id in ids {
                tab(app, ui, id, width, SIDE_TAB_H, false, rects);
            }
        }
    });
}

/// One tab, in a `width` by `height` slot; `line`: a separator after it.
#[allow(clippy::too_many_arguments)]
fn tab(app: &mut SuiteApp, ui: &mut Ui, id: u64, width: f32, height: f32, line: bool, rects: &mut Vec<(u64, Rect)>) {
    let Some(t) = app.tab(id) else { return };
    let s = &app.settings;
    let active = id == app.active;
    let vertical = s.tab_position.vertical();
    let compact = t.pinned && s.compact_pinned && !vertical;
    let w = if compact { SIDE_TAB_H + 8.0 } else { width };
    // Only the drawn tab takes clicks: the gap above it moves the window.
    let (_, slot) = ui.allocate_space(vec2(w, height));
    let rect = tab_area(app, slot);
    let resp = ui.interact(rect, ui.id().with(("craft_suite_tab", id)), Sense::click_and_drag());
    rects.push((id, rect));
    let kind = t.kind();
    let title = t.title();
    let dirty = t.dirty();
    let color = app.tab_color(t);
    let path = t.path();
    let pinned = t.pinned;
    let has_color = t.color.is_some();

    let v = ui.visuals().clone();
    let pal = palette(v.dark_mode);
    let hovered = resp.hovered() || app.tabbar_drag() == Some(id);
    let active_bg = pal.page;
    let hover_bg = pal.hover;
    let (bg, fg) = match s.color_style {
        ColorStyle::Stripe | ColorStyle::Plain => {
            let bg = if active { active_bg } else if hovered { hover_bg } else { Color32::TRANSPARENT };
            (bg, if active { v.strong_text_color() } else { v.text_color() })
        }
        ColorStyle::Tint => {
            let base = if active { active_bg } else { pal.bar };
            let bg = mix(base, color, if active { 0.30 } else if hovered { 0.22 } else { 0.12 });
            (bg, v.strong_text_color())
        }
        ColorStyle::Solid => {
            let bg = if active { color } else { mix(pal.bar, color, if hovered { 0.75 } else { 0.55 }) };
            (bg, contrast(bg))
        }
    };
    let radius = 8u8;
    let corners = match s.tab_position {
        TabPosition::Top => CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 },
        TabPosition::Bottom => CornerRadius { nw: 0, ne: 0, sw: radius, se: radius },
        TabPosition::Left | TabPosition::Right => CornerRadius::same(radius),
    };
    let p = ui.painter_at(rect.expand(1.0));
    p.rect_filled(rect, corners, bg);
    if line && !hovered && !vertical {
        let x = rect.right() - 0.5;
        p.line_segment([pos2(x, rect.top() + 9.0), pos2(x, rect.bottom() - 9.0)], Stroke::new(1.0, pal.separator));
    }
    if s.color_style == ColorStyle::Stripe || (s.color_style == ColorStyle::Plain && has_color) {
        let stripe = if active { color } else { color.gamma_multiply(if hovered { 0.8 } else { 0.5 }) };
        let thick = if active { 3.0 } else { 2.0 };
        let r = match s.tab_position {
            TabPosition::Top => Rect::from_min_size(rect.left_top() + vec2(8.0, 0.0), vec2(rect.width() - 16.0, thick)),
            TabPosition::Bottom => Rect::from_min_size(pos2(rect.left() + 8.0, rect.bottom() - thick), vec2(rect.width() - 16.0, thick)),
            TabPosition::Left => Rect::from_min_size(rect.left_top() + vec2(0.0, 4.0), vec2(thick, rect.height() - 8.0)),
            TabPosition::Right => Rect::from_min_size(pos2(rect.right() - thick, rect.top() + 4.0), vec2(thick, rect.height() - 8.0)),
        };
        p.rect_filled(r, CornerRadius::same(1), stripe);
    }

    let mut x = rect.left() + 10.0;
    if s.show_icons || compact {
        if let Some(icon) = app.icon(kind) {
            let r = if compact {
                Rect::from_center_size(rect.center(), Vec2::splat(18.0))
            } else {
                Rect::from_min_size(pos2(x, rect.center().y - 9.0), Vec2::splat(18.0))
            };
            p.image(icon.id(), r, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
        }
        x += 26.0;
    }
    let show_close = !compact
        && match s.close_buttons {
            CloseButtons::Always => true,
            CloseButtons::ActiveTab => active || hovered,
            CloseButtons::Never => false,
        };
    let close_rect = Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), Vec2::splat(20.0));
    if !compact {
        let right = if show_close || dirty { close_rect.left() - 4.0 } else { rect.right() - 8.0 };
        let max_w = (right - x).max(0.0);
        let mut job = egui::text::LayoutJob::single_section(title.clone(), egui::TextFormat::simple(egui::FontId::proportional(13.0), fg));
        job.wrap = egui::text::TextWrapping { max_width: max_w, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
        let galley = ui.painter().layout_job(job);
        p.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    let mut close_clicked = false;
    if show_close {
        let close = ui.interact(close_rect, ui.id().with(("craft_suite_close", id)), Sense::click());
        if dirty && !close.hovered() {
            p.circle_filled(close_rect.center(), 4.0, fg);
        } else {
            if close.hovered() {
                p.rect_filled(close_rect, CornerRadius::same(10), fg.gamma_multiply(0.15));
            }
            paint_glyph(&p, close_rect, Glyph::Close, fg);
        }
        close_clicked = close.on_hover_text("Close tab (Ctrl+Alt+F4)").clicked();
    } else if dirty && !compact {
        p.circle_filled(close_rect.center(), 4.0, fg);
    }

    if close_clicked || (resp.middle_clicked() && s.middle_click_closes) {
        app.actions.push(Action::Close(id));
    } else if resp.clicked() {
        app.actions.push(Action::Activate(id));
    }
    if resp.drag_started() {
        app.tabbar.set_drag(Some(id));
    }
    let tip = match &path {
        Some(p) => format!("{title}{}\n{} · {p}", if dirty { " (unsaved)" } else { "" }, kind.app_name()),
        None => format!("{title}{}\n{}", if dirty { " (unsaved)" } else { "" }, kind.app_name()),
    };
    let resp = if app.tabbar_drag().is_none() { resp.on_hover_text(tip) } else { resp };
    resp.context_menu(|ui| tab_menu(app, ui, id, kind, pinned, path.as_deref()));
}

fn tab_menu(app: &mut SuiteApp, ui: &mut Ui, id: u64, kind: Kind, pinned: bool, path: Option<&str>) {
    if ui.button(if pinned { "Unpin tab" } else { "Pin tab" }).clicked() {
        app.actions.push(Action::TogglePin(id));
        ui.close();
    }
    ui.menu_button("Tab colour", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(180.0);
            for c in SWATCHES {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgb(c[0], c[1], c[2]));
                if resp.on_hover_text("Use this colour for the tab").clicked() {
                    app.actions.push(Action::SetColor(id, Some(Rgb(c))));
                    ui.close();
                }
            }
        });
        let mut current = app.tab(id).map(|t| app.tab_color(t)).map(|c| [c.r(), c.g(), c.b()]).unwrap_or([128, 128, 128]);
        ui.horizontal(|ui| {
            ui.label("Custom");
            if ui.color_edit_button_srgb(&mut current).changed() {
                app.actions.push(Action::SetColor(id, Some(Rgb(current))));
            }
        });
        if ui.button(format!("Use the {} colour", if kind.is_app() { kind.app_name() } else { "suite" })).clicked() {
            app.actions.push(Action::SetColor(id, None));
            ui.close();
        }
    });
    ui.separator();
    let back = if app.settings.tab_position.vertical() { "Move up" } else { "Move left" };
    let fwd = if app.settings.tab_position.vertical() { "Move down" } else { "Move right" };
    if ui.button(back).clicked() {
        app.actions.push(Action::MoveBy(id, -1));
        ui.close();
    }
    if ui.button(fwd).clicked() {
        app.actions.push(Action::MoveBy(id, 1));
        ui.close();
    }
    if let Some(p) = path
        && let Some(dir) = std::path::Path::new(p).parent()
        && ui.button("Show in folder").clicked()
    {
        crate::system::open(dir);
        ui.close();
    }
    ui.separator();
    ui.menu_button("New tab", |ui| new_menu(app, ui));
    ui.separator();
    if ui.button("Close tab").clicked() {
        app.actions.push(Action::Close(id));
        ui.close();
    }
    if ui.button("Close other tabs").on_hover_text("Pinned tabs stay open").clicked() {
        app.actions.push(Action::CloseOthers(id));
        ui.close();
    }
    let after = if app.settings.tab_position.vertical() { "Close tabs below" } else { "Close tabs to the right" };
    if ui.button(after).clicked() {
        app.actions.push(Action::CloseAfter(id));
        ui.close();
    }
}

/// A dragged tab is dropped before or after the tab under the pointer.
fn finish_drag(app: &mut SuiteApp, ui: &Ui, rects: &[(u64, Rect)], vertical: bool) {
    let Some(dragging) = app.tabbar_drag() else { return };
    let (pos, released, down) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.any_released(), i.pointer.any_down()));
    let target = pos.and_then(|pos| {
        rects.iter().find(|(id, r)| *id != dragging && r.expand(1.0).contains(pos)).map(|(id, r)| {
            let after = if vertical { pos.y > r.center().y } else { pos.x > r.center().x };
            (*id, *r, after)
        })
    });
    if released || !down {
        if let Some((target, _, after)) = target {
            app.actions.push(Action::MoveTo { id: dragging, target, after });
        }
        app.tabbar.set_drag(None);
        return;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    if let Some((_, r, after)) = target {
        let color = ui.visuals().selection.bg_fill;
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("craft_suite_drop")));
        let line = if vertical {
            let y = if after { r.bottom() + 1.0 } else { r.top() - 1.0 };
            [pos2(r.left(), y), pos2(r.right(), y)]
        } else {
            let x = if after { r.right() + 1.0 } else { r.left() - 1.0 };
            [pos2(x, r.top()), pos2(x, r.bottom())]
        };
        painter.line_segment(line, Stroke::new(3.0, color));
    }
}

impl TabBarState {
    fn set_drag(&mut self, id: Option<u64>) {
        self.drag = id;
    }
}

impl SuiteApp {
    fn tabbar_drag(&self) -> Option<u64> {
        self.tabbar.drag
    }
}
