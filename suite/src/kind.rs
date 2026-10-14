//! What a tab holds: one of the three apps, or one of the suite's own pages.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Word,
    Deck,
    Grid,
    Home,
    Settings,
}

impl Kind {
    pub const APPS: [Kind; 3] = [Kind::Word, Kind::Deck, Kind::Grid];

    pub fn is_app(self) -> bool {
        matches!(self, Kind::Word | Kind::Deck | Kind::Grid)
    }

    /// The app's name.
    pub fn app_name(self) -> &'static str {
        match self {
            Kind::Word => "WordCraft",
            Kind::Deck => "DeckCraft",
            Kind::Grid => "GridCraft",
            Kind::Home => "Home",
            Kind::Settings => "Settings",
        }
    }

    /// What the app makes, for the Home page and menus.
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Word => "Document",
            Kind::Deck => "Presentation",
            Kind::Grid => "Spreadsheet",
            Kind::Home => "Home",
            Kind::Settings => "Settings",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Kind::Word => "Word processor: .docx, .doc, .odt, .rtf, .md, .html, .txt",
            Kind::Deck => "Presentations: .pptx, templates and slideshows",
            Kind::Grid => "Spreadsheets: .xlsx, .xlsm, .xlsb, .ods, .csv, .tsv",
            Kind::Home => "",
            Kind::Settings => "",
        }
    }

    /// The command-line names `--new <name>` accepts.
    pub fn from_cli(name: &str) -> Option<Kind> {
        match name.to_ascii_lowercase().as_str() {
            "word" | "wordcraft" | "document" | "doc" | "docx" => Some(Kind::Word),
            "deck" | "deckcraft" | "presentation" | "slides" | "ppt" | "pptx" => Some(Kind::Deck),
            "grid" | "gridcraft" | "spreadsheet" | "sheet" | "excel" | "xlsx" => Some(Kind::Grid),
            "home" => Some(Kind::Home),
            "settings" => Some(Kind::Settings),
            _ => None,
        }
    }

    /// The file extensions each app opens (from the MIME types its upstream package registers).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::Word => &["docx", "docm", "dotx", "dotm", "doc", "dot", "odt", "rtf", "md", "markdown", "tex", "html", "htm", "txt"],
            Kind::Deck => &["pptx", "potx", "ppsx", "deckcraft"],
            Kind::Grid => &["xlsx", "xlsm", "xlsb", "ods", "csv", "tsv"],
            Kind::Home | Kind::Settings => &[],
        }
    }

    /// The app that opens `path`, by its extension.
    pub fn for_path(path: &str) -> Option<Kind> {
        let ext = std::path::Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
        Kind::APPS.into_iter().find(|k| k.extensions().contains(&ext.as_str()))
    }

    pub fn icon_png(self) -> &'static [u8] {
        match self {
            Kind::Word => include_bytes!("../../upstream/wordcraft/assets/app-icon/hicolor/64x64/apps/ai.storyteller.wordcraft.png"),
            Kind::Deck => include_bytes!("../../upstream/deckcraft/assets/app-icon/hicolor/64x64/apps/ai.storyteller.deckcraft.png"),
            Kind::Grid => include_bytes!("../../upstream/gridcraft/assets/app-icon/hicolor/64x64/apps/ai.storyteller.gridcraft.png"),
            Kind::Home | Kind::Settings => include_bytes!("../../assets/hicolor/64x64/apps/craft-suite.png"),
        }
    }

    pub fn large_icon_png(self) -> &'static [u8] {
        match self {
            Kind::Word => include_bytes!("../../upstream/wordcraft/assets/app-icon/hicolor/128x128/apps/ai.storyteller.wordcraft.png"),
            Kind::Deck => include_bytes!("../../upstream/deckcraft/assets/app-icon/hicolor/128x128/apps/ai.storyteller.deckcraft.png"),
            Kind::Grid => include_bytes!("../../upstream/gridcraft/assets/app-icon/hicolor/128x128/apps/ai.storyteller.gridcraft.png"),
            Kind::Home | Kind::Settings => include_bytes!("../../assets/hicolor/128x128/apps/craft-suite.png"),
        }
    }
}
