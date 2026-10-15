//! The Ask AI panel at the window's right edge, and its button in the tab bar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use super::context::{self, Gathered};
use super::stats::{self, Summary};
use super::{GENERAL, Msg, Role, active_chat, markdown, router};
use crate::app::{Content, SuiteApp};
use crate::kind::Kind;
use crate::tabbar::palette;

/// Conversation turns sent with a question (the newest ones).
const HISTORY: usize = 12;
const MAX_FILE: u64 = 50 * 1024 * 1024;

fn suite_visuals(app: &SuiteApp, ctx: &egui::Context) -> egui::Visuals {
    if crate::tabbar::is_dark(app, ctx) { egui::Visuals::dark() } else { egui::Visuals::light() }
}

/// The Ask AI button in the tab bar (`size`: its slot). Shown while Ask AI is on.
pub fn bar_button(app: &mut SuiteApp, ui: &mut Ui, size: egui::Vec2) {
    if !app.ai.config.enabled {
        return;
    }
    let (_, slot) = ui.allocate_space(size);
    let rect = egui::Rect::from_center_size(slot.center(), vec2(slot.width() - 6.0, 28.0_f32.min(slot.height() - 4.0)));
    let resp = ui.interact(rect, ui.id().with("craft_suite_ask_ai"), Sense::click());
    let v = ui.visuals();
    let open = app.ai.open;
    let accent = v.selection.bg_fill;
    let fill = if open { accent.gamma_multiply(0.35) } else if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { Color32::TRANSPARENT };
    ui.painter().rect(rect, CornerRadius::same(8), fill, Stroke::new(1.0, if open { accent } else { v.widgets.noninteractive.bg_stroke.color }), egui::StrokeKind::Inside);
    // A four-pointed sparkle, drawn (fonts may lack the glyph).
    let c = egui::pos2(rect.left() + 14.0, rect.center().y);
    let (r, w) = (6.0, 1.8);
    let pts = vec![
        egui::pos2(c.x, c.y - r),
        egui::pos2(c.x + w, c.y - w),
        egui::pos2(c.x + r, c.y),
        egui::pos2(c.x + w, c.y + w),
        egui::pos2(c.x, c.y + r),
        egui::pos2(c.x - w, c.y + w),
        egui::pos2(c.x - r, c.y),
        egui::pos2(c.x - w, c.y - w),
    ];
    ui.painter().add(egui::Shape::convex_polygon(pts, accent, Stroke::NONE));
    ui.painter().text(egui::pos2(rect.left() + 25.0, rect.center().y), egui::Align2::LEFT_CENTER, "Ask AI", egui::FontId::proportional(13.0), v.text_color());
    if resp.on_hover_text("Ask AI about this tab").clicked() {
        app.ai.open = !app.ai.open;
    }
}

/// The width the bar button takes.
pub const BUTTON_W: f32 = 84.0;

fn suggestions(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Word => &["Summarise this document", "Suggest a clearer title", "Improve the selected text", "Make an outline with headings"],
        Kind::Deck => &["Summarise this presentation", "Suggest a slide outline for this topic", "Write speaker notes for the current slide", "Add a closing slide with key takeaways"],
        Kind::Grid => &["Analyse this sheet", "Explain what this sheet shows", "Suggest a chart for this data", "Add a total row"],
        Kind::Mark => &["Explain Markdown table syntax", "How do I add a footnote in Markdown?", "Attach a CSV file to analyse"],
        _ => &["What can you help me with?", "Attach a CSV file to analyse"],
    }
}

pub fn panel(app: &mut SuiteApp, ui: &mut Ui) {
    if !app.ai.config.enabled || !app.ai.open {
        return;
    }
    let ctx = ui.ctx().clone();
    let visuals = suite_visuals(app, &ctx);
    let pal = palette(visuals.dark_mode);
    let (chat_id, kind) = active_chat(app);
    let title = app.tab(app.active).map(|t| t.title()).unwrap_or_default();
    let width = app.ai.config.panel_width;
    let resp = egui::Panel::right("craft_suite_ai")
        .resizable(true)
        .default_size(width)
        .size_range(300.0..=900.0)
        .frame(egui::Frame::NONE.fill(pal.page).stroke(Stroke::new(1.0, pal.border)).inner_margin(egui::Margin { left: 14, right: 12, top: 10, bottom: 10 }))
        .show(ui, |ui| {
            *ui.visuals_mut() = visuals.clone();
            ui.style_mut().override_font_id = None;
            body(app, ui, chat_id, kind, &title, &pal);
        });
    let w = resp.response.rect.width();
    if (w - app.ai.config.panel_width).abs() > 4.0 && !ctx.input(|i| i.pointer.any_down()) {
        app.ai.config.panel_width = w.clamp(300.0, 900.0);
    }
}

