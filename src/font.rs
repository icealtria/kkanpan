use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::GrayImage;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static FONT_DATA: OnceLock<Vec<u8>> = OnceLock::new();
static WIDTH_CACHE: OnceLock<Mutex<HashMap<(String, u32), u32>>> = OnceLock::new();

fn width_cache() -> &'static Mutex<HashMap<(String, u32), u32>> {
    WIDTH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn font_data() -> Option<&'static [u8]> {
    let d = FONT_DATA.get_or_init(|| {
        let mut cands: Vec<String> = [
            "/mnt/us/extensions/kkanpan/font.ttf",
            "/mnt/us/extensions/kkanpan/font.otf",
            "font.ttf",
            "font.otf",
            "/mnt/us/fonts/Kindle_Hei.ttf",
            "/usr/java/lib/fonts/Kindle_Hei.ttf",
            "/usr/java/lib/fonts/HYGothic.ttf",
            // ttc 需 collection 解析 (ab_glyph 不支持), 只列 ttf/otf
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if let Ok(entries) = std::fs::read_dir("/mnt/us/fonts") {
            let mut extra = vec![];
            for e in entries.flatten() {
                let p = e.path();
                if matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("ttf" | "otf" | "ttc")
                ) {
                    extra.push(p.to_string_lossy().to_string());
                }
            }
            extra.extend(cands);
            cands = extra;
        }
        for p in &cands {
            if let Ok(d) = std::fs::read(p) {
                if !d.is_empty() && FontRef::try_from_slice(&d).is_ok() {
                    eprintln!("[font] loaded {p}");
                    return d;
                }
            }
        }
        eprintln!("[font] no TTF found, text will be skipped");
        vec![]
    });
    if d.is_empty() { None } else { Some(d) }
}

fn with_font<T>(f: impl FnOnce(&FontRef) -> T) -> Option<T> {
    font_data().and_then(|d| FontRef::try_from_slice(d).ok().map(|fr| f(&fr)))
}

/// 灰度图上画文本. v: 0=黑 255=白. 返回 advances 像素宽.
pub fn draw_text(img: &mut GrayImage, x: u32, y: u32, text: &str, size: u32, v: u8) -> u32 {
    let Some(w) = with_font(|font| {
        let px = size as f32;
        let scaled = font.as_scaled(PxScale::from(px));
        let ascent = scaled.ascent();
        let (fw, fh) = (img.width(), img.height());
        let mut cx = x as f32;
        for ch in text.chars() {
            let id = font.glyph_id(ch);
            let mut glyph = scaled.scaled_glyph(ch);
            glyph.position = ab_glyph::point(cx, y as f32 + ascent);
            if let Some(out) = font.outline_glyph(glyph) {
                let (ox, oy) = (out.px_bounds().min.x as i32, out.px_bounds().min.y as i32);
                out.draw(|gx, gy, c| {
                    if c > 0.5 {
                        let (px_, py_) = (ox + gx as i32, oy + gy as i32);
                        if px_ >= 0 && py_ >= 0 && px_ < fw as i32 && py_ < fh as i32 {
                            img.put_pixel(px_ as u32, py_ as u32, image::Luma([v]));
                        }
                    }
                });
            }
            cx += scaled.h_advance(id);
        }
        (cx - x as f32).max(0.0) as u32
    }) else {
        return 0;
    };
    w
}

pub fn text_width(text: &str, size: u32) -> u32 {
    {
        let c = width_cache().lock().unwrap();
        if let Some(&w) = c.get(&(text.to_string(), size)) {
            return w;
        }
    }
    let w = with_font(|font| {
        let scaled = font.as_scaled(PxScale::from(size as f32));
        text.chars().map(|ch| scaled.h_advance(font.glyph_id(ch))).sum::<f32>() as u32
    })
    .unwrap_or((text.chars().count() as u32) * size * 3 / 5);
    width_cache().lock().unwrap().insert((text.to_string(), size), w);
    w
}
