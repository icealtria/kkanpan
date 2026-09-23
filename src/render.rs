use crate::config::{self, StockData};
use minijinja::Environment;
use serde::Serialize;
use std::sync::OnceLock;

// 单位=屏像素；字号 pt→px 按 300DPI 换算
const MARGIN_X: i32 = 30;
const CONTENT_TOP: i32 = 142;
const BOTTOM_RESERVE: i32 = 70;
const DIVIDER_Y: i32 = 128;
const HEADER_H: i32 = 48;
const HEADER_BAR_H: i32 = 38;
const HEADER_GAP: i32 = 4;
const NORMAL_CARD_H: i32 = 103;
const LARGE_CARD_H: i32 = 155;
const TAB_BAR_Y: i32 = 68;
const TAB_BAR_H: i32 = 50;
const TAB_GAP: i32 = 10;

fn px(pt: i32) -> i32 {
    pt * 300 / 72
}

enum Block {
    Header { group: String },
    Card(StockData),
}

fn card_h(style: &str) -> i32 {
    if style == "large" {
        LARGE_CARD_H
    } else {
        NORMAL_CARD_H
    }
}

fn build_blocks(data: &[StockData], eff: &str, is_auto: bool) -> Vec<(String, Vec<StockData>)> {
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

fn paginate(data: &[StockData], height: i32, view: &str) -> Vec<Vec<Block>> {
    let style = crate::input::style_mode();
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
    let _ = card_h(&style);
    let ph = (height - CONTENT_TOP - BOTTOM_RESERVE).max(200);
    let h_of = |b: &Block| match b {
        Block::Header { .. } => HEADER_H,
        Block::Card(_) => card_h(&style),
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

#[derive(Serialize)]
struct Tab<'a> {
    key: &'a str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    selected: bool,
}

#[derive(Serialize)]
struct CtxBlock {
    is_header: bool,
    group: String,
    bar_y: i32,
    card_y: i32,
    card_h: i32,
    name_l1: String,
    name_l2: String,
    code: String,
    spark: String,      // polyline points，空=不画
    ref_y: Option<i32>, // 昨收虚线 y，None=不画
    spark_x: i32,
    spark_y: i32,
    spark_w: i32,
    spark_h: i32,
    price: String,
    chg: String,
    name_x: i32,
    name_y: i32,
    name_px: i32,
    code_y: i32,
    code_px: i32,
    price_y: i32,
    price_px: i32,
    chg_y: i32,
    chg_px: i32,
}

#[derive(Serialize)]
struct Screen<'a> {
    w: i32,
    h: i32,
    layer: &'a str, // "full" | "base" | "dyn"
    font_family: &'a str,
    title_px: i32,
    mode_tag: &'a str,
    mode_px: i32,
    style_label: &'a str,
    tab_px: i32,
    tabs: Vec<Tab<'a>>,
    divider_y: i32,
    margin_x: i32,
    header_bar_h: i32,
    bar_text_px: i32,
    blocks: Vec<CtxBlock>,
    page_text: String,
    status_px: i32,
    status_text: String,
    footer_hint: &'a str,
}

fn stock_strings(price: f64, change: f64, pct: f64) -> (String, String) {
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

fn sparkline_range(prices: &[f64]) -> (f64, f64) {
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
fn char_widths(text: &str, font_px: i32) -> Vec<i32> {
    let fallback = vec![font_px / 2; text.chars().count()];
    let fs = fontset();
    let family = resvg::usvg::fontdb::Family::Name(&fs.family);
    let q = resvg::usvg::fontdb::Query {
        families: &[family],
        ..Default::default()
    };
    let id = match fs.db.query(&q) {
        Some(id) => id,
        None => return fallback,
    };
    fs.db
        .with_face_data(id, |data, idx| {
            use skrifa::MetadataProvider;
            let font = skrifa::FontRef::from_index(data, idx).ok()?;
            let metrics = font.glyph_metrics(
                skrifa::instance::Size::new(font_px as f32),
                skrifa::instance::LocationRef::default(),
            );
            let cmap = font.charmap();
            Some(
                text.chars()
                    .map(|ch| {
                        let w = cmap
                            .map(ch)
                            .and_then(|gid| metrics.advance_width(gid))
                            .unwrap_or(font_px as f32 / 2.0);
                        w.ceil() as i32
                    })
                    .collect(),
            )
        })
        .flatten()
        .unwrap_or(fallback)
}

fn split_name(name: &str, font_px: i32, max_w: i32) -> (String, String) {
    // 股票名静态不变：按 (name, px, max_w) 缓存，渲染循环零 skrifa 解析
    static SPLIT_CACHE: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<(String, i32, i32), (String, String)>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let key = (name.to_string(), font_px, max_w);
    if let Some(hit) = SPLIT_CACHE.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let computed = split_name_uncached(name, font_px, max_w);
    SPLIT_CACHE.lock().unwrap().insert(key, computed.clone());
    computed
}

/// 启动预热：把两种 style 的截断结果一次性算好，首屏即缓存命中；
/// 顺带触发 fontset() 磁盘加载，可与首轮网络 fetch 并行
pub fn precache_names() {
    let _ = fontset();
    for s in config::stocks() {
        let name = if s.name.is_empty() { &s.code } else { &s.name };
        // 与 render_svg 内布局常量保持一致：large(px8, sp_x 210) / normal(px6, sp_x 240)
        split_name(name, px(8), 210 - (MARGIN_X + 15) - 5);
        split_name(name, px(6), 240 - (MARGIN_X + 15) - 5);
    }
    precache_glyphs();
}

/// 高频字形预热：数字/符号 × 价格字号（每帧都画，首刷前备好）；
/// 汉字/字母走懒加载（首次 base 渲染时顺带备好，常驻缓存）
pub fn precache_glyphs() {
    for px in [px(5), px(6), px(8), px(9)] {
        for ch in "0123456789.+-()% ▲▼/".chars() {
            sprite_for(ch, px);
        }
    }
    for s in config::stocks() {
        let name = if s.name.is_empty() { &s.code } else { &s.name };
        for ch in name.chars().chain(s.group.chars()) {
            sprite_for(ch, px(8));
            sprite_for(ch, px(6));
        }
    }
}

fn split_name_uncached(name: &str, font_px: i32, max_w: i32) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    let widths = char_widths(name, font_px);
    let total: i32 = widths.iter().sum();
    if total <= max_w {
        return (name.to_string(), String::new());
    }
    let mut w = 0;
    let mut cut = 0;
    for (i, _) in chars.iter().enumerate() {
        w += widths.get(i).copied().unwrap_or(font_px / 2);
        if w > max_w {
            break;
        }
        cut = i + 1;
    }
    if cut == 0 {
        cut = 1;
    }
    (chars[..cut].iter().collect(), chars[cut..].iter().collect())
}

fn spark_points(
    item: &StockData,
    large: bool,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
) -> (String, Option<i32>) {
    let prices = &item.prices;
    if prices.len() < 2 {
        return (String::new(), None);
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
    let mut rng = mx - mn;
    if rng == 0.0 {
        rng = 1.0;
    }
    let ref_y = if reference > 0.0 {
        Some(sy + 2 + ((mx - reference) * (sh - 4) as f64 / rng) as i32)
    } else {
        None
    };
    let n = prices.len();
    // 抽稀：480px 宽画 240 点=每像素 2 点，纯属过采样；120 点仍 4px 一点，折线视觉一致，
    // 但 points 字符串减半，tpl/parse/raster 三处全受益。x 仍按原下标映射，位置不变
    let stride = n.div_ceil(120).max(1);
    let mut pts = String::with_capacity(n.div_ceil(stride) * 12 + 8);
    use std::fmt::Write as _;
    for (i, &p) in prices.iter().enumerate() {
        if i % stride != 0 && i + 1 != n {
            continue;
        }
        let x = if is_yahoo && item.timestamps.len() == prices.len() {
            let total = (item.regular_end - item.regular_start).max(1) as f64;
            sx + 2
                + ((item.timestamps[i] - item.regular_start) as f64 * (sw - 4) as f64 / total)
                    as i32
        } else {
            let total = config::chart_total(&item.code, prices.len()).max(1) as f64;
            sx + 2 + (i as f64 * (sw - 4) as f64 / total) as i32
        };
        let y = sy + 2 + ((mx - p) * (sh - 4) as f64 / rng) as i32;
        if !pts.is_empty() {
            pts.push(' ');
        }
        let _ = write!(&mut pts, "{x},{y}");
    }
    (pts, ref_y)
}

static TPL: OnceLock<Environment<'static>> = OnceLock::new();

// ---- 文字 sprite：SVG 只画图形，所有文字启动预渲染、刷新时 blit ----
// Sprite 几何统一：画布 H=3*px、基线在 2*px 处，只裁左右白边、保留全高，
// 不同字形基线天然对齐；dx=内容左 edge 相对笔尖，dy=基线相对内容顶
#[derive(Clone)]
struct Sprite {
    w: i32,
    h: i32,
    dx: i32,
    dy: i32,
    px: Vec<u8>, // 白底灰度，255=透明可跳
}

#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    Start,
    Middle,
    End,
}

struct BlitText {
    s: String,
    x: i32,
    y: i32, // 基线（对齐 SVG text 的 y）
    px: i32,
    anchor: Anchor,
    invert: bool, // 白字黑底（表头条/选中 Tab）：blit 时 255-v 反色，与直接渲染等价
}

static SPRITES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<(char, i32), Sprite>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
static ADVS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<(char, i32), i32>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn advance_for(ch: char, px: i32) -> i32 {
    *ADVS
        .lock()
        .unwrap()
        .entry((ch, px))
        .or_insert_with(|| char_widths(&ch.to_string(), px).into_iter().next().unwrap_or(px / 2))
}

fn sprite_for(ch: char, px: i32) -> Sprite {
    SPRITES
        .lock()
        .unwrap()
        .entry((ch, px))
        .or_insert_with(|| render_glyph(ch, px))
        .clone()
}

fn render_glyph(ch: char, px: i32) -> Sprite {
    let empty = Sprite { w: 0, h: 0, dx: 0, dy: 0, px: vec![] };
    if ch == ' ' {
        return empty;
    }
    let (cw, chh, base) = (px * 4, px * 3, px * 2);
    let svg = format!(
        "<svg width=\"{cw}\" height=\"{chh}\" xmlns=\"http://www.w3.org/2000/svg\"><text x=\"{px}\" y=\"{base}\" font-size=\"{px}\" fill=\"black\" font-family=\"{}\">{ch}</text></svg>",
        fontset().family,
    );
    let tree = parse_tree(&svg);
    let mut pix = resvg::tiny_skia::Pixmap::new(cw as u32, chh as u32).expect("pixmap");
    pix.fill(resvg::tiny_skia::Color::WHITE);
    render_tree_into(&tree, &mut pix);
    let data = pix.data();
    let at = |x: i32, y: i32| data[(y * cw + x) as usize * 4];
    let mut cols: Vec<i32> = vec![];
    let mut top = chh;
    let mut bottom = -1;
    for y in 0..chh {
        for x in 0..cw {
            if at(x, y) != 255 {
                cols.push(x);
                if y < top {
                    top = y;
                }
                if y > bottom {
                    bottom = y;
                }
            }
        }
    }
    if bottom < 0 {
        return empty;
    }
    let (x0, x1) = (*cols.iter().min().unwrap(), *cols.iter().max().unwrap());
    let mut out = Vec::with_capacity(((x1 - x0 + 1) * (bottom - top + 1)) as usize);
    for y in top..=bottom {
        for x in x0..=x1 {
            let p = &data[(y * cw + x) as usize * 4..];
            let a = p[3] as u32;
            let r = p[0] as u32 + 255 - a;
            let g = p[1] as u32 + 255 - a;
            let b = p[2] as u32 + 255 - a;
            out.push(((r * 77 + g * 150 + b * 29) >> 8) as u8);
        }
    }
    Sprite { w: x1 - x0 + 1, h: bottom - top + 1, dx: x0 - px, dy: base - top, px: out }
}

fn blit_text(canvas: &mut resvg::tiny_skia::Pixmap, t: &BlitText) {
    if t.s.is_empty() {
        return;
    }
    let advs: Vec<i32> = t.s.chars().map(|ch| advance_for(ch, t.px)).collect();
    let total: i32 = advs.iter().sum();
    let mut pen = match t.anchor {
        Anchor::Start => t.x,
        Anchor::Middle => t.x - total / 2,
        Anchor::End => t.x - total,
    };
    let (cw, chh) = (canvas.width() as i32, canvas.height() as i32);
    let data = canvas.data_mut();
    // 反白时背景是黑(0)：跳过反色后等于背景的像素
    let bg: u8 = if t.invert { 0 } else { 255 };
    for (ch, adv) in t.s.chars().zip(advs) {
        let sp = sprite_for(ch, t.px);
        if !sp.px.is_empty() {
            let (x0, y0) = (pen + sp.dx, t.y - sp.dy);
            let (xa, xb) = (x0.max(0), (x0 + sp.w).min(cw));
            let (ya, yb) = (y0.max(0), (y0 + sp.h).min(chh));
            for yy in ya..yb {
                let srow = (yy - y0) * sp.w;
                let drow = yy * cw * 4;
                for xx in xa..xb {
                    let v = sp.px[(srow + (xx - x0)) as usize];
                    let w = if t.invert { 255 - v } else { v };
                    if w != bg {
                        let o = (drow + xx * 4) as usize;
                        data[o] = w;
                        data[o + 1] = w;
                        data[o + 2] = w;
                        data[o + 3] = 255;
                    }
                }
            }
        }
        pen += adv;
    }
}

fn env() -> &'static Environment<'static> {
    TPL.get_or_init(|| {
        let mut e = Environment::new();
        // 去掉块标签周围的空白行：SVG 变紧凑，XML 词法负担小，模板输出也小一截
        e.set_trim_blocks(true);
        e.set_lstrip_blocks(true);
        e.add_template("screen", include_str!("../templates/screen.svg"))
            .unwrap();
        e
    })
}

pub fn render_svg(data: &[StockData], width: i32, height: i32, view: &str, page: usize) -> String {
    render_svg_layer(data, width, height, view, page, "full").0
}

fn render_svg_layer(
    data: &[StockData],
    width: i32,
    height: i32,
    view: &str,
    page: usize,
    layer: &str,
) -> (String, Vec<BlitText>) {
    // full 只给 server 预览用，texts 用不上；base 收静态字，dyn 收动态字
    let want_static = layer == "base";
    let want_dyn = layer == "dyn";
    let mut texts: Vec<BlitText> = vec![];
    let style = crate::input::style_mode();
    let large = style == "large";
    let (eff, is_auto) = config::effective_group(view);
    let mode_tag = if is_auto {
        format!("[AUTO: {eff}]")
    } else {
        format!("[{view}]")
    };

    let modes = config::tab_modes();
    let n = modes.len().max(1) as i32;
    let total_w = width - 2 * MARGIN_X;
    let tw = (total_w - (n - 1) * TAB_GAP) / n;
    let tabs: Vec<Tab> = modes
        .iter()
        .enumerate()
        .map(|(i, m)| Tab {
            key: m,
            x: MARGIN_X + i as i32 * (tw + TAB_GAP),
            y: TAB_BAR_Y,
            w: tw,
            h: TAB_BAR_H,
            selected: view == m,
        })
        .collect();

    if want_static {
        let tpx = px(6);
        texts.push(BlitText { s: "KKANPAN".into(), x: 30, y: 16 + px(8), px: px(8), anchor: Anchor::Start, invert: false });
        texts.push(BlitText { s: mode_tag.clone(), x: width - 460, y: 20 + tpx, px: tpx, anchor: Anchor::Start, invert: false });
        texts.push(BlitText {
            s: (if large { "L" } else { "S" }).to_string(),
            x: width - 185 + 40,
            y: 10 + 8 + tpx,
            px: tpx,
            anchor: Anchor::Middle,
            invert: false,
        });
        texts.push(BlitText { s: "X".into(), x: width - 95 + 32, y: 40, px: 25, anchor: Anchor::Middle, invert: false });
        for t in &tabs {
            texts.push(BlitText {
                s: t.key.to_string(),
                x: t.x + t.w / 2,
                y: t.y + 12 + tpx,
                px: tpx,
                anchor: Anchor::Middle,
                invert: t.selected, // 选中 Tab 是白字黑底
            });
        }
    }

    let pages = paginate(data, height, view);
    let total = pages.len();
    let cur = page.min(total.saturating_sub(1));

    let (
        name_dy,
        name_px,
        code_dy,
        code_px,
        sp_x,
        sp_dy,
        sp_w,
        sp_h,
        pr_dy,
        pr_px,
        ch_dy,
        ch_px,
        chh,
    ) = if large {
        (
            18,
            px(8),
            62,
            px(5),
            210,
            15,
            width - 490,
            LARGE_CARD_H - 30,
            16,
            px(9),
            64,
            px(6),
            LARGE_CARD_H,
        )
    } else {
        (
            14,
            px(6),
            48,
            px(4),
            240,
            20,
            480,
            63,
            14,
            px(8),
            52,
            px(5),
            NORMAL_CARD_H,
        )
    };

    let mut y = CONTENT_TOP;
    let mut blocks = vec![];
    for b in &pages[cur] {
        match b {
            Block::Header { group } => {
                blocks.push(CtxBlock {
                    is_header: true,
                    group: group.clone(),
                    bar_y: y + HEADER_GAP,
                    card_y: 0,
                    card_h: HEADER_H,
                    name_l1: String::new(),
                    name_l2: String::new(),
                    code: String::new(),
                    spark: String::new(),
                    ref_y: None,
                    spark_x: 0,
                    spark_y: 0,
                    spark_w: 0,
                    spark_h: 0,
                    price: String::new(),
                    chg: String::new(),
                    name_x: 0,
                    name_y: 0,
                    name_px: 0,
                    code_y: 0,
                    code_px: 0,
                    price_y: 0,
                    price_px: 0,
                    chg_y: 0,
                    chg_px: 0,
                });
                y += HEADER_H;
                if want_static {
                    texts.push(BlitText {
                        s: format!("[ {group} ]"),
                        x: MARGIN_X + 15,
                        y: y - HEADER_H + HEADER_GAP + 8 + px(5),
                        px: px(5),
                        anchor: Anchor::Start,
                        invert: true, // 黑底白字条
                    });
                }
            }
            Block::Card(item) => {
                let name = if item.name.is_empty() {
                    item.code.clone()
                } else {
                    item.name.clone()
                };
                let max_w = sp_x - (MARGIN_X + 15) - 5;
                // base/full 才排名字，dyn 只算坐标；反之 spark/价格只在 dyn/full 算
                let (l1, l2) = if layer == "dyn" {
                    (String::new(), String::new())
                } else {
                    split_name(&name, name_px, max_w)
                };
                let code_y = if l2.is_empty() {
                    y + code_dy
                } else {
                    y + name_dy + 2 * name_px + name_px / 3
                };
                let (pts, ref_y) = if layer == "base" {
                    (String::new(), None)
                } else {
                    spark_points(item, large, sp_x, y + sp_dy, sp_w, sp_h)
                };
                let (price_s, chg_s) = if layer == "base" {
                    (String::new(), String::new())
                } else {
                    stock_strings(item.price, item.change, item.pct)
                };
                // sprite 坐标与模板 full 层的 <text> 逐项对齐（x/y/字号/对齐）
                if want_static {
                    let nx = MARGIN_X + 15;
                    let ny = y + name_dy;
                    texts.push(BlitText { s: l1.clone(), x: nx, y: ny + name_px, px: name_px, anchor: Anchor::Start, invert: false });
                    if !l2.is_empty() {
                        texts.push(BlitText {
                            s: l2.clone(),
                            x: nx,
                            y: ny + 2 * name_px + name_px / 3,
                            px: name_px,
                            anchor: Anchor::Start,
                            invert: false,
                        });
                    }
                    texts.push(BlitText { s: item.code.clone(), x: nx, y: code_y + code_px, px: code_px, anchor: Anchor::Start, invert: false });
                }
                if want_dyn {
                    texts.push(BlitText { s: price_s.clone(), x: width - 45, y: y + pr_dy + pr_px, px: pr_px, anchor: Anchor::End, invert: false });
                    texts.push(BlitText { s: chg_s.clone(), x: width - 45, y: y + ch_dy + ch_px, px: ch_px, anchor: Anchor::End, invert: false });
                }
                blocks.push(CtxBlock {
                    is_header: false,
                    group: String::new(),
                    bar_y: 0,
                    card_y: y,
                    card_h: chh,
                    name_l1: l1,
                    name_l2: l2,
                    code: item.code.clone(),
                    spark: pts,
                    ref_y,
                    spark_x: sp_x,
                    spark_y: y + sp_dy,
                    spark_w: sp_w,
                    spark_h: sp_h,
                    price: price_s,
                    chg: chg_s,
                    name_x: MARGIN_X + 15,
                    name_y: y + name_dy,
                    name_px,
                    code_y,
                    code_px,
                    price_y: y + pr_dy,
                    price_px: pr_px,
                    chg_y: y + ch_dy,
                    chg_px: ch_px,
                });
                y += chh;
            }
        }
    }

    let mut page_text = format!("{} / {}", cur + 1, total);
    if total > 1 {
        if cur > 0 {
            page_text = format!("▲ {page_text}");
        }
        if cur + 1 < total {
            page_text = format!("{page_text} ▼");
        }
    } else {
        page_text.clear();
    }

    let status_text = crate::kindle::format_status_bar();
    if want_static {
        if !page_text.is_empty() {
            texts.push(BlitText {
                s: page_text.clone(),
                x: width / 2,
                y: height - 40 + px(4),
                px: px(4),
                anchor: Anchor::Middle,
                invert: false,
            });
        }
        texts.push(BlitText {
            s: "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit".to_string(),
            x: MARGIN_X,
            y: height - 24 + px(4),
            px: px(4),
            anchor: Anchor::Start,
            invert: false,
        });
    }
    if want_dyn {
        texts.push(BlitText {
            s: status_text.clone(),
            x: width - MARGIN_X,
            y: height - 24 + px(4),
            px: px(4),
            anchor: Anchor::End,
            invert: false,
        });
    }

    let ctx = Screen {
        w: width,
        h: height,
        layer,
        font_family: &fontset().family,
        title_px: px(8),
        mode_tag: &mode_tag,
        mode_px: px(6),
        style_label: if large { "L" } else { "S" },
        tab_px: px(6),
        tabs,
        divider_y: DIVIDER_Y,
        margin_x: MARGIN_X,
        header_bar_h: HEADER_BAR_H,
        bar_text_px: px(5),
        blocks,
        page_text,
        status_px: px(4),
        status_text,
        footer_hint: "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit",
    };
    (env().get_template("screen").unwrap().render(&ctx).unwrap(), texts)
}

static FONTDB: OnceLock<FontSet> = OnceLock::new();

struct FontSet {
    db: std::sync::Arc<resvg::usvg::fontdb::Database>,
    // 明确指定覆盖 CJK 的 family，避免 sans-serif 命中纯西文字库导致汉字消失
    family: String,
}

fn is_font_file(p: &std::path::Path) -> bool {
    matches!(
        p.extension()
            .and_then(|x| x.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("ttf" | "otf" | "ttc")
    )
}

fn scan_fonts(db: &mut resvg::usvg::fontdb::Database, dir: &str, depth: usize, to_memory: bool) {
    if depth > 4 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_fonts(db, &p.to_string_lossy(), depth + 1, to_memory);
            continue;
        }
        // 跳过 macOS 传过来的 AppleDouble 垃圾（._*.ttf 不是字库）
        if p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("._") || n.starts_with('.'))
            .unwrap_or(true)
        {
            continue;
        }
        if !is_font_file(&p) {
            continue;
        }
        if to_memory {
            // A9 上反复 mmap/读文件是 parse 慢的主因之一
            match std::fs::read(&p) {
                Ok(data) => {
                    crate::dlog!(
                        "[font] memory-loaded {} ({}KB)",
                        p.display(),
                        data.len() / 1024
                    );
                    db.load_font_source(resvg::usvg::fontdb::Source::Binary(std::sync::Arc::new(
                        data,
                    )));
                }
                Err(_) => {
                    db.load_font_file(&p).ok();
                }
            }
        } else if db.load_font_file(&p).is_ok() {
            crate::dlog!("[font] loaded {}", p.display());
        }
    }
}

