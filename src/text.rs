// text: 字体库/sprite/排字/blit（字形缓存）
use crate::config;
use crate::gray::{parse_tree, render_tree_into};
use crate::layout::{MARGIN_X, px};
use std::sync::OnceLock;
pub(crate) fn char_widths(text: &str, font_px: i32) -> Vec<i32> {
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

pub(crate) fn split_name(name: &str, font_px: i32, max_w: i32) -> (String, String) {
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

pub(crate) fn split_name_uncached(name: &str, font_px: i32, max_w: i32) -> (String, String) {
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
#[derive(Clone)]
pub(crate) struct Sprite {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) dx: i32,
    pub(crate) dy: i32,
    pub(crate) px: Vec<u8>, // 白底灰度，255=透明可跳
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Anchor {
    Start,
    Middle,
    End,
}

pub(crate) struct BlitText {
    pub(crate) s: String,
    pub(crate) x: i32,
    pub(crate) y: i32, // 基线（对齐 SVG text 的 y）
    pub(crate) px: i32,
    pub(crate) anchor: Anchor,
    pub(crate) invert: bool, // 白字黑底（表头条/选中 Tab）：blit 时 255-v 反色，与直接渲染等价
}

pub(crate) static SPRITES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<(char, i32), Sprite>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
pub(crate) static ADVS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<(char, i32), i32>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub(crate) fn advance_for(ch: char, px: i32) -> i32 {
    *ADVS
        .lock()
        .unwrap()
        .entry((ch, px))
        .or_insert_with(|| char_widths(&ch.to_string(), px).into_iter().next().unwrap_or(px / 2))
}

pub(crate) fn sprite_for(ch: char, px: i32) -> Sprite {
    SPRITES
        .lock()
        .unwrap()
        .entry((ch, px))
        .or_insert_with(|| render_glyph(ch, px))
        .clone()
}

pub(crate) fn render_glyph(ch: char, px: i32) -> Sprite {
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

pub(crate) fn blit_text(canvas: &mut resvg::tiny_skia::Pixmap, t: &BlitText) {
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

pub(crate) static FONTDB: OnceLock<FontSet> = OnceLock::new();

pub(crate) struct FontSet {
    pub(crate) db: std::sync::Arc<resvg::usvg::fontdb::Database>,
    // 明确指定覆盖 CJK 的 family，避免 sans-serif 命中纯西文字库导致汉字消失
    pub(crate) family: String,
}

pub(crate) fn is_font_file(p: &std::path::Path) -> bool {
    matches!(
        p.extension()
            .and_then(|x| x.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("ttf" | "otf" | "ttc")
    )
}

pub(crate) fn scan_fonts(db: &mut resvg::usvg::fontdb::Database, dir: &str, depth: usize, to_memory: bool) {
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

pub(crate) fn fontset() -> &'static FontSet {
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
