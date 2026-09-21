// FBInk 显示层：手写 FFI（字段顺序/类型逐项对齐 vendor-fbink/gofbink/fbink.h，
// 不用 bindgen，省掉 libclang 交叉编译），+ 脏矩形差分（对齐 diff.go）。
#[cfg(target_os = "linux")]
use std::os::raw::{c_char, c_int};

pub const WFM_DU: u8 = 1; // 快(~260ms)无闪，黑白，残影累积：小局部用
pub const WFM_GC16: u8 = 2; // 全闪(~600ms)最干净：切视图/去鬼影用
pub const WFM_GC4: u8 = 3; // 4 灰阶，轻闪，居中
pub const WFM_GL16: u8 = 5; // 16 灰阶低闪：整页翻页用，比 DU 干净得多，比 GC16 快

// 零初始化即合法（"Perfectly sane when fully zero-initialized"）
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FBInkConfig {
    pub row: i16,
    pub col: i16,
    pub fontmult: u8,
    pub fontname: u8,
    pub is_inverted: bool,
    pub is_flashing: bool,
    pub is_cleared: bool,
    pub is_centered: bool,
    pub hoffset: i16,
    pub voffset: i16,
    pub is_halfway: bool,
    pub is_padded: bool,
    pub is_rpadded: bool,
    pub fg_color: u8,
    pub bg_color: u8,
    pub is_overlay: bool,
    pub is_bgless: bool,
    pub is_fgless: bool,
    pub no_viewport: bool,
    pub is_verbose: bool,
    pub is_quiet: bool,
    pub ignore_alpha: bool,
    pub halign: u8,
    pub valign: u8,
    pub scaled_width: i16,
    pub scaled_height: i16,
    pub wfm_mode: u8,
    pub dithering_mode: u8,
    pub sw_dithering: bool,
    pub is_nightmode: bool,
    pub no_refresh: bool,
    pub to_syslog: bool,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FBInkRect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
}

#[cfg(target_os = "linux")]
#[link(name = "fbink", kind = "static")]
extern "C" {
    fn fbink_open() -> c_int;
    fn fbink_close(fbfd: c_int) -> c_int;
    fn fbink_init(fbfd: c_int, cfg: *const FBInkConfig) -> c_int;
    fn fbink_version() -> *const c_char;
    fn fbink_print_raw_data(
        fbfd: c_int,
        data: *const u8,
        w: c_int,
        h: c_int,
        len: usize,
        x_off: i16,
        y_off: i16,
        cfg: *const FBInkConfig,
    ) -> c_int;
    fn fbink_cls(fbfd: c_int, cfg: *const FBInkConfig, rect: *const FBInkRect) -> c_int;
}

pub struct Display {
    #[cfg(target_os = "linux")]
    fbfd: c_int,
}

impl Display {
    pub fn open() -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: FBInk C API，参数均为值/只读指针
            unsafe {
                let fbfd = fbink_open();
                let cfg = FBInkConfig::default(); // WFM_AUTO
                if fbink_init(fbfd, &cfg) != 0 {
                    return Err("fbink_init failed".to_string());
                }
                let v = std::ffi::CStr::from_ptr(fbink_version()).to_string_lossy().into_owned();
                eprintln!("[display] FBInk {v} initialized");
                Ok(Display { fbfd })
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            eprintln!("[display] Dev mode: no FBInk, preview via --http /screen.png");
            Ok(Display {})
        }
    }

    // data: Y 灰度行优先无 padding（Kindle 上 ignore_alpha 最快，对齐 fbink.h 注释）
    pub fn write_gray(&self, data: &[u8], w: i32, h: i32, full: bool) -> Result<(), String> {
        self.write_gray_mode(data, w, h, if full { WFM_GC16 } else { WFM_DU }, full)
    }

    pub fn write_gray_mode(&self, data: &[u8], w: i32, h: i32, wfm: u8, flash: bool) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let cfg = FBInkConfig { wfm_mode: wfm, is_flashing: flash, ..Default::default() };
            // SAFETY: data 长度由调用方保证为 w*h
            let rc = unsafe {
                fbink_print_raw_data(self.fbfd, data.as_ptr(), w, h, data.len(), 0, 0, &cfg)
            };
            if rc < 0 { Err(format!("print_raw_data rc={rc}")) } else { Ok(()) }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (data, w, h, wfm, flash);
            Ok(())
        }
    }

    pub fn write_gray_partial(&self, data: &[u8], stride: i32, rects: &[DirtyRect]) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let cfg = FBInkConfig { wfm_mode: WFM_DU, ..Default::default() };
            for r in rects {
                let mut crop = vec![0u8; (r.w * r.h) as usize];
                for y in 0..r.h {
                    let src = ((r.y + y) * stride + r.x) as usize;
                    let dst = (y * r.w) as usize;
                    crop[dst..dst + r.w as usize].copy_from_slice(&data[src..src + r.w as usize]);
                }
                // SAFETY: crop 长度精确为 w*h
                let rc = unsafe {
                    fbink_print_raw_data(
                        self.fbfd, crop.as_ptr(), r.w, r.h, crop.len(), r.x as i16, r.y as i16, &cfg,
                    )
                };
                if rc < 0 {
                    return Err(format!("partial print rc={rc}"));
                }
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (data, stride, rects);
            Ok(())
        }
    }

    // 退出清屏：FBFD_AUTO（-1）单次调用，无需已打开的 fd
    pub fn clear_auto() {
        #[cfg(target_os = "linux")]
        {
            let cfg = FBInkConfig::default();
            unsafe {
                fbink_cls(-1, &cfg, std::ptr::null());
            }
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            fbink_close(self.fbfd);
        }
    }
}