fn fontset() -> &'static FontSet {
    FONTDB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        for p in [
            "/mnt/us/extensions/kkanpan/font.ttf",
            "/mnt/us/extensions/kkanpan/font.otf",
            "font.ttf",
            "font.otf",
        ] {
            if std::path::Path::new(p).exists() {
                db.load_font_file(p).ok();
            }
        }
        scan_fonts(&mut db, "/mnt/us/fonts", 0, true);
        for d in ["/usr/java/lib/fonts", "/usr/share/fonts", "/opt"] {
            scan_fonts(&mut db, d, 0, false);
        }
        db.load_system_fonts();
        eprintln!("[font] {} faces total", db.len());
        // 选 family：固件 STHeiti/STSong 又小又快（文泉驿十几 MB，A9 上 shaping 慢），
        // 缺字时 usvg 会按字符 fallback 到其它已加载字库，不会变豆腐块
        let faces: Vec<_> = db.faces().collect();
        let fam_of = |f: &&resvg::usvg::fontdb::FaceInfo| {
            f.families
                .first()
                .map(|(s, _)| s.clone())
                .unwrap_or_default()
        };
        let exact = ["STHeitiMedium", "STSongMedium", "STHeiti", "STSong"];
        let face = exact
            .iter()
            .find_map(|w| faces.iter().find(|f| &fam_of(f) == w));
        let pick = |keys: &[&str]| {
            faces.iter().find(|f| {
                let n = f
                    .families
                    .first()
                    .map(|(s, _)| s.to_lowercase())
                    .unwrap_or_default();
                keys.iter().any(|k| n.contains(k))
            })
        };
        let face = face
            .or_else(|| {
                pick(&[
                    "stheiti", "stsong", "song", "kai", "hei", "noto", "droid", "wenquan", "uming",
                    "ukai", "cjk", "pingfang", "han",
                ])
            })
            .or_else(|| faces.first());
        let family = face
            .and_then(|f| f.families.first().map(|(s, _)| s.clone()))
            .unwrap_or_else(|| "sans-serif".to_string());
        eprintln!("[font] using family: {family}");
        FontSet {
            db: std::sync::Arc::new(db),
            family,
        }
    })
}