fn body(app: &mut SuiteApp, ui: &mut Ui, chat_id: u64, kind: Kind, title: &str, pal: &crate::tabbar::Palette) {
    // ---- header
    ui.horizontal(|ui| {
        ui.label(RichText::new("Ask AI").size(17.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(RichText::new("×").size(16.0)).frame(false)).on_hover_text("Close the panel").clicked() {
                app.ai.open = false;
            }
            let chat = app.ai.chat(chat_id, kind);
            if ui.add_enabled(!chat.msgs.is_empty() || chat.data.is_some(), egui::Button::new("New chat")).clicked() {
                if let Some(p) = chat.pending.take() {
                    p.store(true, Ordering::Relaxed);
                }
                chat.msgs.clear();
                chat.data = None;
            }
        });
    });
    let about = if chat_id == GENERAL { "General: no document".to_string() } else { format!("{}: {title}", kind.noun()) };
    let chat = app.ai.chat(chat_id, kind);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(about).size(12.5).weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(&mut chat.private, RichText::new("Private").size(12.5)).on_hover_text(
                "On: the AI sees your question, headings or slide titles, the selected text and statistics the app computes. \
                 Full text, speaker notes and data rows stay out.\nOff: they're sent too (up to a size limit).",
            );
        });
    });
    ui.add_space(4.0);
    ui.separator();

    let ready = !app.ai.usable().is_empty();
    let loading = matches!(app.ai.keys.state, super::keys::State::Loading);

    // ---- the input, at the bottom
    let mut send: Option<String> = None;
    let mut analyse = false;
    let mut attach = false;
    egui::Panel::bottom("craft_suite_ai_input").frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 0, right: 0, top: 8, bottom: 0 })).show(ui, |ui| {
        let chat = app.ai.chat(chat_id, kind);
        if let Some((name, _)) = &chat.data {
            let mut remove = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Data: {name} (statistics only)")).size(12.5));
                if ui.small_button("Remove").clicked() {
                    remove = true;
                }
            });
            if remove {
                chat.data = None;
            }
        }
        let id = egui::Id::new(("craft_suite_ai_input", chat_id));
        let busy = chat.pending.is_some();
        // Enter sends; Shift+Enter starts a new line.
        if ui.memory(|m| m.has_focus(id)) && !busy && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) && !chat.input.trim().is_empty() {
            send = Some(std::mem::take(&mut chat.input));
        }
        ui.add(
            egui::TextEdit::multiline(&mut chat.input)
                .id(id)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text(if ready { "Ask about this tab… (Enter sends, Shift+Enter: new line)" } else { "Add an AI channel in Settings › Ask AI first" }),
        );
        ui.horizontal(|ui| {
            if ui.button("Attach CSV…").on_hover_text("Statistics of a CSV or TSV file, computed here; only the numbers go to the AI").clicked() {
                attach = true;
            }
            if kind == Kind::Grid && ui.button("Analyse sheet").on_hover_text("Statistics of the active sheet, computed here").clicked() {
                analyse = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if busy {
                    if ui.button("Stop").clicked()
                        && let Some(p) = chat.pending.take()
                    {
                        p.store(true, Ordering::Relaxed);
                        chat.msgs.push(Msg::new(Role::Note, "Stopped."));
                    }
                    ui.spinner();
                } else if ui.add_enabled(!chat.input.trim().is_empty(), egui::Button::new(RichText::new("Send").strong())).clicked() {
                    send = Some(std::mem::take(&mut chat.input));
                }
            });
        });
    });

    // ---- the conversation
    let mut suggestion: Option<&'static str> = None;
    let mut open_settings = false;
    let mut apply: Option<usize> = None;
    let mut dismiss: Option<usize> = None;
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
        let chat = app.ai.chats.get_mut(&chat_id).expect("chat");
        let scroll_to_end = std::mem::take(&mut chat.scroll);
        egui::ScrollArea::vertical().id_salt(("craft_suite_ai_msgs", chat_id)).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            ui.add_space(6.0);
            if !ready {
                egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if loading {
                        ui.label("Reading your keys…");
                    } else {
                        ui.label(RichText::new("No AI channel yet").strong());
                        ui.label("Add a channel with your own free key from OpenRouter, Google Gemini, Groq or Mistral. Keys stay on this computer.");
                        if ui.button("Open Settings › Ask AI").clicked() {
                            open_settings = true;
                        }
                    }
                });
                ui.add_space(8.0);
            }
            if chat.msgs.is_empty() {
                ui.label(RichText::new("Try:").weak());
                for s in suggestions(kind) {
                    if ui.add(egui::Button::new(RichText::new(*s).size(13.0)).corner_radius(14)).clicked() {
                        suggestion = Some(s);
                    }
                }
            }
            for (i, m) in chat.msgs.iter().enumerate() {
                message(ui, pal, m, i, &mut apply, &mut dismiss);
                ui.add_space(8.0);
            }
            if chat.pending.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Thinking…").weak());
                });
            }
            if scroll_to_end {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
            }
        });
    });

    if open_settings {
        app.actions.push(crate::app::Action::Settings);
    }
    if let Some(s) = suggestion {
        match s {
            "Analyse this sheet" => analyse = true,
            "Attach a CSV file to analyse" => attach = true,
            _ => send = Some(s.to_string()),
        }
    }
    if attach {
        attach_file(app.ai.chat(chat_id, kind));
    }
    if analyse {
        analyse_sheet(app, chat_id, kind);
    }
    if let Some(i) = dismiss
        && let Some(m) = app.ai.chats.get_mut(&chat_id).and_then(|c| c.msgs.get_mut(i))
    {
        m.applied = Some(Err("Dismissed.".into()));
    }
    if let Some(i) = apply {
        apply_steps(app, chat_id, i);
    }
    if let Some(q) = send {
        ask(app, ui.ctx(), chat_id, kind, q);
    }
}

