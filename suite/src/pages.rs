//! The suite's own pages, Home (new documents, Open, recent files) and Settings, each with a
//! side bar that creates documents and switches between the two.

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Align2, Color32, CornerRadius, FontId, Galley, Painter, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{Action, SuiteApp};
use crate::kind::Kind;
use crate::settings::{CloseButtons, ColorStyle, Mixing, NewTabOpens, NewTabPlacement, Settings, SuiteTheme, TabPosition, TitleBar};
use crate::tabbar::{Palette, palette};
use crate::update;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Settings,
}

const SIDE_W: f32 = 236.0;

/// The side bar and the page's own content beside it, scrolling.
fn shell(app: &mut SuiteApp, ui: &mut Ui, page: Page, body: impl FnOnce(&mut SuiteApp, &mut Ui, &Palette)) {
    let pal = palette(ui.visuals().dark_mode);
    egui::Panel::left("craft_suite_page_side")
        .resizable(false)
        .exact_size(SIDE_W)
        .frame(egui::Frame::NONE.fill(pal.side).inner_margin(egui::Margin { left: 14, right: 14, top: 20, bottom: 14 }))
        .show(ui, |ui| side_bar(app, ui, page, &pal));
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(pal.page)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 34, bottom: 34 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                body(app, ui, &pal);
            });
        });
    });
}

#[derive(Clone, Copy)]
enum SideIcon {
    App(Kind),
    Open,
    Settings,
}

fn side_bar(app: &mut SuiteApp, ui: &mut Ui, page: Page, pal: &Palette) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        if let Some(icon) = app.big_icon(Kind::Home) {
            ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(30.0)));
        }
        ui.add_space(2.0);
        ui.label(RichText::new("Craft Suite").size(17.0).strong());
    });
    ui.add_space(20.0);
    let tab = app.active;
    ui.spacing_mut().item_spacing.y = 2.0;
    if nav_item(app, ui, pal, SideIcon::App(Kind::Home), "Home", page == Page::Home).clicked() && page != Page::Home {
        app.actions.push(Action::ShowPage { tab, settings: false });
    }
    if nav_item(app, ui, pal, SideIcon::Open, "Open…", false).on_hover_text("Open documents, presentations or spreadsheets").clicked() {
        app.actions.push(Action::OpenDialog);
    }
    ui.add_space(16.0);
    ui.label(RichText::new("NEW").size(11.0).weak());
    ui.add_space(4.0);
    for k in app.settings.app_order.clone() {
        if nav_item(app, ui, pal, SideIcon::App(k), k.noun(), false).on_hover_text(format!("A blank {} in {}", k.noun().to_lowercase(), k.app_name())).clicked() {
            app.actions.push(Action::New { kind: k, replace_page: true });
        }
    }
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        // Settings has its own About and updates card.
        if page == Page::Home {
            ui.scope(|ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                app.updater.ui(ui);
            });
        }
        ui.label(RichText::new(format!("Version {}", update::installed_version())).small().weak());
        ui.add_space(6.0);
        if nav_item(app, ui, pal, SideIcon::Settings, "Settings", page == Page::Settings).clicked() && page != Page::Settings {
            app.actions.push(Action::ShowPage { tab, settings: true });
        }
    });
}