thread_local! {
    // 同尺寸重复利用：6MB Pixmap 只分配一次，后续 fill(白) 复用
    static PIXMAP: std::cell::RefCell<Option<resvg::tiny_skia::Pixmap>> =
        const { std::cell::RefCell::new(None) };
}

fn parse_tree(svg: &str) -> resvg::usvg::Tree {
    let fs = fontset();
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fs.db.clone();
    resvg::usvg::Tree::from_str(svg, &opt).expect("svg parse")
}

fn render_tree_into(tree: &resvg::usvg::Tree, pix: &mut resvg::tiny_skia::Pixmap) {
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
}

fn render_tree(tree: &resvg::usvg::Tree) {
    PIXMAP.with(|cell| {
        let (w, h) = (tree.size().width() as u32, tree.size().height() as u32);
        let mut slot = cell.borrow_mut();
        let reuse = matches!(&*slot, Some(p) if p.width() == w && p.height() == h);
        if !reuse {
            *slot = Some(resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap"));
        }
        let pix = slot.as_mut().expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::WHITE);
        render_tree_into(tree, pix);
    });
}

pub fn render_pixmap(svg: &str) -> resvg::tiny_skia::Pixmap {
    let tree = parse_tree(svg);
    render_tree(&tree);
    PIXMAP.with(|cell| cell.borrow().as_ref().expect("pixmap").clone())
}

