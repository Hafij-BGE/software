//! Craft Suite's own settings: how tabs are arranged, coloured and mixed, and how the window
//! behaves. Stored as `settings.json` in `$XDG_CONFIG_HOME/craft-suite` (`~/.config/craft-suite`).
//! The apps keep their own settings in their own folders, as when they run on their own.

use serde::{Deserialize, Serialize};

use crate::kind::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl TabPosition {
    pub const ALL: [TabPosition; 4] = [TabPosition::Top, TabPosition::Bottom, TabPosition::Left, TabPosition::Right];
    pub fn label(self) -> &'static str {
        match self {
            TabPosition::Top => "Top",
            TabPosition::Bottom => "Bottom",
            TabPosition::Left => "Left (vertical)",
            TabPosition::Right => "Right (vertical)",
        }
    }
    pub fn vertical(self) -> bool {
        matches!(self, TabPosition::Left | TabPosition::Right)
    }
}

/// How tabs of different apps share the tab bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mixing {
    /// One strip, any order: documents, decks and sheets side by side, as you arrange them.
    Mixed,
    /// One strip, sorted by app (in the app order below); you arrange tabs within each app.
    Grouped,
    /// A row (or, on the side, a section) per app.
    Separate,
}

impl Mixing {
    pub const ALL: [Mixing; 3] = [Mixing::Mixed, Mixing::Grouped, Mixing::Separate];
    pub fn label(self) -> &'static str {
        match self {
            Mixing::Mixed => "Mixed: any order",
            Mixing::Grouped => "Grouped by app",
            Mixing::Separate => "Separate row per app",
        }
    }
}

/// How a tab shows its app colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorStyle {
    /// A coloured line along the tab's edge.
    Stripe,
    /// A light wash of the colour behind the tab.
    Tint,
    /// The tab filled with the colour.
    Solid,
    /// No colour; the icon tells the apps apart.
    Plain,
}

impl ColorStyle {
    pub const ALL: [ColorStyle; 4] = [ColorStyle::Stripe, ColorStyle::Tint, ColorStyle::Solid, ColorStyle::Plain];
    pub fn label(self) -> &'static str {
        match self {
            ColorStyle::Stripe => "Stripe",
            ColorStyle::Tint => "Tint",
            ColorStyle::Solid => "Solid",
            ColorStyle::Plain => "None",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseButtons {
    Always,
    ActiveTab,
    Never,
}

impl CloseButtons {
    pub const ALL: [CloseButtons; 3] = [CloseButtons::Always, CloseButtons::ActiveTab, CloseButtons::Never];
    pub fn label(self) -> &'static str {
        match self {
            CloseButtons::Always => "On every tab",
            CloseButtons::ActiveTab => "On the active tab",
            CloseButtons::Never => "Hidden",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewTabPlacement {
    End,
    AfterCurrent,
}

/// What the tab bar (and the Home and Settings pages) look like; the apps follow their own theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuiteTheme {
    System,
    Light,
    Dark,
}

impl SuiteTheme {
    pub const ALL: [SuiteTheme; 3] = [SuiteTheme::System, SuiteTheme::Light, SuiteTheme::Dark];
    pub fn label(self) -> &'static str {
        match self {
            SuiteTheme::System => "Follow the system",
            SuiteTheme::Light => "Light",
            SuiteTheme::Dark => "Dark",
        }
    }
}

/// The interface font of every app (`Settings::ui_font`; see `uifont.rs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiFont {
    /// Each app's own interface font.
    Own,
    /// Inter, bundled (the face WordCraft and DeckCraft draw their interfaces with).
    Inter,
    /// The desktop's interface font.
    System,
    /// An installed font family.
    Family(String),
}

/// AutoSave in every app, or each app's own choice (Settings › You and your files).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoSave {
    EachApp,
    On,
    Off,
}

impl AutoSave {
    pub const ALL: [AutoSave; 3] = [AutoSave::EachApp, AutoSave::On, AutoSave::Off];

    pub fn label(self) -> &'static str {
        match self {
            AutoSave::EachApp => "Each app's own",
            AutoSave::On => "On",
            AutoSave::Off => "Off",
        }
    }
}

/// The suite's logo: on the window, the Home page and Settings, and (on Linux) in the desktop's
/// app grid and dock. Classic is the icon the suite installs with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Logo {
    Classic,
    Tiles,
    Ring,
    Sheets,
    Stack,
    Page,
}

impl Logo {
    pub const ALL: [Logo; 6] = [Logo::Classic, Logo::Tiles, Logo::Ring, Logo::Sheets, Logo::Stack, Logo::Page];

    pub fn label(self) -> &'static str {
        match self {
            Logo::Classic => "Classic",
            Logo::Tiles => "Tiles",
            Logo::Ring => "Ring",
            Logo::Sheets => "Sheets",
            Logo::Stack => "Stack",
            Logo::Page => "Page",
        }
    }
}

