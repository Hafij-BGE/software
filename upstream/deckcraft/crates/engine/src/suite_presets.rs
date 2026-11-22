//! Craft Suite: connectors craft-libs draws but leaves out of its Shapes gallery
//! (`craft_drawingml::preset`'s unlisted presets), offered in Insert ▸ Shapes ▸ Lines.

use deckcraft_geom::preset::{CATALOG, Category, PresetInfo};

/// The suite's extra gallery entries, after the built-in Lines.
pub static EXTRA: &[PresetInfo] = &[
    PresetInfo { name: "bentConnector4", label: "Connector: Elbow (two bends)", category: Category::Lines, defaults: &[50000.0, 50000.0] },
    PresetInfo { name: "curvedConnector2", label: "Connector: Curved (one bend)", category: Category::Lines, defaults: &[] },
];

/// `deckcraft_geom::preset::info`, also knowing the suite's extra entries.
pub fn info(name: &str) -> Option<&'static PresetInfo> {
    deckcraft_geom::preset::info(name).or_else(|| EXTRA.iter().find(|p| p.name == name))
}

/// Every gallery preset: the built-in catalog, then the suite's extras.
pub fn catalog() -> impl Iterator<Item = &'static PresetInfo> {
    CATALOG.iter().chain(EXTRA)
}

#[cfg(test)]
mod tests {
    #[test]
    fn extras_are_known_and_drawn() {
        for p in super::EXTRA {
            assert!(super::info(p.name).is_some(), "{}", p.name);
            assert!(deckcraft_geom::preset::info(p.name).is_none(), "{} is in craft-libs' gallery now: drop it here", p.name);
        }
    }
}
