//! E-ink panel update policy for Kindle.
//!
//! Ported from the battle-tested strategy in hosts/pocketbook/src/refresh.rs
//! (originally from inkview-slint). Adapted for FBInk's waveform modes:
//!
//! - DU (Direct Update): fast, low quality — used for partial/animated updates
//! - GC16 (Grayscale Clear 16): slow, high quality — used for full/clean updates
//! - A2: fastest, 2-level only — used for very fast animations (not used here)
//!
//! Strategy:
//! - Gate on whether a panel update is in flight (tracked by time)
//! - DU partial update on the damage box when idle
//! - Throttled DU updates (≥20ms apart) while an update may be in flight
//! - After ~200ms of quiet, do a final GC16 partial to clear ghosting

use std::time::{Duration, Instant};

use crate::fbink_ffi;
use crate::framebuffer::DirtyRect;

const DYNAMIC_MIN_INTERVAL: Duration = Duration::from_millis(20);
const CLEANUP_QUIET: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

pub struct Refresh {
    last_draw: Instant,
    /// Damage accumulated while a panel update may be in flight; needs a cleanup
    /// partial update once things go quiet.
    pending_cleanup: Option<Rect>,
    cleanup_after: Option<Instant>,
}

impl Refresh {
    pub fn new() -> Self {
        Self {
            last_draw: Instant::now(),
            pending_cleanup: None,
            cleanup_after: None,
        }
    }

    /// Drive the panel for this frame. `dirty` is in render-buffer coordinates.
    pub fn present(&mut self, fbfd: i32, dirty: &[DirtyRect]) {
        // Quiet-period cleanup: a final high-quality partial update on the
        // region we hammered with dynamic updates (clears ghosting).
        if let Some(at) = self.cleanup_after {
            if Instant::now() >= at {
                if let Some(r) = self.pending_cleanup.take() {
                    self.fbink_refresh(fbfd, r, fbink_ffi::WFM_GC16);
                    self.last_draw = Instant::now();
                }
                self.cleanup_after = None;
            }
        }

        if dirty.is_empty() {
            return;
        }
        let d = merge(dirty);

        if self.last_draw.elapsed() < DYNAMIC_MIN_INTERVAL {
            // An update may still be in flight. Queue a fast DU update,
            // throttled to ≥20ms, on the accumulated damage.
            self.pending_cleanup = Some(union(self.pending_cleanup, d));
        } else {
            // Idle panel: fast DU partial on the damage box.
            self.fbink_refresh(fbfd, d, fbink_ffi::WFM_DU);
            self.last_draw = Instant::now();
        }

        // Schedule a cleanup GC16 partial update 200ms after the last draw.
        self.cleanup_after = Some(Instant::now() + CLEANUP_QUIET);
    }

    /// Full flashing redraw — call on Show / orientation change / mode switch.
    pub fn full(&mut self, fbfd: i32) {
        let full_rect = Rect {
            x: 0,
            y: 0,
            w: 1072, // KPW3 physical width
            h: 1448, // KPW3 physical height
        };
        self.fbink_refresh(fbfd, full_rect, fbink_ffi::WFM_GC16);
        self.last_draw = Instant::now();
        self.pending_cleanup = None;
        self.cleanup_after = None;
    }

    fn fbink_refresh(&self, fbfd: i32, r: Rect, wfm: u8) {
        unsafe {
            let cfg = fbink_ffi::FBInkConfig {
                wfm_mode: wfm,
                ..Default::default()
            };
            fbink_ffi::fbink_refresh(
                fbfd,
                r.y as u32,
                r.x as u32,
                r.w,
                r.h,
                &cfg,
            );
        }
    }
}

impl Default for Refresh {
    fn default() -> Self {
        Self::new()
    }
}

fn merge(rects: &[DirtyRect]) -> Rect {
    let (mut x0, mut y0) = (i32::MAX, i32::MAX);
    let (mut x1, mut y1) = (0i32, 0i32);
    for r in rects {
        x0 = x0.min(r.x as i32);
        y0 = y0.min(r.y as i32);
        x1 = x1.max((r.x + r.w) as i32);
        y1 = y1.max((r.y + r.h) as i32);
    }
    Rect {
        x: x0,
        y: y0,
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    }
}

fn union(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        None => b,
        Some(a) => {
            let x0 = a.x.min(b.x);
            let y0 = a.y.min(b.y);
            let x1 = (a.x + a.w as i32).max(b.x + b.w as i32);
            let y1 = (a.y + a.h as i32).max(b.y + b.h as i32);
            Rect {
                x: x0,
                y: y0,
                w: (x1 - x0) as u32,
                h: (y1 - y0) as u32,
            }
        }
    }
}