fn nav_item(app: &SuiteApp, ui: &mut Ui, pal: &Palette, icon: SideIcon, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let v = ui.visuals();
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(8), pal.card);
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, pal.border), StrokeKind::Inside);
        let bar = Rect::from_min_size(pos2(rect.left() + 1.0, rect.top() + 9.0), vec2(3.0, rect.height() - 18.0));
        p.rect_filled(bar, CornerRadius::same(2), v.selection.bg_fill);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(8), v.widgets.hovered.weak_bg_fill);
    }
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), Vec2::splat(20.0));
    let color = v.text_color();
    match icon {
        SideIcon::App(k) => {
            if let Some(t) = app.icon(k) {
                p.image(t.id(), icon_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        SideIcon::Open => paint_folder(p, icon_rect.shrink(1.0), Color32::from_rgb(0xe8, 0xa8, 0x2c)),
        SideIcon::Settings => paint_sliders(p, icon_rect.shrink(2.0), color),
    }
    let text = if selected { v.strong_text_color() } else { color };
    p.text(pos2(rect.left() + 42.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(14.0), text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A folder: its tab and body.
fn paint_folder(p: &Painter, r: Rect, color: Color32) {
    let tab = Rect::from_min_size(r.left_top() + vec2(0.0, r.height() * 0.1), vec2(r.width() * 0.45, r.height() * 0.3));
    p.rect_filled(tab, CornerRadius::same(2), color.gamma_multiply(0.8));
    let body = Rect::from_min_max(pos2(r.left(), r.top() + r.height() * 0.25), pos2(r.right(), r.bottom() - r.height() * 0.08));
    p.rect_filled(body, CornerRadius::same(3), color);
}

/// Three sliders: the settings icon.
fn paint_sliders(p: &Painter, r: Rect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    for (i, knob) in [0.3f32, 0.7, 0.45].iter().enumerate() {
        let y = r.top() + r.height() * (0.18 + 0.32 * i as f32);
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], stroke);
        p.circle(pos2(r.left() + r.width() * knob, y), 2.6, color, Stroke::NONE);
    }
}

// ---- Home

pub fn home(app: &mut SuiteApp, ui: &mut Ui) {
    shell(app, ui, Page::Home, home_body);
}

fn home_body(app: &mut SuiteApp, ui: &mut Ui, pal: &Palette) {
    let returning = !app.settings.recent.is_empty();
    ui.label(RichText::new(if returning { "Welcome back" } else { "Welcome to Craft Suite" }).size(28.0).strong());
    ui.add_space(2.0);
    ui.label(RichText::new("Start something new, or pick up where you left off.").size(14.0).weak());
    ui.add_space(26.0);
    ui.label(RichText::new("New").size(16.0).strong());
    ui.add_space(10.0);
    // Four tiles across, growing with the window up to a point.
    let gap = 18.0;
    let w = ((ui.available_width() - 3.0 * gap) / 4.0).clamp(150.0, 250.0);
    let size = vec2(w, (w * 0.7).round());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        for k in app.settings.app_order.clone() {
            if tile(app, ui, pal, Some(k), size).clicked() {
                app.actions.push(Action::New { kind: k, replace_page: true });
            }
        }
        if tile(app, ui, pal, None, size).on_hover_text("Open documents, presentations or spreadsheets").clicked() {
            app.actions.push(Action::OpenDialog);
        }
    });
    ui.add_space(32.0);
    recent(app, ui, pal);
}

/// A blank document of an app (or Open, for `None`): a `size` picture of the page, and its name.
fn tile(app: &SuiteApp, ui: &mut Ui, pal: &Palette, kind: Option<Kind>, size: Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size.x, size.y + 48.0), Sense::click());
    let thumb = Rect::from_min_size(rect.min, size);
    let accent = match kind {
        Some(k) => app.settings.color_of(k).color32(),
        None => Color32::from_rgb(0xe8, 0xa8, 0x2c),
    };
    let hovered = resp.hovered();
    let p = ui.painter();
    if hovered {
        p.rect_filled(thumb.translate(vec2(0.0, 3.0)).expand(1.0), CornerRadius::same(12), Color32::from_black_alpha(18));
    }
    let stroke = if hovered { Stroke::new(2.0, accent) } else { Stroke::new(1.0, pal.border) };
    p.rect(thumb, CornerRadius::same(12), pal.card, stroke, StrokeKind::Inside);
    let dark = ui.visuals().dark_mode;
    let paper = if dark { Color32::from_gray(232) } else { Color32::WHITE };
    let ink = Color32::from_gray(if dark { 175 } else { 214 });
    let area = thumb.shrink2(vec2(size.x * 0.1, size.y * 0.12));
    match kind {
        Some(Kind::Word) => {
            let h = area.height();
            let page = Rect::from_center_size(area.center(), vec2(h * 0.78, h));
            page_shadow(p, page, paper, pal);
            let left = page.left() + 9.0;
            let w = page.width() - 18.0;
            p.rect_filled(Rect::from_min_size(pos2(left, page.top() + 10.0), vec2(w * 0.6, 5.0)), CornerRadius::same(2), accent);
            for (i, f) in [1.0f32, 0.92, 0.97, 0.6, 0.0, 1.0, 0.85].iter().enumerate() {
                if *f > 0.0 {
                    p.rect_filled(Rect::from_min_size(pos2(left, page.top() + 22.0 + i as f32 * 9.0), vec2(w * f, 3.0)), CornerRadius::same(1), ink);
                }
            }
        }
        Some(Kind::Deck) => {
            let w = area.width().min(area.height() * 16.0 / 9.0);
            let slide = Rect::from_center_size(area.center(), vec2(w, w * 9.0 / 16.0));
            page_shadow(p, slide, paper, pal);
            let x = slide.left() + slide.width() * 0.1;
            p.rect_filled(Rect::from_min_size(pos2(x, slide.top() + slide.height() * 0.32), vec2(slide.width() * 0.5, 9.0)), CornerRadius::same(2), accent);
            p.rect_filled(Rect::from_min_size(pos2(x, slide.top() + slide.height() * 0.32 + 16.0), vec2(slide.width() * 0.34, 4.0)), CornerRadius::same(1), ink);
            p.circle_filled(pos2(slide.right() - slide.width() * 0.2, slide.center().y + 4.0), slide.height() * 0.2, accent.gamma_multiply(0.35));
        }
        Some(Kind::Grid) => {
            let sheet = area;
            page_shadow(p, sheet, paper, pal);
            let (cols, rows) = (5, 6);
            let cw = sheet.width() / cols as f32;
            let rh = sheet.height() / rows as f32;
            p.rect_filled(Rect::from_min_size(sheet.min, vec2(sheet.width(), rh)), CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, accent.gamma_multiply(0.85));
            p.rect_filled(Rect::from_min_size(sheet.min + vec2(0.0, rh), vec2(cw, sheet.height() - rh)), CornerRadius::ZERO, accent.gamma_multiply(0.18));
            let line = Stroke::new(1.0, ink);
            for c in 1..cols {
                let x = sheet.left() + c as f32 * cw;
                p.line_segment([pos2(x, sheet.top() + rh), pos2(x, sheet.bottom())], line);
            }
            for r in 2..rows {
                let y = sheet.top() + r as f32 * rh;
                p.line_segment([pos2(sheet.left(), y), pos2(sheet.right(), y)], line);
            }
        }
        _ => {
            paint_folder(p, Rect::from_center_size(area.center(), vec2(72.0, 58.0)), accent);
        }
    }
    let (title, sub) = match kind {
        Some(k) => (format!("Blank {}", k.noun().to_lowercase()), k.app_name().to_string()),
        None => ("Open…".to_string(), "From this computer".to_string()),
    };
    let v = ui.visuals();
    p.text(pos2(rect.left() + 2.0, thumb.bottom() + 10.0), Align2::LEFT_TOP, title, FontId::proportional(14.0), v.strong_text_color());
    p.text(pos2(rect.left() + 2.0, thumb.bottom() + 29.0), Align2::LEFT_TOP, sub, FontId::proportional(12.0), v.weak_text_color());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn page_shadow(p: &Painter, r: Rect, paper: Color32, pal: &Palette) {
    p.rect_filled(r.translate(vec2(0.0, 2.0)), CornerRadius::same(3), Color32::from_black_alpha(22));
    p.rect(r, CornerRadius::same(3), paper, Stroke::new(1.0, pal.border), StrokeKind::Inside);
}