/// Where the window's title bar (move, minimize, maximize, close) is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleBar {
    /// Drawn by the suite: the tabs sit in the title bar, with the window buttons beside them.
    Integrated,
    /// The desktop's own title bar above the tabs.
    System,
    /// No title bar of its own: the window buttons sit at the right end of each app's top row
    /// (AutoSave, the file name, Share, Search), at that row's height. Home, Settings and Markd
    /// tabs, which have no such row, keep the suite's.
    InApp,
}

impl TitleBar {
    pub const ALL: [TitleBar; 3] = [TitleBar::Integrated, TitleBar::InApp, TitleBar::System];
    pub fn label(self) -> &'static str {
        match self {
            TitleBar::Integrated => "Tabs in the title bar",
            TitleBar::InApp => "Window buttons in each app's top bar",
            TitleBar::System => "The desktop's title bar",
        }
    }
}

/// What a new tab (the + button, Ctrl+Alt+T) opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewTabOpens {
    Home,
    App(Kind),
}

/// An RGB colour, stored as "#rrggbb".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub [u8; 3]);

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2]))
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let hex = s.trim_start_matches('#');
        let byte = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (hex.len(), byte(0), byte(2), byte(4)) {
            (6, Some(r), Some(g), Some(b)) => Ok(Rgb([r, g, b])),
            _ => Err(serde::de::Error::custom(format!("not a #rrggbb colour: {s}"))),
        }
    }
}

impl Rgb {
    pub fn color32(self) -> egui::Color32 {
        egui::Color32::from_rgb(self.0[0], self.0[1], self.0[2])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // ---- arrangement
    pub tab_position: TabPosition,
    pub mixing: Mixing,
    /// The order of the apps when tabs are grouped or in separate rows (and on the Home page).
    pub app_order: Vec<Kind>,
    pub new_tab_placement: NewTabPlacement,
    /// The widest a tab gets, in points (tabs shrink towards `tab_min_width` when space is short).
    pub tab_max_width: f32,
    pub tab_min_width: f32,
    /// Width of the side tab bar when tabs are on the left or right.
    pub side_width: f32,
    /// The side tab bar folds to a thin strip at the window's edge and opens over the page
    /// while the pointer is on it (the arrow at its top switches this too).
    pub side_auto_hide: bool,
    pub show_icons: bool,
    pub close_buttons: CloseButtons,
    /// Pinned tabs show their icon only.
    pub compact_pinned: bool,

    // ---- colour
    pub color_style: ColorStyle,
    pub word_color: Rgb,
    pub deck_color: Rgb,
    pub grid_color: Rgb,
    pub mark_color: Rgb,
    pub home_color: Rgb,
    /// Colour the whole tab bar with the active tab's app colour.
    pub tint_bar_with_active: bool,
    pub theme: SuiteTheme,
    /// Every app (WordApp, SlideApp, SheetApp, Markd) follows `theme` instead of its own choice.
    pub apps_follow_theme: bool,

    // ---- window
    pub title_bar: TitleBar,
    /// The one font of every app's interface (menus, ribbons, panels, the tab bar), or each
    /// app's own (`UiFont::Own`). Documents keep their fonts.
    pub ui_font: UiFont,
    /// Every app's interface at one size (`ui_size`) instead of each app's own interface size;
    /// the tab bar then keeps its size from tab to tab.
    pub same_size: bool,
    /// The interface size with `same_size`, in percent of the display's own scale.
    pub ui_size: u16,
    pub logo: Logo,

    // ---- you and your files (every app takes these; empty or EachApp leaves the app's own)
    /// Your name on comments, tracked changes and a document's author.
    pub user_name: String,
    /// Your initials on comments (empty: from the name).
    pub user_initials: String,
    pub autosave: AutoSave,
    /// The language spelling is checked in, as a tag like "en-US" (empty: each app's own).
    pub spell_language: String,
    /// New presentations save as PowerPoint (.pptx) instead of DeckCraft's own format.
    pub pptx_for_new: bool,

    // ---- behaviour
    pub new_tab_opens: NewTabOpens,
    /// Opening a file reuses an untouched blank tab of the same app instead of adding a tab.
    pub reuse_blank_tab: bool,
    pub restore_tabs: bool,
    /// Files opened from the file manager go to the running window as new tabs.
    pub single_window: bool,
    pub middle_click_closes: bool,
    pub double_click_bar_new_tab: bool,
    pub confirm_quit_with_tabs: bool,
    pub check_updates_on_start: bool,
    /// The suite's keyboard shortcuts (Ctrl+Alt+PageUp/PageDown and the rest; see Settings).
    pub shortcuts: bool,
    /// Ctrl+N opens a new tab of the active tab's app (instead of the app's own New).
    pub ctrl_n_new_tab: bool,
    /// A bar over a document whose fonts aren't installed, offering to install them (src/fontbar.rs).
    pub font_notice: bool,

    // ---- remembered
    pub recent: Vec<String>,
    /// Files open when the suite last closed ([`Settings::restore_tabs`]), with their pinned state.
    pub session: Vec<SavedTab>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub path: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub color: Option<Rgb>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            tab_position: TabPosition::Top,
            mixing: Mixing::Mixed,
            app_order: Kind::APPS.to_vec(),
            new_tab_placement: NewTabPlacement::AfterCurrent,
            tab_max_width: 220.0,
            tab_min_width: 90.0,
            side_width: 220.0,
            side_auto_hide: false,
            show_icons: true,
            close_buttons: CloseButtons::Always,
            compact_pinned: true,
            color_style: ColorStyle::Stripe,
            word_color: Rgb([0x3b, 0x5b, 0xdb]),
            deck_color: Rgb([0xf2, 0x6b, 0x1d]),
            grid_color: Rgb([0x1f, 0x9d, 0x55]),
            mark_color: Rgb([0x6d, 0x4a, 0xe8]),
            home_color: Rgb([0x6b, 0x6f, 0x7a]),
            tint_bar_with_active: false,
            theme: SuiteTheme::System,
            title_bar: TitleBar::Integrated,
            logo: Logo::Classic,
            apps_follow_theme: true,
            user_name: String::new(),
            user_initials: String::new(),
            autosave: AutoSave::EachApp,
            spell_language: String::new(),
            pptx_for_new: false,
            ui_font: UiFont::System,
            same_size: true,
            ui_size: 100,
            new_tab_opens: NewTabOpens::Home,
            reuse_blank_tab: true,
            font_notice: true,
            restore_tabs: true,
            single_window: true,
            middle_click_closes: true,
            double_click_bar_new_tab: true,
            confirm_quit_with_tabs: false,
            check_updates_on_start: false,
            shortcuts: true,
            ctrl_n_new_tab: true,
            recent: Vec::new(),
            session: Vec::new(),
        }
    }
}

