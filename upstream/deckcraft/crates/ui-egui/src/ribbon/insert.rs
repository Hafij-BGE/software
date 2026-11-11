use super::galleries::shapes_gallery;
use super::home::layout_menu;
use super::view::zoom_menu;
use super::{
    CornerRadius, Icon, Rect, Sense, SlideApp, Stroke, Tokens, Ui, big, big_button, big_button_ex, enabled, group, json, menu_item, run, tip, vec2,
};

/// Photo Album is a split button: the main click starts a new album, the arrow edits one.
fn photo_album_button(app: &mut SlideApp, ui: &mut Ui) {
    let en = enabled(app, "insert.photoAlbum");
    let (main, arrow) = big_button_ex(ui, Icon::Picture, "Photo\nAlbum", en, true, false);
    let _ = main.clone().on_hover_text(tip("insert.photoAlbum", "Photo Album…"));
    main.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, en, "Photo Album…"));
    if let Some(a) = &arrow {
        let _ = a.clone().on_hover_text("Photo Album options");
        a.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, en, "Photo Album options"));
    }
    if main.clicked() {
        run(app, "insert.photoAlbum", json!({}));
    }
    if let Some(a) = arrow {
        egui::Popup::menu(&a).show(|ui| {
            menu_item(app, ui, Icon::Picture, "New Photo Album…", "insert.photoAlbum", json!({}));
            menu_item(app, ui, Icon::Picture, "Edit Photo Album…", "insert.photoAlbumEdit", json!({}));
        });
    }
}

// ---------- Insert ----------

