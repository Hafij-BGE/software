//! The Insert galleries the craft apps share.
//!
//! - [`icons`]: 5,500+ icons — Tabler Icons outline (MIT) and Health Icons (CC0, the medical and
//!   biology set) — searchable by name, tag and category, drawn as vectors or rendered to PNG.
//! - With the `ui` feature, [`picker`] is an egui icon picker and [`stock`] the Stock Images
//!   dialog: icons plus free pictures from Openverse (Creative Commons and public domain).

pub mod icons;

#[cfg(feature = "ui")]
pub mod online;
#[cfg(feature = "ui")]
pub mod picker;
#[cfg(feature = "ui")]
pub mod stock;

pub use icons::{Icon, Kind};

/// Credits for the bundled artwork, for an About box or NOTICE file.
pub const CREDITS: &str = "Icons: Tabler Icons (MIT, tabler.io/icons) and Health Icons (CC0, healthicons.org).";
