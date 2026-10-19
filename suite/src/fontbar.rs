//! The font bar: over a Word, Slide or Spreadsheet tab whose document uses fonts that aren't
//! installed, a bar says so in plain sight ("“Aptos” is not available on this device") and offers
//! to fix it:
//! - **Download font**: fetches the family from Google Fonts (free fonts under the SIL Open Font
//!   Licence) and installs it on this computer, for this user;
//! - **Install from Windows on this PC**: copies Microsoft's fonts (Aptos, Calibri…) from a Windows
//!   installation on the same computer, as the apps' own `font.add {windows}` does;
//! - **Add font files…**: installs the fonts in a folder you pick;
//! - **Choose a similar font**: uses an installed font wherever the document uses the missing one
//!   (one step Undo takes back).
//!
//! The apps' own, smaller notices for the same fonts are left out while the bar shows. Settings ›
//! Behaviour turns the bar off ("Don't show this again" in its card does too).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde_json::json;

use crate::hosts::{Doc, MissingFont};
use crate::kind::Kind;
use crate::settings::Settings;

/// How often the open document's fonts are looked at again.
const RECHECK_SECS: f64 = 2.0;

#[derive(Default)]
pub struct FontBar {
    tabs: HashMap<u64, TabFonts>,
    /// The options card, open for a tab: (tab, the similar-fonts list shown).
    card: Option<(u64, bool)>,
    /// A download in progress.
    download: Option<Download>,
}

/// (tab, family, the folder the files went to or what went wrong).
type Download = (u64, String, Receiver<Result<PathBuf, String>>);

#[derive(Default)]
struct TabFonts {
    key: String,
    checked: f64,
    missing: Vec<MissingFont>,
    /// Documents whose bar was closed.
    closed: HashSet<String>,
    /// Which missing font the card is about (an index into `missing`).
    pick: usize,
    /// The font typed under "Choose a similar font".
    other: String,
    /// The last action's outcome: (text, it worked, when).
    note: Option<(String, bool, f64)>,
}

struct Colors {
    bg: Color32,
    border: Color32,
    red: Color32,
    text: Color32,
    soft: Color32,
    ok_bg: Color32,
    ok: Color32,
}

fn colors(dark: bool) -> Colors {
    if dark {
        Colors {
            bg: Color32::from_rgb(62, 30, 32),
            border: Color32::from_rgb(112, 52, 56),
            red: Color32::from_rgb(236, 88, 76),
            text: Color32::from_gray(238),
            soft: Color32::from_gray(200),
            ok_bg: Color32::from_rgb(26, 56, 38),
            ok: Color32::from_rgb(110, 210, 140),
        }
    } else {
        Colors {
            bg: Color32::from_rgb(253, 236, 236),
            border: Color32::from_rgb(244, 199, 199),
            red: Color32::from_rgb(217, 48, 37),
            text: Color32::from_gray(28),
            soft: Color32::from_gray(70),
            ok_bg: Color32::from_rgb(230, 244, 234),
            ok: Color32::from_rgb(24, 128, 56),
        }
    }
}