/// The Home page's search and filter, kept while the page shows.
#[derive(Clone, Default)]
struct RecentView {
    filter: Option<Kind>,
    query: String,
}

fn recent(app: &mut SuiteApp, ui: &mut Ui, pal: &Palette) {
    let id = egui::Id::new("craft_suite_recent_view");
    let mut view: RecentView = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Recent").size(16.0).strong());
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        for (f, label) in [(None, "All"), (Some(Kind::Word), "Documents"), (Some(Kind::Deck), "Presentations"), (Some(Kind::Grid), "Spreadsheets")] {
            if chip(ui, pal, label, view.filter == f).clicked() {
                view.filter = f;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.settings.recent.is_empty()
                && ui.add(egui::Button::new(RichText::new("Clear list").size(13.0)).frame(false)).on_hover_text("Forget the recent files (the files stay)").clicked()
            {
                app.settings.recent.clear();
            }
            ui.add_space(6.0);
            egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut view.query).hint_text("Search recent files").frame(egui::Frame::NONE).desired_width(200.0));
            });
        });
    });
    ui.add_space(10.0);

    let query = view.query.trim().to_lowercase();
    let files: Vec<String> = app
        .settings
        .recent
        .iter()
        .filter(|p| view.filter.is_none() || Kind::for_path(p) == view.filter)
        .filter(|p| query.is_empty() || p.to_lowercase().contains(&query))
        .cloned()
        .collect();

    egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(0, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let columns = Columns::new(width);
        let (header, _) = ui.allocate_exact_size(vec2(width, 32.0), Sense::hover());
        let weak = ui.visuals().weak_text_color();
        let font = FontId::proportional(12.0);
        ui.painter().text(pos2(header.left() + columns.name, header.center().y), Align2::LEFT_CENTER, "Name", font.clone(), weak);
        if let Some(x) = columns.location {
            ui.painter().text(pos2(header.left() + x, header.center().y), Align2::LEFT_CENTER, "Location", font.clone(), weak);
        }
        ui.painter().text(pos2(header.left() + columns.modified, header.center().y), Align2::LEFT_CENTER, "Modified", font, weak);
        if files.is_empty() {
            let text = if app.settings.recent.is_empty() { "Files you open show up here." } else { "No recent files match." };
            ui.add_space(14.0);
            ui.vertical_centered(|ui| ui.label(RichText::new(text).weak()));
            ui.add_space(18.0);
        }
        let mut remove = None;
        for path in &files {
            if let Some(gone) = recent_row(app, ui, pal, &columns, path) {
                remove = Some(gone);
            }
        }
        if let Some(path) = remove {
            app.settings.recent.retain(|p| *p != path);
        }
    });
    ui.data_mut(|d| d.insert_temp(id, view));
}

