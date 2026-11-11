//! The Stock Images dialog every app's Insert › Pictures opens: free online pictures (Openverse)
//! by topic or search, and the icon library.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use egui::{Color32, ColorImage, Context, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2};

use crate::icons::Icon;
use crate::online::{self, Hit, Licence, Page, Sort};
use crate::picker::{IconPicker, thousands};

/// Topics offered as one-click searches, like Office's stock image categories.
pub const TOPICS: [&str; 20] = [
    "Business",
    "Nature",
    "Science",
    "Biology",
    "Medical",
    "Technology",
    "People",
    "Education",
    "Food",
    "Travel",
    "Animals",
    "Abstract",
    "Backgrounds",
    "Architecture",
    "City",
    "Sports",
    "Space",
    "Flowers",
    "Ocean",
    "Mountains",
];

const THUMB_PX: u32 = 320;
const CELL: egui::Vec2 = vec2(172.0, 124.0);

/// Which side of the dialog shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Images,
    Icons,
}

/// A picture chosen to insert.
#[derive(Clone, Debug)]
pub struct Chosen {
    /// JPEG or PNG bytes, at most [`online::MAX_SIDE`] pixels a side.
    pub bytes: Vec<u8>,
    /// `jpg` or `png`.
    pub ext: &'static str,
    /// Alt text: the title and the credit the licence asks for.
    pub alt: String,
    /// A short name for the picture object.
    pub name: String,
    pub width: u32,
    pub height: u32,
}

/// What the dialog hands back.
pub enum Pick {
    Picture(Chosen),
    Icon(&'static Icon),
}

enum Msg {
    Page { generation: u64, more: bool, result: Result<Page, String> },
    Thumb { id: String, image: Option<([usize; 2], Vec<u8>)> },
    Full { result: Result<Chosen, String> },
}

enum Thumb {
    Loading,
    Ready(TextureHandle),
    Failed,
}

/// The dialog's state; keep one per app window.
pub struct StockDialog {
    pub open: bool,
    pub tab: Tab,
    icons: IconPicker,
    query: String,
    searched: String,
    licence: Licence,
    sort: Sort,
    hits: Vec<Hit>,
    page: u32,
    pages: u32,
    total: u64,
    selected: Option<usize>,
    busy: bool,
    downloading: bool,
    status: String,
    generation: Arc<AtomicU64>,
    thumbs: HashMap<String, Thumb>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    jobs: Option<Sender<(u64, String, String)>>,
    focus_search: bool,
}

impl Default for StockDialog {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            open: false,
            tab: Tab::Images,
            icons: IconPicker::new(false),
            query: String::new(),
            searched: String::new(),
            licence: Licence::default(),
            sort: Sort::default(),
            hits: Vec::new(),
            page: 0,
            pages: 0,
            total: 0,
            selected: None,
            busy: false,
            downloading: false,
            status: String::new(),
            generation: Arc::new(AtomicU64::new(0)),
            thumbs: HashMap::new(),
            tx,
            rx,
            jobs: None,
            focus_search: false,
        }
    }
}

impl StockDialog {
    /// Opens on the stock pictures.
    pub fn open_images(&mut self) {
        self.open = true;
        self.tab = Tab::Images;
    }

    /// Opens on the pictures with the cursor in the search box (Insert › Online Pictures).
    pub fn open_online(&mut self) {
        self.open_images();
        self.focus_search = true;
    }

    /// Opens on the icons.
    pub fn open_icons(&mut self) {
        self.open = true;
        self.tab = Tab::Icons;
    }

