// layout: 坐标/分页/度量/波形几何（纯计算，无副作用）
use crate::config::{self, StockData};

// 单位=屏像素；字号 pt→px 按 300DPI 换算
pub(crate) const MARGIN_X: i32 = 30;
pub(crate) const CONTENT_TOP: i32 = 142;
pub(crate) const BOTTOM_RESERVE: i32 = 70;
pub(crate) const DIVIDER_Y: i32 = 128;
pub(crate) const HEADER_H: i32 = 48;
pub(crate) const HEADER_BAR_H: i32 = 38;
pub(crate) const HEADER_GAP: i32 = 4;
pub(crate) const NORMAL_CARD_H: i32 = 103;
pub(crate) const LARGE_CARD_H: i32 = 155;
pub(crate) const TAB_BAR_Y: i32 = 68;
pub(crate) const TAB_BAR_H: i32 = 50;
pub(crate) const TAB_GAP: i32 = 10;

pub(crate) fn px(pt: i32) -> i32 {
    pt * 300 / 72
}

pub(crate) enum Block {
    Header { group: String },
    Card(StockData),
}

pub(crate) fn card_h(large: bool) -> i32 {
    if large {
        LARGE_CARD_H
    } else {
        NORMAL_CARD_H
    }
}

pub(crate) fn build_blocks(data: &[StockData], eff: &str, is_auto: bool) -> Vec<(String, Vec<StockData>)> {
    let mut groups: Vec<(String, Vec<StockData>)> = vec![];
    for d in data {
        match groups.iter_mut().find(|(g, _)| g == &d.group) {
            Some((_, v)) => v.push(d.clone()),
            None => groups.push((d.group.clone(), vec![d.clone()])),
        }
    }
    let pick = |name: &str| groups.iter().find(|(g, _)| g == name).cloned();
    if eff == "ALL" {
        return config::all_groups()
            .into_iter()
            .filter_map(|g| pick(&g))
            .collect();
    }
    if is_auto {
        return config::matching_auto_groups()
            .into_iter()
            .filter_map(|g| pick(&g))
            .collect();
    }
    pick(eff).into_iter().collect()
}

