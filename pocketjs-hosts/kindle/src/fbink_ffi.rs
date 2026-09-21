//! Raw FFI bindings to libfbink.
//!
//! Only the subset of the FBInk API needed by the Kindle host is declared here.
//! For the full API, see fbink.h.

use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ushort};

// Magic constant: open the framebuffer for the duration of a single call.
pub const FBFD_AUTO: c_int = -1;

// Waveform modes (subset used by the host)
pub const WFM_AUTO: u8 = 0;
pub const WFM_DU: u8 = 1;      // Direct Update: fast, low quality (partial refresh)
pub const WFM_GC16: u8 = 2;    // Grayscale Clear 16: slow, high quality (full refresh)
pub const WFM_A2: u8 = 4;      // Animation: fastest, lowest quality (2-level)

// HW dithering
pub const HWD_PASSTHROUGH: u8 = 0;

/// Minimal FBInkConfig for raw data writes.
#[repr(C)]
pub struct FBInkConfig {
    pub row: c_ushort,
    pub col: c_ushort,
    pub fontmult: c_uchar,
    pub fontname: c_uchar,
    pub is_inverted: bool,
    pub is_flashing: bool,
    pub is_cleared: bool,
    pub is_centered: bool,
    pub hoffset: c_ushort,
    pub voffset: c_ushort,
    pub is_halfway: bool,
    pub is_padded: bool,
    pub is_rpadded: bool,
    pub fg_color: c_uchar,
    pub bg_color: c_uchar,
    pub is_overlay: bool,
    pub is_bgless: bool,
    pub is_fgless: bool,
    pub no_viewport: bool,
    pub is_verbose: bool,
    pub is_quiet: bool,
    pub ignore_alpha: bool,
    pub halign: c_uchar,
    pub valign: c_uchar,
    pub scaled_width: c_ushort,
    pub scaled_height: c_ushort,
    pub wfm_mode: c_uchar,
    pub dithering_mode: c_uchar,
    pub sw_dithering: bool,
    pub is_nightmode: bool,
    pub no_refresh: bool,
    pub to_syslog: bool,
}

impl Default for FBInkConfig {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

extern "C" {
    pub fn fbink_open() -> c_int;
    pub fn fbink_close(fbfd: c_int) -> c_int;
    pub fn fbink_init(fbfd: c_int, cfg: *const FBInkConfig) -> c_int;
    pub fn fbink_print_raw_data(
        fbfd: c_int,
        data: *const c_uchar,
        w: c_int,
        h: c_int,
        len: usize,
        x_off: c_ushort,
        y_off: c_ushort,
        cfg: *const FBInkConfig,
    ) -> c_int;
    pub fn fbink_refresh(
        fbfd: c_int,
        region_top: c_uint,
        region_left: c_uint,
        region_width: c_uint,
        region_height: c_uint,
        cfg: *const FBInkConfig,
    ) -> c_int;
    pub fn fbink_cls(
        fbfd: c_int,
        cfg: *const FBInkConfig,
        rect: *const std::ffi::c_void, // FBInkRect* — we don't need the full type
    ) -> c_int;
    pub fn fbink_version() -> *const c_char;
}