fn message(ui: &mut Ui, pal: &crate::tabbar::Palette, m: &Msg, i: usize, apply: &mut Option<usize>, dismiss: &mut Option<usize>) {
    let v = ui.visuals().clone();
    match m.role {
        Role::User => {
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                egui::Frame::NONE.fill(v.selection.bg_fill.gamma_multiply(0.25)).corner_radius(10).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
                    ui.set_max_width(ui.available_width() * 0.85);
                    ui.add(egui::Label::new(RichText::new(&m.text).size(13.5)).wrap());
                });
                for n in &m.notes {
                    ui.label(RichText::new(n).size(11.0).weak());
                }
            });
        }
        Role::Note | Role::Error => {
            let color = if m.role == Role::Error { Color32::from_rgb(0xd0, 0x3b, 0x2f) } else { v.weak_text_color() };
            egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if !m.text.is_empty() {
                    ui.label(RichText::new(&m.text).size(13.0).color(color));
                }
                if let Some((name, s)) = &m.stats {
                    stats_table(ui, name, s);
                }
                notes(ui, m, i);
            });
        }
        Role::Assistant => {
            egui::Frame::NONE.fill(pal.card).stroke(Stroke::new(1.0, pal.border)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                markdown::show(ui, &m.text, 13.5);
                if !m.steps.is_empty() {
                    ui.add_space(6.0);
                    egui::Frame::NONE.fill(v.faint_bg_color).corner_radius(8).inner_margin(8).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(format!("Suggested changes ({})", m.steps.len())).strong().size(13.0));
                        for s in &m.steps {
                            ui.label(RichText::new(format!("• {}", s.describe())).size(12.5).monospace());
                        }
                        match &m.applied {
                            None => {
                                ui.horizontal(|ui| {
                                    if ui.button(RichText::new("Apply").strong()).on_hover_text("Make these changes in this tab (undo with Ctrl+Z)").clicked() {
                                        *apply = Some(i);
                                    }
                                    if ui.button("Dismiss").clicked() {
                                        *dismiss = Some(i);
                                    }
                                });
                            }
                            Some(Ok(t)) => {
                                ui.label(RichText::new(t).size(12.5).color(Color32::from_rgb(0x1f, 0x9d, 0x55)));
                            }
                            Some(Err(t)) => {
                                ui.label(RichText::new(t).size(12.5).weak());
                            }
                        }
                    });
                }
                if !m.refused.is_empty() {
                    ui.label(RichText::new(format!("Left out (not allowed): {}", m.refused.join(", "))).size(12.0).weak());
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if let Some(meta) = &m.meta {
                        ui.label(RichText::new(format!("Answered by {meta}")).size(11.5).weak());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Copy").on_hover_text("Copy the answer").clicked() {
                            ui.ctx().copy_text(m.text.clone());
                        }
                    });
                });
                notes(ui, m, i);
            });
        }
    }
}