pub fn pixmap_to_gray_into(pix: &resvg::tiny_skia::Pixmap, out: &mut Vec<u8>) {
    let n = pix.width() as usize * pix.height() as usize;
    out.clear();
    out.reserve(n);
    // SAFETY: 刚 reserve，set_len 后逐字节写入，无未初始化读取
    #[allow(clippy::uninit_vec)]
    unsafe {
        out.set_len(n);
    }
    let src = pix.data();
    // 白底 premultiplied 展开：out = src + 255 - a；系数和 256，位移代除法，无分支可向量化
    for (i, p) in src.chunks_exact(4).enumerate() {
        let a = p[3] as u32;
        let r = p[0] as u32 + 255 - a;
        let g = p[1] as u32 + 255 - a;
        let b = p[2] as u32 + 255 - a;
        // SAFETY: i < n，out 长度已设为 n
        unsafe {
            *out.get_unchecked_mut(i) = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        }
    }
}

#[allow(dead_code)]
pub fn pixmap_to_gray(pix: &resvg::tiny_skia::Pixmap) -> Vec<u8> {
    let mut out = Vec::new();
    pixmap_to_gray_into(pix, &mut out);
    out
}

// 静态层缓存：Tab/表头/股票名只与 (w,h,view,style,page) 有关，数据刷新不失效；
// 命中后每帧只剩动态小 SVG（价格/波形/状态）的 parse+raster
static BASE: std::sync::LazyLock<
    std::sync::Mutex<
        std::collections::HashMap<(i32, i32, String, String, usize, u64), resvg::tiny_skia::Pixmap>,
    >,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