// ---- 脏矩形差分（对齐 diff.go：8px 分块 + 行段合并） ----

#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct ScreenDiffer {
    prev: Option<Vec<u8>>,
    w: i32,
    h: i32,
    block: i32,
}

impl ScreenDiffer {
    pub fn new() -> Self {
        // 块 24px：碎片 rect 少一个量级；碎了就整屏 DU，不逐个 ioctl
        ScreenDiffer { prev: None, w: 0, h: 0, block: 24 }
    }

    fn find_dirty(&self, old: &[u8], new: &[u8], w: i32, h: i32) -> Vec<DirtyRect> {
        let (bs, cols, rows) = (self.block, (w + self.block - 1) / self.block, (h + self.block - 1) / self.block);
        let mut dirty = vec![false; (cols * rows) as usize];
        let mut any = false;
        for by in 0..rows {
            for bx in 0..cols {
                let (x0, y0) = (bx * bs, by * bs);
                let mut hit = false;
                'blk: for y in y0..(y0 + bs).min(h) {
                    let a = (y * w + x0) as usize;
                    let b = (y * w + (x0 + bs).min(w)) as usize;
                    if old[a..b] != new[a..b] {
                        hit = true;
                        break 'blk;
                    }
                }
                if hit {
                    dirty[(by * cols + bx) as usize] = true;
                    any = true;
                }
            }
        }
        if !any {
            return vec![];
        }
        // 行段纵向合并
        let mut spans: Vec<(i32, i32, i32)> = vec![]; // (bx0, bx1, by)
        for by in 0..rows {
            let mut bx = 0;
            while bx < cols {
                if !dirty[(by * cols + bx) as usize] {
                    bx += 1;
                    continue;
                }
                let s = bx;
                while bx < cols && dirty[(by * cols + bx) as usize] {
                    bx += 1;
                }
                spans.push((s, bx, by));
            }
        }
        let mut used = vec![false; spans.len()];
        let mut out = vec![];
        for (i, &(x0, x1, y0)) in spans.iter().enumerate() {
            if used[i] {
                continue;
            }
            let (mut y1, _) = (y0 + 1, used[i] = true);
            for (j, &s) in spans.iter().enumerate().skip(i + 1) {
                if !used[j] && s.2 == y1 && s.0 == x0 && s.1 == x1 {
                    y1 += 1;
                    used[j] = true;
                }
            }
            out.push(DirtyRect {
                x: x0 * bs,
                y: y0 * bs,
                w: ((x1 - x0) * bs).min(w - x0 * bs),
                h: ((y1 - y0) * bs).min(h - y0 * bs),
            });
        }
        out
    }

    pub fn update(&mut self, disp: &Display, gray: &[u8], w: i32, h: i32, full: bool) -> Result<(), String> {
        if full || self.prev.is_none() {
            disp.write_gray(gray, w, h, true)?;
            self.prev = Some(gray.to_vec());
            self.w = w;
            self.h = h;
            return Ok(());
        }
        let old = self.prev.take().unwrap();
        if self.w != w || self.h != h {
            disp.write_gray(gray, w, h, false)?;
            self.prev = Some(gray.to_vec());
            self.w = w;
            self.h = h;
            return Ok(());
        }
        let rects = self.find_dirty(&old, gray, w, h);
        if rects.is_empty() {
            crate::dlog!("[diff] No changes, skip update");
            self.prev = Some(old);
            return Ok(());
        }
        let total = (w * h) as f64;
        let dirty: f64 = rects.iter().map(|r| (r.w * r.h) as f64).sum();
        crate::dlog!("[diff] {} rects, {:.1}% changed", rects.len(), dirty / total * 100.0);
        // 碎片多时逐个 ioctl 发几百轮 e-ink 刷新，不如一次整屏 GL16（低闪，比 DU 干净得多）
        if dirty / total > 0.40 || rects.len() > 15 {
            disp.write_gray_mode(gray, w, h, WFM_GL16, false)?;
        } else if disp.write_gray_partial(gray, w, &rects).is_err() {
            disp.write_gray(gray, w, h, false)?;
        }
        self.prev = Some(gray.to_vec());
        self.w = w;
        self.h = h;
        Ok(())
    }
}