fn notes(ui: &mut Ui, m: &Msg, i: usize) {
    if m.notes.is_empty() {
        return;
    }
    let label = if m.role == Role::Assistant { format!("Switched channels ({})", m.notes.len()) } else { format!("What happened ({})", m.notes.len()) };
    egui::CollapsingHeader::new(RichText::new(label).size(11.5).weak()).id_salt(("ai_notes", i)).default_open(m.role == Role::Error).show(ui, |ui| {
        for n in &m.notes {
            ui.label(RichText::new(format!("• {n}")).size(11.5).weak());
        }
    });
}

fn stats_table(ui: &mut Ui, name: &str, s: &Summary) {
    ui.label(RichText::new(format!("{name}: {} rows, {} columns", s.rows, s.columns.len())).strong().size(13.0));
    egui::ScrollArea::horizontal().id_salt(("ai_stats", name)).show(ui, |ui| {
        egui::Grid::new(("ai_stats_grid", name)).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
            for h in ["Column", "n", "Missing", "Mean ± SD", "Median (IQR)", "Min – Max"] {
                ui.label(RichText::new(h).strong().size(12.0));
            }
            ui.end_row();
            for c in &s.columns {
                ui.label(RichText::new(&c.name).size(12.0));
                ui.label(RichText::new(c.n.to_string()).size(12.0));
                ui.label(RichText::new(c.missing.to_string()).size(12.0));
                match &c.numeric {
                    Some(x) => {
                        ui.label(RichText::new(format!("{} ± {}", stats::fmt(x.mean), x.sd.map(stats::fmt).unwrap_or("–".into()))).size(12.0));
                        ui.label(RichText::new(format!("{} ({} – {})", stats::fmt(x.median), stats::fmt(x.q1), stats::fmt(x.q3))).size(12.0));
                        ui.label(RichText::new(format!("{} – {}", stats::fmt(x.min), stats::fmt(x.max))).size(12.0));
                    }
                    None => {
                        let top: Vec<String> = c.top.iter().take(3).map(|(v, n)| format!("{v} ({n})")).collect();
                        ui.label(RichText::new(format!("text, {} values", c.distinct)).size(12.0).weak());
                        ui.label(RichText::new(top.join(", ")).size(12.0).weak());
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    });
    for cmp in &s.comparisons {
        let g: Vec<String> = cmp.groups.iter().map(|g| format!("{}: {} ± {} (n={})", g.level, stats::fmt(g.mean), g.sd.map(stats::fmt).unwrap_or("–".into()), g.n)).collect();
        ui.label(RichText::new(format!("{} by {}: {}", cmp.value, cmp.by, g.join("; "))).size(12.0));
    }
    for c in &s.correlations {
        ui.label(RichText::new(format!("{} vs {}: r = {} (n={})", c.a, c.b, stats::fmt(c.r), c.n)).size(12.0));
    }
}

/// Attach CSV…: the file's statistics, computed here.
fn attach_file(chat: &mut super::Chat) {
    let Some(path) = rfd::FileDialog::new().add_filter("CSV or TSV", &["csv", "tsv", "tab", "txt"]).pick_file() else { return };
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let result = (|| {
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if size > MAX_FILE {
            return Err(format!("{name} is larger than 50 MB"));
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let table = stats::Table::from_rows(stats::parse_delimited(&text, stats::guess_delimiter(&name, &text)));
        if table.headers.is_empty() {
            return Err(format!("{name} has no rows"));
        }
        Ok(stats::summarize(&table))
    })();
    add_data(chat, name, result);
}

/// Analyse sheet: the active sheet's statistics (its used range, the first row as headers).
fn analyse_sheet(app: &mut SuiteApp, chat_id: u64, kind: Kind) {
    let result = match app.tabs.iter_mut().find(|t| t.id == chat_id).map(|t| &mut t.content) {
        Some(Content::Doc(doc)) => match context::sheet_table(doc, None) {
            Some((range, table)) if !table.headers.is_empty() => Ok((range, stats::summarize(&table))),
            _ => Err("The sheet is empty.".to_string()),
        },
        _ => Err("No sheet here.".to_string()),
    };
    let chat = app.ai.chat(chat_id, kind);
    match result {
        Ok((range, s)) => {
            add_data(chat, range, Ok(s));
            // Every question on this tab sends the sheet's statistics already.
            chat.data = None;
        }
        Err(e) => add_data(chat, String::new(), Err(e)),
    }
}

fn add_data(chat: &mut super::Chat, name: String, result: Result<Summary, String>) {
    match result {
        Ok(s) => {
            let mut m = Msg::new(Role::Note, "Computed here. Only these numbers are sent to the AI, never the rows.");
            m.stats = Some((name.clone(), s.clone()));
            chat.msgs.push(m);
            chat.data = Some((name, s));
            if chat.input.trim().is_empty() {
                chat.input = "Explain the main findings in these statistics.".into();
            }
        }
        Err(e) => chat.msgs.push(Msg::new(Role::Error, e)),
    }
    chat.scroll = true;
}

fn apply_steps(app: &mut SuiteApp, chat_id: u64, i: usize) {
    let steps = match app.ai.chats.get(&chat_id).and_then(|c| c.msgs.get(i)) {
        Some(m) if m.applied.is_none() => m.steps.clone(),
        _ => return,
    };
    let result = match app.tabs.iter_mut().find(|t| t.id == chat_id).map(|t| &mut t.content) {
        Some(Content::Doc(doc)) => match context::apply(doc, &steps) {
            Ok(n) => Ok(format!("Applied {n} change{} — undo with Ctrl+Z.", if n == 1 { "" } else { "s" })),
            Err((done, e)) => Err(format!("Applied {done} of {}; stopped: {e}", steps.len())),
        },
        _ => Err("This tab has no document to change.".into()),
    };
    if let Some(m) = app.ai.chats.get_mut(&chat_id).and_then(|c| c.msgs.get_mut(i)) {
        m.applied = Some(result);
    }
}

/// Sends a question: gathers the tab's context now, then asks on a background thread.
fn ask(app: &mut SuiteApp, ctx: &egui::Context, chat_id: u64, kind: Kind, question: String) {
    let question = question.trim().to_string();
    if question.is_empty() {
        return;
    }
    let private = app.ai.chat(chat_id, kind).private;
    let title = app.tab(chat_id).map(|t| t.title()).unwrap_or_default();
    let gathered = match app.tabs.iter_mut().find(|t| t.id == chat_id).map(|t| &mut t.content) {
        Some(Content::Doc(doc)) => context::gather(doc, &title, private),
        _ => Gathered { prompt: String::new(), shared: String::new() },
    };
    let channels = app.ai.usable();
    let rests = app.ai.config.rests.clone();
    let chat = app.ai.chat(chat_id, kind);
    let data = chat.data.as_ref().map(|(name, s)| format!("{name}\n{}", s.to_prompt())).unwrap_or_default();
    let mut messages = vec![super::client::Message { role: "system", content: context::system_prompt(kind, &gathered.prompt, &data) }];
    let history: Vec<&Msg> = chat.msgs.iter().filter(|m| matches!(m.role, Role::User | Role::Assistant)).collect();
    for m in history.iter().skip(history.len().saturating_sub(HISTORY)) {
        let content: String = m.text.chars().take(4000).collect();
        messages.push(super::client::Message { role: if m.role == Role::User { "user" } else { "assistant" }, content });
    }
    messages.push(super::client::Message { role: "user", content: question.clone() });
    let mut m = Msg::new(Role::User, question);
    if !gathered.shared.is_empty() || chat.data.is_some() {
        let mut shared = gathered.shared.clone();
        if chat.data.is_some() {
            shared = if shared.is_empty() { "statistics of the attached data".into() } else { format!("{shared}, statistics of the attached data") };
        }
        m.notes = vec![format!("Shared with the AI: {shared}")];
    }
    chat.msgs.push(m);
    chat.scroll = true;
    let cancel = Arc::new(AtomicBool::new(false));
    chat.pending = Some(cancel.clone());
    let job = router::Job { chat: chat_id, channels, rests, messages, cancel };
    let (tx, ctx) = (app.ai.tx.clone(), ctx.clone());
    std::thread::spawn(move || router::ask(job, tx, move || ctx.request_repaint()));
}
