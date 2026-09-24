// gray: 光栅化/灰度/波形直绘/动态合成（画布操作）
use crate::config::StockData;
use crate::layout::{
    Block, CONTENT_TOP, HEADER_H, MARGIN_X, metrics, paginate, px, spark_geom, spark_ref_y,
    spark_stride, spark_xy, stock_strings,
};
use crate::text::{Anchor, BlitText, advance_for, fontset, sprite_for};
pub(crate) fn draw_line_gray(
    gray: &mut [u8],
    stride: i32,
    height: i32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: u8,
    width: i32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for wy in 0..width {
            for wx in 0..width {
                let px = x0 + wx;
                let py = y0 + wy;
                if px >= 0 && px < stride && py >= 0 && py < height {
                    let idx = (py * stride + px) as usize;
                    if idx < gray.len() {
                        gray[idx] = color;
                    }
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

// 波形灰度直绘：替代 tiny-skia tile 光栅化，直接写 1D 灰度数组，无 RGBA 中间层
pub(crate) fn draw_sparkline(
    gray: &mut [u8],
    width: i32,
    height: i32,
    item: &StockData,
    large: bool,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
) {
    let Some(g) = spark_geom(item, large) else {
        return;
    };
    if let Some(ry) = spark_ref_y(&g, sy, sh) {
        // 昨收虚线：3 on / 3 off，灰 128，对齐旧 tiny-skia dash 效果
        let (mut x, end) = (sx + 4, sx + sw - 4);
        while x < end {
            let seg = (x + 3).min(end);
            draw_line_gray(gray, width, height, x, ry, seg, ry, 128, 1);
            x += 6;
        }
    }
    let n = item.prices.len();
    let stride = spark_stride(n);
    let mut prev: Option<(i32, i32)> = None;
    for (i, &p) in item.prices.iter().enumerate() {
        if i % stride != 0 && i + 1 != n {
            continue;
        }
        let (x, y) = spark_xy(item, &g, sx, sy, sw, sh, i, p);
        if let Some((px, py)) = prev {
            draw_line_gray(gray, width, height, px, py, x, y, 0, 2);
        }
        prev = Some((x, y));
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
// 波形 Bresenham 直绘 + 文字灰度 blit，无 tile/RGBA/tiny-skia。返回 (tiles, blits)
pub(crate) fn compose_dynamic(
    gray: &mut [u8],
    data: &[StockData],
    width: i32,
    height: i32,
    view: &str,
    page: usize,
) -> (usize, usize, Vec<crate::fbink::DirtyRect>) {
    let large = crate::input::style_mode().is_large();
    let m = metrics(large, width);
    let pages = paginate(data, height, view);
    let cur = page.min(pages.len().saturating_sub(1));
    let (mut tiles, mut blits) = (0, 0);
    let mut dirties = Vec::new();
    let mut y = CONTENT_TOP;
    for b in &pages[cur] {
        match b {
            Block::Header { .. } => y += HEADER_H,
            Block::Card(item) => {
                let (sx, sy, sw, sh) = (m.sp_x, y + m.sp_dy, m.sp_w, m.sp_h);
                if item.prices.len() >= 2 {
                    draw_sparkline(gray, width, height, item, large, sx, sy, sw, sh);
                    dirties.push(crate::fbink::DirtyRect { x: sx, y: sy, w: sw, h: sh });
                    tiles += 1;
                }
                // 右侧价格区右对齐、变长：保守覆盖 spark 尾到屏右整列，新旧文本并集全在内
                let rx = sx + sw;
                dirties.push(crate::fbink::DirtyRect {
                    x: rx,
                    y,
                    w: width - rx,
                    h: m.chh,
                });
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
    // 底部状态栏右半（右对齐变长文本，并集保守覆盖）
    dirties.push(crate::fbink::DirtyRect {
        x: width / 2,
        y: height - 50,
        w: width / 2,
        h: 50,
    });
    (tiles, blits + 1, dirties)
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
// 只存 1.5MB 灰度（动态层 Bresenham 直绘，不再需要 6MB RGBA 底裁切）
pub(crate) struct BaseEntry {
    pub(crate) gray: Vec<u8>,
}

// 成员指纹：空数据→有数据（或股票增减）时 base 版式会变，必须换 key，否则复用无卡片底图
