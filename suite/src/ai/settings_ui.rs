//! Settings › Ask AI: the on/off switch, the channels and their keys, and removing it all.

use egui::{Color32, RichText, Stroke, Ui};

use super::channels::{Channel, Provider, new_id, now, when};
use super::keys::{self, State};
use crate::app::SuiteApp;
use crate::tabbar::Palette;

const GREEN: Color32 = Color32::from_rgb(0x1f, 0x9d, 0x55);
const AMBER: Color32 = Color32::from_rgb(0xc7, 0x7c, 0x02);

pub fn settings_card(app: &mut SuiteApp, ui: &mut Ui, pal: &Palette) {
    crate::pages::card(ui, pal, "Ask AI", "An assistant beside your tabs, through AI providers you choose, with your own keys.", |ui| {
        let ai = &mut app.ai;
        let ctx = ui.ctx().clone();
        ui.checkbox(&mut ai.config.enabled, "Turn on Ask AI").on_hover_text("Off: no Ask AI button and nothing is sent anywhere. Your channels and keys are kept.");
        if !ai.config.enabled {
            ui.label(RichText::new("Ask AI is off.").size(12.5).weak());
            remove_all(ai, ui);
            return;
        }
        ai.keys.ensure_started();
        ui.checkbox(&mut ai.config.private, "New chats start Private (full text, speaker notes and data rows stay out)");
        ui.add_space(8.0);
        match &ai.keys.state {
            State::Loading | State::Idle => {
                ui.label(RichText::new("Reading your keys…").size(12.5).weak());
            }
            State::Failed(e) => {
                ui.label(RichText::new(format!("Your keys couldn't be read: {e}")).size(12.5).color(Color32::from_rgb(0xd0, 0x3b, 0x2f)));
            }
            State::Ready => {
                ui.label(RichText::new(format!("Keys are kept in {} — never in the settings file, the program or the repository.", ai.keys.backend.label())).size(12.5).weak());
            }
        }
        if let Some(e) = &ai.keys.save_error {
            ui.label(RichText::new(format!("A key couldn't be saved: {e}")).size(12.5).color(Color32::from_rgb(0xd0, 0x3b, 0x2f)));
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Channels").strong());
        ui.label(RichText::new("Tried in this order: the first one that can answer does. One that hits a limit rests until the limit resets.").size(12.5).weak());
        ui.add_space(6.0);

        let n = ai.config.channels.len();
        let mut remove: Option<usize> = None;
        let mut swap: Option<(usize, usize)> = None;
        let t = now();
        for i in 0..n {
            egui::Frame::NONE.stroke(Stroke::new(1.0, pal.border)).corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let ch = ai.config.channels[i].clone();
                let id = ch.id.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{}.", i + 1)).strong());
                    let mut on = ch.on;
                    if ui.checkbox(&mut on, "On").changed() {
                        ai.config.channels[i].on = on;
                    }
                    let mut provider = ch.provider;
                    egui::ComboBox::from_id_salt(("ai_provider", &id)).selected_text(provider.label()).show_ui(ui, |ui| {
                        for p in Provider::ALL {
                            ui.selectable_value(&mut provider, p, p.label());
                        }
                    });
                    if provider != ch.provider {
                        let c = &mut ai.config.channels[i];
                        c.provider = provider;
                        c.model = provider.default_model().into();
                        ai.config.rests.remove(&id);
                        ai.models.remove(&id);
                        ai.tests.remove(&id);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("↓").small()).on_hover_text("Try later").clicked() {
                            swap = Some((i, i + 1));
                        }
                        if ui.add_enabled(i > 0, egui::Button::new("↑").small()).on_hover_text("Try sooner").clicked() {
                            swap = Some((i - 1, i));
                        }
                    });
                });
                // Key
                ui.horizontal_wrapped(|ui| {
                    ui.label("Key:");
                    let editing = ai.key_edit.contains_key(&id);
                    if editing {
                        let buf = ai.key_edit.get_mut(&id).expect("editing");
                        ui.add(egui::TextEdit::singleline(buf).password(true).desired_width(260.0).hint_text("Paste your key here"));
                        let typed = buf.trim().to_string();
                        if ui.add_enabled(!typed.is_empty(), egui::Button::new("Save key")).clicked() {
                            ai.keys.set(&id, &typed);
                            ai.key_edit.remove(&id);
                            // A new key ends a "key refused" rest.
                            ai.config.rests.remove(&id);
                            ai.tests.remove(&id);
                            if ai.config.channels[i].model.trim().is_empty() {
                                ai.find_models(&ctx, &id);
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            ai.key_edit.remove(&id);
                        }
                    } else {
                        match ai.keys.get(&id) {
                            Some(k) => {
                                ui.label(RichText::new(keys::masked(k)).monospace());
                                if ui.small_button("Change").clicked() {
                                    ai.key_edit.insert(id.clone(), String::new());
                                }
                                if ui.small_button("Delete key").clicked() {
                                    ai.keys.remove(&id);
                                }
                            }
                            None => {
                                if ui.button("Add key").clicked() {
                                    ai.key_edit.insert(id.clone(), String::new());
                                }
                                ui.hyperlink_to(RichText::new(format!("Get a free {} key", ch.provider.label())).size(12.5), ch.provider.keys_page());
                            }
                        }
                    }
                });
                // Model
                ui.horizontal_wrapped(|ui| {
                    ui.label("Model:");
                    let model = &mut ai.config.channels[i].model;
                    ui.add(egui::TextEdit::singleline(model).desired_width(240.0).hint_text(if ch.provider == Provider::OpenRouter { "auto" } else { "choose with Find models" }));
                    if let Some(Ok(list)) = ai.models.get(&id) {
                        egui::ComboBox::from_id_salt(("ai_models", &id)).selected_text(format!("{} models", list.len())).height(320.0).show_ui(ui, |ui| {
                            for m in list.iter().take(400) {
                                let label = if m == "auto" { "auto — free models, strongest first".to_string() } else { m.clone() };
                                if ui.selectable_label(*model == *m, label).clicked() {
                                    *model = m.clone();
                                }
                            }
                        });
                    }
                    let finding = ai.finding.contains(&id);
                    if ui.add_enabled(!finding, egui::Button::new(if finding { "Finding…" } else { "Find models" })).on_hover_text("List the provider's models now").clicked() {
                        ai.find_models(&ctx, &id);
                    }
                    let testing = matches!(ai.tests.get(&id), Some(None));
                    if ui.add_enabled(!testing, egui::Button::new(if testing { "Testing…" } else { "Test this channel" })).clicked() {
                        ai.test(&ctx, &id);
                    }
                });
                if ch.provider == Provider::OpenRouter && ch.model.trim() == "auto" {
                    ui.label(RichText::new("auto: reads OpenRouter's free models now, strongest first (by its intelligence index where listed, else size), and tries them in turn.").size(12.0).weak());
                }
                // Status
                let status = if !ch.on {
                    ("Off".to_string(), ui.visuals().weak_text_color())
                } else if let Some(r) = ai.config.resting(&id, t) {
                    if r.until > t + 300 * 86_400 { (format!("Stopped: {} — change the key or test again", r.reason), AMBER) } else { (format!("Resting until {} ({})", when(r.until, t), r.reason), AMBER) }
                } else if ai.keys.get(&id).is_none() {
                    ("No key yet".to_string(), AMBER)
                } else if ch.model.trim().is_empty() {
                    ("Choose a model (Find models)".to_string(), AMBER)
                } else {
                    ("Ready".to_string(), GREEN)
                };
                ui.label(RichText::new(status.0).size(12.5).color(status.1));
                match ai.tests.get(&id) {
                    Some(Some(Ok(m))) => {
                        ui.label(RichText::new(m).size(12.5).color(GREEN));
                    }
                    Some(Some(Err(e))) => {
                        ui.label(RichText::new(format!("Test failed: {e}")).size(12.5).color(Color32::from_rgb(0xd0, 0x3b, 0x2f)));
                    }
                    _ => {}
                }
                if let Some(Err(e)) = ai.models.get(&id) {
                    ui.label(RichText::new(format!("Find models: {e}")).size(12.5).color(Color32::from_rgb(0xd0, 0x3b, 0x2f)));
                }
            });
            ui.add_space(6.0);
        }
        if let Some((a, b)) = swap {
            ai.config.channels.swap(a, b);
        }
        if let Some(i) = remove {
            let ch = ai.config.channels.remove(i);
            ai.keys.remove(&ch.id);
            ai.config.rests.remove(&ch.id);
            ai.key_edit.remove(&ch.id);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Add a channel:");
            for p in Provider::ALL {
                if ui.button(p.label()).clicked() {
                    let id = new_id();
                    ai.config.channels.push(Channel { id: id.clone(), provider: p, model: p.default_model().into(), on: true });
                    ai.key_edit.insert(id, String::new());
                }
            }
        });
        ui.add_space(10.0);
        ui.label(
            RichText::new(
                "Good to know: free models are weaker than paid ones. Free plans may use what you send to improve their models \
                 (Google's and Mistral's free tiers say so; OpenRouter's free models depend on the provider behind them), so keep Private on \
                 for anything sensitive. Each key is sent only to its own provider.",
            )
            .size(12.5)
            .weak(),
        );
        ui.add_space(8.0);
        remove_all(ai, ui);
    });
}

fn remove_all(ai: &mut super::Ai, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        if ai.confirm_remove {
            ui.label(RichText::new("Delete every key, channel and chat, and turn Ask AI off?").strong());
            if ui.button(RichText::new("Delete all").color(Color32::from_rgb(0xd0, 0x3b, 0x2f))).clicked() {
                ai.remove_all();
                ai.confirm_remove = false;
            }
            if ui.button("Cancel").clicked() {
                ai.confirm_remove = false;
            }
        } else if ui.button("Remove all AI data…").on_hover_text("Deletes your keys (from the keyring too), channels and chats").clicked() {
            ai.confirm_remove = true;
        }
    });
}
