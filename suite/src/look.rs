//! Each app styles the one egui context the whole window shares: its fonts, light and dark
//! visuals, spacing, zoom factor and a few options. The apps do it once and remember that they
//! did, so with several apps in one window the suite keeps each tab's look: when a tab is left
//! its look is captured, and when it's shown again that look is put back.
//!
//! Two egui rules shape how. Fonts given to `Context::set_fonts` only take effect at the start of
//! the next pass, while style changes take effect at once; and drawing text in a font family the
//! installed fonts don't define makes egui panic. So:
//!
//! - a switch takes two frames: the first installs the new tab's fonts and keeps the old style
//!   (which still matches the fonts in use), the second, with the new fonts in, puts the new
//!   tab's style back (see `SuiteApp::logic`);
//! - the fonts installed always include every font family any app defines (the "union", seeded
//!   from each app's own font definitions), so no style ever names a missing family, including
//!   during an app's first frames, while it installs its own fonts.

use egui::{FontDefinitions, FontFamily};

use crate::uifont::Face;

#[derive(Clone)]
pub struct Look {
    options: egui::Options,
    fonts: FontDefinitions,
}

impl Look {
    /// The context's current look. Call during a pass (the fonts exist from the first pass on).
    pub fn capture(ctx: &egui::Context) -> Look {
        Look { options: ctx.options(|o| o.clone()), fonts: ctx.fonts(|f| f.definitions().clone()) }
    }

    pub fn fonts(&self) -> &FontDefinitions {
        &self.fonts
    }

    /// First half of putting this look back: its fonts (with every other family in `union`, and
    /// the interface in the suite's font `face`, if any), in from the next pass. Identical
    /// fonts cost a comparison.
    pub fn restore_fonts(&self, ctx: &egui::Context, union: &FontDefinitions, face: Option<&Face>) {
        ctx.set_fonts(with_suite_font(&merged(&self.fonts, union), face));
    }

    /// Second half, once the fonts are in: the options (visuals, spacing, theme, zoom).
    /// `zoom` replaces the look's own zoom factor (the suite's one interface size).
    pub fn restore_options(&self, ctx: &egui::Context, zoom: Option<f32>) {
        ctx.options_mut(|o| {
            // The zoom factor goes through `set_zoom_factor`, which also updates the scale.
            let zoom = o.zoom_factor;
            *o = self.options.clone();
            o.zoom_factor = zoom;
        });
        ctx.set_zoom_factor(zoom.unwrap_or(self.options.zoom_factor));
    }

    /// The suite's own look (Home and Settings pages), with the theme from the suite settings.
    pub fn with_theme(&self, theme: egui::ThemePreference) -> Look {
        // The pages' size is the suite's: `restore_options` gets it when one size is set.
        let mut look = self.clone();
        look.options.theme_preference = theme;
        // Where the desktop doesn't say light or dark, light, as the tab bar does.
        look.options.fallback_theme = egui::Theme::Light;
        look.options.zoom_factor = 1.0;
        look
    }
}

/// `primary` with every font and named family of `union` it lacks. Proportional and Monospace,
/// and any family both define, stay `primary`'s, so the tab on show looks as in its own app.
pub fn merged(primary: &FontDefinitions, union: &FontDefinitions) -> FontDefinitions {
    let mut out = primary.clone();
    for (name, data) in &union.font_data {
        out.font_data.entry(name.clone()).or_insert_with(|| data.clone());
    }
    for (family, faces) in &union.families {
        if matches!(family, egui::FontFamily::Name(_)) {
            out.families.entry(family.clone()).or_insert_with(|| faces.clone());
        }
    }
    out
}

/// The union to start from: the suite's fonts and each app's interface fonts as the apps define
/// them, so every family an app draws with exists from its first frame on.
pub fn seed_union(home: &FontDefinitions) -> FontDefinitions {
    let mut union = home.clone();
    for defs in [
        wordcraft_ui_egui::theme::font_definitions(false, false),
        deckcraft_ui_egui::theme::font_definitions(&[]),
        gridcraft_ui_egui::theme::font_definitions(),
    ] {
        union = merged(&union, &defs);
    }
    union
}

/// The suite's interface font (`Settings::ui_font`, see `uifont.rs`) goes first in the
/// families the apps draw interface text with, under these names plus the font's key.
const SUITE_UI: &str = "craft-suite-ui";
const SUITE_UI_BOLD: &str = "craft-suite-ui-bold";

/// The families the apps draw interface text with: egui's default (every app), GridCraft's
/// "ui" (plain), and its "ui-bold" (bold).
const PLAIN: [&str; 1] = ["ui"];
const BOLD: [&str; 1] = ["ui-bold"];

fn names(face: &Face) -> (String, String) {
    (format!("{SUITE_UI}:{}", face.key), format!("{SUITE_UI_BOLD}:{}", face.key))
}

fn is_suite_name(name: &str) -> bool {
    name.starts_with(SUITE_UI)
}

/// Whether `defs` draw the interface in `face` (or, for `None`, in the app's own font).
pub fn has_suite_font(defs: &FontDefinitions, face: Option<&Face>) -> bool {
    let first = defs.families.get(&FontFamily::Proportional).and_then(|chain| chain.first());
    match face {
        Some(face) => first.is_some_and(|name| *name == names(face).0),
        None => defs.families.values().all(|chain| !chain.iter().any(|n| is_suite_name(n))),
    }
}

/// `defs` with the interface in `face`, or in the app's own font (`None`): the suite's faces go
/// first in the interface families, or come out of every family again.
pub fn with_suite_font(defs: &FontDefinitions, face: Option<&Face>) -> FontDefinitions {
    let mut out = defs.clone();
    for chain in out.families.values_mut() {
        chain.retain(|name| !is_suite_name(name));
    }
    out.font_data.retain(|name, _| !is_suite_name(name));
    let Some(face) = face else {
        return out;
    };
    let (regular, bold) = names(face);
    out.font_data.insert(regular.clone(), face.regular.clone());
    out.font_data.insert(bold.clone(), face.bold.clone());
    let plain = std::iter::once(FontFamily::Proportional)
        .chain(PLAIN.iter().map(|n| FontFamily::Name((*n).into())));
    for family in plain {
        if let Some(chain) = out.families.get_mut(&family) {
            chain.insert(0, regular.clone());
        }
    }
    for name in BOLD {
        if let Some(chain) = out.families.get_mut(&FontFamily::Name(name.into())) {
            chain.insert(0, bold.clone());
        }
    }
    out
}
