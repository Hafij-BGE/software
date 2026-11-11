//! Touchpad pinch-to-zoom under X11 (the suite runs on XWayland by default).
//!
//! winit reports pinch gestures only on macOS and iOS, so on Linux a two-finger pinch never
//! reached the apps. XInput 2.4 delivers it as gesture events: a thread with its own X
//! connection selects them on the suite's window and turns each update into the zoom factor
//! egui expects (`egui::Event::Zoom`), which the Word and Slide canvases already read
//! through `zoom_delta()`, as they do Ctrl+scroll.
//!
//! The suite on native Wayland (`CRAFT_SUITE_WAYLAND=1`) has no X window, so this stays off
//! there.

use std::sync::mpsc::{Receiver, Sender, channel};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{self, ConnectionExt as _};

/// XI 2.4 gesture event bits; x11rb's `XIEventMask` constants stop at `BARRIER_LEAVE`.
const PINCH_MASK: u32 = (1 << xinput::GESTURE_PINCH_BEGIN_EVENT) | (1 << xinput::GESTURE_PINCH_UPDATE_EVENT) | (1 << xinput::GESTURE_PINCH_END_EVENT);
/// `XIAllMasterDevices`.
const ALL_MASTER_DEVICES: u16 = 1;

/// Zoom factors from the pinch thread, waiting for the next frame's input.
pub struct Pinch {
    rx: Receiver<f32>,
}

impl Pinch {
    /// Starts listening on X11 `window`; repaints `ctx` when a pinch moves.
    pub fn start(window: u32, ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new().name("craft-suite-pinch".into()).spawn(move || {
            if let Err(e) = listen(window, &tx, &ctx) {
                log::info!("touchpad pinch zoom unavailable: {e}");
            }
        });
        if let Err(e) = spawned {
            log::info!("touchpad pinch zoom unavailable: {e}");
        }
        Self { rx }
    }

    /// Adds this frame's pinch steps to `raw` as zoom events.
    pub fn feed(&self, raw: &mut egui::RawInput) {
        while let Ok(factor) = self.rx.try_recv() {
            raw.events.push(egui::Event::Zoom(factor));
        }
    }
}

fn listen(window: u32, tx: &Sender<f32>, ctx: &egui::Context) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _) = x11rb::connect(None)?;
    let v = conn.xinput_xi_query_version(2, 4)?.reply()?;
    if (v.major_version, v.minor_version) < (2, 4) {
        return Err(format!("the X server has XInput {}.{}, gestures need 2.4", v.major_version, v.minor_version).into());
    }
    let mask = xinput::EventMask { deviceid: ALL_MASTER_DEVICES, mask: vec![xinput::XIEventMask::from(PINCH_MASK)] };
    conn.xinput_xi_select_events(window, &[mask])?.check()?;
    // The gesture's scale is cumulative from its start; egui wants the step since last frame.
    let mut last = 1.0f32;
    loop {
        match conn.wait_for_event()? {
            Event::XinputGesturePinchBegin(_) => last = 1.0,
            Event::XinputGesturePinchUpdate(e) => {
                if let Some(step) = step(&mut last, e.scale) {
                    if tx.send(step).is_err() {
                        return Ok(());
                    }
                    ctx.request_repaint();
                }
            }
            _ => {}
        }
    }
}

/// The zoom since the previous update, from a cumulative 16.16 fixed-point `scale`.
fn step(last: &mut f32, scale: xinput::Fp1616) -> Option<f32> {
    let scale = scale as f32 / 65536.0;
    if !(scale.is_finite() && scale > 0.0 && *last > 0.0) {
        return None;
    }
    let factor = scale / *last;
    *last = scale;
    ((factor - 1.0).abs() > f32::EPSILON).then_some(factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_scale_becomes_per_update_steps() {
        let mut last = 1.0;
        let fp = |s: f32| (s * 65536.0) as i32;
        assert!((step(&mut last, fp(1.5)).unwrap() - 1.5).abs() < 1e-4);
        assert!((step(&mut last, fp(3.0)).unwrap() - 2.0).abs() < 1e-4);
        assert_eq!(step(&mut last, fp(3.0)), None);
        assert!((step(&mut last, fp(1.5)).unwrap() - 0.5).abs() < 1e-4);
        assert_eq!(step(&mut last, 0), None);
        assert_eq!(PINCH_MASK, 0b111 << 27);
    }
}
