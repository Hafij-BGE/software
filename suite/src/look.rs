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

use egui::FontDefinitions;

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

    /// First half of putting this look back: its fonts (with every other family in `union`),
    /// in from the next pass. Identical fonts cost a comparison.
    pub fn restore_fonts(&self, ctx: &egui::Context, union: &FontDefinitions) {
        ctx.set_fonts(merged(&self.fonts, union));
    }

    /// Second half, once the fonts are in: the options (visuals, spacing, theme, zoom).
    pub fn restore_options(&self, ctx: &egui::Context) {
        ctx.options_mut(|o| {
            // The zoom factor goes through `set_zoom_factor`, which also updates the scale.
            let zoom = o.zoom_factor;
            *o = self.options.clone();
            o.zoom_factor = zoom;
        });
        ctx.set_zoom_factor(self.options.zoom_factor);
    }

    /// The suite's own look (Home and Settings pages), with the theme from the suite settings.
    pub fn with_theme(&self, theme: egui::ThemePreference) -> Look {
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
