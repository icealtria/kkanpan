//! Incremental RGBA8 raster + tile-based damage refinement for the e-ink panel.
//!
//! Two damage layers compose here:
//!
//! 1. **DrawList damage** (pocketjs_core::damage::DamageTracker): compares the
//!    retained DrawList against the one whose pixels live in `rgba` and repaints
//!    only the changed regions.
//! 2. **Pixel tile diff** (16×16, scoped to damage regions): decides what the
//!    *panel* must refresh. E-ink updates are the expensive resource.
//!
//! `rgba` is the persistent render target (always the complete current frame);
//! `gray` is the luminance-converted Gray8 for FBInk;
//! `prev` mirrors what was last diffed against, advanced only inside dirty tiles.

use crate::fbink_ffi;
use pocketjs_core::damage::{DamagePlan, DamagePolicy, DamageRect, DamageTracker};
use pocketjs_core::{raster, Ui};

/// One changed tile, in render-buffer pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// Dirty-tile granularity. 16×16 matches the pocketbook backend.
const TILE: usize = 16;

pub struct FramebufferPipeline {
    /// Persistent render target, RGBA8 — always the complete current frame.
    rgba: Vec<u8>,
    /// Gray8 luminance conversion of `rgba`.
    gray: Vec<u8>,
    /// What the last diff ran against; advanced only inside dirty tiles.
    prev: Vec<u8>,
    w: usize,
    h: usize,
    /// DrawList snapshot for the pixels retained in `rgba`.
    tracker: DamageTracker,
}

impl FramebufferPipeline {
    /// `w`/`h` are the RENDER-buffer dimensions (logical × density).
    pub fn new(w: usize, h: usize) -> Self {
        let n = w * h;
        Self {
            rgba: vec![0u8; n * 4],
            gray: vec![0u8; n],
            prev: vec![0u8; n],
            w,
            h,
            tracker: DamageTracker::new(),
        }
    }

    /// Rasterize the DrawList into the retained RGBA8 buffer, repainting only
    /// DrawList damage. Returns the logical damage plan.
    pub fn rasterize(&mut self, ui: &Ui, words: &[u32]) -> DamagePlan {
        match raster::render_scaled_incremental(
            ui,
            words,
            &mut self.rgba,
            2, // density
            &mut self.tracker,
            DamagePolicy::default(),
        ) {
            Ok(plan) => plan,
            Err(_) => {
                raster::render_scaled(ui, words, &mut self.rgba, 2);
                self.tracker.invalidate();
                let logical = DamageRect::new(
                    0,
                    0,
                    (self.w / 2) as i32,
                    (self.h / 2) as i32,
                );
                DamagePlan::full(logical)
            }
        }
    }

    /// Diff the current frame against `prev` inside the plan's regions and
    /// return the changed tiles (buffer coordinates).
    pub fn diff(&self, plan: &DamagePlan) -> Vec<DirtyRect> {
        if plan.is_empty() {
            return Vec::new();
        }
        let (w, h) = (self.w, self.h);
        let tx = w.div_ceil(TILE);
        let ty = h.div_ceil(TILE);
        let mut flags = vec![false; tx * ty];

        // Convert RGBA8 damage regions to Gray8 coordinates and diff
        for region in plan.regions() {
            let x0 = (region.x0.max(0) as usize * 2).min(w);
            let y0 = (region.y0.max(0) as usize * 2).min(h);
            let x1 = (region.x1.max(0) as usize * 2).min(w);
            let y1 = (region.y1.max(0) as usize * 2).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    if self.gray[i] != self.prev[i] {
                        flags[(y / TILE) * tx + (x / TILE)] = true;
                    }
                }
            }
        }

        flags
            .iter()
            .enumerate()
            .filter(|(_, d)| **d)
            .map(|(i, _)| {
                let px = (i % tx) * TILE;
                let py = (i / tx) * TILE;
                DirtyRect {
                    x: px,
                    y: py,
                    w: TILE.min(w - px),
                    h: TILE.min(h - py),
                }
            })
            .collect()
    }

    /// Latch the blitted tiles into `prev`.
    pub fn advance(&mut self, dirty: &[DirtyRect]) {
        for r in dirty {
            for dy in 0..r.h {
                let start = (r.y + dy) * self.w + r.x;
                let end = start + r.w;
                self.prev[start..end].copy_from_slice(&self.gray[start..end]);
            }
        }
    }

    /// Latch the complete frame (after a full presentation).
    pub fn advance_full(&mut self) {
        self.prev.copy_from_slice(&self.gray);
    }

    /// Blit changed tiles to the Kindle framebuffer via FBInk.
    /// Converts RGBA8 → Gray8 inline and writes via fbink_print_raw_data.
    pub fn blit_dirty(&self, fbfd: i32, dirty: &[DirtyRect]) {
        for r in dirty {
            // Convert the tile from RGBA8 to Gray8
            let mut tile_gray = vec![0u8; r.w * r.h];
            for dy in 0..r.h {
                for dx in 0..r.w {
                    let src_x = r.x + dx;
                    let src_y = r.y + dy;
                    let src_i = (src_y * self.w + src_x) * 4;
                    let r_val = self.rgba[src_i] as u32;
                    let g_val = self.rgba[src_i + 1] as u32;
                    let b_val = self.rgba[src_i + 2] as u32;
                    // Luminance: 0.299R + 0.587G + 0.114B (ITU-R BT.601)
                    tile_gray[dy * r.w + dx] =
                        ((54 * r_val + 183 * g_val + 19 * b_val) >> 8) as u8;
                }
            }

            // Write to framebuffer
            unsafe {
                let cfg = fbink_ffi::FBInkConfig {
                    wfm_mode: fbink_ffi::WFM_DU, // fast partial update
                    ignore_alpha: true,
                    ..Default::default()
                };
                fbink_ffi::fbink_print_raw_data(
                    fbfd,
                    tile_gray.as_ptr(),
                    r.w as i32,
                    r.h as i32,
                    tile_gray.len(),
                    r.x as u16,
                    r.y as u16,
                    &cfg,
                );
            }
        }
    }

    /// Full-buffer blit (used on Show / before a full_update).
    pub fn blit_all(&self, fbfd: i32) {
        // Convert entire RGBA8 buffer to Gray8
        let n = self.w * self.h;
        let mut gray = vec![0u8; n];
        for i in 0..n {
            let src_i = i * 4;
            let r_val = self.rgba[src_i] as u32;
            let g_val = self.rgba[src_i + 1] as u32;
            let b_val = self.rgba[src_i + 2] as u32;
            gray[i] = ((54 * r_val + 183 * g_val + 19 * b_val) >> 8) as u8;
        }

        unsafe {
            let cfg = fbink_ffi::FBInkConfig {
                wfm_mode: fbink_ffi::WFM_GC16, // high-quality full update
                is_flashing: true,
                ignore_alpha: true,
                ..Default::default()
            };
            fbink_ffi::fbink_print_raw_data(
                fbfd,
                gray.as_ptr(),
                self.w as i32,
                self.h as i32,
                gray.len(),
                0,
                0,
                &cfg,
            );
        }
    }
}
