use crate::config::{self, StockData};
use minijinja::Environment;
use serde::Serialize;
use std::sync::OnceLock;

use crate::gray::{
    BaseEntry, compose_dynamic, parse_tree, pixmap_to_gray_into, render_tree_into,
};
use crate::layout::{
    Block, CONTENT_TOP, DIVIDER_Y, HEADER_BAR_H, HEADER_GAP, HEADER_H, MARGIN_X, Metrics,
    TAB_BAR_H, TAB_BAR_Y, TAB_GAP, metrics, paginate, px, spark_points, stock_strings,
    total_pages,
};
use crate::text::{Anchor, BlitText, blit_text, fontset, split_name};

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
    layer: &'a str, // "full"（server 预览） | "base"（静态底）
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

static TPL: OnceLock<Environment<'static>> = OnceLock::new();

// ---- 文字 sprite：SVG 只画图形，所有文字启动预渲染、刷新时 blit ----
// Sprite 几何统一：画布 H=3*px、基线在 2*px 处，只裁左右白边、保留全高，
// 不同字形基线天然对齐；dx=内容左 edge 相对笔尖，dy=基线相对内容顶

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

// 卡片布局度量（SVG 模板版与直绘版共享一份数字，零漂移）

fn render_svg_layer(
    data: &[StockData],
    width: i32,
    height: i32,
    view: &str,
    page: usize,
    layer: &str,
) -> (String, Vec<BlitText>) {
    // full 只给 server 预览用，texts 用不上；base 收静态字（动态字由 compose 直 blit）
    let want_static = layer == "base";
    let mut texts: Vec<BlitText> = vec![];
    let large = crate::input::style_mode().is_large();
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

    let Metrics {
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
    } = metrics(large, width);

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
                let (l1, l2) = split_name(&name, name_px, max_w);
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

static BASE: std::sync::LazyLock<
    std::sync::Mutex<
        std::collections::HashMap<(i32, i32, String, String, usize, u64), BaseEntry>,
    >,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
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
    let style = crate::input::style_mode().as_str().to_string();
    // base 命中则零 parse/raster；未命中才渲染一次并缓存（RGBA+灰度双份）
    let t_base0 = std::time::Instant::now();
    let mut base = BASE.lock().unwrap();
    if base.len() >= 8 {
        base.clear();
    }
    let key = (width, height, view.to_string(), style, page, stocks_fp(data));
    let hit = base.contains_key(&key);
    let be = base.entry(key).or_insert_with(|| {
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
        let mut gray = Vec::new();
        pixmap_to_gray_into(&pix, &mut gray);
        BaseEntry { pix, gray }
    });
    let t_base = t_base0.elapsed();
    // 灰度域合成：1.5MB base 灰度快拷 + tile 波形 + 灰度 blit，无全屏 RGBA 拷贝
    let t_dyn0 = std::time::Instant::now();
    let mut gray = be.gray.clone();
    let (tiles, blits) = compose_dynamic(&be.pix, &mut gray, data, width, height, view, page);
    let t_dyn = t_dyn0.elapsed();
    drop(base);
    crate::dlog!(
        "[render] page {page}: base={}{}ms dyn={}ms total={}ms ({tiles} tiles, {blits} blits)",
        if hit { "hit+" } else { "miss+" },
        t_base.as_millis(),
        t_dyn.as_millis(),
        t0.elapsed().as_millis(),
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
        // full 含全部文字；base 的 SVG 里无 <text>，静态字走 blit；动态字由 compose 直 blit
        assert!(full.contains("KKANPAN") && full.contains("10.26") && full.contains("浦发银行"));
        assert!(!base.contains("<text"));
        assert!(base_texts.iter().any(|t| t.s.contains("浦发银行")));
    }

    #[test]
    fn sparkline_draws_dark_pixels() {
        let mut d = sample().pop().unwrap();
        d.prices = (0..200).map(|i| 10.0 + (i as f64 * 0.3).sin()).collect();
        let mut pix = resvg::tiny_skia::Pixmap::new(600, 120).expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::WHITE);
        crate::gray::draw_sparkline(&mut pix, &d, false, 10, 10, 480, 63);
        assert!(pix.data().chunks_exact(4).any(|p| p[0] < 128));
    }

    #[test]
    fn gray_blit_writes_dark_pixels() {
        use crate::gray::blit_text_gray;
        use crate::text::{Anchor, BlitText};
        let mut buf = vec![255u8; 200 * 60];
        blit_text_gray(
            &mut buf,
            200,
            &BlitText { s: "10.26".into(), x: 10, y: 40, px: 33, anchor: Anchor::Start, invert: false },
        );
        assert!(buf.iter().any(|&v| v < 128));
    }

    #[test]
    fn glyph_blits_visible_pixels() {
        use crate::text::{blit_text, sprite_for, Anchor, BlitText};
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
        use crate::text::{blit_text, Anchor, BlitText};
        let mut pix = resvg::tiny_skia::Pixmap::new(200, 60).expect("pixmap");
        pix.fill(resvg::tiny_skia::Color::BLACK);
        blit_text(&mut pix, &BlitText { s: "A股".into(), x: 10, y: 40, px: 25, anchor: Anchor::Start, invert: true });
        let bright = pix.data().chunks_exact(4).filter(|p| p[0] > 200).count();
        assert!(bright > 50, "bright={bright}");
    }
}