/// Where the recent list's columns start, from the list's left edge.
struct Columns {
    name: f32,
    location: Option<f32>,
    modified: f32,
    width: f32,
}

impl Columns {
    fn new(width: f32) -> Columns {
        let modified = (width - 150.0).max(200.0);
        let location = (width >= 640.0).then(|| (width * 0.42).max(260.0));
        Columns { name: 18.0, location, modified, width }
    }
}

/// A file in the recent list; returns its path when "Remove from list" was chosen.
fn recent_row(app: &mut SuiteApp, ui: &mut Ui, pal: &Palette, columns: &Columns, path: &str) -> Option<String> {
    let (rect, resp) = ui.allocate_exact_size(vec2(columns.width, 48.0), Sense::click());
    let p = Path::new(path);
    let exists = p.exists();
    let v = ui.visuals().clone();
    let painter = ui.painter();
    painter.line_segment([pos2(rect.left() + 12.0, rect.top()), pos2(rect.right() - 12.0, rect.top())], Stroke::new(1.0, pal.border));
    if resp.hovered() && exists {
        painter.rect_filled(rect.shrink2(vec2(6.0, 2.0)), CornerRadius::same(8), pal.hover);
    }
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    let dir = p.parent().map(|d| tilde(&d.to_string_lossy())).unwrap_or_default();
    let kind = Kind::for_path(path);
    let x = rect.left() + columns.name;
    if let Some(icon) = kind.and_then(|k| app.icon(k)) {
        let r = Rect::from_min_size(pos2(x, rect.center().y - 12.0), Vec2::splat(24.0));
        painter.image(icon.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if exists { Color32::WHITE } else { Color32::from_white_alpha(110) });
    }
    let name_x = x + 36.0;
    let name_end = rect.left() + columns.location.unwrap_or(columns.modified) - 16.0;
    let strong = if exists { v.strong_text_color() } else { v.weak_text_color() };
    match columns.location {
        Some(lx) => {
            let g = elided(ui, &name, 14.0, strong, name_end - name_x);
            painter.galley(pos2(name_x, rect.center().y - g.size().y / 2.0), g, strong);
            let g = elided(ui, &dir, 13.0, v.weak_text_color(), columns.modified - lx - 16.0);
            painter.galley(pos2(rect.left() + lx, rect.center().y - g.size().y / 2.0), g, v.weak_text_color());
        }
        None => {
            let g = elided(ui, &name, 14.0, strong, name_end - name_x);
            painter.galley(pos2(name_x, rect.center().y - g.size().y + 1.0), g, strong);
            let g = elided(ui, &dir, 12.0, v.weak_text_color(), name_end - name_x);
            painter.galley(pos2(name_x, rect.center().y + 2.0), g, v.weak_text_color());
        }
    }
    let when = if exists { modified(p) } else { "Not found".to_string() };
    painter.text(pos2(rect.left() + columns.modified, rect.center().y), Align2::LEFT_CENTER, when, FontId::proportional(13.0), v.weak_text_color());

    if resp.clicked() && exists {
        app.actions.push(Action::Open(path.to_string()));
    }
    let mut remove = None;
    let resp = if exists { resp.on_hover_text(path).on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp.on_hover_text(format!("{path}\nThe file isn't there any more.")) };
    resp.context_menu(|ui| {
        if ui.add_enabled(exists, egui::Button::new("Open")).clicked() {
            app.actions.push(Action::Open(path.to_string()));
            ui.close();
        }
        if let Some(dir) = p.parent()
            && ui.add_enabled(dir.exists(), egui::Button::new("Show in folder")).clicked()
        {
            crate::system::open(dir);
            ui.close();
        }
        ui.separator();
        if ui.button("Remove from list").clicked() {
            remove = Some(path.to_string());
            ui.close();
        }
    });
    remove
}

