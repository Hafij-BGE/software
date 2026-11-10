//! The suite's logo (Settings › Window › Logo): the window icon, the Home and Settings tab
//! icons, and on Linux the icon of the desktop's launchers. Classic is the icon the suite
//! installs with; the others live in `assets/logos` as SVG sources and 256 px renders.

use image::imageops::FilterType;

use crate::settings::Logo;

impl Logo {
    /// The logo at 256 px, as PNG.
    pub fn png(self) -> &'static [u8] {
        match self {
            Logo::Classic => include_bytes!("../../assets/hicolor/256x256/apps/craft-suite.png"),
            Logo::Tiles => include_bytes!("../../assets/logos/tiles-256.png"),
            Logo::Ring => include_bytes!("../../assets/logos/ring-256.png"),
            Logo::Sheets => include_bytes!("../../assets/logos/sheets-256.png"),
            Logo::Stack => include_bytes!("../../assets/logos/stack-256.png"),
            Logo::Page => include_bytes!("../../assets/logos/page-256.png"),
        }
    }

    /// The logo as SVG, for the desktop's scalable icon.
    pub fn svg(self) -> &'static str {
        match self {
            Logo::Classic => include_str!("../../assets/hicolor/scalable/apps/craft-suite.svg"),
            Logo::Tiles => include_str!("../../assets/logos/tiles.svg"),
            Logo::Ring => include_str!("../../assets/logos/ring.svg"),
            Logo::Sheets => include_str!("../../assets/logos/sheets.svg"),
            Logo::Stack => include_str!("../../assets/logos/stack.svg"),
            Logo::Page => include_str!("../../assets/logos/page.svg"),
        }
    }

    /// The logo as RGBA pixels at `size` × `size`.
    pub fn rgba(self, size: u32) -> Option<image::RgbaImage> {
        let img = image::load_from_memory_with_format(self.png(), image::ImageFormat::Png).ok()?.to_rgba8();
        Some(if img.width() == size { img } else { image::imageops::resize(&img, size, size, FilterType::Lanczos3) })
    }

    pub fn icon_data(self) -> egui::IconData {
        match self.rgba(128) {
            Some(img) => egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() },
            None => egui::IconData::default(),
        }
    }

    /// A texture of the logo at `size` px (drawn smaller than 256 px, a plain texture would alias).
    pub fn texture(self, ctx: &egui::Context, name: &str, size: u32) -> Option<egui::TextureHandle> {
        let img = self.rgba(size)?;
        let image = egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], img.as_raw());
        Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
    }
}

/// The sizes the desktop looks up an app icon at.
#[cfg(all(unix, not(target_os = "macos")))]
const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// Shows `logo` in the desktop's app grid, dock and menus: puts it in the user's own icon folder
/// (`~/.local/share/icons/hicolor`), which comes before the installed icon. Classic takes the
/// user's copies away again, so the installed icon shows.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn set_desktop_icon(logo: Logo) {
    let Some(base) = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
    else {
        return;
    };
    let theme = base.join("icons/hicolor");
    let scalable = theme.join("scalable/apps/craft-suite.svg");
    let png = |size: u32| theme.join(format!("{size}x{size}/apps/craft-suite.png"));
    if logo == Logo::Classic {
        let _ = std::fs::remove_file(&scalable);
        for size in SIZES {
            let _ = std::fs::remove_file(png(size));
        }
    } else {
        for size in SIZES {
            let path = png(size);
            if let (Some(img), Some(dir)) = (logo.rgba(size), path.parent()) {
                let _ = std::fs::create_dir_all(dir);
                let _ = img.save_with_format(&path, image::ImageFormat::Png);
            }
        }
        if let Some(dir) = scalable.parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(&scalable, logo.svg());
        }
    }
    // Icon themes notice new icons by the folder's modification time.
    let _ = std::fs::File::open(&theme).and_then(|f| f.set_modified(std::time::SystemTime::now()));
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn set_desktop_icon(_logo: Logo) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_logo_decodes() {
        for logo in Logo::ALL {
            let size = logo.rgba(64).map(|img| (img.width(), img.height()));
            assert_eq!(size, Some((64, 64)), "{logo:?} doesn't decode");
            assert!(logo.svg().starts_with("<svg"), "{logo:?}");
        }
    }
}
