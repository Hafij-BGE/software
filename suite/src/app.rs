//! The suite's window: the tabs, which one is shown, opening and closing them, and the prompts
//! the suite itself asks (unsaved changes when a tab or the window closes).

use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use crate::hosts::{CloseCheck, Doc};
use crate::kind::Kind;
use crate::look::Look;
use crate::settings::{Mixing, NewTabOpens, NewTabPlacement, Rgb, SavedTab, Settings, SuiteTheme, TitleBar};
use crate::{Request, hosts, instance, update};

pub enum Content {
    Home,
    Settings,
    Doc(Doc),
}

pub struct Tab {
    pub id: u64,
    pub content: Content,
    pub pinned: bool,
    /// This tab's own colour, over its app's.
    pub color: Option<Rgb>,
    /// The tab's look while it isn't shown (see `look`); None until it has been shown and left.
    look: Option<Look>,
    /// When the tab was last shown (a counter), so the newest app preferences are saved last.
    last_active: u64,
}

impl Tab {
    pub fn kind(&self) -> Kind {
        match &self.content {
            Content::Home => Kind::Home,
            Content::Settings => Kind::Settings,
            Content::Doc(d) => d.kind(),
        }
    }

    pub fn title(&self) -> String {
        match &self.content {
            Content::Home => "Home".into(),
            Content::Settings => "Settings".into(),
            Content::Doc(d) => d.title(),
        }
    }

    pub fn dirty(&self) -> bool {
        matches!(&self.content, Content::Doc(d) if d.dirty())
    }

    pub fn path(&self) -> Option<String> {
        match &self.content {
            Content::Doc(d) => d.path(),
            _ => None,
        }
    }

    fn is_page(&self) -> bool {
        !matches!(self.content, Content::Doc(_))
    }
}

/// Things the tab bar, the pages, shortcuts and other windows ask for; done in `logic`.
#[derive(Clone, Debug)]
pub enum Action {
    /// A blank document of an app, or a Home or Settings page. `replace_page`: in place of the
    /// active tab when that's a Home page (the Home page's own buttons).
    New { kind: Kind, replace_page: bool },
    /// What a new tab opens by default (the + button, Ctrl+Alt+Insert).
    NewDefault,
    Open(String),
    OpenDialog,
    /// A new, untitled document made from a template (the Home page's Templates).
    Template { path: String, title: String },
    Activate(u64),
    Close(u64),
    CloseOthers(u64),
    CloseAfter(u64),
    /// The next (1) or previous (-1) tab in the order the tab bar shows.
    Cycle(i32),
    /// Swap a tab with its neighbour in the tab bar.
    MoveBy(u64, i32),
    /// Drop `id` next to `target` (dragging in the tab bar).
    MoveTo { id: u64, target: u64, after: bool },
    TogglePin(u64),
    SetColor(u64, Option<Rgb>),
    /// Show the Home page (a new one when none is open).
    Home,
    Settings,
    /// Turn a Home or Settings tab into the other page (the pages' side bar).
    ShowPage { tab: u64, settings: bool },
}

enum Prompt {
    /// Save / Don't Save / Cancel for a DeckCraft or GridCraft tab.
    SaveTab(u64),
    /// The window is closing with unsaved tabs.
    Quit(Vec<u64>),
    /// The window is closing with several tabs open ("Ask before closing several tabs").
    ConfirmQuit(usize),
    Message(String),
}

pub struct SuiteApp {
    pub settings: Settings,
    /// The settings as last written, to save only changes.
    saved: Settings,
    pub tabs: Vec<Tab>,
    pub active: u64,
    /// The tab whose look is installed in the context.
    shown: Option<u64>,
    next_id: u64,
    clock: u64,
    /// The context's look before any app styled it: the Home and Settings pages use it.
    home_look: Option<Look>,
    /// Every font family the apps define (see `look`): always installed alongside a tab's fonts.
    font_union: Option<egui::FontDefinitions>,
    /// A switch in progress: the new tab's look, whose options go in once its fonts are in.
    pending_look: Option<Look>,
    /// This frame started a switch: no app runs until the restored fonts are in (see `look`).
    skip_frame: bool,
    icons: HashMap<Kind, egui::TextureHandle>,
    big_icons: HashMap<Kind, egui::TextureHandle>,
    incoming: Option<Receiver<instance::Message>>,
    pub actions: Vec<Action>,
    prompt: Option<Prompt>,
    /// The window may close now (the suite has asked what it needed to).
    allow_close: bool,
    close_requested: bool,
    pub updater: update::Updater,
    /// The Home page's templates (src/templates.rs).
    pub templates: crate::templates::Library,
    /// The bar over documents whose fonts aren't installed (src/fontbar.rs).
    fontbar: crate::fontbar::FontBar,
    /// Ask AI: its channels, keys, panel and conversations.
    #[cfg(feature = "ai")]
    pub ai: crate::ai::Ai,
    pub title_sent: String,
    pub tabbar: crate::tabbar::TabBarState,
    /// A short message at the bottom of the window, and when it appeared.
    toast: Option<(String, f64)>,
    /// Where the pointer last was (window edges resize it when the suite draws the title bar).
    pointer: Option<egui::Pos2>,
    /// A press on a window edge: resize from there (sent from `logic`).
    pending_resize: Option<egui::ResizeDirection>,
    /// The window decorations last asked for.
    decorations_sent: Option<bool>,
}