/// The bar over tab `tab` (call before the tab's app draws), and its card when open.
pub fn show(bar: &mut FontBar, settings: &mut Settings, tab: u64, doc: &mut Doc, ui: &mut Ui) {
    if !matches!(doc.kind(), Kind::Word | Kind::Deck | Kind::Grid) {
        return;
    }
    doc.quiet_own_font_notice();
    if !settings.font_notice {
        bar.card = None;
        return;
    }
    let now = ui.input(|i| i.time);
    poll_download(bar, tab, doc, now);
    let st = bar.tabs.entry(tab).or_default();
    if now - st.checked > RECHECK_SECS || st.checked == 0.0 {
        st.checked = now;
        if let Some((key, missing)) = doc.missing_fonts() {
            if key != st.key {
                st.key = key;
                st.pick = 0;
            }
            // A font with a metric-compatible twin installed (Calibri and Carlito) lays out the same,
            // so there's nothing to fix.
            st.missing = missing.into_iter().filter(|m| !metric_twin(&m.name).iter().any(|t| doc.has_font(t))).collect();
            st.pick = st.pick.min(st.missing.len().saturating_sub(1));
        }
    }
    ui.ctx().request_repaint_after(Duration::from_secs(2));
    let dark = ui.visuals().dark_mode;
    let c = colors(dark);

    // After an install that left nothing missing: a short "installed" line instead.
    if st.missing.is_empty() || st.closed.contains(&st.key) {
        if bar.card.is_some_and(|(t, _)| t == tab) {
            bar.card = None;
        }
        if let Some((text, true, at)) = &st.note
            && now - at < 6.0
        {
            let text = text.clone();
            egui::Panel::top(egui::Id::new(("craft_suite_fontbar", tab))).frame(egui::Frame::NONE.inner_margin(Margin { left: 10, right: 10, top: 8, bottom: 4 })).show(ui, |ui| {
                egui::Frame::NONE.fill(c.ok_bg).corner_radius(10).inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("✓  {text}")).size(15.0).color(c.ok));
                });
            });
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        return;
    }

    let names: Vec<String> = st.missing.iter().map(|m| m.name.clone()).collect();
    let first = &st.missing[0];
    let title = match names.len() {
        1 => format!("“{}” is not available on this device.", first.name),
        2 => format!("“{}” and “{}” are not available on this device.", names[0], names[1]),
        n => format!("“{}” and {} other fonts are not available on this device.", names[0], n - 1),
    };
    let sub = match (&first.shown_with, names.len()) {
        (Some(with), 1) => format!("{} will use a substitute font ({with}).", doc.kind().app_name()),
        _ => format!("{} will use substitute fonts.", doc.kind().app_name()),
    };
    let mut open_card: Option<bool> = None;
    let mut close = false;
    let mut download_rect = None;
    egui::Panel::top(egui::Id::new(("craft_suite_fontbar", tab))).frame(egui::Frame::NONE.inner_margin(Margin { left: 10, right: 10, top: 8, bottom: 4 })).show(ui, |ui| {
        egui::Frame::NONE.fill(c.bg).stroke(Stroke::new(1.0, c.border)).corner_radius(10).inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if close_button(ui, c.soft).on_hover_text("Close: not for this document again").clicked() {
                    close = true;
                }
                if outline_button(ui, "Choose font", c.red).on_hover_text("Use an installed font in its place").clicked() {
                    open_card = Some(true);
                }
                let busy = bar.download.as_ref().is_some_and(|(t, _, _)| *t == tab);
                let label = if busy { "Downloading…" } else { "⬇  Download font" };
                let resp = filled_button(ui, label, c.red);
                download_rect = Some(resp.rect);
                if resp.on_hover_text("Ways to install it on this computer").clicked() && !busy {
                    open_card = Some(false);
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    error_icon(ui, c.red, 30.0);
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(&title).size(15.5).strong().color(c.text)).wrap());
                    ui.add(egui::Label::new(RichText::new(&sub).size(15.0).color(c.soft)).wrap());
                });
            });
        });
    });
    if close {
        st.closed.insert(st.key.clone());
        bar.card = None;
        return;
    }
    if let Some(similar) = open_card {
        bar.card = match bar.card {
            Some((t, s)) if t == tab && s == similar => None,
            _ => Some((tab, similar)),
        };
    }
    if let Some((t, similar)) = bar.card
        && t == tab
        && let Some(anchor) = download_rect
    {
        card(bar, settings, tab, doc, ui, anchor, similar, &c, now);
    }
}

