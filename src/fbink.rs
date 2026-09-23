// 手写 FFI，不用 bindgen（省掉 libclang 交叉编译）
#[cfg(target_os = "linux")]
use std::os::raw::{c_char, c_int};

pub const WFM_DU: u8 = 1; // 快(~260ms)无闪，黑白，残影累积：小局部用
pub const WFM_GC16: u8 = 2; // 全闪(~600ms)最干净：切视图/去鬼影用
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

    // Kindle 上 Y 灰度 + ignore_alpha 最快
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


#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

// 碎片合并：数据刷新多为“同列散点”（各卡价格/波形同 x、不同 y），按 x 重叠扫成一列一个
// 高条（如价格列、波形列、状态栏），把“29 rects / 7%”收成 2~3 个 DU 局部刷新
fn merge_rects(rects: Vec<DirtyRect>, limit: usize) -> Vec<DirtyRect> {
    let n = rects.len();
    if n <= 8 {
        return rects;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (rects[i].x, rects[i].y));
    let mut groups: Vec<DirtyRect> = Vec::new();
    for &i in &order {
        let r = rects[i];
        match groups
            .iter_mut()
            .find(|g| r.x <= g.x + g.w && g.x <= r.x + r.w)
        {
            Some(g) => {
                let (x0, y0) = (g.x.min(r.x), g.y.min(r.y));
                let (x1, y1) = ((g.x + g.w).max(r.x + r.w), (g.y + g.h).max(r.y + r.h));
                g.x = x0;
                g.y = y0;
                g.w = x1 - x0;
                g.h = y1 - y0;
            }
            None => groups.push(r),
        }
    }
    // 没收敛（组数不比原来少，或仍超限）就原样返回，调用方按老规则整屏 GL16
    if groups.len() < n && groups.len() <= limit {
        groups
    } else {
        rects
    }
}

#[cfg(test)]
mod tests {
    use super::merge_rects;
    use super::DirtyRect;

    #[test]
    fn merges_stacked_neighbors() {
        let v: Vec<DirtyRect> = (0..12)
            .map(|i| DirtyRect { x: 0, y: i * 24, w: 24, h: 24 })
            .collect();
        assert_eq!(merge_rects(v, 15).len(), 1);
    }

    #[test]
    fn keeps_far_apart() {
        let v: Vec<DirtyRect> = (0..9)
            .map(|i| DirtyRect { x: i * 200, y: 0, w: 24, h: 24 })
            .collect();
        assert_eq!(merge_rects(v, 15).len(), 9);
    }

    #[test]
    fn merges_price_column() {
        // 数据刷新典型：同 x 列、y 散开的价格小块 → 收成 1 个高条
        let v: Vec<DirtyRect> = (0..10)
            .map(|i| DirtyRect { x: 840, y: 200 + i * 103, w: 192, h: 48 })
            .collect();
        let m = merge_rects(v, 15);
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].x, m[0].w), (840, 192));
    }
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
        let mut spans: Vec<(i32, i32, i32)> = vec![];
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
        merge_rects(out, 15)
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