pub(crate) fn paginate(data: &[StockData], height: i32, view: &str) -> Vec<Vec<Block>> {
    let large = crate::input::style_mode().is_large();
    let (eff, is_auto) = config::effective_group(view);
    let mut blocks: Vec<Block> = vec![];
    for (g, list) in build_blocks(data, &eff, is_auto) {
        if list.is_empty() {
            continue;
        }
        blocks.push(Block::Header { group: g });
        for it in list {
            blocks.push(Block::Card(it));
        }
    }
    let _ = card_h(large);
    let ph = (height - CONTENT_TOP - BOTTOM_RESERVE).max(200);
    let h_of = |b: &Block| match b {
        Block::Header { .. } => HEADER_H,
        Block::Card(_) => card_h(large),
    };
    let mut pages: Vec<Vec<Block>> = vec![];
    let (mut cur, mut cur_h) = (vec![], 0);
    for b in blocks {
        let h = h_of(&b);
        if cur_h + h > ph && cur_h > 0 {
            pages.push(cur);
            cur = vec![];
            cur_h = 0;
        }
        cur.push(b);
        cur_h += h;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    if pages.is_empty() {
        pages.push(vec![]);
    }
    pages
}

pub fn total_pages(data: &[StockData], height: i32, view: &str) -> usize {
    paginate(data, height, view).len()
}

pub(crate) fn stock_strings(price: f64, change: f64, pct: f64) -> (String, String) {
    let price_str = if price > 0.0 {
        format!("{price:.2}")
    } else {
        "--".to_string()
    };
    let arrow = if change > 0.0 {
        "▲"
    } else if change < 0.0 {
        "▼"
    } else {
        " "
    };
    let sign_c = if change >= 0.0 { "+" } else { "" };
    let sign_p = if pct >= 0.0 { "+" } else { "" };
    (
        price_str,
        format!("{arrow} {sign_c}{change:.2} ({sign_p}{pct:.2}%)"),
    )
}

pub(crate) fn sparkline_range(prices: &[f64]) -> (f64, f64) {
    if prices.len() < 2 {
        return (0.0, 1.0);
    }
    let (mut mn, mut mx) = (prices[0], prices[0]);
    for &p in &prices[1..] {
        mn = mn.min(p);
        mx = mx.max(p);
    }
    if mx == mn {
        (mn, mn + 1.0)
    } else {
        (mn, mx)
    }
}

// advance 求和：字符数启发式对数字/拉丁误差近一倍

pub(crate) fn spark_points(
    item: &StockData,
    large: bool,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
) -> (String, Option<i32>) {
    let Some(g) = spark_geom(item, large) else {
        return (String::new(), None);
    };
    let n = item.prices.len();
    let stride = spark_stride(n);
    let mut pts = String::with_capacity(n.div_ceil(stride) * 12 + 8);
    use std::fmt::Write as _;
    for (i, &p) in item.prices.iter().enumerate() {
        if i % stride != 0 && i + 1 != n {
            continue;
        }
        let (x, y) = spark_xy(item, &g, sx, sy, sw, sh, i, p);
        if !pts.is_empty() {
            pts.push(' ');
        }
        let _ = write!(&mut pts, "{x},{y}");
    }
    (pts, spark_ref_y(&g, sy, sh))
}

// 波形几何（SVG 字符串版与 tiny-skia 直绘版共享，公式逐字一致，零漂移）
pub(crate) struct SparkGeom {
    pub(crate) mn: f64,
    pub(crate) mx: f64,
    pub(crate) reference: f64, // 0.0=不画基准线
}

pub(crate) fn spark_geom(item: &StockData, large: bool) -> Option<SparkGeom> {
    let prices = &item.prices;
    if prices.len() < 2 {
        return None;
    }
    let (mut mn, mut mx) = sparkline_range(prices);
    let is_yahoo = item.timestamps.len() == prices.len() && item.regular_end > item.regular_start;
    let mut reference = if is_yahoo {
        item.chart_prev_close
    } else {
        item.prev
    };
    if reference == 0.0 {
        reference = (mn + mx) / 2.0;
    }
    if large {
        mn = mn.min(reference);
        mx = mx.max(reference);
    } else if reference < mn || reference > mx {
        reference = 0.0; // 超出当日范围就不画（对齐 render.go）
    }
    Some(SparkGeom { mn, mx, reference })
}

pub(crate) fn spark_stride(n: usize) -> usize {
    // 抽稀：480px 宽画 240 点=每像素 2 点，纯属过采样；120 点仍 4px 一点，折线视觉一致
    n.div_ceil(120).max(1)
}

pub(crate) fn spark_xy(
    item: &StockData,
    g: &SparkGeom,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
    i: usize,
    p: f64,
) -> (i32, i32) {
    let mut rng = g.mx - g.mn;
    if rng == 0.0 {
        rng = 1.0;
    }
    let x = if item.timestamps.len() == item.prices.len() && item.regular_end > item.regular_start
    {
        let total = (item.regular_end - item.regular_start).max(1) as f64;
        sx + 2 + ((item.timestamps[i] - item.regular_start) as f64 * (sw - 4) as f64 / total) as i32
    } else {
        let total = config::chart_total(&item.code, item.prices.len()).max(1) as f64;
        sx + 2 + (i as f64 * (sw - 4) as f64 / total) as i32
    };
    let y = sy + 2 + ((g.mx - p) * (sh - 4) as f64 / rng) as i32;
    (x, y)
}

pub(crate) fn spark_ref_y(g: &SparkGeom, sy: i32, sh: i32) -> Option<i32> {
    if g.reference > 0.0 {
        let mut rng = g.mx - g.mn;
        if rng == 0.0 {
            rng = 1.0;
        }
        Some(sy + 2 + ((g.mx - g.reference) * (sh - 4) as f64 / rng) as i32)
    } else {
        None
    }
}

// 波形直绘：与模板 polyline/ref-line 逐像素一致（同为 tiny-skia 光栅化，
// 默认 Butt/Miter 与 SVG 缺省一致），跳过 minijinja+usvg 整条管线
pub(crate) struct Metrics {
    pub(crate) name_dy: i32,
    pub(crate) name_px: i32,
    pub(crate) code_dy: i32,
    pub(crate) code_px: i32,
    pub(crate) sp_x: i32,
    pub(crate) sp_dy: i32,
    pub(crate) sp_w: i32,
    pub(crate) sp_h: i32,
    pub(crate) pr_dy: i32,
    pub(crate) pr_px: i32,
    pub(crate) ch_dy: i32,
    pub(crate) ch_px: i32,
    pub(crate) chh: i32,
}

pub(crate) fn metrics(large: bool, width: i32) -> Metrics {
    if large {
        Metrics {
            name_dy: 18,
            name_px: px(8),
            code_dy: 62,
            code_px: px(5),
            sp_x: 210,
            sp_dy: 15,
            sp_w: width - 490,
            sp_h: LARGE_CARD_H - 30,
            pr_dy: 16,
            pr_px: px(9),
            ch_dy: 64,
            ch_px: px(6),
            chh: LARGE_CARD_H,
        }
    } else {
        Metrics {
            name_dy: 14,
            name_px: px(6),
            code_dy: 48,
            code_px: px(4),
            sp_x: 240,
            sp_dy: 20,
            sp_w: 480,
            sp_h: 63,
            pr_dy: 14,
            pr_px: px(8),
            ch_dy: 52,
            ch_px: px(5),
            chh: NORMAL_CARD_H,
        }
    }
}