// 成员指纹：空数据→有数据（或股票增减）时 base 版式会变，必须换 key，否则复用无卡片底图
fn stocks_fp(data: &[StockData]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.len().hash(&mut h);
    for d in data {
        d.code.hash(&mut h);
        d.group.hash(&mut h);
    }
    h.finish()
}

pub fn render_gray_page(
    data: &[StockData],
    width: i32,
    height: i32,
    view: &str,
    page: usize,
) -> Vec<u8> {
    let t0 = std::time::Instant::now();
    let style = crate::input::style_mode();
    let (svg, dyn_texts) = render_svg_layer(data, width, height, view, page, "dyn");
    let t_tpl = t0.elapsed();
    let tree = parse_tree(&svg);
    let t_parse = t0.elapsed() - t_tpl;
    // base 命中则零 parse/raster；未命中才渲染一次并缓存
    let t_base0 = std::time::Instant::now();
    let mut base = BASE.lock().unwrap();
    if base.len() >= 8 {
        base.clear();
    }
    let key = (width, height, view.to_string(), style, page, stocks_fp(data));
    let hit = base.contains_key(&key);
    let bp = base.entry(key).or_insert_with(|| {
        let (bsvg, base_texts) = render_svg_layer(data, width, height, view, page, "base");
        let btree = parse_tree(&bsvg);
        let size = btree.size();
        let mut pix = resvg::tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32)
            .expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::WHITE);
        render_tree_into(&btree, &mut pix);
        for t in &base_texts {
            blit_text(&mut pix, t);
        }
        pix
    });
    let t_base = t_base0.elapsed();
    // 合成到底图画布：拷 base（6MB memcpy）+ 叠动态层 + blit 动态字，全程复用线程局部 Pixmap
    let t_dyn0 = std::time::Instant::now();
    let mut gray = Vec::new();
    let mut t_gray = std::time::Duration::ZERO;
    let mut t_blit = std::time::Duration::ZERO;
    PIXMAP.with(|cell| {
        let (w, h) = (tree.size().width() as u32, tree.size().height() as u32);
        let mut slot = cell.borrow_mut();
        if !matches!(&*slot, Some(p) if p.width() == w && p.height() == h) {
            *slot = Some(resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap"));
        }
        let canvas = slot.as_mut().expect("pixmap");
        canvas.data_mut().copy_from_slice(bp.data());
        render_tree_into(&tree, canvas);
        let b0 = std::time::Instant::now();
        for t in &dyn_texts {
            blit_text(canvas, t);
        }
        t_blit = b0.elapsed();
        let g0 = std::time::Instant::now();
        pixmap_to_gray_into(canvas, &mut gray);
        t_gray = g0.elapsed();
    });
    drop(base);
    let t_dyn = t_dyn0.elapsed();
    crate::dlog!(
        "[render] page {page}: tpl={}ms base={}{}ms parse={}ms dyn={}ms blit={}ms gray={}ms total={}ms (dyn svg {}B, {} blits)",
        t_tpl.as_millis(),
        if hit { "hit+" } else { "miss+" },
        t_base.as_millis(),
        t_parse.as_millis(),
        (t_dyn - t_gray - t_blit).as_millis(),
        t_blit.as_millis(),
        t_gray.as_millis(),
        t0.elapsed().as_millis(),
        svg.len(),
        dyn_texts.len(),
    );
    gray
}

