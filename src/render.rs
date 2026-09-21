use crate::config::{self, StockData};
use minijinja::Environment;
use serde::Serialize;
use std::sync::OnceLock;

// ---- 版式常量（对齐 layout.go，单位=屏像素；字号 pt→px 按 300DPI 换算） ----
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

// ---- 分页（对齐 pagination.go） ----

enum Block {
    Header { group: String },
    Card(StockData),
}

fn card_h(style: &str) -> i32 {
    if style == "large" { LARGE_CARD_H } else { NORMAL_CARD_H }
}

fn build_blocks(data: &[StockData], eff: &str, is_auto: bool) -> Vec<(String, Vec<StockData>)> {
    // (group, items) 有序分组
    let mut groups: Vec<(String, Vec<StockData>)> = vec![];
    for d in data {
        match groups.iter_mut().find(|(g, _)| g == &d.group) {
            Some((_, v)) => v.push(d.clone()),
            None => groups.push((d.group.clone(), vec![d.clone()])),
        }
    }
    let pick = |name: &str| groups.iter().find(|(g, _)| g == name).cloned();
    if eff == "ALL" {
        return config::all_groups().into_iter().filter_map(|g| pick(&g)).collect();
    }
    if is_auto {
        return config::matching_auto_groups().into_iter().filter_map(|g| pick(&g)).collect();
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

// ---- 模板上下文 ----

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
    // header
    bar_y: i32,
    // card
    card_y: i32,
    card_h: i32,
    name_l1: String,
    name_l2: String,
    code: String,
    spark: String,       // polyline points，空=不画
    ref_y: Option<i32>,  // 昨收虚线 y，None=不画
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
    let price_str = if price > 0.0 { format!("{price:.2}") } else { "--".to_string() };
    let arrow = if change > 0.0 { "▲" } else if change < 0.0 { "▼" } else { " " };
    let sign_c = if change >= 0.0 { "+" } else { "" };
    let sign_p = if pct >= 0.0 { "+" } else { "" };
    (price_str, format!("{arrow} {sign_c}{change:.2} ({sign_p}{pct:.2}%)"))
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
    if mx == mn { (mn, mn + 1.0) } else { (mn, mx) }
}

// 用展示字库的真实 advance 量文本宽度（字符数启发式对数字/拉丁误差近一倍）
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

fn spark_points(item: &StockData, large: bool, sx: i32, sy: i32, sw: i32, sh: i32) -> (String, Option<i32>) {
    let prices = &item.prices;
    if prices.len() < 2 {
        return (String::new(), None);
    }
    let (mut mn, mut mx) = sparkline_range(prices);
    let is_yahoo = item.timestamps.len() == prices.len() && item.regular_end > item.regular_start;
    let mut reference = if is_yahoo { item.chart_prev_close } else { item.prev };
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
    let mut pts = String::new();
    for (i, &p) in prices.iter().enumerate() {
        let x = if is_yahoo && item.timestamps.len() == prices.len() {
            let total = (item.regular_end - item.regular_start).max(1) as f64;
            sx + 2 + ((item.timestamps[i] - item.regular_start) as f64 * (sw - 4) as f64 / total) as i32
        } else {
            let total = config::chart_total(&item.code, prices.len()).max(1) as f64;
            sx + 2 + (i as f64 * (sw - 4) as f64 / total) as i32
        };
        let y = sy + 2 + ((mx - p) * (sh - 4) as f64 / rng) as i32;
        if i > 0 {
            pts.push(' ');
        }
        pts.push_str(&format!("{x},{y}"));
    }
    (pts, ref_y)
}

// ---- 渲染 ----

static TPL: OnceLock<Environment<'static>> = OnceLock::new();

fn env() -> &'static Environment<'static> {
    TPL.get_or_init(|| {
        let mut e = Environment::new();
        e.add_template("screen", include_str!("../templates/screen.svg")).unwrap();
        e
    })
}

pub fn render_svg(data: &[StockData], width: i32, height: i32, view: &str, page: usize) -> String {
    let style = crate::input::style_mode();
    let large = style == "large";
    let (eff, is_auto) = config::effective_group(view);
    let mode_tag = if is_auto { format!("[AUTO: {eff}]") } else { format!("[{view}]") };

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

    let pages = paginate(data, height, view);
    let total = pages.len();
    let cur = page.min(total.saturating_sub(1));

    let (name_dy, name_px, code_dy, code_px, sp_x, sp_dy, sp_w, sp_h, pr_dy, pr_px, ch_dy, ch_px, chh) =
        if large {
            (18, px(8), 62, px(5), 210, 15, width - 490, LARGE_CARD_H - 30, 16, px(9), 64, px(6), LARGE_CARD_H)
        } else {
            (14, px(6), 48, px(4), 240, 20, 480, 63, 14, px(8), 52, px(5), NORMAL_CARD_H)
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
            }
            Block::Card(item) => {
                let name = if item.name.is_empty() { item.code.clone() } else { item.name.clone() };
                let max_w = sp_x - (MARGIN_X + 15) - 5;
                let (l1, l2) = split_name(&name, name_px, max_w);
                // 两行时 code 顶到第二行下面：name 基线 + 一行 + 行隙（对齐 Go 版 codeY 公式）
                let code_y = if l2.is_empty() { y + code_dy } else { y + name_dy + 2 * name_px + name_px / 3 };
                let (pts, ref_y) = spark_points(item, large, sp_x, y + sp_dy, sp_w, sp_h);
                let (price_s, chg_s) = stock_strings(item.price, item.change, item.pct);
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

    let ctx = Screen {
        w: width,
        h: height,
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
        status_text: crate::kindle::format_status_bar(),
        footer_hint: "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit",
    };
    env().get_template("screen").unwrap().render(&ctx).unwrap()
}

// ---- 光栅化：SVG → 灰度（FBInk 要 Y 灰度，Kindle 上 ignore_alpha 更快） ----

static FONTDB: OnceLock<FontSet> = OnceLock::new();

struct FontSet {
    db: std::sync::Arc<resvg::usvg::fontdb::Database>,
    // 明确指定覆盖 CJK 的 family，避免 sans-serif 命中纯西文字库导致汉字消失
    family: String,
}

fn is_font_file(p: &std::path::Path) -> bool {
    matches!(
        p.extension().and_then(|x| x.to_str()).map(|s| s.to_lowercase()).as_deref(),
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
        if p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("._") || n.starts_with('.')).unwrap_or(true) {
            continue;
        }
        if !is_font_file(&p) {
            continue;
        }
        if to_memory {
            // 用户字库一次性读进 RAM：usvg 给每个 <text> shaping 都要取字库数据，
            // A9 上反复 mmap/读文件是 parse 慢的主因之一
            match std::fs::read(&p) {
                Ok(data) => {
                    crate::dlog!("[font] memory-loaded {} ({}KB)", p.display(), data.len() / 1024);
                    db.load_font_source(resvg::usvg::fontdb::Source::Binary(std::sync::Arc::new(data)));
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
    FONTDB
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            // 插件自带优先
            for p in ["/mnt/us/extensions/kkanpan/font.ttf", "/mnt/us/extensions/kkanpan/font.otf", "font.ttf", "font.otf"] {
                if std::path::Path::new(p).exists() {
                    db.load_font_file(p).ok();
                }
            }
            // 递归扫：固件/用户字库可能随版本搬家，按目录扫比枚举文件名可靠
            // 用户字库进 RAM，固件字库走文件（量大，STHeiti 等 Apple 字库表结构紧凑，取数快）
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
                f.families.first().map(|(s, _)| s.clone()).unwrap_or_default()
            };
            let exact = ["STHeitiMedium", "STSongMedium", "STHeiti", "STSong"];
            let face = exact.iter().find_map(|w| faces.iter().find(|f| &fam_of(f) == w));
            let pick = |keys: &[&str]| {
                faces.iter().find(|f| {
                    let n = f.families.first().map(|(s, _)| s.to_lowercase()).unwrap_or_default();
                    keys.iter().any(|k| n.contains(k))
                })
            };
            let face = face
                .or_else(|| pick(&["stheiti", "stsong", "song", "kai", "hei", "noto", "droid", "wenquan", "uming", "ukai", "cjk", "pingfang", "han"]))
                .or_else(|| faces.first());
            let family = face
                .and_then(|f| f.families.first().map(|(s, _)| s.clone()))
                .unwrap_or_else(|| "sans-serif".to_string());
            eprintln!("[font] using family: {family}");
            FontSet { db: std::sync::Arc::new(db), family }
        })
}

pub fn render_pixmap(svg: &str) -> resvg::tiny_skia::Pixmap {
    let fs = fontset();
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fs.db.clone();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("svg parse");
    let size = tree.size();
    let (w, h) = (size.width() as u32, size.height() as u32);
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap");
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pix.as_mut());
    pix
}

pub fn pixmap_to_gray(pix: &resvg::tiny_skia::Pixmap) -> Vec<u8> {
    // 白底 premultiplied 展开：out = src + 255 - a，精确无损；
    // 亮度系数和 256，用位移代替除法；全程无分支，LLVM 可向量化
    pix.data()
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as u32;
            let r = p[0] as u32 + 255 - a;
            let g = p[1] as u32 + 255 - a;
            let b = p[2] as u32 + 255 - a;
            ((r * 77 + g * 150 + b * 29) >> 8) as u8
        })
        .collect()
}