/// How close to the window's edge a press resizes the window (tabs in the title bar).
const EDGE: f32 = 6.0;
const CORNER: f32 = 14.0;

/// The window edge or corner at `pos`, if any.
fn edge_at(screen: egui::Rect, pos: egui::Pos2) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection as D;
    if !screen.expand(1.0).contains(pos) {
        return None;
    }
    let (l, r) = (pos.x - screen.left() < EDGE, screen.right() - pos.x < EDGE);
    let (t, b) = (pos.y - screen.top() < EDGE, screen.bottom() - pos.y < EDGE);
    let (l2, r2) = (pos.x - screen.left() < CORNER, screen.right() - pos.x < CORNER);
    let (t2, b2) = (pos.y - screen.top() < CORNER, screen.bottom() - pos.y < CORNER);
    Some(match () {
        _ if (t && l2) || (l && t2) => D::NorthWest,
        _ if (t && r2) || (r && t2) => D::NorthEast,
        _ if (b && l2) || (l && b2) => D::SouthWest,
        _ if (b && r2) || (r && b2) => D::SouthEast,
        _ if t => D::North,
        _ if b => D::South,
        _ if l => D::West,
        _ if r => D::East,
        _ => return None,
    })
}

fn resize_cursor(dir: egui::ResizeDirection) -> egui::CursorIcon {
    use egui::{CursorIcon as C, ResizeDirection as D};
    match dir {
        D::North => C::ResizeNorth,
        D::South => C::ResizeSouth,
        D::East => C::ResizeEast,
        D::West => C::ResizeWest,
        D::NorthEast => C::ResizeNorthEast,
        D::NorthWest => C::ResizeNorthWest,
        D::SouthEast => C::ResizeSouthEast,
        D::SouthWest => C::ResizeSouthWest,
    }
}

pub fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    Some((img.width(), img.height(), img.into_raw()))
}

pub fn icon_data() -> egui::IconData {
    let (width, height, rgba) =
        decode_png(include_bytes!("../../assets/hicolor/128x128/apps/craft-suite.png")).unwrap_or((0, 0, Vec::new()));
    egui::IconData { rgba, width, height }
}