/// The options card under the bar's Download button.
#[allow(clippy::too_many_arguments)]
fn card(bar: &mut FontBar, settings: &mut Settings, tab: u64, doc: &mut Doc, ui: &mut Ui, anchor: egui::Rect, similar: bool, c: &Colors, now: f64) {
    let windows = !deckcraft_fonts::install::windows_font_dirs().is_empty();
    let busy = bar.download.as_ref().is_some_and(|(t, _, _)| *t == tab);
    let Some(st) = bar.tabs.get_mut(&tab) else { return };
    let Some(font) = st.missing.get(st.pick).map(|m| (m.name.clone(), m.shown_with.clone())) else { return };
    let (name, shown_with) = font;
    let names: Vec<String> = st.missing.iter().map(|m| m.name.clone()).collect();
    let mut action: Option<Action> = None;
    let mut close = false;
    let mut show_similar = similar;
    let width = 430.0;
    let pos = pos2((anchor.right() + 60.0 - width).max(8.0), anchor.bottom() + 10.0);
    let page = ui.visuals().window_fill;
    egui::Area::new(egui::Id::new(("craft_suite_fontcard", tab))).order(egui::Order::Foreground).fixed_pos(pos).show(ui.ctx(), |ui| {
        egui::Frame::NONE
            .fill(page)
            .stroke(Stroke::new(1.0, c.border))
            .corner_radius(14)
            .inner_margin(Margin::same(18))
            .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(46) })
            .show(ui, |ui| {
                ui.set_width(width - 36.0);
                ui.horizontal(|ui| {
                    error_icon(ui, c.red, 40.0);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Font not available").size(18.0).strong());
                        ui.label(RichText::new(format!("“{name}” is not installed on this device.")).size(14.0).weak());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if close_button(ui, ui.visuals().text_color()).clicked() {
                            close = true;
                        }
                    });
                });
                if names.len() > 1 {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Font:").size(13.0).weak());
                        for (i, n) in names.iter().enumerate() {
                            if ui.selectable_label(i == st.pick, n).clicked() {
                                st.pick = i;
                                st.other.clear();
                            }
                        }
                    });
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                let pink = c.bg;
                if option_row(ui, "☁", "Download and install", &format!("Get “{name}” from Google Fonts"), Some(pink), c, busy).clicked() && !busy {
                    action = Some(Action::Download(name.clone()));
                }
                if windows && option_row(ui, "⊞", "Install from Windows on this PC", "Copy Microsoft fonts (Aptos, Calibri…) from Windows", None, c, false).clicked() {
                    action = Some(Action::Windows);
                }
                if option_row(ui, "📁", "Add font files…", "Install the fonts in a folder on this computer", None, c, false).clicked() {
                    action = Some(Action::Folder);
                }
                if option_row(ui, "Aa", "Choose a similar font", "Use a matching font from the list", None, c, false).clicked() {
                    show_similar = !show_similar;
                }
                if show_similar {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for f in suggestions(&name, shown_with.as_deref()).into_iter().filter(|f| doc.has_font(f)) {
                            if ui.button(RichText::new(f).size(14.0).family(egui::FontFamily::Proportional)).on_hover_text(format!("Use {f} for “{name}” throughout the document")).clicked() {
                                action = Some(Action::Replace(name.clone(), f.to_string()));
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut st.other).hint_text("Another installed font").desired_width(220.0));
                        let typed = st.other.trim().to_string();
                        let ok = !typed.is_empty() && doc.has_font(&typed);
                        if ui.add_enabled(ok, egui::Button::new("Use")).on_disabled_hover_text("Type the name of an installed font").clicked() {
                            action = Some(Action::Replace(name.clone(), typed));
                        }
                    });
                }
                if let Some((text, ok, at)) = &st.note
                    && now - at < 12.0
                {
                    ui.add_space(6.0);
                    ui.label(RichText::new(text).size(13.0).color(if *ok { c.ok } else { c.red }));
                }
                ui.add_space(8.0);
                let mut hide = !settings.font_notice;
                if ui.checkbox(&mut hide, "Don't show this again").on_hover_text("Settings › Behaviour turns it back on").changed() && hide {
                    settings.font_notice = false;
                    close = true;
                }
            });
    });
    // A click outside the card and the bar closes it.
    let clicked_outside = ui.input(|i| i.pointer.any_click())
        && ui.ctx().layer_id_at(ui.input(|i| i.pointer.interact_pos().unwrap_or_default())).is_none_or(|l| l.id != egui::Id::new(("craft_suite_fontcard", tab)))
        && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| anchor.expand2(vec2(400.0, 30.0)).contains(p));
    if close || clicked_outside {
        bar.card = None;
    } else {
        bar.card = Some((tab, show_similar));
    }
    if let Some(a) = action {
        run(bar, tab, doc, a, now);
    }
}

enum Action {
    Download(String),
    Windows,
    Folder,
    Replace(String, String),
}