pub fn render_gray_page(data: &[StockData], width: i32, height: i32, view: &str, page: usize) -> Vec<u8> {
    let t0 = std::time::Instant::now();
    let svg = render_svg(data, width, height, view, page);
    let t_tpl = t0.elapsed();
    let fs = fontset();
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fs.db.clone();
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).expect("svg parse");
    let t_parse = t0.elapsed() - t_tpl;
    let size = tree.size();
    let mut pix = resvg::tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32).expect("pixmap");
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pix.as_mut());
    let t_raster = t0.elapsed() - t_tpl - t_parse;
    let gray = pixmap_to_gray(&pix);
    let t_gray = t0.elapsed() - t_tpl - t_parse - t_raster;
    let texts = svg.matches("<text").count();
    crate::dlog!(
        "[render] page {page}: tpl={}ms parse={}ms raster={}ms gray={}ms total={}ms (svg {}B, {texts} texts)",
        t_tpl.as_millis(), t_parse.as_millis(), t_raster.as_millis(),
        t_gray.as_millis(), t0.elapsed().as_millis(), svg.len(),
    );
    gray
}

// 全页预渲染：数据更新时一次做完，点按翻页只走缓存
pub fn render_all_pages(data: &[StockData], width: i32, height: i32, view: &str) -> Vec<Vec<u8>> {
    let total = total_pages(data, height, view).max(1);
    (0..total).map(|p| render_gray_page(data, width, height, view, p)).collect()
}

pub fn render_gray(data: &[StockData], width: i32, height: i32, view: &str) -> Vec<u8> {
    let total = total_pages(data, height, view).max(1);
    render_gray_page(data, width, height, view, crate::input::clamp_page(total))
}