pub fn render_gray(data: &[StockData], width: i32, height: i32, view: &str) -> Vec<u8> {
    let total = total_pages(data, height, view).max(1);
    render_gray_page(data, width, height, view, crate::input::clamp_page(total))
}

#[cfg(test)]
mod render_tests {
    use super::{render_svg_layer, StockData};

    fn sample() -> Vec<StockData> {
        vec![StockData {
            code: "sh600000".to_string(),
            name: "浦发银行".to_string(),
            group: "a-share".to_string(),
            price: 10.26,
            change: 0.12,
            pct: 1.18,
            prev: 10.14,
            prices: vec![10.10, 10.20, 10.26],
            timestamps: vec![],
            regular_start: 0,
            regular_end: 0,
            chart_prev_close: 0.0,
        }]
    }

    #[test]
    fn spark_decimated_keeps_endpoints() {
        let mut d = sample().pop().unwrap();
        d.prices = (0..300).map(|i| 10.0 + i as f64 * 0.01).collect();
        let (pts, _) = super::spark_points(&d, false, 240, 100, 480, 63);
        let n = if pts.is_empty() { 0 } else { pts.split(' ').count() };
        assert!(n <= 121 && n >= 100, "n={n}");
        let first_x: i32 = pts.split([' ', ',']).next().unwrap().parse().unwrap();
        assert_eq!(first_x, 242);
    }