pub(super) fn insert(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let (main, arrow) = big_button_ex(ui, Icon::NewSlide, "New\nSlide", enabled(app, "slide.new"), true, false);
        if main.clicked() {
            run(app, "slide.new", json!({}));
        }
        if let Some(a) = arrow {
            egui::Popup::menu(&a).width(420.0).show(|ui| layout_menu(app, ui, "slide.new"));
        }
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Table, "Table", enabled(app, "insert.table"));
        egui::Popup::menu(&r).show(|ui| {
            if let Some((rr, cc)) = table_grid_picker(ui) {
                run(app, "insert.table", json!({"rows": rr, "cols": cc}));
                ui.close();
            }
            ui.separator();
            if ui.button("Insert Table…").clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("table"));
                ui.close();
            }
            if ui.button("Draw Table").clicked() {
                app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: deckcraft_engine::tools::DRAW_TABLE.into() });
                ui.close();
            }
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Picture, "Pictures", enabled(app, "insert.picture"));
        egui::Popup::menu(&r).show(|ui| {
            menu_item(app, ui, Icon::Picture, "This Device…", "app.insertPictureDialog", json!({}));
            menu_item(app, ui, Icon::Picture, "Stock Images…", "app.stockImages", json!({}));
            menu_item(app, ui, Icon::Link, "Online Pictures…", "app.onlinePictures", json!({}));
        });
        photo_album_button(app, ui);
        let r = big_button(ui, Icon::Screenshot, "Screenshot", false);
        let _ = r.on_hover_text("Screenshot (not available yet)");
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Shapes, "Shapes", enabled(app, "shape.insert"));
        egui::Popup::menu(&r).show(|ui| {
            if let Some(p) = shapes_gallery(ui) {
                app.session.set_tool(deckcraft_engine::ToolKind::Shape { preset: p.to_string() });
            }
        });
        let r = big_button(ui, Icon::Icons, "Icons", enabled(app, "insert.icon"));
        egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).width(460.0).show(|ui| icon_library(app, ui));
        big(app, ui, Icon::SmartArt, "SmartArt", "app.dialog", json!({"id": "smartart"}));
        let r = big_button(ui, Icon::Chart, "Chart", enabled(app, "insert.chart"));
        egui::Popup::menu(&r).show(|ui| {
            for k in chart_menu_types() {
                if k.is_none() {
                    ui.separator();
                }
                if let Some(k) = k
                    && ui.button(k.label()).clicked()
                {
                    run(app, "insert.chart", json!({"type": format!("{k:?}")}));
                }
            }
        });
    });
    group(ui, |ui| {
        let r = big_button(ui, Icon::Zoom, "Zoom", enabled(app, "insert.zoom"));
        egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).width(300.0).show(|ui| zoom_menu(app, ui));
        big(app, ui, Icon::Link, "Link", "app.dialog", json!({"id": "hyperlink"}));
        let r = big_button(ui, Icon::Action, "Action", enabled(app, "insert.actionButton"));
        egui::Popup::menu(&r).show(|ui| {
            for k in ["back", "forward", "beginning", "end", "home", "information", "return", "movie", "document", "sound", "help", "custom"] {
                if ui.button(format!("Action Button: {k}")).clicked() {
                    run(app, "insert.actionButton", json!({"kind": k}));
                }
            }
            ui.separator();
            let has_sel = app.session.active().is_some_and(|s| !s.selection.shapes.is_empty());
            if ui.add_enabled(has_sel, egui::Button::new("Action Settings…")).clicked() {
                app.dialog = Some(crate::dialogs::Dialog::new("action"));
            }
        });
    });
    group(ui, |ui| {
        big(app, ui, Icon::Comment, "Comment", "app.dialog", json!({"id": "comment"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::TextBox, "Text\nBox", "insert.textBox", json!({}));
        big(app, ui, Icon::HeaderFooter, "Header &\nFooter", "app.dialog", json!({"id": "headerFooter"}));
        let r = big_button(ui, Icon::WordArt, "WordArt", enabled(app, "insert.wordArt"));
        egui::Popup::menu(&r).show(|ui| {
            for i in 0..6 {
                if ui.button(format!("WordArt style {}", i + 1)).clicked() {
                    run(app, "insert.wordArt", json!({"style": i}));
                }
            }
        });
        big(app, ui, Icon::DateTime, "Date &\nTime", "insert.dateTime", json!({}));
        big(app, ui, Icon::SlideNumber, "Slide\nNumber", "insert.slideNumber", json!({}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::Equation, "Equation", "app.dialog", json!({"id": "equation"}));
        big(app, ui, Icon::Symbol, "Symbol", "app.dialog", json!({"id": "symbol"}));
    });
    group(ui, |ui| {
        big(app, ui, Icon::Video, "Video", "app.insertVideoDialog", json!({}));
        big(app, ui, Icon::Audio, "Audio", "app.insertAudioDialog", json!({}));
    });
}

/// Hover-to-size table grid. Returns (rows, cols) when clicked.
pub fn table_grid_picker(ui: &mut Ui) -> Option<(usize, usize)> {
    let t = Tokens::get(ui.ctx());
    let id = ui.scope_id().with("tgrid");
    let mut hover: (usize, usize) = ui.data_mut(|d| d.get_temp(id).unwrap_or((0, 0)));
    let cell = 16.0;
    let label = if hover.0 > 0 { format!("{}x{} Table", hover.1, hover.0) } else { "Insert Table".into() };
    ui.label(label);
    let (rect, resp) = ui.allocate_exact_size(vec2(10.0 * (cell + 2.0), 8.0 * (cell + 2.0)), Sense::click());
    if let Some(p) = resp.hover_pos() {
        let c = ((p.x - rect.min.x) / (cell + 2.0)).floor() as usize + 1;
        let r = ((p.y - rect.min.y) / (cell + 2.0)).floor() as usize + 1;
        hover = (r.min(8), c.min(10));
    }
    for r in 0..8 {
        for c in 0..10 {
            let cr = Rect::from_min_size(rect.min + vec2(c as f32 * (cell + 2.0), r as f32 * (cell + 2.0)), vec2(cell, cell));
            let on = r < hover.0 && c < hover.1;
            ui.painter().rect(
                cr,
                CornerRadius::ZERO,
                if on { t.accent.gamma_multiply(0.3) } else { t.card },
                Stroke::new(1.0, if on { t.accent } else { t.border }),
                egui::StrokeKind::Inside,
            );
        }
    }
    ui.data_mut(|d| d.insert_temp(id, hover));
    if resp.clicked() && hover.0 > 0 { Some(hover) } else { None }
}

/// Insert › Icons: the shared 5,500-icon gallery (search, categories). One click inserts the
/// icon as an editable shape; the full Stock Images window is one more click away.
fn icon_library(app: &mut SlideApp, ui: &mut Ui) {
    let mut picked = None;
    ui.allocate_ui(vec2(440.0, 400.0), |ui| {
        ui.set_min_size(vec2(440.0, 400.0));
        picked = app.icon_picker.show(ui);
    });
    ui.separator();
    if ui.button("More in Stock Images…").clicked() {
        run(app, "app.stockImages", json!({"tab": "icons"}));
        ui.close();
    }
    if let Some(icon) = picked {
        run(app, "insert.icon", json!({"name": icon.id, "source": "gallery"}));
        ui.close();
    }
}

/// The chart types in the Insert Chart and Change Chart Type menus: the main ones, a separator
/// (`None`), then the Office 2016 types not already listed.
pub(super) fn chart_menu_types() -> Vec<Option<deckcraft_model::chart::ChartType>> {
    use deckcraft_model::chart::ChartType;
    let mut v: Vec<Option<ChartType>> = ChartType::MAIN.iter().copied().map(Some).collect();
    v.push(None);
    v.extend(ChartType::OFFICE_2016.iter().copied().filter(|k| !ChartType::MAIN.contains(k)).map(Some));
    v
}