impl Settings {
    pub fn color_of(&self, kind: Kind) -> Rgb {
        match kind {
            Kind::Word => self.word_color,
            Kind::Deck => self.deck_color,
            Kind::Grid => self.grid_color,
            Kind::Mark => self.mark_color,
            Kind::Home | Kind::Settings => self.home_color,
        }
    }

    pub fn color_mut(&mut self, kind: Kind) -> &mut Rgb {
        match kind {
            Kind::Word => &mut self.word_color,
            Kind::Deck => &mut self.deck_color,
            Kind::Grid => &mut self.grid_color,
            Kind::Mark => &mut self.mark_color,
            Kind::Home | Kind::Settings => &mut self.home_color,
        }
    }

    /// Position of `kind` in the app order (Home and Settings pages sort last).
    pub fn rank(&self, kind: Kind) -> usize {
        self.app_order.iter().position(|k| *k == kind).unwrap_or(usize::MAX)
    }

    /// Repairs an app order from an old or hand-edited file: every app exactly once.
    fn sanitize(&mut self) {
        let mut order: Vec<Kind> = Vec::new();
        for k in self.app_order.iter().copied().chain(Kind::APPS) {
            if k.is_app() && !order.contains(&k) {
                order.push(k);
            }
        }
        self.app_order = order;
        self.tab_min_width = self.tab_min_width.clamp(40.0, 400.0);
        self.tab_max_width = self.tab_max_width.clamp(self.tab_min_width, 600.0);
        self.side_width = self.side_width.clamp(120.0, 480.0);
        self.ui_size = self.ui_size.clamp(75, 200);
        self.recent.truncate(RECENT_MAX);
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(RECENT_MAX);
    }

    pub fn load() -> Settings {
        let mut s = path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Settings>(&b).map_err(|e| log::warn!("settings.json: {e}")).ok())
            .unwrap_or_default();
        s.sanitize();
        s
    }

    pub fn save(&self) {
        if no_prefs() {
            return;
        }
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_vec_pretty(self) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&p, bytes) {
                    log::warn!("{}: {e}", p.display());
                }
            }
            Err(e) => log::warn!("settings: {e}"),
        }
    }
}

const RECENT_MAX: usize = 12;

/// `CRAFT_SUITE_NO_PREFS` (tests, trying things out) neither reads nor writes any settings,
/// the suite's or the apps'.
pub fn no_prefs() -> bool {
    std::env::var_os("CRAFT_SUITE_NO_PREFS").is_some()
}

pub fn config_dir() -> Option<std::path::PathBuf> {
    // Windows: %APPDATA% (C:\Users\<you>\AppData\Roaming), as WordCraft keeps its settings.
    if cfg!(windows) {
        return std::env::var_os("APPDATA").filter(|d| !d.is_empty()).map(std::path::PathBuf::from);
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
}

fn path() -> Option<std::path::PathBuf> {
    if no_prefs() {
        return None;
    }
    Some(config_dir()?.join("craft-suite").join("settings.json"))
}
