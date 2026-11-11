//! Insert › Header, Footer, Page Number, Cover Page and Quick Parts galleries: the built-in
//! designs the engine lists, a header/footer design as a drawn tile.

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, vec2};
use wordcraft_engine::cmd::insert::{BUILT_IN_BLOCKS, COVER_DESIGNS, HF_DESIGNS, PAGE_NUMBER_STYLES};

use super::super::common::mi;
use super::{Ui, WordApp, json};

/// What a slot of a tile's sketch holds: nothing, grey text, accent text, dark bold text or a
/// red notice.
#[derive(Clone, Copy, PartialEq)]
enum Ink {
    None,
    Grey,
    Accent,
    Bold,
    Alert,
}

/// How a design decorates its band.
#[derive(Clone, Copy, PartialEq)]
enum Deco {
    None,
    /// A thick accent rule along the edge next to the text.
    Bar,
    /// A thin grey rule along that edge.
    Rule,
    /// The whole band filled with the accent.
    Fill,
    /// A thick accent line on the left.
    Side,
    /// An accent box round the text.
    Frame,
    /// The left text on an accent tab.
    Tab,
}

/// The sketch of design `id`: its left, center and right text and decoration.
fn sketch(id: &str) -> ([Ink; 3], Deco) {
    use Ink::{Accent, Alert, Bold, Grey, None as No};
    match id {
        "accentBar" => ([Accent, No, Accent], Deco::Bar),
        "banded" => ([Bold, No, Grey], Deco::Fill),
        "dateTitle" => ([Grey, Bold, Grey], Deco::None),
        "authorPage" => ([No, No, Grey], Deco::None),
        "pageXofY" => ([No, Bold, No], Deco::None),
        "chapter" => ([Grey, No, Accent], Deco::None),
        "rule" => ([No, Grey, No], Deco::Rule),
        "confidential" => ([Alert, No, Grey], Deco::None),
        "twoTone" => ([Bold, No, Accent], Deco::Tab),
        "minimalDot" => ([No, Grey, No], Deco::None),
        "sideline" => ([Accent, No, No], Deco::Side),
        "framed" => ([No, Accent, No], Deco::Frame),
        "fileDate" => ([Grey, No, Grey], Deco::None),
        "dash" => ([No, Accent, No], Deco::None),
        _ => ([No, Grey, No], Deco::None),
    }
}

const ACCENT: Color32 = Color32::from_rgb(0x15, 0x60, 0x82);
const ALERT: Color32 = Color32::from_rgb(0xC0, 0x00, 0x00);
const TILE: egui::Vec2 = vec2(108.0, 50.0);
const TILES_PER_ROW: usize = 3;

/// Paints design `id` as a strip of page: text slots as bars, at the top of the page for a
/// header and at the bottom for a footer.
fn paint_sketch(painter: &egui::Painter, r: Rect, id: &str, header: bool) {
    let (inks, deco) = sketch(id);
    painter.rect_filled(r, 2.0, Color32::WHITE);
    painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_gray(0xC8)), StrokeKind::Inside);
    let band = r.shrink2(vec2(8.0, 6.0));
    let y = band.center().y;
    // Body text lines below a header (above a footer), so the tile reads as a page edge.
    for i in 0..2 {
        let ly = if header { band.bottom() - 2.0 * i as f32 } else { band.top() + 2.0 * i as f32 };
        painter.line_segment([Pos2::new(band.left(), ly), Pos2::new(band.right() - 14.0 * i as f32, ly)], Stroke::new(1.0, Color32::from_gray(0xE2)));
    }
    let line_y = if header { y - 4.0 } else { y + 4.0 };
    let text_band = Rect::from_center_size(Pos2::new(band.center().x, line_y), vec2(band.width(), 8.0));
    match deco {
        Deco::Fill => {
            painter.rect_filled(text_band.expand2(vec2(4.0, 2.0)), 1.0, ACCENT);
        }
        Deco::Bar | Deco::Rule => {
            let edge = if header { text_band.bottom() + 3.0 } else { text_band.top() - 3.0 };
            let stroke = if deco == Deco::Bar { Stroke::new(2.0, ACCENT) } else { Stroke::new(1.0, Color32::from_gray(0x8C)) };
            painter.line_segment([Pos2::new(text_band.left(), edge), Pos2::new(text_band.right(), edge)], stroke);
        }
        Deco::Side => {
            painter.line_segment(
                [Pos2::new(text_band.left(), text_band.top()), Pos2::new(text_band.left(), text_band.bottom())],
                Stroke::new(2.5, ACCENT),
            );
        }
        Deco::Frame => {
            painter.rect_stroke(Rect::from_center_size(text_band.center(), vec2(46.0, 10.0)), 0.0, Stroke::new(1.0, ACCENT), StrokeKind::Middle);
        }
        Deco::Tab => {
            painter.rect_filled(Rect::from_min_size(text_band.left_top(), vec2(34.0, text_band.height())), 1.0, ACCENT);
        }
        Deco::None => {}
    }
    let filled = deco == Deco::Fill;
    for (slot, ink) in inks.iter().enumerate() {
        let (color, width) = match ink {
            Ink::None => continue,
            Ink::Grey => (if filled { Color32::from_gray(0xDD) } else { Color32::from_gray(0x9A) }, 22.0),
            Ink::Accent => (ACCENT, 20.0),
            Ink::Bold => (if filled || deco == Deco::Tab { Color32::WHITE } else { Color32::from_gray(0x40) }, 26.0),
            Ink::Alert => (ALERT, 26.0),
        };
        let pad = if deco == Deco::Side {
            5.0
        } else if deco == Deco::Tab {
            3.0
        } else {
            0.0
        };
        let x0 = match slot {
            0 => text_band.left() + pad,
            1 => text_band.center().x - width / 2.0,
            _ => text_band.right() - width,
        };
        let thick = if matches!(ink, Ink::Bold | Ink::Alert) { 3.0 } else { 2.0 };
        painter.line_segment([Pos2::new(x0, line_y), Pos2::new(x0 + width, line_y)], Stroke::new(thick, color));
    }
}