fn texture(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<egui::TextureHandle> {
    let (w, h, rgba) = decode_png(png)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
}

impl SuiteApp {
    pub fn new(ctx: &egui::Context, settings: Settings, request: Request, incoming: Option<Receiver<instance::Message>>) -> SuiteApp {
        let mut icons = HashMap::new();
        let mut big_icons = HashMap::new();
        for k in Kind::APPS.into_iter().chain([Kind::Home, Kind::Settings]) {
            if let Some(t) = texture(ctx, &format!("icon-{k:?}"), k.icon_png()) {
                icons.insert(k, t);
            }
            if let Some(t) = texture(ctx, &format!("big-icon-{k:?}"), k.large_icon_png()) {
                big_icons.insert(k, t);
            }
        }
        let mut app = SuiteApp {
            saved: settings.clone(),
            settings,
            tabs: Vec::new(),
            active: 0,
            shown: None,
            next_id: 1,
            clock: 0,
            home_look: None,
            font_union: None,
            pending_look: None,
            skip_frame: false,
            icons,
            big_icons,
            incoming,
            actions: Vec::new(),
            prompt: None,
            allow_close: false,
            close_requested: false,
            updater: update::Updater::new(),
            templates: Default::default(),
            fontbar: Default::default(),
            #[cfg(feature = "ai")]
            ai: crate::ai::Ai::new(),
            title_sent: String::new(),
            tabbar: Default::default(),
            toast: None,
            pointer: None,
            pending_resize: None,
            decorations_sent: None,
        };
        if app.settings.restore_tabs {
            for saved in app.settings.session.clone() {
                if std::path::Path::new(&saved.path).exists() {
                    match Doc::open(&saved.path, ctx) {
                        Ok(doc) => {
                            let id = app.push_tab(Content::Doc(doc), false);
                            if let Some(t) = app.tab_mut(id) {
                                t.pinned = saved.pinned;
                                t.color = saved.color;
                            }
                        }
                        Err(e) => log::warn!("{}: {e}", saved.path),
                    }
                }
            }
        }
        for f in request.files {
            app.actions.push(Action::Open(f));
        }
        for k in request.new {
            app.actions.push(Action::New { kind: k, replace_page: false });
        }
        if app.settings.check_updates_on_start {
            app.updater.start(ctx, false);
        }
        app
    }

    pub fn icon(&self, kind: Kind) -> Option<&egui::TextureHandle> {
        self.icons.get(&kind)
    }

    pub fn big_icon(&self, kind: Kind) -> Option<&egui::TextureHandle> {
        self.big_icons.get(&kind)
    }

    pub fn tab(&self, id: u64) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// The colour a tab shows: its own, or its app's.
    pub fn tab_color(&self, tab: &Tab) -> egui::Color32 {
        tab.color.unwrap_or_else(|| self.settings.color_of(tab.kind())).color32()
    }

    pub fn theme_preference(&self) -> egui::ThemePreference {
        match self.settings.theme {
            SuiteTheme::System => egui::ThemePreference::System,
            SuiteTheme::Light => egui::ThemePreference::Light,
            SuiteTheme::Dark => egui::ThemePreference::Dark,
        }
    }

    /// Where each app sorts when tabs are grouped: the suite's pages first, then the app order.
    fn group_rank(&self, kind: Kind) -> usize {
        if kind.is_app() { 1 + self.settings.rank(kind) } else { 0 }
    }

    /// The tabs as the tab bar shows them: groups (one unless tabs are in a row per app) of tab
    /// ids, with the app each group is for (None: mixed, or the suite's pages).
    pub fn display_groups(&self) -> Vec<(Option<Kind>, Vec<u64>)> {
        let mut order: Vec<(usize, &Tab)> = self.tabs.iter().enumerate().collect();
        match self.settings.mixing {
            Mixing::Mixed => {
                order.sort_by_key(|(i, t)| (!t.pinned, *i));
                vec![(None, order.iter().map(|(_, t)| t.id).collect())]
            }
            Mixing::Grouped => {
                order.sort_by_key(|(i, t)| (!t.pinned, self.group_rank(t.kind()), *i));
                vec![(None, order.iter().map(|(_, t)| t.id).collect())]
            }
            Mixing::Separate => {
                order.sort_by_key(|(i, t)| (!t.pinned, *i));
                let mut groups: Vec<(Option<Kind>, Vec<u64>)> = Vec::new();
                let pages: Vec<u64> = order.iter().filter(|(_, t)| t.is_page()).map(|(_, t)| t.id).collect();
                if !pages.is_empty() {
                    groups.push((None, pages));
                }
                for k in self.settings.app_order.clone() {
                    let ids: Vec<u64> = order.iter().filter(|(_, t)| t.kind() == k).map(|(_, t)| t.id).collect();
                    if !ids.is_empty() {
                        groups.push((Some(k), ids));
                    }
                }
                groups
            }
        }
    }

    pub fn display_order(&self) -> Vec<u64> {
        self.display_groups().into_iter().flat_map(|(_, ids)| ids).collect()
    }

    /// Whether two tabs may change places: in one group, and both pinned or both not.
    fn same_group(&self, a: &Tab, b: &Tab) -> bool {
        let group = |t: &Tab| match self.settings.mixing {
            Mixing::Mixed => 0,
            Mixing::Grouped | Mixing::Separate => self.group_rank(t.kind()),
        };
        a.pinned == b.pinned && group(a) == group(b)
    }

    fn push_tab(&mut self, content: Content, after_active: bool) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let tab = Tab { id, content, pinned: false, color: None, look: None, last_active: 0 };
        let at = match (after_active, self.settings.new_tab_placement) {
            (true, NewTabPlacement::AfterCurrent) => self.index_of(self.active).map(|i| i + 1),
            _ => None,
        };
        match at {
            Some(i) => self.tabs.insert(i.min(self.tabs.len()), tab),
            None => self.tabs.push(tab),
        }
        id
    }

    /// Shows `content` in a new tab, or in place of the active Home page when `replace_page`.
    fn place(&mut self, content: Content, replace_page: bool) -> u64 {
        if replace_page
            && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active)
            && matches!(tab.content, Content::Home)
        {
            tab.content = content;
            tab.look = None;
            // A different app now: install its look afresh.
            self.shown = None;
            return tab.id;
        }
        let id = self.push_tab(content, true);
        self.active = id;
        id
    }

    fn message(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        self.toast = Some((text.into(), ctx.input(|i| i.time)));
    }

    // ---- actions ----------------------------------------------------------------------------

    fn apply_actions(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            self.apply(ctx, action);
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::New { kind: Kind::Home, .. } | Action::Home => {
                match self.tabs.iter().find(|t| matches!(t.content, Content::Home)) {
                    Some(t) if matches!(action, Action::Home) => self.active = t.id,
                    _ => self.active = self.push_tab(Content::Home, true),
                }
            }
            Action::New { kind: Kind::Settings, .. } | Action::Settings => {
                match self.tabs.iter().find(|t| matches!(t.content, Content::Settings)) {
                    Some(t) => self.active = t.id,
                    None => self.active = self.push_tab(Content::Settings, true),
                }
            }
            Action::ShowPage { tab, settings } => {
                let other = self.tabs.iter().find(|t| {
                    t.id != tab && if settings { matches!(t.content, Content::Settings) } else { matches!(t.content, Content::Home) }
                });
                match other {
                    Some(t) => self.active = t.id,
                    None => {
                        if let Some(t) = self.tab_mut(tab)
                            && t.is_page()
                        {
                            t.content = if settings { Content::Settings } else { Content::Home };
                        }
                    }
                }
            }
            Action::New { kind, replace_page } => {
                if let Some(doc) = Doc::new_blank(kind, ctx) {
                    self.active = self.place(Content::Doc(doc), replace_page);
                }
            }
            Action::NewDefault => {
                let kind = match self.settings.new_tab_opens {
                    NewTabOpens::Home => Kind::Home,
                    NewTabOpens::App(k) => k,
                };
                // A new Home tab even when one is open: the + button always adds a tab.
                if kind == Kind::Home {
                    self.active = self.push_tab(Content::Home, true);
                } else {
                    self.apply(ctx, Action::New { kind, replace_page: false });
                }
            }
            Action::Open(path) => self.open(ctx, &path),
            Action::Template { path, title } => self.open_template(ctx, &path, &title),
            Action::OpenDialog => {
                let mut all: Vec<&str> = Vec::new();
                for k in Kind::APPS {
                    all.extend_from_slice(k.extensions());
                }
                let mut dialog = rfd::FileDialog::new().add_filter("All documents", &all);
                for k in Kind::APPS {
                    dialog = dialog.add_filter(format!("{} ({})", k.noun(), k.app_name()), k.extensions());
                }
                if let Some(files) = dialog.pick_files() {
                    for f in files {
                        self.open(ctx, &f.to_string_lossy());
                    }
                }
            }
            Action::Activate(id) => {
                if self.tab(id).is_some() {
                    self.active = id;
                }
            }
            Action::Close(id) => self.close(ctx, id),
            Action::CloseOthers(id) => {
                let others: Vec<u64> = self.display_order().into_iter().filter(|t| *t != id && !self.tab(*t).is_some_and(|t| t.pinned)).collect();
                self.active = id;
                for t in others {
                    self.close(ctx, t);
                }
            }
            Action::CloseAfter(id) => {
                let order = self.display_order();
                let after: Vec<u64> = order.iter().skip_while(|t| **t != id).skip(1).copied().collect();
                for t in after {
                    self.close(ctx, t);
                }
            }
            Action::Cycle(step) => {
                let order = self.display_order();
                if let Some(i) = order.iter().position(|t| *t == self.active)
                    && !order.is_empty()
                {
                    let n = order.len() as i64;
                    self.active = order[((i as i64 + step as i64).rem_euclid(n)) as usize];
                }
            }
            Action::MoveBy(id, step) => {
                let order = self.display_order();
                if let Some(i) = order.iter().position(|t| *t == id) {
                    let j = i as i64 + step as i64;
                    if j >= 0 && (j as usize) < order.len() {
                        let target = order[j as usize];
                        self.apply(ctx, Action::MoveTo { id, target, after: step > 0 });
                    }
                }
            }
            Action::MoveTo { id, target, after } => {
                let (Some(a), Some(b)) = (self.tab(id), self.tab(target)) else { return };
                if id == target || !self.same_group(a, b) {
                    return;
                }
                let Some(from) = self.index_of(id) else { return };
                let tab = self.tabs.remove(from);
                let to = self.index_of(target).map(|i| if after { i + 1 } else { i }).unwrap_or(self.tabs.len());
                self.tabs.insert(to.min(self.tabs.len()), tab);
            }
            Action::TogglePin(id) => {
                if let Some(t) = self.tab_mut(id) {
                    t.pinned = !t.pinned;
                }
            }
            Action::SetColor(id, color) => {
                if let Some(t) = self.tab_mut(id) {
                    t.color = color;
                }
            }
        }
    }

    /// Opens a file: shows its tab when it's already open, else in an untouched blank tab of its
    /// app (when that's the active tab and the setting is on), in place of an active Home page,
    /// or in a new tab.
    fn open(&mut self, ctx: &egui::Context, path: &str) {
        if let Some(t) = self.tabs.iter().find(|t| t.path().as_deref() == Some(path)) {
            self.active = t.id;
            return;
        }
        let Some(kind) = Kind::for_path(path) else {
            self.message(ctx, format!("No app in the suite opens {path}"));
            return;
        };
        let reuse = self.settings.reuse_blank_tab;
        let active = self.active;
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active)
            && let Content::Doc(doc) = &mut tab.content
            && reuse
            && doc.kind() == kind
            && doc.untouched()
        {
            match doc.open_here(path) {
                Ok(()) => self.settings.add_recent(path),
                Err(e) => self.message(ctx, format!("{path}: {e}")),
            }
            return;
        }
        match Doc::open(path, ctx) {
            Ok(doc) => {
                self.active = self.place(Content::Doc(doc), true);
                self.settings.add_recent(path);
            }
            Err(e) => self.message(ctx, format!("{path}: {e}")),
        }
    }

    /// A new tab with an untitled copy of a template: saving it asks where, and the template
    /// isn't added to Recent.
    fn open_template(&mut self, ctx: &egui::Context, path: &str, title: &str) {
        match Doc::open(path, ctx) {
            Ok(mut doc) => {
                doc.detach(title);
                self.active = self.place(Content::Doc(doc), true);
            }
            Err(e) => self.message(ctx, format!("{path}: {e}")),
        }
    }

    fn close(&mut self, ctx: &egui::Context, id: u64) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else { return };
        let check = match &mut tab.content {
            Content::Doc(doc) => doc.check_close(),
            _ => CloseCheck::Now,
        };
        match check {
            CloseCheck::Now => self.remove(ctx, id),
            CloseCheck::Ask => {
                self.active = id;
                self.prompt = Some(Prompt::SaveTab(id));
            }
            // WordCraft shows its prompt; its answer closes the tab (`Doc::close_confirmed`).
            CloseCheck::AppAsks => self.active = id,
            CloseCheck::Busy(msg) => self.message(ctx, msg),
        }
    }

    /// Takes a tab away; the window is never left without one.
    fn remove(&mut self, ctx: &egui::Context, id: u64) {
        let order = self.display_order();
        let Some(i) = self.index_of(id) else { return };
        let tab = self.tabs.remove(i);
        if let Content::Doc(doc) = &tab.content {
            doc.save_prefs();
        }
        drop(tab);
        if self.shown == Some(id) {
            self.shown = None;
        }
        if self.active == id {
            // The tab to its right in the tab bar, else the one to its left.
            let pos = order.iter().position(|t| *t == id).unwrap_or(0);
            let next = order.iter().skip(pos + 1).chain(order.iter().take(pos).rev()).find(|t| **t != id).copied();
            match next {
                Some(n) => self.active = n,
                None => {
                    self.active = self.push_tab(Content::Home, false);
                }
            }
        }
        if self.tabs.is_empty() {
            self.active = self.push_tab(Content::Home, false);
        }
        ctx.request_repaint();
    }

    // ---- quitting ---------------------------------------------------------------------------

    fn begin_quit(&mut self, ctx: &egui::Context) {
        let dirty: Vec<u64> = self.tabs.iter().filter(|t| t.dirty()).map(|t| t.id).collect();
        let docs = self.tabs.iter().filter(|t| !t.is_page()).count();
        if !dirty.is_empty() {
            self.prompt = Some(Prompt::Quit(dirty));
        } else if self.settings.confirm_quit_with_tabs && docs > 1 {
            self.prompt = Some(Prompt::ConfirmQuit(docs));
        } else {
            self.quit(ctx);
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        for t in &mut self.tabs {
            if let Content::Doc(d) = &mut t.content {
                d.allow_window_close();
            }
        }
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // ---- per frame --------------------------------------------------------------------------

    fn drain_incoming(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.incoming else { return };
        let messages: Vec<instance::Message> = rx.try_iter().collect();
        for m in messages {
            match m {
                instance::Message::Open(p) => self.actions.push(Action::Open(p)),
                instance::Message::New(k) => self.actions.push(Action::New { kind: k, replace_page: false }),
                instance::Message::Focus => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }

    /// The suite's shortcuts pressed while a Markd page had the keyboard.
    #[cfg(feature = "markd")]
    fn markd_shortcuts(&mut self) {
        let active = self.active;
        let mut actions = Vec::new();
        for t in &mut self.tabs {
            if let Content::Doc(Doc::Mark(m)) = &mut t.content {
                m.take_actions(&self.settings, active, &mut actions);
            }
        }
        self.actions.extend(actions);
    }

    /// The suite's keyboard shortcuts, taken out of the input before the app sees them.
    fn take_shortcuts(&mut self, raw: &mut egui::RawInput) {
        let active = self.active;
        let mut actions = Vec::new();
        // Ctrl+N: a new tab of the active tab's app (on Home or Settings, what a new tab shows).
        if self.settings.ctrl_n_new_tab {
            let new = match self.tab(active).map(|t| t.kind()) {
                Some(k) if k.is_app() => Action::New { kind: k, replace_page: false },
                _ => Action::NewDefault,
            };
            let mut new = Some(new);
            raw.events.retain(|e| {
                let egui::Event::Key { key: egui::Key::N, pressed, modifiers, .. } = e else { return true };
                if !(modifiers.ctrl && !modifiers.alt && !modifiers.shift) {
                    return true;
                }
                if *pressed && !matches!(e, egui::Event::Key { repeat: true, .. }) {
                    actions.extend(new.take());
                }
                false
            });
        }
        if !self.settings.shortcuts {
            self.actions.extend(actions);
            return;
        }
        raw.events.retain(|e| {
            let egui::Event::Key { key, pressed, modifiers, .. } = e else { return true };
            if !(modifiers.ctrl && modifiers.alt) {
                return true;
            }
            let action = match (key, modifiers.shift) {
                (egui::Key::PageDown, false) => Action::Cycle(1),
                (egui::Key::PageUp, false) => Action::Cycle(-1),
                (egui::Key::PageDown, true) => Action::MoveBy(active, 1),
                (egui::Key::PageUp, true) => Action::MoveBy(active, -1),
                (egui::Key::Insert, _) => Action::NewDefault,
                (egui::Key::F4, _) => Action::Close(active),
                (egui::Key::Home, _) => Action::Home,
                _ => return true,
            };
            if *pressed {
                actions.push(action);
            }
            false
        });
        self.actions.extend(actions);
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.tab(self.active) {
            Some(t) if !t.is_page() => format!("{}{} — Craft Suite", t.title(), if t.dirty() { " •" } else { "" }),
            _ => "Craft Suite".to_string(),
        };
        if title != self.title_sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_sent = title;
        }
    }

    fn save_settings(&mut self) {
        if self.settings != self.saved {
            self.settings.save();
            self.saved = self.settings.clone();
        }
    }

    fn show_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.prompt.take() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::new("craft_suite_prompt")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            match &prompt {
                Prompt::SaveTab(id) => {
                    let Some(tab) = self.tab(*id) else {
                        keep = false;
                        return;
                    };
                    let name = tab.title();
                    ui.label(egui::RichText::new(format!("Do you want to save the changes to “{name}”?")).strong());
                    ui.label("Your changes will be lost if you don't save them.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            keep = false;
                            let result = match self.tabs.iter_mut().find(|t| t.id == *id).map(|t| &mut t.content) {
                                Some(Content::Doc(doc)) => doc.save_for_close(),
                                _ => Ok(true),
                            };
                            match result {
                                Ok(true) => self.actions.push(Action::Close(*id)),
                                Ok(false) => {}
                                Err(e) => self.prompt = Some(Prompt::Message(format!("Couldn't save “{name}”: {e}"))),
                            }
                        }
                        if ui.button("Don't Save").clicked() {
                            keep = false;
                            if let Some(Content::Doc(doc)) = self.tabs.iter_mut().find(|t| t.id == *id).map(|t| &mut t.content) {
                                doc.discard();
                            }
                            self.remove(ui.ctx(), *id);
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Prompt::Quit(ids) => {
                    ui.label(egui::RichText::new("Some tabs have unsaved changes").strong());
                    for id in ids {
                        if let Some(t) = self.tab(*id) {
                            ui.label(format!("• {} ({})", t.title(), t.kind().app_name()));
                        }
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Review").on_hover_text("Show the first unsaved tab").clicked() {
                            keep = false;
                            if let Some(id) = ids.first() {
                                self.active = *id;
                            }
                        }
                        if ui.button("Quit without saving").clicked() {
                            keep = false;
                            for t in &mut self.tabs {
                                if let Content::Doc(d) = &mut t.content
                                    && d.dirty()
                                {
                                    d.discard();
                                }
                            }
                            self.quit(ui.ctx());
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Prompt::ConfirmQuit(n) => {
                    ui.label(egui::RichText::new(format!("Close Craft Suite and its {n} tabs?")).strong());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            keep = false;
                            self.quit(ui.ctx());
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Prompt::Message(text) => {
                    ui.label(text);
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        keep = false;
                    }
                }
            }
        });
        if modal.should_close() && !matches!(prompt, Prompt::Quit(_) | Prompt::ConfirmQuit(_)) {
            keep = false;
        }
        if keep && self.prompt.is_none() {
            self.prompt = Some(prompt);
        }
    }

    fn show_toast(&mut self, ui: &mut egui::Ui) {
        let Some((text, since)) = &self.toast else { return };
        let now = ui.input(|i| i.time);
        if now - since > 5.0 {
            self.toast = None;
            return;
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        let text = text.clone();
        egui::Area::new(egui::Id::new("craft_suite_toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -36.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(text);
                });
            });
    }
}

impl SuiteApp {
    /// The suite draws the title bar (Settings › Window).
    pub fn integrated(&self) -> bool {
        self.settings.title_bar == TitleBar::Integrated
    }

    /// Whether the window fills the screen (no edges to resize, no border).
    pub fn maximized(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false))
    }

    /// Without the desktop's title bar the window has no frame to resize it by: a press within
    /// a few points of the window's edge resizes it instead of reaching the tab or app under it.
    fn window_edges(&mut self, raw: &mut egui::RawInput) {
        let info = raw.viewports.get(&egui::ViewportId::ROOT);
        let full = info.is_some_and(|i| i.maximized.unwrap_or(false) || i.fullscreen.unwrap_or(false));
        let Some(screen) = raw.screen_rect else { return };
        let mut resize = None;
        raw.events.retain(|e| match e {
            egui::Event::PointerMoved(p) => {
                self.pointer = Some(*p);
                true
            }
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } if !full => {
                match edge_at(screen, *pos) {
                    Some(dir) => {
                        resize = Some(dir);
                        false
                    }
                    None => true,
                }
            }
            _ => true,
        });
        if resize.is_some() {
            self.pending_resize = resize;
        }
    }

    /// Resize cursors along the window's edges, and a thin border around it.
    fn window_frame(&self, ctx: &egui::Context) {
        if Self::maximized(ctx) {
            return;
        }
        let screen = ctx.content_rect();
        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()).or(self.pointer)
            && let Some(dir) = edge_at(screen, pos)
            && ctx.input(|i| i.pointer.hover_pos().is_some())
        {
            ctx.set_cursor_icon(resize_cursor(dir));
        }
        let dark = crate::tabbar::is_dark(self, ctx);
        let color = if dark { egui::Color32::from_gray(70) } else { egui::Color32::from_gray(170) };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("craft_suite_window_border")));
        painter.rect_stroke(screen.shrink(0.5), 0.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Middle);
    }
}

