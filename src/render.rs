use crate::config::{chart_total, Quote};
use crate::font::{draw_text, text_width};
use crate::layout::{Block, Style};
use crate::quote::{ref_line, sparkline_range, stock_strings};
use crate::touch::tab_layout;
use image::{GrayImage, Luma};

// ---- 底层图元 (原 render.go drawLine/drawRect/fillRect, 直接写 buffer) ----

fn line(img: &mut GrayImage, mut x0: i32, mut y0: i32, x1: i32, y1: i32, v: u8, w: i32) {
    let (fw, fh) = (img.width() as i32, img.height() as i32);
    let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let mut err = dx - dy;
    let half = w / 2;
    loop {
        for oy in -half..=half {
            for ox in -half..=half {
                let (px, py) = (x0 + ox, y0 + oy);
                if px >= 0 && py >= 0 && px < fw && py < fh {
                    img.put_pixel(px as u32, py as u32, Luma([v]));
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn rect(img: &mut GrayImage, x: i32, y: i32, w: i32, h: i32, v: u8, border: i32) {
    let (fw, fh) = (img.width() as i32, img.height() as i32);
    for b in 0..border {
        for i in x..x + w {
            for &yy in &[y + b, y + h - 1 - b] {
                if i >= 0 && yy >= 0 && i < fw && yy < fh {
                    img.put_pixel(i as u32, yy as u32, Luma([v]));
                }
            }
        }
        for j in y..y + h {
            for &xx in &[x + b, x + w - 1 - b] {
                if xx >= 0 && j >= 0 && xx < fw && j < fh {
                    img.put_pixel(xx as u32, j as u32, Luma([v]));
                }
            }
        }
    }
}

fn fill(img: &mut GrayImage, x: i32, y: i32, w: i32, h: i32, v: u8) {
    let (fw, fh) = (img.width() as i32, img.height() as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(fw), (y + h).min(fh));
    for yy in y0..y1 {
        for xx in x0..x1 {
            img.put_pixel(xx as u32, yy as u32, Luma([v]));
        }
    }
}

fn text(img: &mut GrayImage, x: i32, y: i32, s: &str, size: u32, v: u8) {
    if x < 0 || y < 0 {
        return;
    }
    draw_text(img, x as u32, y as u32, s, size, v);
}

// ---- 共享的图表几何 (原三处复制的 min/max/ref/total 计算, 收敛到一处) ----

struct Geom {
    max: f64,
    rng: f64,
    refv: Option<f64>,
    total: usize,
    is_yahoo: bool,
}

fn geom(q: &Quote, large: bool) -> Option<Geom> {
    if q.prices.len() < 2 {
        return None;
    }
    let (mut min, mut max, mut rng) = sparkline_range(&q.prices)?;
    let is_yahoo = q.timestamps.len() == q.prices.len() && q.regular_end > q.regular_start;
    let mut refv = ref_line(q.prev, q.chart_prev_close, is_yahoo, min, max, large);
    if large {
        if let Some(r) = refv {
            if r < min {
                min = r;
            }
            if r > max {
                max = r;
            }
            rng = max - min;
            if rng == 0.0 {
                rng = 1.0;
            }
            refv = Some(r);
        }
    }
    Some(Geom { max, rng, refv, total: chart_total(&q.code, q.prices.len()), is_yahoo })
}

fn spark_x(q: &Quote, g: &Geom, gx: i32, gw: i32, i: usize) -> f64 {
    if g.is_yahoo {
        let sec = (q.regular_end - q.regular_start).max(1) as f64;
        gx as f64 + 2.0 + (q.timestamps[i] - q.regular_start) as f64 * (gw - 4) as f64 / sec
    } else {
        gx as f64 + 2.0 + i as f64 * (gw - 4) as f64 / g.total.max(1) as f64
    }
}

fn draw_spark(img: &mut GrayImage, q: &Quote, gx: i32, gy: i32, gw: i32, gh: i32, large: bool) {
    let Some(g) = geom(q, large) else { return };
    rect(img, gx, gy, gw, gh, 0, 1);
    if let Some(r) = g.refv {
        let ry = gy + 2 + ((g.max - r) * (gh - 4) as f64 / g.rng) as i32;
        let mut lx = gx + 4;
        while lx < gx + gw - 4 {
            for o in 0..2 {
                if lx + o < gx + gw - 4 && ry >= 0 && ry < img.height() as i32 {
                    img.put_pixel((lx + o) as u32, ry as u32, Luma([128]));
                }
            }
            lx += 6;
        }
    }
    for i in 0..q.prices.len() - 1 {
        let x0 = spark_x(q, &g, gx, gw, i) as i32;
        let x1 = spark_x(q, &g, gx, gw, i + 1) as i32;
        let y0 = gy + 2 + ((g.max - q.prices[i]) * (gh - 4) as f64 / g.rng) as i32;
        let y1 = gy + 2 + ((g.max - q.prices[i + 1]) * (gh - 4) as f64 / g.rng) as i32;
        line(img, x0, y0, x1, y1, 0, 2);
    }
}

// ---- 屏幕装配 ----

pub struct Screen<'a> {
    pub mode_tag: String,
    pub style: Style,
    pub tabs: Vec<String>,
    pub selected_tab: usize,
    pub blocks: &'a [Block],
    pub page: usize,
    pub pages: usize,
    pub status: String,
    pub width: u32,
    pub height: u32,
}

pub fn render_image(s: &Screen) -> GrayImage {
    let (w, h) = (s.width as i32, s.height as i32);
    let mut img = GrayImage::from_pixel(s.width, s.height, Luma([255u8]));

    text(&mut img, 30, 16, "KKANPAN", 32, 0);
    text(&mut img, w - 460, 20, &s.mode_tag, 24, 0);

    let label = s.style.label();
    rect(&mut img, w - 185, 10, 80, 46, 0, 2);
    let lw = text_width(label, 24);
    text(&mut img, w - 185 + (80 - lw as i32) / 2, 18, label, 24, 0);
    rect(&mut img, w - 95, 10, 65, 46, 0, 3);
    text(&mut img, w - 72, 18, "X", 26, 0);

    let layout = tab_layout(w, s.tabs.len());
    for (i, t) in s.tabs.iter().enumerate() {
        let (x0, x1) = layout[i];
        let bw = x1 - x0;
        let selt = i == s.selected_tab;
        let tw = text_width(t, 24) as i32;
        let padx = ((bw - tw) / 2).max(2);
        if selt {
            fill(&mut img, x0, 68, bw, 50, 0);
            text(&mut img, x0 + padx, 68 + 12, t, 24, 255);
        } else {
            rect(&mut img, x0, 68, bw, 50, 0, 2);
            text(&mut img, x0 + padx, 68 + 12, t, 24, 0);
        }
    }
    line(&mut img, 30, 128, w - 30, 128, 0, 3);

    let mut y = 142;
    for b in s.blocks {
        if b.is_header {
            fill(&mut img, 30, y, w - 60, 38, 0);
            text(&mut img, 45, y + 8, &format!("[ {} ]", b.group), 22, 255);
            y += b.h;
            continue;
        }
        let q = b.quote.as_ref().unwrap();
        let card_h = b.h - b.gap;
        rect(&mut img, 30, y, w - 60, card_h, 0, 2);
        let name = if q.name.is_empty() { &q.code } else { &q.name };
        let (price, chg) = stock_strings(q.price, q.change, q.pct, false);
        if s.style == Style::Large {
            text(&mut img, 45, y + 18, name, 32, 0);
            text(&mut img, 45, y + 62, &q.code, 20, 100);
            if q.prices.len() > 2 {
                draw_spark(&mut img, q, 210, y + 15, w - 490, card_h - 30, true);
            }
            let (pw, cw) = (text_width(&price, 38) as i32, text_width(&chg, 24) as i32);
            text(&mut img, w - 45 - pw, y + 16, &price, 38, 0);
            text(&mut img, w - 45 - cw, y + 64, &chg, 24, 0);
        } else {
            text(&mut img, 45, y + 14, name, 26, 0);
            text(&mut img, 45, y + 48, &q.code, 18, 100);
            if q.prices.len() > 2 {
                draw_spark(&mut img, q, 240, y + 12, 480, 70, false);
            }
            let (pw, cw) = (text_width(&price, 34) as i32, text_width(&chg, 20) as i32);
            text(&mut img, w - 45 - pw, y + 14, &price, 34, 0);
            text(&mut img, w - 45 - cw, y + 52, &chg, 20, 0);
        }
        y += b.h;
    }

    if s.pages > 1 {
        let mut ind = format!("{} / {}", s.page + 1, s.pages);
        if s.page > 0 {
            ind = format!("▲ {ind}");
        }
        if s.page + 1 < s.pages {
            ind = format!("{ind} ▼");
        }
        let iw = text_width(&ind, 18) as i32;
        text(&mut img, (w - iw) / 2, h - 40, &ind, 18, 80);
    }
    line(&mut img, 30, h - 58, w - 30, h - 58, 0, 1);
    text(&mut img, 30, h - 24, "Swipe H: switch Tab | Swipe V: flip | Tap [X] exit", 18, 120);
    let sw = text_width(&s.status, 18) as i32;
    text(&mut img, w - 30 - sw, h - 24, &s.status, 18, 0);
    img
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn render_svg(s: &Screen) -> String {
    use std::fmt::Write;
    let (w, h) = (s.width as i32, s.height as i32);
    let mut o = String::with_capacity(16 << 10);
    let _ = write!(
        o,
        "<svg width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" xmlns=\"http://www.w3.org/2000/svg\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/>"
    );
    let _ = write!(
        o,
        "<text x=\"30\" y=\"38\" font-family=\"monospace\" font-size=\"32\" font-weight=\"bold\">KKANPAN</text>"
    );
    let _ = write!(
        o,
        "<text x=\"{}\" y=\"38\" font-family=\"monospace\" font-size=\"24\" font-weight=\"bold\" text-anchor=\"end\">{}</text>",
        w - 270,
        esc(&s.mode_tag)
    );
    let _ = write!(
        o,
        "<a href=\"/style\"><rect x=\"{}\" y=\"10\" width=\"80\" height=\"46\" fill=\"white\" stroke=\"black\" stroke-width=\"2\"/><text x=\"{}\" y=\"34\" font-family=\"monospace\" font-size=\"20\" font-weight=\"bold\" text-anchor=\"middle\">{}</text></a>",
        w - 185,
        w - 145,
        s.style.label()
    );
    let _ = write!(
        o,
        "<a href=\"/exit\"><rect x=\"{}\" y=\"10\" width=\"65\" height=\"46\" fill=\"white\" stroke=\"black\" stroke-width=\"3\"/><text x=\"{}\" y=\"34\" font-family=\"monospace\" font-size=\"20\" font-weight=\"bold\" text-anchor=\"middle\">X</text></a>",
        w - 95,
        w - 62
    );
    let layout = tab_layout(w, s.tabs.len());
    for (i, t) in s.tabs.iter().enumerate() {
        let (x0, x1) = layout[i];
        let bw = x1 - x0;
        if i == s.selected_tab {
            let _ = write!(
                o,
                "<a href=\"/switch?view={}\"><rect x=\"{x0}\" y=\"68\" width=\"{bw}\" height=\"50\" fill=\"black\"/><text x=\"{}\" y=\"98\" font-family=\"monospace\" font-size=\"18\" fill=\"white\" text-anchor=\"middle\">{}</text></a>",
                esc(t),
                x0 + bw / 2,
                esc(t)
            );
        } else {
            let _ = write!(
                o,
                "<a href=\"/switch?view={}\"><rect x=\"{x0}\" y=\"68\" width=\"{bw}\" height=\"50\" fill=\"white\" stroke=\"black\" stroke-width=\"2\"/><text x=\"{}\" y=\"98\" font-family=\"monospace\" font-size=\"18\" fill=\"black\" text-anchor=\"middle\">{}</text></a>",
                esc(t),
                x0 + bw / 2,
                esc(t)
            );
        }
    }
    let _ = write!(o, "<line x1=\"30\" y1=\"128\" x2=\"{}\" y2=\"128\" stroke=\"black\" stroke-width=\"3\"/>", w - 30);
    let mut y = 142;
    let large = s.style == Style::Large;
    for b in s.blocks {
        if b.is_header {
            let _ = write!(
                o,
                "<rect x=\"30\" y=\"{y}\" width=\"{}\" height=\"38\" fill=\"black\"/><text x=\"45\" y=\"{}\" font-family=\"monospace\" font-size=\"22\" fill=\"white\">[ {} ]</text>",
                w - 60,
                y + 24,
                esc(&b.group)
            );
            y += b.h;
            continue;
        }
        let q = b.quote.as_ref().unwrap();
        let card_h = b.h - b.gap;
        let name = if q.name.is_empty() { &q.code } else { &q.name };
        let (price, chg) = stock_strings(q.price, q.change, q.pct, true);
        let _ = write!(
            o,
            "<rect x=\"30\" y=\"{y}\" width=\"{}\" height=\"{card_h}\" fill=\"none\" stroke=\"black\" stroke-width=\"2\"/>",
            w - 60
        );
        if large {
            let _ = write!(o, "<text x=\"45\" y=\"{}\" font-family=\"monospace\" font-size=\"28\">{}</text>", y + 30, esc(name));
            let _ = write!(o, "<text x=\"45\" y=\"{}\" font-family=\"monospace\" font-size=\"16\" fill=\"#666\">{}</text>", y + 52, esc(&q.code));
        } else {
            let _ = write!(o, "<text x=\"45\" y=\"{}\" font-family=\"monospace\" font-size=\"26\">{}</text>", y + 28, esc(name));
            let _ = write!(o, "<text x=\"45\" y=\"{}\" font-family=\"monospace\" font-size=\"18\" fill=\"#666\">{}</text>", y + 62, esc(&q.code));
        }
        if let Some(g) = geom(q, large) {
            let (gx, gy, gw, gh) = if large { (210, y + 15, w - 490, card_h - 30) } else { (240, y + 12, 480, 70) };
            let mut pts = String::with_capacity(q.prices.len() * 12);
            for (i, p) in q.prices.iter().enumerate() {
                if i > 0 {
                    pts.push(' ');
                }
                let yy = gy as f64 + 2.0 + (g.max - p) * (gh - 4) as f64 / g.rng;
                pts.push_str(&format!("{:.1},{:.1}", spark_x(q, &g, gx, gw, i), yy));
            }
            if let Some(r) = g.refv {
                let ry = gy as f64 + 2.0 + (g.max - r) * (gh - 4) as f64 / g.rng;
                let _ = write!(
                    o,
                    "<line x1=\"{}\" y1=\"{ry:.1}\" x2=\"{}\" y2=\"{ry:.1}\" stroke=\"black\" stroke-width=\"1\" stroke-dasharray=\"4,4\"/>",
                    gx + 4,
                    gx + gw - 4
                );
            }
            let _ = write!(
                o,
                "<rect x=\"{gx}\" y=\"{gy}\" width=\"{gw}\" height=\"{gh}\" fill=\"none\" stroke=\"black\" stroke-width=\"1\"/><polyline fill=\"none\" stroke=\"black\" stroke-width=\"1.5\" points=\"{pts}\"/>"
            );
        }
        if large {
            let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"28\" font-weight=\"bold\" text-anchor=\"end\">{}</text>", w - 45, y + 32, esc(&price));
            let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"18\" text-anchor=\"end\">{}</text>", w - 45, y + 58, esc(&chg));
        } else {
            let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"34\" font-weight=\"bold\" text-anchor=\"end\">{}</text>", w - 45, y + 28, esc(&price));
            let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"20\" text-anchor=\"end\">{}</text>", w - 45, y + 66, esc(&chg));
        }
        y += b.h;
    }
    if s.pages > 1 {
        let mut ind = format!("{} / {}", s.page + 1, s.pages);
        if s.page > 0 {
            ind = format!("▲ {ind}");
        }
        if s.page + 1 < s.pages {
            ind = format!("{ind} ▼");
        }
        let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"18\" fill=\"#555\" text-anchor=\"middle\">{ind}</text>", w / 2, h - 40);
    }
    let _ = write!(o, "<line x1=\"30\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"black\" stroke-width=\"1\"/>", h - 58, w - 30, h - 58);
    let _ = write!(o, "<text x=\"30\" y=\"{}\" font-family=\"monospace\" font-size=\"18\" fill=\"#888\">Swipe V: flip | Tap tabs | Tap [X] exit</text>", h - 16);
    let _ = write!(o, "<text x=\"{}\" y=\"{}\" font-family=\"monospace\" font-size=\"18\" font-weight=\"bold\" text-anchor=\"end\">{}</text>", w - 30, h - 16, esc(&s.status));
    o.push_str("</svg>");
    o
}