    /// Draws the dialog while it's open; returns what the user chose to insert.
    pub fn show(&mut self, ctx: &Context) -> Option<Pick> {
        if !self.open {
            return None;
        }
        let mut picked = self.poll(ctx);
        if self.tab == Tab::Images && self.hits.is_empty() && !self.busy && self.searched.is_empty() && self.status.is_empty() {
            self.query = TOPICS[1].to_string();
            self.search(ctx, false);
        }
        let mut open = true;
        egui::Window::new("Stock images")
            .id(egui::Id::unique("craft-gallery-stock"))
            .open(&mut open)
            .collapsible(false)
            .default_size([780.0, 600.0])
            .min_size([420.0, 360.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Images, "Images");
                    ui.selectable_value(&mut self.tab, Tab::Icons, "Icons");
                });
                ui.separator();
                match self.tab {
                    Tab::Images => self.images(ui, ctx),
                    Tab::Icons => {
                        if let Some(icon) = self.icons_tab(ui) {
                            picked = Some(Pick::Icon(icon));
                        }
                    }
                }
            });
        if !open || picked.is_some() {
            self.open = false;
        }
        picked
    }

    fn icons_tab(&mut self, ui: &mut Ui) -> Option<&'static Icon> {
        let mut out = None;
        egui::Panel::bottom("craft-gallery-icons-bottom").show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = self.icons.selected.map_or("Double-click an icon, or pick one and Insert.".to_string(), |i| i.label.to_string());
                ui.label(label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(self.icons.selected.is_some(), egui::Button::new("Insert")).clicked() {
                        out = self.icons.selected;
                    }
                });
            });
            ui.weak(crate::CREDITS);
        });
        if let Some(icon) = self.icons.show(ui) {
            out = Some(icon);
        }
        out
    }

    fn images(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.query).hint_text("Search free pictures on the web").desired_width(ui.available_width() - 80.0),
            );
            if std::mem::take(&mut self.focus_search) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Search").clicked() || enter {
                self.search(ctx, false);
            }
        });
        ui.horizontal(|ui| {
            let (licence, sort) = (self.licence, self.sort);
            egui::ComboBox::from_id_salt("craft-gallery-licence").selected_text(self.licence.label()).show_ui(ui, |ui| {
                for l in Licence::ALL {
                    ui.selectable_value(&mut self.licence, l, l.label());
                }
            });
            egui::ComboBox::from_id_salt("craft-gallery-sort").selected_text(self.sort.label()).show_ui(ui, |ui| {
                for s in Sort::ALL {
                    ui.selectable_value(&mut self.sort, s, s.label());
                }
            });
            if (licence, sort) != (self.licence, self.sort) && !self.searched.is_empty() {
                self.query = self.searched.clone();
                self.search(ctx, false);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for topic in TOPICS {
                let on = self.searched.eq_ignore_ascii_case(topic);
                if ui.selectable_label(on, topic).clicked() {
                    self.query = topic.to_string();
                    self.search(ctx, false);
                }
            }
        });
        ui.add_space(4.0);

        egui::Panel::bottom("craft-gallery-images-bottom").show(ui, |ui| {
            let hit = self.selected.and_then(|k| self.hits.get(k)).cloned();
            ui.horizontal(|ui| {
                match &hit {
                    Some(h) => {
                        ui.label(h.credit());
                        if !h.page.is_empty() {
                            ui.hyperlink_to("Source", &h.page);
                        }
                    }
                    None => {
                        ui.label("Pick a picture, then Insert. Double-click inserts at once.");
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let text = if self.downloading { "Downloading…" } else { "Insert" };
                    if ui.add_enabled(hit.is_some() && !self.downloading, egui::Button::new(text)).clicked()
                        && let Some(k) = self.selected
                    {
                        self.download(ctx, k);
                    }
                });
            });
            ui.weak("Pictures from Openverse (Creative Commons and public domain). Check each licence; the credit goes into the picture's alt text.");
        });

        if self.busy && self.hits.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching…");
            });
            return;
        }
        if !self.status.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, &self.status);
        }
        if self.hits.is_empty() {
            if !self.searched.is_empty() && !self.busy && self.status.is_empty() {
                ui.label(format!("No free pictures found for “{}”. Try other words or another licence.", self.searched));
            }
            return;
        }
        ui.weak(format!("{} pictures for “{}”", thousands(self.total as usize), self.searched));
        self.grid(ui, ctx);
    }

    fn grid(&mut self, ui: &mut Ui, ctx: &Context) {
        let cols = ((ui.available_width() / CELL.x).floor() as usize).max(1);
        let rows = self.hits.len().div_ceil(cols);
        let more = self.page < self.pages;
        let accent = ui.visuals().selection.stroke.color;
        let hover = ui.visuals().widgets.hovered.weak_bg_fill;
        let mut want: Vec<(String, String)> = Vec::new();
        let mut insert = None;
        let mut load_more = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, CELL.y, rows + usize::from(more), |ui, range| {
            for row in range {
                if row == rows {
                    ui.vertical_centered(|ui| {
                        if self.busy {
                            ui.spinner();
                        } else if ui.button("More pictures").clicked() {
                            load_more = true;
                        }
                    });
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for k in row * cols..((row + 1) * cols).min(self.hits.len()) {
                        let hit = &self.hits[k];
                        let (rect, resp) = ui.allocate_exact_size(CELL, Sense::click());
                        let inner = rect.shrink(4.0);
                        if resp.hovered() {
                            ui.painter().rect_filled(inner.expand(2.0), 6.0, hover);
                        }
                        match self.thumbs.get(&hit.id) {
                            Some(Thumb::Ready(t)) => {
                                let size = t.size_vec2();
                                let scale = (inner.width() / size.x).min(inner.height() / size.y);
                                let r = Rect::from_center_size(inner.center(), size * scale);
                                ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            Some(Thumb::Loading) => {
                                ui.painter().rect_filled(inner, 4.0, Color32::from_gray(128).gamma_multiply(0.15));
                            }
                            Some(Thumb::Failed) => {
                                ui.painter().rect_filled(inner, 4.0, Color32::from_gray(128).gamma_multiply(0.08));
                            }
                            None => want.push((hit.id.clone(), hit.thumbnail.clone())),
                        }
                        if self.selected == Some(k) {
                            ui.painter().rect_stroke(inner.expand(1.0), 6.0, egui::Stroke::new(3.0, accent), egui::StrokeKind::Outside);
                        }
                        let resp = resp.on_hover_text(hit.credit());
                        if resp.clicked() {
                            self.selected = Some(k);
                        }
                        if resp.double_clicked() {
                            insert = Some(k);
                        }
                    }
                });
            }
        });
        for (id, url) in want {
            self.thumbs.insert(id.clone(), Thumb::Loading);
            self.queue_thumb(ctx, id, url);
        }
        if let Some(k) = insert {
            self.selected = Some(k);
            self.download(ctx, k);
        }
        if load_more && !self.busy {
            self.search(ctx, true);
        }
    }

    /// Starts a search for the query (or the next page of the last one).
    fn search(&mut self, ctx: &Context, more: bool) {
        let query = if more { self.searched.clone() } else { self.query.trim().to_string() };
        if query.is_empty() {
            return;
        }
        let page = if more { self.page + 1 } else { 1 };
        let generation = if more {
            self.generation.load(Ordering::Relaxed)
        } else {
            self.hits.clear();
            self.selected = None;
            self.searched = query.clone();
            self.generation.fetch_add(1, Ordering::Relaxed) + 1
        };
        self.busy = true;
        self.status.clear();
        let (tx, ctx, licence, sort) = (self.tx.clone(), ctx.clone(), self.licence, self.sort);
        std::thread::spawn(move || {
            let result = online::search(&query, page, licence, sort);
            let _ = tx.send(Msg::Page { generation, more, result });
            ctx.request_repaint();
        });
    }

    fn queue_thumb(&mut self, ctx: &Context, id: String, url: String) {
        let generation = self.generation.load(Ordering::Relaxed);
        let jobs = self.jobs.get_or_insert_with(|| {
            let (jobs_tx, jobs_rx) = channel::<(u64, String, String)>();
            let jobs_rx = Arc::new(Mutex::new(jobs_rx));
            for _ in 0..4 {
                let (rx, tx, ctx, current) = (jobs_rx.clone(), self.tx.clone(), ctx.clone(), self.generation.clone());
                std::thread::spawn(move || {
                    loop {
                        let job = match rx.lock() {
                            Ok(r) => r.recv(),
                            Err(_) => return,
                        };
                        let Ok((generation, id, url)) = job else { return };
                        // A newer search made this thumbnail moot: say so, so it can be asked again.
                        let image = if generation == current.load(Ordering::Relaxed) {
                            online::fetch(&url).ok().and_then(|b| online::thumbnail(&b, THUMB_PX))
                        } else {
                            None
                        };
                        if tx.send(Msg::Thumb { id, image }).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                });
            }
            jobs_tx
        });
        let _ = jobs.send((generation, id, url));
    }

    fn download(&mut self, ctx: &Context, k: usize) {
        let Some(hit) = self.hits.get(k).cloned() else { return };
        self.downloading = true;
        self.status.clear();
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = online::fetch(&hit.url).and_then(|b| online::prepare(&b)).map(|(bytes, ext)| {
                let (width, height) = image::load_from_memory(&bytes).map(|i| (i.width(), i.height())).unwrap_or((hit.width, hit.height));
                let name = if hit.title.trim().is_empty() { "Picture".to_string() } else { hit.title.trim().chars().take(60).collect() };
                Chosen { bytes, ext, alt: hit.credit(), name, width, height }
            });
            let _ = tx.send(Msg::Full { result });
            ctx.request_repaint();
        });
    }

    fn poll(&mut self, ctx: &Context) -> Option<Pick> {
        let mut out = None;
        let current = self.generation.load(Ordering::Relaxed);
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Page { generation, more, result } if generation == current => {
                    self.busy = false;
                    match result {
                        Ok(page) => {
                            if !more {
                                self.hits.clear();
                            }
                            self.hits.extend(page.hits);
                            self.page = page.page;
                            self.pages = page.pages;
                            self.total = page.total.max(self.hits.len() as u64);
                        }
                        Err(e) => self.status = e,
                    }
                }
                Msg::Page { .. } => {}
                Msg::Thumb { id, image } => {
                    let thumb = match image {
                        Some((size, rgba)) => {
                            let img = ColorImage::from_rgba_unmultiplied(size, &rgba);
                            Thumb::Ready(ctx.load_texture(format!("craft-stock-{id}"), img, TextureOptions::LINEAR))
                        }
                        None if self.hits.iter().any(|h| h.id == id) => Thumb::Failed,
                        // Dropped for a newer search: forget it so a later search can ask again.
                        None => {
                            self.thumbs.remove(&id);
                            continue;
                        }
                    };
                    self.thumbs.insert(id, thumb);
                }
                Msg::Full { result } => {
                    self.downloading = false;
                    match result {
                        Ok(chosen) => out = Some(Pick::Picture(chosen)),
                        Err(e) => self.status = e,
                    }
                }
            }
        }
        // Keep the texture cache to the pictures on show.
        if self.thumbs.len() > 600 {
            let live: std::collections::HashSet<&str> = self.hits.iter().map(|h| h.id.as_str()).collect();
            self.thumbs.retain(|id, _| live.contains(id.as_str()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_draws_both_tabs_headless() {
        let ctx = egui::Context::default();
        let mut d = StockDialog::default();
        d.open_icons();
        d.searched = "offline".into(); // don't start a web search in a test
        for _ in 0..2 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                assert!(d.show(ui.ctx()).is_none());
            })
            .drop_without_applying_deltas();
        }
        d.tab = Tab::Images;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(d.show(ui.ctx()).is_none());
        })
        .drop_without_applying_deltas();
        assert!(d.open);
    }
}