/// The header (or footer) designs as a grid of drawn tiles, each named under it; a click runs
/// `cmd` with the design as its preset.
pub(super) fn hf_gallery(ui: &mut Ui, app: &mut WordApp, cmd: &str, header: bool) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let enabled = crate::widgets::enabled(app, cmd);
    ui.add_enabled_ui(enabled, |ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for row in HF_DESIGNS.chunks(TILES_PER_ROW) {
            ui.horizontal(|ui| {
                for (id, label) in row {
                    let (r, resp) = ui.allocate_exact_size(TILE, Sense::click());
                    let name = crate::tl!(label);
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name));
                    crate::widgets::bg(ui, r, &resp, false, &t);
                    let art = Rect::from_min_size(r.min + vec2(4.0, 3.0), vec2(TILE.x - 8.0, 30.0));
                    paint_sketch(ui.painter(), art, id, header);
                    let font = egui::FontId::proportional(11.0);
                    let text = ui.painter().layout(name.to_string(), font, t.text, TILE.x - 4.0);
                    let at = Pos2::new(r.center().x - text.size().x / 2.0, art.bottom() + 2.0);
                    ui.painter().galley(at, text, t.text);
                    if resp.on_hover_text(name).clicked() {
                        crate::widgets::run_from_ribbon(app, cmd, json!({"preset": id}));
                        ui.close();
                    }
                }
            });
        }
    });
}

/// Insert › Page Number: each position lists every style; Page Margins puts the number in a box
/// in the left or right margin.
pub(super) fn page_number_menu(ui: &mut Ui, app: &mut WordApp) {
    for (label, position, align) in [("Top of Page", "top", "right"), ("Bottom of Page", "bottom", "center")] {
        ui.menu_button(crate::tl!(label), |ui| {
            for (side, side_align) in [("Plain Number (Left)", "left"), ("Plain Number (Center)", "center"), ("Plain Number (Right)", "right")] {
                mi(ui, app, side, "insert.pageNumber", json!({"position": position, "align": side_align, "style": "plain"}));
            }
            ui.separator();
            for (style, name) in PAGE_NUMBER_STYLES.iter().filter(|(s, _)| *s != "plain") {
                mi(ui, app, name, "insert.pageNumber", json!({"position": position, "align": align, "style": style}));
            }
        });
    }
    ui.menu_button(crate::tl!("Page Margins"), |ui| {
        for (label, position) in [("Left Margin", "leftMargin"), ("Right Margin", "rightMargin")] {
            ui.menu_button(crate::tl!(label), |ui| {
                for (style, name) in PAGE_NUMBER_STYLES {
                    mi(ui, app, name, "insert.pageNumber", json!({"position": position, "style": style}));
                }
            });
        }
    });
    ui.menu_button(crate::tl!("Current Position"), |ui| {
        for (style, name) in PAGE_NUMBER_STYLES {
            mi(ui, app, name, "insert.pageNumber", json!({"position": "current", "style": style}));
        }
    });
    ui.separator();
    mi(ui, app, "Format Page Numbers…", "ui.dialog", json!({"name": "pageNumbers"}));
}

/// Insert › Cover Page: every built-in design.
pub(super) fn cover_page_menu(ui: &mut Ui, app: &mut WordApp) {
    for (id, label) in COVER_DESIGNS {
        mi(ui, app, label, "insert.coverPage", json!({"design": id}));
    }
}

/// Insert › Quick Parts › Building Blocks: the built-in blocks.
pub(super) fn building_blocks_menu(ui: &mut Ui, app: &mut WordApp) {
    ui.menu_button(crate::tl!("Building Blocks"), |ui| {
        for (name, label) in BUILT_IN_BLOCKS {
            mi(ui, app, label, "insert.buildingBlock", json!({"name": name}));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every design the engine offers has its own sketch, so a new one isn't drawn as a blank.
    #[test]
    fn every_header_footer_design_has_a_sketch() {
        let fallback = sketch("");
        for (id, _) in HF_DESIGNS {
            let (inks, deco) = sketch(id);
            assert!(inks != fallback.0 || deco != fallback.1 || *id == "minimalDot", "{id} has no sketch");
        }
    }
}
