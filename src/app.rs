use crate::auto::{cst_now, effective_group, matching_groups};
use crate::config::{all_groups, default_view, tab_modes, AppConfig, Quote, StockConfig};
use crate::fetch::Store;
use crate::kindle::{Eips, SysCache};
use crate::layout::{build_blocks, paginate, Style};
use crate::render::{render_image, render_svg, Screen};
use image::GrayImage;
use std::sync::{mpsc, Mutex};

/// 主循环事件: 二合一 channel, view/style/page 触发 Kick, /exit 与 [X] 触发 Quit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Kick,
    Quit,
}

/// 全局可变状态的替代品: 所有状态收敛到一个 Arc<App>,
/// 原 Go 版是 10+ 个包级全局 + 互斥锁, 这里一次看清.
pub struct App {
    pub cfg: AppConfig,
    pub stocks: Vec<StockConfig>,
    pub store: Store,
    pub width: u32,
    pub height: u32,
    view: Mutex<String>,
    style: Mutex<Style>,
    page: Mutex<usize>,
    last: Mutex<Vec<Quote>>,
    eips: Mutex<Eips>,
    sys: Mutex<SysCache>,
    kick_tx: mpsc::SyncSender<Event>,
    touch_file: Mutex<Option<std::fs::File>>,
    touch_on: std::sync::atomic::AtomicBool,
}

impl App {
    pub fn new(
        cfg: AppConfig,
        stocks: Vec<StockConfig>,
        width: u32,
        height: u32,
        kick_tx: mpsc::SyncSender<Event>,
    ) -> Self {
        let ttl = cfg.cache_ttl;
        let proxy = cfg.proxy.clone();
        let view = default_view(&cfg, &stocks);
        Self {
            store: Store::new(stocks.clone(), proxy, ttl),
            cfg,
            stocks,
            width,
            height,
            view: Mutex::new(view),
            style: Mutex::new(Style::Normal),
            page: Mutex::new(0),
            last: Mutex::new(vec![]),
            eips: Mutex::new(Eips::new()),
            sys: Mutex::new(SysCache::new()),
            kick_tx,
            touch_file: Mutex::new(None),
            touch_on: std::sync::atomic::AtomicBool::new(true),
        }
    }

    // ---- view / style / page ----

    pub fn view(&self) -> String {
        self.view.lock().unwrap().clone()
    }

    pub fn set_view(&self, v: &str) {
        *self.view.lock().unwrap() = v.to_string();
        *self.page.lock().unwrap() = 0;
        self.eips.lock().unwrap().clear_cache();
        let _ = self.kick_tx.try_send(Event::Kick);
    }

    pub fn style(&self) -> Style {
        *self.style.lock().unwrap()
    }

    pub fn set_style(&self, s: Style) {
        *self.style.lock().unwrap() = s;
        *self.page.lock().unwrap() = 0;
        self.eips.lock().unwrap().clear_cache();
        let _ = self.kick_tx.try_send(Event::Kick);
    }

    pub fn next_style(&self) -> Style {
        let n = match self.style() {
            Style::Normal => Style::Large,
            Style::Large => Style::Normal,
        };
        self.set_style(n);
        n
    }

    pub fn tabs(&self) -> Vec<String> {
        tab_modes(&self.stocks, &self.cfg.auto_rules)
    }

    pub fn selected_tab(&self) -> usize {
        let v = self.view();
        self.tabs().iter().position(|t| t == &v).unwrap_or(0)
    }

    pub fn mode_tag(&self) -> String {
        let v = self.view();
        let (eff, auto) = effective_group(&v, &self.cfg.auto_rules, &cst_now());
        if auto {
            format!("[AUTO: {eff}]")
        } else {
            format!("[{v}]")
        }
    }

    /// (effective_group, is_auto, matching_groups)
    pub fn resolved(&self) -> (String, bool, Vec<String>) {
        let now = cst_now();
        let v = self.view();
        let (eff, auto) = effective_group(&v, &self.cfg.auto_rules, &now);
        let matching = matching_groups(&self.cfg.auto_rules, &now);
        (eff, auto, matching)
    }

    pub fn data(&self) -> Vec<Quote> {
        let (eff, auto, matching) = self.resolved();
        self.store.get(&eff, auto, &matching)
    }

    pub fn refresh(&self) -> Vec<Quote> {
        let (eff, auto, matching) = self.resolved();
        self.store.refresh(&eff, auto, &matching)
    }

    pub fn set_last(&self, q: &[Quote]) {
        *self.last.lock().unwrap() = q.to_vec();
    }

    pub fn last(&self) -> Vec<Quote> {
        self.last.lock().unwrap().clone()
    }

