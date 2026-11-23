//! A screenshot of the window for checking the layout without a desktop: with
//! `CRAFT_SUITE_SCREENSHOT=<file.png>` the suite saves its window there once it has drawn
//! for a while (`CRAFT_SUITE_SCREENSHOT_FRAMES`, 90 by default) and quits.

use std::sync::atomic::{AtomicU32, Ordering};

static FRAMES: AtomicU32 = AtomicU32::new(0);

/// Before each frame: asks for the screenshot at the set frame; it is saved when it arrives.
pub fn take(ctx: &egui::Context, _raw: &egui::RawInput) {
    let Some(path) = std::env::var_os("CRAFT_SUITE_SCREENSHOT") else { return };
    let wait = std::env::var("CRAFT_SUITE_SCREENSHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(90);
    let n = FRAMES.fetch_add(1, Ordering::Relaxed);
    if n == wait {
        let path = std::path::PathBuf::from(path);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            match image::RgbaImage::from_raw(w as u32, h as u32, bytes).map(|img| img.save(&path)) {
                Some(Ok(())) => log::warn!("screenshot saved to {}", path.display()),
                _ => log::warn!("screenshot could not be saved"),
            }
            std::process::exit(0);
        })));
    }
    ctx.request_repaint();
}
