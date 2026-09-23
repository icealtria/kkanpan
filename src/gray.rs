// gray: 光栅化/灰度/波形直绘/动态合成（画布操作）
use crate::config::StockData;
use crate::layout::{
    Block, CONTENT_TOP, HEADER_H, MARGIN_X, metrics, paginate, px, spark_geom, spark_ref_y,
    spark_stride, spark_xy, stock_strings,
};
use crate::text::{Anchor, BlitText, advance_for, fontset, sprite_for};
pub(crate) fn draw_sparkline(
    canvas: &mut resvg::tiny_skia::Pixmap,
    item: &StockData,
    large: bool,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
) {
    use resvg::tiny_skia::{Paint, PathBuilder, Stroke, StrokeDash, Transform};
    let Some(g) = spark_geom(item, large) else {
        return;
    };
    if let Some(ry) = spark_ref_y(&g, sy, sh) {
        let mut pb = PathBuilder::new();
        pb.move_to((sx + 4) as f32, ry as f32);
        pb.line_to((sx + sw - 4) as f32, ry as f32);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(128, 128, 128, 255); // #808080
            let stroke = Stroke {
                width: 1.0,
                dash: StrokeDash::new(vec![3.0, 3.0], 0.0),
                ..Default::default()
            };
            canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    let n = item.prices.len();
    let stride = spark_stride(n);
    let mut pb = PathBuilder::new();
    let mut first = true;
    for (i, &p) in item.prices.iter().enumerate() {
        if i % stride != 0 && i + 1 != n {
            continue;
        }
        let (x, y) = spark_xy(item, &g, sx, sy, sw, sh, i, p);
        if first {
            pb.move_to(x as f32, y as f32);
            first = false;
        } else {
            pb.line_to(x as f32, y as f32);
        }
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        let stroke = Stroke { width: 2.0, ..Default::default() };
        canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

// 动态层直绘（无 minijinja、无 usvg、无 String 拼接）：画波形 + blit 数字/状态
// 返回 blit 个数（日志用）
// 灰度域直接 blit（动态文字）：sprite 本就是白底灰度，价格区底色也是白，
// 逐字节写，无 RGBA 中间层、无 alpha 展开
pub(crate) fn blit_text_gray(buf: &mut [u8], stride: i32, t: &BlitText) {
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
    let h = buf.len() as i32 / stride;
    let bg: u8 = if t.invert { 0 } else { 255 };
    for (ch, adv) in t.s.chars().zip(advs) {
        let sp = sprite_for(ch, t.px);
        if !sp.px.is_empty() {
            let (x0, y0) = (pen + sp.dx, t.y - sp.dy);
            let (xa, xb) = (x0.max(0), (x0 + sp.w).min(stride));
            let (ya, yb) = (y0.max(0), (y0 + sp.h).min(h));
            for yy in ya..yb {
                let srow = (yy - y0) * sp.w;
                let drow = yy * stride;
                for xx in xa..xb {
                    let v = sp.px[(srow + (xx - x0)) as usize];
                    let w = if t.invert { 255 - v } else { v };
                    if w != bg {
                        buf[(drow + xx) as usize] = w;
                    }
                }
            }
        }
        pen += adv;
    }
}

// 灰度域合成（无全屏 RGBA 画布）：整屏只拷 1.5MB base 灰度；
// 波形逐卡切小 tile 光栅化贴回；文字直接灰度 blit。返回 (tiles, blits)
pub(crate) fn compose_dynamic(
    base_pix: &resvg::tiny_skia::Pixmap,
    gray: &mut [u8],
    data: &[StockData],
    width: i32,
    height: i32,
    view: &str,
    page: usize,
) -> (usize, usize) {
    let large = crate::input::style_mode().is_large();
    let m = metrics(large, width);
    let pages = paginate(data, height, view);
    let cur = page.min(pages.len().saturating_sub(1));
    let (mut tiles, mut blits) = (0, 0);
    TILE.with(|cell| {
        let mut slot = cell.borrow_mut();
        if !matches!(&*slot, Some(p) if p.width() == m.sp_w as u32 && p.height() == m.sp_h as u32)
        {
            *slot = Some(
                resvg::tiny_skia::Pixmap::new(m.sp_w as u32, m.sp_h as u32).expect("pixmap"),
            );
        }
        let tile = slot.as_mut().expect("pixmap");
        let mut y = CONTENT_TOP;
        for b in &pages[cur] {
            match b {
                Block::Header { .. } => y += HEADER_H,
                Block::Card(item) => {
                    let (sx, sy, sw, sh) = (m.sp_x, y + m.sp_dy, m.sp_w, m.sp_h);
                    if item.prices.len() >= 2 {
                        // base 切 tile → 相对坐标画线 → 灰度贴回（点位钳在框内 2px，线宽 1px 不外溢）
                        {
                            let td = tile.data_mut();
                            let bd = base_pix.data();
                            for r in 0..sh {
                                let s = ((sy + r) * width + sx) as usize * 4;
                                let d = (r * sw) as usize * 4;
                                td[d..d + (sw * 4) as usize]
                                    .copy_from_slice(&bd[s..s + (sw * 4) as usize]);
                            }
                        }
                        draw_sparkline(tile, item, large, 0, 0, sw, sh);
                        let td = tile.data();
                        for r in 0..sh {
                            for c in 0..sw {
                                let p = &td[((r * sw + c) * 4) as usize..];
                                let a = p[3] as u32;
                                let rr = p[0] as u32 + 255 - a;
                                let gg = p[1] as u32 + 255 - a;
                                let bb = p[2] as u32 + 255 - a;
                                gray[((sy + r) * width + sx + c) as usize] =
                                    ((rr * 77 + gg * 150 + bb * 29) >> 8) as u8;
                            }
                        }
                        tiles += 1;
                    }
                    let (price_s, chg_s) = stock_strings(item.price, item.change, item.pct);
                    blit_text_gray(
                        gray,
                        width,
                        &BlitText {
                            s: price_s,
                            x: width - 45,
                            y: y + m.pr_dy + m.pr_px,
                            px: m.pr_px,
                            anchor: Anchor::End,
                            invert: false,
                        },
                    );
                    blit_text_gray(
                        gray,
                        width,
                        &BlitText {
                            s: chg_s,
                            x: width - 45,
                            y: y + m.ch_dy + m.ch_px,
                            px: m.ch_px,
                            anchor: Anchor::End,
                            invert: false,
                        },
                    );
                    blits += 2;
                    y += m.chh;
                }
            }
        }
    });
    blit_text_gray(
        gray,
        width,
        &BlitText {
            s: crate::kindle::format_status_bar(),
            x: width - MARGIN_X,
            y: height - 24 + px(4),
            px: px(4),
            anchor: Anchor::End,
            invert: false,
        },
    );
    (tiles, blits + 1)
}

thread_local! {
    // 同尺寸重复利用：6MB Pixmap 只分配一次，后续 fill(白) 复用
    static PIXMAP: std::cell::RefCell<Option<resvg::tiny_skia::Pixmap>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn parse_tree(svg: &str) -> resvg::usvg::Tree {
    let fs = fontset();
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fs.db.clone();
    resvg::usvg::Tree::from_str(svg, &opt).expect("svg parse")
}

pub(crate) fn render_tree_into(tree: &resvg::usvg::Tree, pix: &mut resvg::tiny_skia::Pixmap) {
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
}

pub(crate) fn render_tree(tree: &resvg::usvg::Tree) {
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

// 静态层缓存：Tab/表头/股票名只与 (w,h,view,style,page) 有关，数据刷新不失效；
// pix 供波形 tile 裁切，gray 供整屏 1.5MB 快拷（代替 6MB RGBA 拷贝）
pub(crate) struct BaseEntry {
    pub(crate) pix: resvg::tiny_skia::Pixmap,
    pub(crate) gray: Vec<u8>,
}

thread_local! {
    // 波形 tile：单卡火花区大小，同 style 复用，逐卡裁切→画线→灰度贴回
    static TILE: std::cell::RefCell<Option<resvg::tiny_skia::Pixmap>> =
        const { std::cell::RefCell::new(None) };
}

// 成员指纹：空数据→有数据（或股票增减）时 base 版式会变，必须换 key，否则复用无卡片底图