    pub fn total_pages(&self, quotes: &[Quote]) -> usize {
        let (eff, auto, matching) = self.resolved();
        let order = all_groups(&self.stocks);
        let blocks = build_blocks(quotes, &order, &eff, auto, &matching, self.style());
        paginate(&blocks, self.height as i32).len().max(1)
    }

    pub fn next_page(&self, total: usize) -> bool {
        let mut p = self.page.lock().unwrap();
        if *p + 1 < total {
            *p += 1;
            self.eips.lock().unwrap().clear_cache();
            let _ = self.kick_tx.try_send(Event::Kick);
            true
        } else {
            false
        }
    }

    pub fn prev_page(&self) -> bool {
        let mut p = self.page.lock().unwrap();
        if *p > 0 {
            *p -= 1;
            self.eips.lock().unwrap().clear_cache();
            let _ = self.kick_tx.try_send(Event::Kick);
            true
        } else {
            false
        }
    }

    pub fn status(&self) -> String {
        self.sys.lock().unwrap().get().status()
    }

    /// 渲染当前页 eink 图. 返回 (图, 页码, 总页数).
    pub fn render_current(&self, quotes: &[Quote]) -> (GrayImage, usize, usize) {
        let (eff, auto, matching) = self.resolved();
        let order = all_groups(&self.stocks);
        let style = self.style();
        let blocks = build_blocks(quotes, &order, &eff, auto, &matching, style);
        let pages = paginate(&blocks, self.height as i32);
        let total = pages.len().max(1);
        let mut p = self.page.lock().unwrap();
        if *p >= total {
            *p = total - 1;
        }
        let idx = *p;
        drop(p);
        let (a, b) = pages[idx.min(pages.len() - 1)];
        let screen = Screen {
            mode_tag: self.mode_tag(),
            style,
            tabs: self.tabs(),
            selected_tab: self.selected_tab(),
            blocks: &blocks[a..b],
            page: idx,
            pages: total,
            status: self.status(),
            width: self.width,
            height: self.height,
        };
        (render_image(&screen), idx, total)
    }

    pub fn render_svg_current(&self, quotes: &[Quote]) -> String {
        let (eff, auto, matching) = self.resolved();
        let order = all_groups(&self.stocks);
        let style = self.style();
        let blocks = build_blocks(quotes, &order, &eff, auto, &matching, style);
        let pages = paginate(&blocks, self.height as i32);
        let total = pages.len().max(1);
        let mut p = self.page.lock().unwrap();
        if *p >= total {
            *p = total - 1;
        }
        let idx = *p;
        drop(p);
        let (a, b) = pages[idx.min(pages.len() - 1)];
        let screen = Screen {
            mode_tag: self.mode_tag(),
            style,
            tabs: self.tabs(),
            selected_tab: self.selected_tab(),
            blocks: &blocks[a..b],
            page: idx,
            pages: total,
            status: self.status(),
            width: self.width,
            height: self.height,
        };
        render_svg(&screen)
    }

    /// 渲染 + 推屏 + 记 last. 主循环与 server 共用.
    pub fn show(&self, quotes: &[Quote], full: bool) {
        let (img, _, _) = self.render_current(quotes);
        self.eips.lock().unwrap().update(&img, full);
        self.set_last(quotes);
    }

    pub fn clear_diff(&self) {
        self.eips.lock().unwrap().clear_cache();
    }

    pub fn kick(&self) {
        let _ = self.kick_tx.try_send(Event::Kick);
    }

    pub fn quit(&self) {
        let _ = self.kick_tx.try_send(Event::Quit);
    }

    // ---- 触控设备共享 (电源键线程需要对 grab 时的 fd 发 ioctl) ----

    pub fn set_touch_file(&self, f: std::fs::File) {
        *self.touch_file.lock().unwrap() = Some(f);
    }

    pub fn grab_touch(&self, on: bool) {
        use std::os::fd::AsRawFd;
        if let Some(f) = self.touch_file.lock().unwrap().as_ref() {
            unsafe {
                libc::ioctl(f.as_raw_fd(), 0x40044590 as _, on as i32);
            }
        }
        self.touch_on.store(on, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn touch_on(&self) -> bool {
        self.touch_on.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn toggle_touch(&self) {
        self.grab_touch(!self.touch_on());
    }

    /// 数据无变化则跳过 render (原 main.go dataEqual, 阈值 1e-9).
    pub fn same_quotes(a: &[Quote], b: &[Quote]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b).all(|(x, y)| {
            x.code == y.code
                && (x.price - y.price).abs() <= 1e-9
                && (x.change - y.change).abs() <= 1e-9
                && (x.pct - y.pct).abs() <= 1e-9
        })
    }
}