impl eframe::App for SuiteApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // The window's close button: the suite decides (unsaved tabs), not the app on show.
        if let Some(info) = raw.viewports.get_mut(&egui::ViewportId::ROOT)
            && !self.allow_close
            && info.events.iter().any(|e| matches!(e, egui::ViewportEvent::Close))
        {
            info.events.retain(|e| !matches!(e, egui::ViewportEvent::Close));
            self.close_requested = true;
        }
        if self.integrated() {
            self.window_edges(raw);
        }
        // Documents dropped on the window open as tabs; anything else (a picture, a font) goes
        // to the app on show, which inserts it.
        let dropped = std::mem::take(&mut raw.dropped_files);
        for f in dropped {
            let p = f.path().to_string_lossy().to_string();
            if !p.is_empty() && Kind::for_path(&p).is_some() {
                self.actions.push(Action::Open(p));
            } else {
                raw.dropped_files.push(f);
            }
        }
        self.take_shortcuts(raw);
        if self.shown == Some(self.active)
            && let Some(Tab { content: Content::Doc(doc), .. }) = self.tabs.iter_mut().find(|t| t.id == self.active)
        {
            doc.raw_input_hook(ctx, raw);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.home_look.is_none() {
            let home = Look::capture(ctx);
            self.font_union = Some(crate::look::seed_union(home.fonts()));
            self.home_look = Some(home);
        }
        self.drain_incoming(ctx);
        #[cfg(feature = "markd")]
        self.markd_shortcuts();
        let decorations = !self.integrated();
        if self.decorations_sent != Some(decorations) {
            if self.decorations_sent.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(decorations));
            }
            self.decorations_sent = Some(decorations);
        }
        if let Some(dir) = self.pending_resize.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
        self.apply_actions(ctx);
        #[cfg(feature = "ai")]
        crate::ai::logic(self, ctx);
        if self.tabs.is_empty() {
            let kind = match self.settings.new_tab_opens {
                NewTabOpens::App(k) => k,
                NewTabOpens::Home => Kind::Home,
            };
            match Doc::new_blank(kind, ctx) {
                Some(doc) => self.active = self.push_tab(Content::Doc(doc), false),
                None => self.active = self.push_tab(Content::Home, false),
            }
        }
        if self.tab(self.active).is_none() {
            self.active = self.tabs[0].id;
        }
        if std::mem::take(&mut self.close_requested) && self.prompt.is_none() {
            self.begin_quit(ctx);
        }
        // WordCraft tabs whose Save prompt was answered (or File › Close chosen) close.
        let done: Vec<u64> =
            self.tabs.iter_mut().filter_map(|t| match &mut t.content {
                Content::Doc(d) => d.close_confirmed().then_some(t.id),
                _ => None,
            }).collect();
        for id in done {
            self.remove(ctx, id);
        }

        // Switching tabs, first frame: keep the old tab's look and install the new one's fonts;
        // no app draws this frame. Second frame (below): the new tab's style (see `look`).
        if self.shown != Some(self.active) {
            let leaving = self.shown;
            if let Some(prev) = leaving.and_then(|id| self.tabs.iter_mut().find(|t| t.id == id))
                && !prev.is_page()
            {
                let look = Look::capture(ctx);
                if let Some(union) = &mut self.font_union {
                    *union = crate::look::merged(union, look.fonts());
                }
                prev.look = Some(look);
            }
            let home = self.home_look.as_ref().map(|l| l.with_theme(self.theme_preference()));
            self.clock += 1;
            let clock = self.clock;
            let target = self.tabs.iter_mut().find(|t| t.id == self.active).and_then(|tab| {
                tab.last_active = clock;
                match &tab.look {
                    Some(look) if !tab.is_page() => Some(look.clone()),
                    _ => home.clone(),
                }
            });
            if let (Some(target), Some(union)) = (&target, &self.font_union) {
                target.restore_fonts(ctx, union);
            }
            self.pending_look = target;
            self.shown = Some(self.active);
            self.skip_frame = true;
            ctx.request_repaint();
            return;
        }
        if let Some(look) = self.pending_look.take() {
            look.restore_options(ctx);
            if let Some(Tab { content: Content::Doc(doc), .. }) = self.tabs.iter_mut().find(|t| t.id == self.active) {
                doc.activated(ctx);
            }
        }
        let pages_theme = self.theme_preference();
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == self.active) {
            match &mut tab.content {
                Content::Doc(doc) => doc.logic(ctx, frame),
                // The suite theme can change while a page shows.
                _ => {
                    if ctx.options(|o| o.theme_preference) != pages_theme {
                        ctx.options_mut(|o| o.theme_preference = pages_theme);
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        crate::tabbar::show(self, ui);
        #[cfg(feature = "ai")]
        crate::ai::panel(self, ui);
        let skipped = self.skip_frame;
        if std::mem::take(&mut self.skip_frame) {
            ui.ctx().request_repaint();
        } else {
            let active = self.active;
            let is_page = self.tab(active).map(|t| t.is_page());
            match is_page {
                Some(true) => {
                    if matches!(self.tab(active).map(|t| &t.content), Some(Content::Settings)) {
                        crate::pages::settings(self, ui);
                    } else {
                        crate::pages::home(self, ui);
                    }
                }
                Some(false) => {
                    if let Some(Tab { content: Content::Doc(doc), id, .. }) = self.tabs.iter_mut().find(|t| t.id == active) {
                        let id = *id;
                        crate::fontbar::show(&mut self.fontbar, &mut self.settings, id, doc, ui);
                        ui.push_id(("craft_suite_tab", id), |ui| doc.ui(ui, frame));
                    }
                }
                None => {}
            }
        }
        self.show_prompt(ui.ctx());
        self.show_toast(ui);
        if self.integrated() {
            self.window_frame(ui.ctx());
        }
        #[cfg(feature = "markd")]
        {
            let active = self.active;
            let views = self.tabs.iter_mut().filter_map(|t| match &mut t.content {
                Content::Doc(Doc::Mark(m)) => Some((t.id, m.as_mut())),
                _ => None,
            });
            crate::markd::sync(ui.ctx(), frame, Some(active), views, skipped);
        }
        #[cfg(not(feature = "markd"))]
        let _ = skipped;
        self.update_title(ui.ctx());
        self.save_settings();
    }

    fn on_exit(&mut self) {
        hosts::finish_print_jobs();
        // The newest preferences of each app are written last.
        let mut order: Vec<&Tab> = self.tabs.iter().collect();
        order.sort_by_key(|t| t.last_active);
        for t in order {
            if let Content::Doc(d) = &t.content {
                d.save_prefs();
            }
        }
        self.settings.session = self
            .tabs
            .iter()
            .filter_map(|t| t.path().map(|path| SavedTab { path, pinned: t.pinned, color: t.color }))
            .collect();
        self.settings.save();
        if self.incoming.is_some() {
            instance::cleanup();
        }
    }
}
