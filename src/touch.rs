//! 触控纯逻辑 (原 touch.go).
//! 修复 Go 版两处 bug:
//! 1. tab 宽度: touch 用 total/count, render 用 (total-(n-1)*gap)/n -> 点按错位.
//!    Rust 由 tab_layout 统一, render 与 touch 共用.
//! 2. input_event 只按 32 位 16B 解析, 64 位内核是 24B -> 电源键/触控全错.
//!    Rust 按读取长度自适应 (16B=>32位, 24B=>64位).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapAction {
    Close,
    Style,
    Tab(usize),
    PrevPage,
    NextPage,
    Refresh,
    None,
}

pub const TAB_Y0: i32 = 60;
pub const TAB_Y1: i32 = 135;
pub const TOP_Y: i32 = 65;

/// 与 render 共用的 tab 几何: tx = 30 + i*(tabW+gap)
pub fn tab_layout(width: i32, n: usize) -> Vec<(i32, i32)> {
    if n == 0 {
        return vec![];
    }
    let gap = 10;
    let total = width - 60;
    let w = (total - (n as i32 - 1) * gap) / n as i32;
    (0..n).map(|i| (30 + i as i32 * (w + gap), 30 + i as i32 * (w + gap) + w)).collect()
}

pub fn hit_test(x: i32, y: i32, width: i32, height: i32, tabs: usize) -> TapAction {
    if y <= TOP_Y {
        if x >= width - 95 && x <= width - 10 {
            return TapAction::Close;
        }
        if x >= width - 185 && x <= width - 105 {
            return TapAction::Style;
        }
    }
    if (TAB_Y0..=TAB_Y1).contains(&y) {
        for (i, (x0, x1)) in tab_layout(width, tabs).iter().enumerate() {
            if x >= *x0 && x <= *x1 {
                return TapAction::Tab(i);
            }
        }
        return TapAction::None;
    }
    if y >= height - 100 {
        if x < width / 3 {
            return TapAction::PrevPage;
        }
        if x > width * 2 / 3 {
            return TapAction::NextPage;
        }
        return TapAction::Refresh;
    }
    TapAction::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swipe {
    Up,   // 下一页
    Down, // 上一页
}

/// 纯手势分类 (原 handleSwipe). dur_ms 为按住时长.
pub fn classify_swipe(sx: i32, sy: i32, ex: i32, ey: i32, dur_ms: i64) -> Option<Swipe> {
    if dur_ms > 700 || dur_ms < 80 {
        return None;
    }
    let (dx, dy) = (ex - sx, ey - sy);
    let (adx, ady) = (dx.abs(), dy.abs());
    if adx < 12 && ady < 12 {
        return None;
    }
    let dist = ((dx * dx + dy * dy) as f64).sqrt();
    if dist / dur_ms.max(1) as f64 <= 0.4 {
        return None;
    }
    // 仅垂直翻页 (水平手势易误触, 已移除)
    if ady > 120 && ady > adx * 2 {
        return Some(if dy < 0 { Swipe::Up } else { Swipe::Down });
    }
    None
}

/// input_event 解析: 16B(32位) 与 24B(64位) 自适应. 返回 (type, code, value).
pub fn parse_event(buf: &[u8]) -> Option<(u16, u16, i32)> {
    if buf.len() == 16 {
        let t = u16::from_le_bytes([buf[8], buf[9]]);
        let c = u16::from_le_bytes([buf[10], buf[11]]);
        let v = i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        Some((t, c, v))
    } else if buf.len() >= 24 {
        let t = u16::from_le_bytes([buf[16], buf[17]]);
        let c = u16::from_le_bytes([buf[18], buf[19]]);
        let v = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
        Some((t, c, v))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_hit_inside_gap_is_none() {
        // width=1072, 4 tabs: w=(1012-30)/4=245; tab0=[30,275], gap=(275,285), tab1=[285,530]
        let a = hit_test(280, 100, 1072, 1448, 4);
        assert_eq!(a, TapAction::None); // 落在 gap 里, Go 旧算法会误判为 tab1
        assert_eq!(hit_test(100, 100, 1072, 1448, 4), TapAction::Tab(0));
        assert_eq!(hit_test(300, 100, 1072, 1448, 4), TapAction::Tab(1));
    }

    #[test]
    fn swipe_thresholds() {
        assert_eq!(classify_swipe(0, 500, 0, 300, 300), Some(Swipe::Up));
        assert_eq!(classify_swipe(0, 300, 0, 500, 300), Some(Swipe::Down));
        assert_eq!(classify_swipe(0, 0, 5, 5, 200), None); // 漂移容差
        assert_eq!(classify_swipe(0, 500, 0, 300, 50), None); // 太快
        assert_eq!(classify_swipe(0, 500, 200, 500, 300), None); // 水平忽略
    }

    #[test]
    fn event_32_and_64() {
        let mut b32 = [0u8; 16];
        b32[8] = 1;
        b32[10] = 0x4a;
        b32[12] = 1;
        assert_eq!(parse_event(&b32), Some((1, 0x4a, 1)));
        let mut b64 = [0u8; 24];
        b64[16] = 3;
        b64[18] = 0x35;
        b64[20..24].copy_from_slice(&500i32.to_le_bytes());
        assert_eq!(parse_event(&b64), Some((3, 0x35, 500)));
    }
}