/// One line of text, cut short with "…" to fit `max_w`.
fn elided(ui: &Ui, text: &str, size: f32, color: Color32, max_w: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat::simple(FontId::proportional(size), color));
    job.wrap = egui::text::TextWrapping { max_width: max_w.max(10.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

/// `/home/me/Documents` as `~/Documents`.
fn tilde(dir: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && dir.starts_with(&home) => format!("~{}", &dir[home.len()..]),
        _ => dir.to_string(),
    }
}

/// When a file last changed, as "3 hours ago" or a date.
fn modified(path: &Path) -> String {
    let Some(t) = std::fs::metadata(path).and_then(|m| m.modified()).ok() else { return String::new() };
    let secs = SystemTime::now().duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match secs {
        0..60 => "Just now".to_string(),
        60..3600 => plural(secs / 60, "minute"),
        3600..86_400 => plural(secs / 3600, "hour"),
        86_400..172_800 => "Yesterday".to_string(),
        172_800..604_800 => plural(secs / 86_400, "day"),
        604_800..2_419_200 => plural(secs / 604_800, "week"),
        _ => t.duration_since(UNIX_EPOCH).map(|d| date((d.as_secs() / 86_400) as i64)).unwrap_or_default(),
    }
}

/// Days since 1970-01-01 as "14 Oct 2026" (UTC; Howard Hinnant's civil-from-days).
fn date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// A rounded filter button.
fn chip(ui: &mut Ui, pal: &Palette, label: &str, selected: bool) -> egui::Response {
    let v = ui.visuals().clone();
    let color = if selected { v.selection.stroke.color } else { v.text_color() };
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), color);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 26.0, 28.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(14), v.selection.bg_fill.gamma_multiply(0.35));
        p.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, v.selection.bg_fill), StrokeKind::Inside);
    } else {
        p.rect(rect, CornerRadius::same(14), if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { pal.card }, Stroke::new(1.0, pal.border), StrokeKind::Inside);
    }
    p.galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---- Settings

/// A titled card of settings.
fn card(ui: &mut Ui, pal: &Palette, title: &str, note: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(16.0);
    egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(12).inner_margin(20).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(16.0).strong());
        if !note.is_empty() {
            ui.label(RichText::new(note).size(12.5).weak());
        }
        ui.add_space(12.0);
        add(ui);
    });
}

fn radio_row<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        for (v, label) in options {
            ui.radio_value(value, *v, *label);
        }
    });
}

fn grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id).num_columns(2).spacing(vec2(28.0, 12.0)).min_col_width(170.0).show(ui, add);
}

pub fn settings(app: &mut SuiteApp, ui: &mut Ui) {
    shell(app, ui, Page::Settings, settings_body);
}

