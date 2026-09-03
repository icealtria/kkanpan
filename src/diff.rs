//! 脏矩形检测纯函数 (原 diff.go FindDirtyRects/mergeBlocks).
//! 与图片类型解耦: 输入 serve 灰度字节 + 宽高 stride, 输出矩形. 单测无需 image crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub const BLOCK: u32 = 8;

/// 旧帧 None = 全屏. 尺寸不一致 = 全屏.
pub fn find_dirty(
    old: Option<(&[u8], u32, u32)>,
    new_pix: &[u8],
    w: u32,
    h: u32,
) -> Vec<DirtyRect> {
    let Some((old_pix, ow, oh)) = old else {
        return vec![DirtyRect { x: 0, y: 0, w, h }];
    };
    if ow != w || oh != h {
        return vec![DirtyRect { x: 0, y: 0, w, h }];
    }
    let bs = BLOCK;
    let cols = (w + bs - 1) / bs;
    let rows = (h + bs - 1) / bs;
    let mut dirty = vec![false; (cols * rows) as usize];
    let mut any = false;
    for by in 0..rows {
        for bx in 0..cols {
            if block_dirty(old_pix, new_pix, w, h, bx * bs, by * bs, bs) {
                dirty[(by * cols + bx) as usize] = true;
                any = true;
            }
        }
    }
    if !any {
        return vec![];
    }
    merge(dirty, cols, rows, bs, w, h)
}

fn block_dirty(old: &[u8], new: &[u8], w: u32, h: u32, x0: u32, y0: u32, bs: u32) -> bool {
    for y in y0..(y0 + bs).min(h) {
        let base = (y * w) as usize;
        let a = base + x0 as usize;
        let b = base + ((x0 + bs).min(w)) as usize;
        if old[a..b] != new[a..b] {
            return true;
        }
    }
    false
}

fn merge(dirty: Vec<bool>, cols: u32, rows: u32, bs: u32, w: u32, h: u32) -> Vec<DirtyRect> {
    // 行内 span + 纵向同宽合并 (与 Go mergeBlocks 同策略)
    let mut spans: Vec<(u32, u32, u32)> = vec![];
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
    let mut rects = vec![];
    for (i, &(x0, x1, y)) in spans.iter().enumerate() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let y0 = y;
        let mut y1 = y + 1;
        for (j, &s) in spans.iter().enumerate().skip(i + 1) {
            if used[j] {
                continue;
            }
            if s.2 == y1 && s.0 == x0 && s.1 == x1 {
                y1 = s.2 + 1;
                used[j] = true;
            }
        }
        let (px, py) = (x0 * bs, y0 * bs);
        let (mut pw, mut ph) = ((x1 - x0) * bs, (y1 - y0) * bs);
        if px + pw > w {
            pw = w - px;
        }
        if py + ph > h {
            ph = h - py;
        }
        rects.push(DirtyRect { x: px, y: py, w: pw, h: ph });
    }
    rects
}

/// 脏占比超过阈值或矩形太多 -> 全屏更快 (fork+exec 开销, 原 60%/5 矩形).
pub fn should_full(rects: &[DirtyRect], w: u32, h: u32) -> bool {
    if rects.len() > 5 {
        return true;
    }
    let total = w as u64 * h as u64;
    let dirty: u64 = rects.iter().map(|r| r.w as u64 * r.h as u64).sum();
    dirty * 100 > total * 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_clean() {
        let px = vec![255u8; 16 * 16];
        assert!(find_dirty(Some((&px, 16, 16)), &px, 16, 16).is_empty());
    }

    #[test]
    fn one_pixel_dirties_one_block() {
        let old = vec![255u8; 16 * 16];
        let mut new = old.clone();
        new[0] = 0;
        let r = find_dirty(Some((&old, 16, 16)), &new, 16, 16);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], DirtyRect { x: 0, y: 0, w: 8, h: 8 });
    }

    #[test]
    fn size_mismatch_full() {
        let old = vec![0u8; 4];
        let new = vec![0u8; 9];
        assert_eq!(find_dirty(Some((&old, 2, 2)), &new, 3, 3).len(), 1);
    }
}