fn run(bar: &mut FontBar, tab: u64, doc: &mut Doc, action: Action, now: f64) {
    let note = match action {
        Action::Download(family) => {
            let (tx, rx) = channel();
            let f = family.clone();
            std::thread::spawn(move || {
                let _ = tx.send(google_fonts(&f));
            });
            bar.download = Some((tab, family, rx));
            return;
        }
        Action::Windows => {
            let wanted: Vec<String> = bar.tabs.get(&tab).map(|st| st.missing.iter().map(|m| m.name.clone()).collect()).unwrap_or_default();
            if doc.kind() != Kind::Deck {
                share(&deckcraft_fonts::install::windows_font_dirs(), Some(&wanted));
            }
            installed(doc.add_fonts(json!({ "windows": true })), "Windows")
        }
        Action::Folder => match rfd::FileDialog::new().set_title("Add fonts from a folder").pick_folder() {
            Some(dir) => {
                if doc.kind() != Kind::Deck {
                    share(std::slice::from_ref(&dir), None);
                }
                installed(doc.add_fonts(json!({ "folder": dir.to_string_lossy() })), "that folder")
            }
            None => return,
        },
        Action::Replace(from, to) => match doc.replace_font(&from, &to) {
            Ok(0) => (format!("“{from}” wasn't found in the document's text or styles."), false),
            Ok(_) => (format!("“{from}” is now {to} throughout. Undo takes it back."), true),
            Err(e) => (format!("Couldn't replace the font: {e}"), false),
        },
    };
    finish(bar, tab, note, now);
}

/// A finished download: install what came, from the folder it went to.
fn poll_download(bar: &mut FontBar, tab: u64, doc: &mut Doc, now: f64) {
    let Some((t, family, rx)) = &bar.download else { return };
    if *t != tab {
        return;
    }
    let family = family.clone();
    let note = match rx.try_recv() {
        Ok(Ok(dir)) => {
            if doc.kind() != Kind::Deck {
                share(std::slice::from_ref(&dir), None);
            }
            let r = installed(doc.add_fonts(json!({ "folder": dir.to_string_lossy() })), "Google Fonts");
            let _ = std::fs::remove_dir_all(&dir);
            r
        }
        Ok(Err(e)) => (e, false),
        Err(std::sync::mpsc::TryRecvError::Empty) => return,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => (format!("The download of “{family}” stopped."), false),
    };
    bar.download = None;
    finish(bar, tab, note, now);
}

fn finish(bar: &mut FontBar, tab: u64, note: (String, bool), now: f64) {
    if let Some(st) = bar.tabs.get_mut(&tab) {
        st.note = Some((note.0, note.1, now));
        st.checked = 0.0; // look again at once
    }
}

/// Also copies the fonts into this user's font folder (`~/.local/share/fonts/Imported`, Windows
/// `%LOCALAPPDATA%\\Microsoft\\Windows\\Fonts`), where every app finds them, not only the tab's
/// (WordCraft keeps the fonts it adds in a folder of its own). Slide's own `font.add` already
/// copies them there.
fn share(dirs: &[PathBuf], only: Option<&[String]>) {
    if !dirs.is_empty() {
        let r = deckcraft_fonts::install::install_fonts_from(dirs, only, None);
        for e in r.errors {
            log::warn!("installing fonts: {e}");
        }
    }
}

/// The apps' `font.add` answer, in words.
fn installed(r: Result<serde_json::Value, String>, from: &str) -> (String, bool) {
    match r {
        Ok(v) => {
            let list = |k: &str| v.get(k).and_then(serde_json::Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let (added, missing) = (list("added"), list("notFound"));
            match (added.is_empty(), missing.is_empty()) {
                (false, true) => (format!("Installed {added} on this computer."), true),
                (false, false) => (format!("Installed {added}. Still not installed: {missing}."), true),
                (true, false) => (format!("{missing} wasn't found in {from}."), false),
                (true, true) => ("Nothing new to install: these fonts are already there.".into(), true),
            }
        }
        Err(e) => (e, false),
    }
}

/// Downloads `family` from Google Fonts (regular, bold, italic and bold italic when it has them)
/// into a new folder, for `font.add {folder}`. Fonts there are free (SIL Open Font Licence or
/// Apache 2.0); Microsoft's own fonts, such as Aptos or Calibri, aren't among them.
fn google_fonts(family: &str) -> Result<PathBuf, String> {
    let fam = family.trim();
    if fam.is_empty() || fam.len() > 100 || !fam.chars().all(|c| c.is_alphanumeric() || " -_".contains(c)) {
        return Err(format!("“{family}” isn't a name Google Fonts uses."));
    }
    let q = fam.replace(' ', "+");
    let agent = agent();
    let mut css = None;
    for variant in [format!("{q}:ital,wght@0,400;0,700;1,400;1,700"), format!("{q}:wght@400;700"), q.clone()] {
        let url = format!("https://fonts.googleapis.com/css2?family={variant}");
        let mut resp = agent.get(&url).call().map_err(|e| format!("Couldn't reach Google Fonts: {e}"))?;
        match resp.status().as_u16() {
            200 => {
                css = Some(resp.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| e.to_string())?);
                break;
            }
            400 | 404 => continue,
            s => return Err(format!("Google Fonts answered {s}.")),
        }
    }
    let Some(css) = css else {
        return Err(format!(
            "“{fam}” isn't on Google Fonts (Microsoft's fonts, such as Aptos and Calibri, aren't). Install it from Windows, add its files, or choose a similar font."
        ));
    };
    let dir = std::env::temp_dir().join(format!("craft-suite-font-{}-{}", std::process::id(), fam.replace(' ', "")));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut got = HashSet::new();
    for face in css.split("@font-face").skip(1) {
        let Some(url) = face.split("url(").nth(1).and_then(|s| s.split(')').next()) else { continue };
        if !url.starts_with("https://fonts.gstatic.com/") || !(url.ends_with(".ttf") || url.ends_with(".otf")) || !got.insert(url.to_string()) {
            continue;
        }
        let weight = face.split("font-weight:").nth(1).and_then(|s| s.trim().split(';').next()).unwrap_or("400").trim().to_string();
        let italic = face.contains("font-style: italic");
        let ext = if url.ends_with(".otf") { "otf" } else { "ttf" };
        let file = dir.join(format!("{}-{}{}.{ext}", fam.replace(' ', ""), weight, if italic { "Italic" } else { "" }));
        let mut resp = agent.get(url).call().map_err(|e| format!("Couldn't download {fam}: {e}"))?;
        if resp.status().as_u16() != 200 {
            return Err(format!("Couldn't download {fam} ({}).", resp.status()));
        }
        let bytes = resp.body_mut().with_config().limit(32 << 20).read_to_vec().map_err(|e| e.to_string())?;
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    }
    if got.is_empty() {
        return Err(format!("Google Fonts had no font files for “{fam}”."));
    }
    Ok(dir)
}

fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(90)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent(concat!("CraftSuite/", env!("CARGO_PKG_VERSION")))
        .tls_config(tls)
        .build()
        .into()
}

/// Open fonts with the same widths as `name`, so text set in them breaks lines and pages alike.
fn metric_twin(name: &str) -> &'static [&'static str] {
    match name.trim().to_lowercase().as_str() {
        "calibri" => &["Carlito"],
        "cambria" => &["Caladea"],
        "arial" | "helvetica" => &["Liberation Sans", "Arimo"],
        "arial narrow" => &["Liberation Sans Narrow"],
        "times new roman" | "times" => &["Liberation Serif", "Tinos"],
        "courier new" | "courier" => &["Liberation Mono", "Cousine"],
        "georgia" => &["Gelasio"],
        _ => &[],
    }
}

/// Installed fonts that look like `name`: metric-compatible ones first (they keep lines and pages
/// where they were), then ones of the same kind.
pub fn suggestions(name: &str, shown_with: Option<&str>) -> Vec<&'static str> {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let mut out: Vec<&'static str> = if has(&["calibri"]) {
        vec!["Carlito"]
    } else if has(&["cambria"]) {
        vec!["Caladea"]
    } else if has(&["arial", "helvetica"]) {
        vec!["Liberation Sans", "Arimo"]
    } else if has(&["times"]) {
        vec!["Liberation Serif", "Tinos"]
    } else if has(&["courier"]) {
        vec!["Liberation Mono", "Cousine"]
    } else if has(&["aptos"]) {
        vec!["Source Sans 3", "Inter", "Open Sans"]
    } else if has(&["segoe", "frutiger", "myriad"]) {
        vec!["Open Sans", "Noto Sans", "Source Sans 3"]
    } else if has(&["georgia"]) {
        vec!["Gelasio", "Source Serif 4"]
    } else if has(&["verdana", "tahoma"]) {
        vec!["DejaVu Sans", "Noto Sans"]
    } else if has(&["garamond"]) {
        vec!["EB Garamond", "Source Serif 4"]
    } else if has(&["palatino", "book antiqua"]) {
        vec!["TeX Gyre Pagella", "Source Serif 4"]
    } else if has(&["century gothic", "futura", "avenir"]) {
        vec!["Montserrat", "Poppins", "Nunito Sans"]
    } else if has(&["gill sans", "trebuchet"]) {
        vec!["Lato", "Fira Sans"]
    } else if has(&["consolas", "menlo", "monaco", "lucida console"]) {
        vec!["JetBrains Mono", "DejaVu Sans Mono", "Liberation Mono"]
    } else if has(&["comic"]) {
        vec!["Comic Neue"]
    } else {
        Vec::new()
    };
    let kind: &[&'static str] = if has(&["mono", "code", "console", "typewriter"]) {
        &["JetBrains Mono", "DejaVu Sans Mono", "Liberation Mono"]
    } else if has(&["serif", "roman", "book", "garamond", "georgia", "times", "cambria", "minion", "baskerville"]) && !n.contains("sans") {
        &["Source Serif 4", "Liberation Serif", "Merriweather", "Noto Serif", "DejaVu Serif"]
    } else {
        &["Inter", "Source Sans 3", "Open Sans", "Liberation Sans", "Noto Sans", "DejaVu Sans"]
    };
    out.extend(kind.iter().copied());
    if let Some(w) = shown_with {
        // The font shown now first: choosing it keeps the look and stops the notice.
        let known: &[&'static str] = &["Carlito", "Caladea", "Liberation Sans", "Liberation Serif", "Liberation Mono", "Source Sans 3", "Source Serif 4", "Inter", "Open Sans", "DejaVu Sans", "Noto Sans", "Arimo", "Tinos", "Cousine"];
        if let Some(k) = known.iter().find(|k| k.eq_ignore_ascii_case(w)) {
            out.insert(0, k);
        }
    }
    let mut seen = HashSet::new();
    out.retain(|f| seen.insert(*f));
    out.truncate(7);
    out
}