fn settings_body(app: &mut SuiteApp, ui: &mut Ui, pal: &Palette) {
    ui.set_max_width(900.0);
    ui.label(RichText::new("Settings").size(28.0).strong());
    ui.add_space(2.0);
    ui.label(RichText::new("How Craft Suite arranges and colours its tabs. Each app keeps its own options (File › Options in the app).").size(14.0).weak());

    card(ui, pal, "Window", "", |ui| {
        grid(ui, "window", |ui| {
            let s = &mut app.settings;
            ui.label("Title bar");
            ui.vertical(|ui| {
                for t in TitleBar::ALL {
                    ui.radio_value(&mut s.title_bar, t, t.label());
                }
                ui.label(
                    RichText::new(match s.title_bar {
                        TitleBar::Integrated => "Drag an empty part of the tab bar to move the window; double-click it to maximize.",
                        TitleBar::System => "The desktop draws the title bar, with its own buttons, above the tabs.",
                    })
                    .size(12.5)
                    .weak(),
                );
            });
            ui.end_row();

            ui.label("Theme");
            let themes: Vec<(SuiteTheme, &str)> = SuiteTheme::ALL.iter().map(|t| (*t, t.label())).collect();
            radio_row(ui, &mut s.theme, &themes);
            ui.end_row();
        });
    });

    card(ui, pal, "Tab arrangement", "", |ui| {
        grid(ui, "arrangement", |ui| {
            let s = &mut app.settings;
            ui.label("Tab bar position");
            let positions: Vec<(TabPosition, &str)> = TabPosition::ALL.iter().map(|p| (*p, p.label())).collect();
            radio_row(ui, &mut s.tab_position, &positions);
            ui.end_row();

            ui.label("Mixing apps");
            ui.vertical(|ui| {
                for m in Mixing::ALL {
                    ui.radio_value(&mut s.mixing, m, m.label());
                }
                ui.label(
                    RichText::new(match s.mixing {
                        Mixing::Mixed => "Documents, presentations and spreadsheets share one strip in the order you drag them.",
                        Mixing::Grouped => "One strip, tabs kept together by app in the app order below.",
                        Mixing::Separate => "Each app gets its own row of tabs (a section when the tabs are on the side).",
                    })
                    .weak()
                    .size(12.5),
                );
            });
            ui.end_row();

            ui.label("App order");
            ui.vertical(|ui| {
                let mut swap: Option<(usize, usize)> = None;
                let n = s.app_order.len();
                for (i, k) in s.app_order.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}. {} ({})", i + 1, k.app_name(), k.noun()));
                        if ui.add_enabled(i > 0, egui::Button::new("Up").small()).clicked() {
                            swap = Some((i, i - 1));
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("Down").small()).clicked() {
                            swap = Some((i, i + 1));
                        }
                    });
                }
                if let Some((a, b)) = swap {
                    s.app_order.swap(a, b);
                }
            });
            ui.end_row();

            ui.label("New tabs open");
            radio_row(ui, &mut s.new_tab_placement, &[(NewTabPlacement::AfterCurrent, "Next to the current tab"), (NewTabPlacement::End, "At the end")]);
            ui.end_row();

            ui.label("Tab width");
            ui.vertical(|ui| {
                ui.add(egui::Slider::new(&mut s.tab_max_width, 100.0..=400.0).text("widest").suffix(" pt"));
                ui.add(egui::Slider::new(&mut s.tab_min_width, 50.0..=200.0).text("narrowest").suffix(" pt"));
                if s.tab_min_width > s.tab_max_width {
                    s.tab_min_width = s.tab_max_width;
                }
            });
            ui.end_row();

            ui.label("Side tab bar width");
            ui.add(egui::Slider::new(&mut s.side_width, 120.0..=480.0).suffix(" pt"));
            ui.end_row();

            ui.label("Icons");
            ui.checkbox(&mut s.show_icons, "Show app icons on tabs");
            ui.end_row();

            ui.label("Close buttons");
            let options: Vec<(CloseButtons, &str)> = CloseButtons::ALL.iter().map(|c| (*c, c.label())).collect();
            radio_row(ui, &mut s.close_buttons, &options);
            ui.end_row();

            ui.label("Pinned tabs");
            ui.checkbox(&mut s.compact_pinned, "Show only the icon (tabs along the top or bottom)");
            ui.end_row();
        });
    });

    card(ui, pal, "Colour", "Right-click a tab to give it a colour of its own.", |ui| {
        grid(ui, "colour", |ui| {
            let s = &mut app.settings;
            ui.label("App colour on tabs");
            let styles: Vec<(ColorStyle, &str)> = ColorStyle::ALL.iter().map(|c| (*c, c.label())).collect();
            radio_row(ui, &mut s.color_style, &styles);
            ui.end_row();

            for k in [Kind::Word, Kind::Deck, Kind::Grid, Kind::Home] {
                ui.label(if k == Kind::Home { "Home and Settings".to_string() } else { format!("{} colour", k.app_name()) });
                ui.horizontal(|ui| {
                    let c = s.color_mut(k);
                    ui.color_edit_button_srgb(&mut c.0);
                    let default = Settings::default().color_of(k);
                    if ui.add_enabled(*c != default, egui::Button::new("Reset").small()).clicked() {
                        *c = default;
                    }
                });
                ui.end_row();
            }

            ui.label("Tab bar");
            ui.checkbox(&mut s.tint_bar_with_active, "Tint the whole bar with the active tab's colour");
            ui.end_row();
        });
    });

    card(ui, pal, "Behaviour", "", |ui| {
        grid(ui, "behaviour", |ui| {
            let s = &mut app.settings;
            ui.label("A new tab shows");
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut s.new_tab_opens, NewTabOpens::Home, "The Home page");
                for k in Kind::APPS {
                    ui.radio_value(&mut s.new_tab_opens, NewTabOpens::App(k), format!("A blank {}", k.noun().to_lowercase()));
                }
            });
            ui.end_row();

            ui.label("Opening files");
            ui.vertical(|ui| {
                ui.checkbox(&mut s.reuse_blank_tab, "Replace an untouched blank tab of the same app");
                ui.checkbox(&mut s.single_window, "Open files from the file manager in this window (next start)");
            });
            ui.end_row();

            ui.label("Starting");
            ui.vertical(|ui| {
                ui.checkbox(&mut s.restore_tabs, "Reopen the files that were open last time");
                ui.checkbox(&mut s.check_updates_on_start, "Check for updates");
            });
            ui.end_row();

            ui.label("Mouse");
            ui.vertical(|ui| {
                ui.checkbox(&mut s.middle_click_closes, "Middle-click closes a tab");
                ui.checkbox(&mut s.double_click_bar_new_tab, "Double-click the empty tab bar for a new tab");
                if s.title_bar == TitleBar::Integrated && s.tab_position == TabPosition::Top {
                    ui.label(RichText::new("With the tabs in the title bar, double-clicking it maximizes the window instead.").size(12.5).weak());
                }
            });
            ui.end_row();

            ui.label("Closing");
            ui.checkbox(&mut s.confirm_quit_with_tabs, "Ask before closing the window with several tabs");
            ui.end_row();

            ui.label("Keyboard");
            ui.vertical(|ui| {
                ui.checkbox(&mut s.shortcuts, "Suite shortcuts");
                for (keys, what) in [
                    ("Ctrl+Alt+PageDown / PageUp", "Next / previous tab"),
                    ("Ctrl+Shift+Alt+PageDown / PageUp", "Move the tab right / left"),
                    ("Ctrl+Alt+Insert", "New tab"),
                    ("Ctrl+Alt+F4", "Close the tab"),
                    ("Ctrl+Alt+Home", "Home page"),
                ] {
                    ui.label(RichText::new(format!("{keys}: {what}")).size(12.5));
                }
            });
            ui.end_row();
        });
    });

    card(ui, pal, "About and updates", "", |ui| {
        ui.label(format!("Craft Suite {}", update::installed_version()));
        let upstream = update::upstream_summary();
        if !upstream.is_empty() {
            ui.collapsing("Built from", |ui| {
                for line in upstream.lines() {
                    ui.label(RichText::new(line).monospace().small());
                }
            });
        }
        ui.add_space(4.0);
        app.updater.ui(ui);
    });

    ui.add_space(18.0);
    if ui.button("Restore default settings").on_hover_text("Recent files and reopened tabs are kept").clicked() {
        let keep = (app.settings.recent.clone(), app.settings.session.clone());
        app.settings = Settings::default();
        (app.settings.recent, app.settings.session) = keep;
    }
}
