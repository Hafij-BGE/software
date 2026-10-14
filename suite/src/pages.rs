//! The suite's own pages: Home (new documents, Open, recent files) and Settings.

use egui::{RichText, Ui, vec2};

use crate::app::{Action, SuiteApp};
use crate::kind::Kind;
use crate::settings::{CloseButtons, ColorStyle, Mixing, NewTabOpens, NewTabPlacement, Settings, SuiteTheme, TabPosition};
use crate::update;

fn page_frame(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::CentralPanel::default().frame(egui::Frame::central_panel(ui.style()).inner_margin(24.0)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(900.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), add);
            });
        });
    });
}

pub fn home(app: &mut SuiteApp, ui: &mut Ui) {
    page_frame(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(icon) = app.big_icon(Kind::Home) {
                ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(56.0, 56.0)));
            }
            ui.vertical(|ui| {
                ui.label(RichText::new("Craft Suite").size(26.0).strong());
                ui.label(RichText::new("Documents, presentations and spreadsheets, side by side in tabs").weak());
            });
        });
        ui.add_space(20.0);
        ui.label(RichText::new("New").size(16.0).strong());
        ui.add_space(6.0);
        let order = app.settings.app_order.clone();
        let wide = ui.available_width() >= 680.0;
        let card_w = if wide { (ui.available_width() - 24.0) / 3.0 } else { ui.available_width() };
        let mut cards = |ui: &mut Ui| {
            for k in &order {
                app_card(app, ui, *k, card_w);
            }
        };
        if wide {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                cards(ui);
            });
        } else {
            cards(ui);
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new("Open…").min_size(vec2(110.0, 30.0))).on_hover_text("Open documents, presentations or spreadsheets").clicked() {
                app.actions.push(Action::OpenDialog);
            }
            if ui.add(egui::Button::new("Settings").min_size(vec2(110.0, 30.0))).clicked() {
                app.actions.push(Action::Settings);
            }
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Recent").size(16.0).strong());
            if !app.settings.recent.is_empty() && ui.small_button("Clear").clicked() {
                app.settings.recent.clear();
            }
        });
        ui.add_space(4.0);
        if app.settings.recent.is_empty() {
            ui.label(RichText::new("Files you open show up here.").weak());
        }
        for path in app.settings.recent.clone() {
            let kind = Kind::for_path(&path);
            ui.horizontal(|ui| {
                if let Some(icon) = kind.and_then(|k| app.icon(k)) {
                    ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(18.0, 18.0)));
                }
                let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                let exists = std::path::Path::new(&path).exists();
                let link = ui.add_enabled(exists, egui::Link::new(name));
                if link.clicked() {
                    app.actions.push(Action::Open(path.clone()));
                }
                let dir = std::path::Path::new(&path).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                ui.label(RichText::new(if exists { dir } else { format!("{dir} (missing)") }).weak().small());
            });
        }
        ui.add_space(24.0);
        ui.separator();
        about(app, ui);
    });
}

fn app_card(app: &mut SuiteApp, ui: &mut Ui, kind: Kind, width: f32) {
    let color = app.settings.color_of(kind).color32();
    egui::Frame::group(ui.style()).inner_margin(14.0).corner_radius(10.0).show(ui, |ui| {
        // The cards sit in a row; their own contents run top to bottom.
        ui.vertical(|ui| {
            ui.set_width(width - 30.0);
            ui.horizontal(|ui| {
                if let Some(icon) = app.big_icon(kind) {
                    ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(48.0, 48.0)));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(kind.noun()).size(16.0).strong().color(color));
                    ui.label(RichText::new(kind.app_name()).weak());
                });
            });
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(kind.blurb()).small()).wrap());
            ui.add_space(6.0);
            if ui.add(egui::Button::new(format!("New {}", kind.noun())).min_size(vec2(150.0, 28.0))).clicked() {
                app.actions.push(Action::New { kind, replace_page: true });
            }
        });
    });
}

fn about(app: &mut SuiteApp, ui: &mut Ui) {
    ui.label(RichText::new(format!("Craft Suite {}", update::installed_version())).weak());
    let upstream = update::upstream_summary();
    if !upstream.is_empty() {
        ui.collapsing("Built from", |ui| {
            for line in upstream.lines() {
                ui.label(RichText::new(line).monospace().small());
            }
        });
    }
    app.updater.ui(ui);
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(16.0);
    ui.label(RichText::new(title).size(16.0).strong());
    ui.add_space(4.0);
}

fn radio_row<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        for (v, label) in options {
            ui.radio_value(value, *v, *label);
        }
    });
}

pub fn settings(app: &mut SuiteApp, ui: &mut Ui) {
    page_frame(ui, |ui| {
        ui.label(RichText::new("Settings").size(26.0).strong());
        ui.label(RichText::new("How Craft Suite arranges and colours its tabs. Each app keeps its own options (File › Options in the app).").weak());

        section(ui, "Tab arrangement");
        egui::Grid::new("arrangement").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
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
                    .small(),
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

        section(ui, "Colour");
        egui::Grid::new("colour").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
            let s = &mut app.settings;
            ui.label("Tab bar theme");
            let themes: Vec<(SuiteTheme, &str)> = SuiteTheme::ALL.iter().map(|t| (*t, t.label())).collect();
            radio_row(ui, &mut s.theme, &themes);
            ui.end_row();

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
        ui.label(RichText::new("Right-click a tab to give it a colour of its own.").weak().small());

        section(ui, "Behaviour");
        egui::Grid::new("behaviour").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
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
                    ui.label(RichText::new(format!("{keys}: {what}")).small());
                }
            });
            ui.end_row();
        });

        section(ui, "Updates");
        about(app, ui);

        ui.add_space(20.0);
        if ui.button("Restore default settings").on_hover_text("Recent files and reopened tabs are kept").clicked() {
            let keep = (app.settings.recent.clone(), app.settings.session.clone());
            app.settings = Settings::default();
            (app.settings.recent, app.settings.session) = keep;
        }
    });
}