// ---- widgets

fn error_icon(ui: &mut Ui, red: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.circle_filled(r.center(), size / 2.0, red.gamma_multiply(0.18));
    p.circle_filled(r.center(), size * 0.36, red);
    p.text(r.center(), Align2::CENTER_CENTER, "!", FontId::proportional(size * 0.5), Color32::WHITE);
}

fn filled_button(ui: &mut Ui, label: &str, red: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).size(15.0).strong().color(Color32::WHITE)).fill(red).corner_radius(8).min_size(vec2(0.0, 36.0)))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn outline_button(ui: &mut Ui, label: &str, red: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).size(15.0).color(red)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.5, red)).corner_radius(8).min_size(vec2(0.0, 36.0)))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn close_button(ui: &mut Ui, color: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, CornerRadius::same(6), ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let s = Stroke::new(1.8, color);
    let c = r.center();
    let d = 7.0;
    p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], s);
    p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], s);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of the card: an icon, a title and a line under it, and an arrow; `fill` highlights it.
fn option_row(ui: &mut Ui, icon: &str, title: &str, sub: &str, fill: Option<Color32>, c: &Colors, busy: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 62.0), Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    let bg = match (fill, hovered) {
        (Some(f), _) => Some(f),
        (None, true) => Some(ui.visuals().widgets.hovered.weak_bg_fill),
        _ => None,
    };
    if let Some(bg) = bg {
        p.rect_filled(rect, CornerRadius::same(10), bg);
    }
    if hovered {
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, c.border), StrokeKind::Inside);
    }
    let accent = if fill.is_some() { c.red } else { ui.visuals().strong_text_color() };
    p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::CENTER_CENTER, icon, FontId::proportional(22.0), accent);
    let title = if busy { "Downloading…" } else { title };
    p.text(pos2(rect.left() + 52.0, rect.top() + 12.0), Align2::LEFT_TOP, title, FontId::proportional(16.0), accent);
    p.text(pos2(rect.left() + 52.0, rect.top() + 35.0), Align2::LEFT_TOP, sub, FontId::proportional(13.5), ui.visuals().weak_text_color());
    p.text(pos2(rect.right() - 16.0, rect.center().y), Align2::CENTER_CENTER, "›", FontId::proportional(22.0), ui.visuals().weak_text_color());
    ui.add_space(4.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_fonts_start_with_metric_compatible_ones() {
        assert_eq!(suggestions("Calibri", Some("Carlito"))[0], "Carlito");
        assert_eq!(suggestions("Times New Roman", None)[0], "Liberation Serif");
        assert!(suggestions("Aptos", Some("Source Sans 3")).contains(&"Inter"));
        assert!(suggestions("Fancy Mono", None).contains(&"JetBrains Mono"));
        let s = suggestions("Unknown Display", None);
        assert!(!s.is_empty() && s.len() <= 7);
    }

    #[test]
    fn google_fonts_refuses_odd_names() {
        assert!(google_fonts("../etc").is_err());
        assert!(google_fonts("").is_err());
    }
}