    #[test]
    fn layers_split_static_dynamic() {
        crate::input::init_state("ALL");
        let d = sample();
        let (full, _) = render_svg_layer(&d, 1072, 1448, "ALL", 0, "full");
        let (base, base_texts) = render_svg_layer(&d, 1072, 1448, "ALL", 0, "base");
        let (dyn_, dyn_texts) = render_svg_layer(&d, 1072, 1448, "ALL", 0, "dyn");
        // full 照旧含全部文字；base/dyn 的 SVG 里已无 <text>，文字走 blit
        assert!(full.contains("KKANPAN") && full.contains("10.26") && full.contains("浦发银行"));
        assert!(!base.contains("<text") && !dyn_.contains("<text"));
        assert!(dyn_.contains("<polyline"));
        assert!(base_texts.iter().any(|t| t.s.contains("浦发银行")));
        assert!(dyn_texts.iter().any(|t| t.s.contains("10.26")));
    }

    #[test]
    fn glyph_blits_visible_pixels() {
        use super::{blit_text, sprite_for, Anchor, BlitText};
        let sp = sprite_for('0', 33);
        assert!(!sp.px.is_empty() && sp.w > 0 && sp.h > 0);
        let mut pix = resvg::tiny_skia::Pixmap::new(200, 60).expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::WHITE);
        blit_text(&mut pix, &BlitText { s: "10.26".into(), x: 10, y: 40, px: 33, anchor: Anchor::Start, invert: false });
        assert!(pix.data().iter().any(|&v| v < 128));
    }

    #[test]
    fn inverted_blits_white_on_black() {
        // 表头条/选中 Tab：黑底上必须看到亮字，而不是糊成一片
        use super::{blit_text, Anchor, BlitText};
        let mut pix = resvg::tiny_skia::Pixmap::new(200, 60).expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::BLACK);
        blit_text(&mut pix, &BlitText { s: "A股".into(), x: 10, y: 40, px: 25, anchor: Anchor::Start, invert: true });
        let bright = pix.data().chunks_exact(4).filter(|p| p[0] > 200).count();
        assert!(bright > 50, "bright={bright}");
    }
}
