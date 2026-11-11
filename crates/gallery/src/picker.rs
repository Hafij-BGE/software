//! The egui icon picker: a search box, a category list and a scrolling grid of every icon.

use std::collections::HashMap;

use egui::{Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2};

use crate::icons::{self, Icon};

/// Icons drawn per frame at most; the rest show next frame, so the grid never stalls.
const RENDERS_PER_FRAME: usize = 64;
const CELL: f32 = 52.0;
const GLYPH: f32 = 32.0;

/// Search-and-pick state for one icon grid.
#[derive(Default)]
pub struct IconPicker {
    pub query: String,
    pub category: Option<&'static str>,
    /// The icon last clicked.
    pub selected: Option<&'static Icon>,
    /// Pick on one click (an Icons menu) instead of a double-click (a dialog with an Insert button).
    pub single_click: bool,
    results: Vec<&'static Icon>,
    searched: Option<(String, Option<&'static str>)>,
    textures: HashMap<(&'static str, u32), TextureHandle>,
}

impl IconPicker {
    pub fn new(single_click: bool) -> Self {
        Self { single_click, ..Default::default() }
    }

    /// Draws the picker filling `ui`; returns the icon picked this frame.
    pub fn show(&mut self, ui: &mut Ui) -> Option<&'static Icon> {
        ui.horizontal(|ui| {
            let hint = format!("Search {} icons", thousands(icons::all().len()));
            let field = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text(hint).desired_width(ui.available_width() - 190.0));
            if field.changed() {
                self.selected = None;
            }
            egui::ComboBox::from_id_salt("craft-gallery-icon-category")
                .width(170.0)
                .selected_text(self.category.unwrap_or("All categories"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category, None, "All categories");
                    for c in icons::categories() {
                        ui.selectable_value(&mut self.category, Some(*c), *c);
                    }
                });
        });
        let key = (self.query.trim().to_string(), self.category);
        if self.searched.as_ref() != Some(&key) {
            self.results = icons::search(&key.0, key.1);
            self.searched = Some(key);
        }
        ui.add_space(4.0);
        if self.results.is_empty() {
            ui.label("No icons match. Try another word, or All categories.");
            return None;
        }
        ui.weak(format!("{} icons", thousands(self.results.len())));

        let ppp = ui.ctx().pixels_per_point();
        let px = ((GLYPH * ppp).round() as u32).clamp(16, 256);
        if self.textures.len() > 4000 {
            self.textures.clear();
        }
        let tint = ui.visuals().text_color();
        let accent = ui.visuals().selection.bg_fill;
        let hover = ui.visuals().widgets.hovered.weak_bg_fill;
        let cols = ((ui.available_width() / CELL).floor() as usize).max(1);
        let rows = self.results.len().div_ceil(cols);
        let mut picked = None;
        let mut budget = RENDERS_PER_FRAME;
        let mut pending = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, CELL, rows, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for icon in self.results.iter().skip(row * cols).take(cols).copied() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
                        let selected = self.selected.is_some_and(|s| std::ptr::eq(s, icon));
                        if selected {
                            ui.painter().rect_filled(rect.shrink(2.0), 6.0, accent);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect.shrink(2.0), 6.0, hover);
                        }
                        let tex = match self.textures.get(&(icon.id, px)) {
                            Some(t) => Some(t.clone()),
                            None if budget > 0 => {
                                budget -= 1;
                                let rgba = icon.render(px as u16, [255, 255, 255, 255]);
                                let img = ColorImage::from_rgba_premultiplied([px as usize, px as usize], &rgba);
                                let t = ui.ctx().load_texture(format!("craft-icon-{}-{px}", icon.id), img, TextureOptions::LINEAR);
                                self.textures.insert((icon.id, px), t.clone());
                                Some(t)
                            }
                            None => {
                                pending = true;
                                None
                            }
                        };
                        let glyph = Rect::from_center_size(rect.center(), vec2(GLYPH, GLYPH));
                        match tex {
                            Some(t) => {
                                ui.painter().image(t.id(), glyph, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
                            }
                            None => {
                                ui.painter().rect_filled(glyph.shrink(8.0), 4.0, Color32::from_gray(128).gamma_multiply(0.15));
                            }
                        }
                        let resp = resp.on_hover_text(format!("{}\n{}", icon.label, icon.category));
                        if resp.clicked() {
                            self.selected = Some(icon);
                            if self.single_click {
                                picked = Some(icon);
                            }
                        }
                        if resp.double_clicked() {
                            picked = Some(icon);
                        }
                    }
                });
            }
        });
        if pending {
            ui.ctx().request_repaint();
        }
        picked
    }
}

/// 5567 → "5,567".
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (k, c) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(5567), "5,567");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn the_picker_draws_and_searches_headless() {
        let ctx = egui::Context::default();
        let mut picker = IconPicker::new(true);
        picker.query = "dna".into();
        for _ in 0..2 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                picker.show(ui);
            })
            .drop_without_applying_deltas();
        }
        assert!(picker.results.iter().any(|i| i.id == "dna"));
        assert!(!picker.textures.is_empty());
    }
}
