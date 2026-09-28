mod config;
mod fbink;
mod fetch;
mod gray;
mod input;
mod kindle;
mod layout;
mod render;
mod server;
mod text;
mod util;

fn data_changed(a: &[config::StockData], b: &[config::StockData]) -> bool {
    if a.len() != b.len() {
        return true;
    }
    a.iter().zip(b.iter()).any(|(x, y)| {
        x.code != y.code
            || (x.price - y.price).abs() > 1e-9
            || (x.change - y.change).abs() > 1e-9
            || (x.pct - y.pct).abs() > 1e-9
            || (x.prev - y.prev).abs() > 1e-9
            || (x.chart_prev_close - y.chart_prev_close).abs() > 1e-9
            || x.regular_start != y.regular_start
            || x.regular_end != y.regular_end
            || x.prices != y.prices
            || x.timestamps != y.timestamps
    })
}

fn arg_val(args: &[String], name: &str, def: &str) -> String {
    args.windows(2)
        .find_map(|w| (w[0] == name).then(|| w[1].clone()))
        .or_else(|| {
            args.iter().find_map(|a| {
                a.strip_prefix(&format!("{name}=")).map(|v| v.to_string())
            })
        })
        .unwrap_or_else(|| def.to_string())
}

fn page_cache_key(view: &str, style: &str, page: usize, ver: u64) -> String {
    // 只与数据版本号绑定：后台空轮询（时间戳变、数据不变）不再使缓存失效，避免无效重绘与 pool 泄漏
    format!("{view}|{style}|{}|{page}|{ver}", crate::input::touch_enabled())
}

fn do_update(
    disp: &fbink::Display,
    res: &render::RenderResult,
    w: i32,
    h: i32,
    do_full: bool,
    screen_switched: bool,
) {
    // 底图失效（首刷/切 Tab/翻页/成员变化）或强刷 → 整屏；仅数据变动 → 解析式脏矩形局部推
    if do_full || screen_switched || res.dirties.is_none() {
        disp.write_gray(&res.gray, w, h, do_full).unwrap();
        return;
    }
    let rects = fbink::merge_rects(res.dirties.as_ref().unwrap().clone(), 15);
    let total = (w * h) as f64;
    let dirty: f64 = rects.iter().map(|r| (r.w * r.h) as f64).sum();
    crate::dlog!("[diff] {} rects, {:.1}% changed", rects.len(), dirty / total * 100.0);
    // 碎片多时逐个 ioctl 发几百轮 e-ink 刷新，不如一次整屏 GL16（低闪，比 DU 干净得多）
    if dirty / total > 0.80 || rects.len() > 15 {
        disp.write_gray_mode(&res.gray, w, h, fbink::WFM_GL16, false).unwrap();
    } else if disp.write_gray_partial(&res.gray, w, &rects).is_err() {
        disp.write_gray(&res.gray, w, h, false).unwrap();
    }
}

fn screen_id(view: &str, page: usize) -> String {
    format!("{view}|{}|{page}", crate::input::style_mode().as_str())
}

fn main() {
    // launch.sh / run_on_kindle.sh 用单横线（-once），统一归一成双横线
    let args: Vec<String> = std::env::args().skip(1).map(|a| {
        if let Some(rest) = a.strip_prefix('-') {
            if !rest.starts_with('-') && rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
                return format!("--{rest}");
            }
        }
        a
    }).collect();
    let args: Vec<String> = std::iter::once(String::new()).chain(args).collect();
    let has = |n: &str| args.iter().any(|a| a == n);
    let port: u16 = arg_val(&args, "--port", "8000").parse().unwrap_or(8000);
    let host = arg_val(&args, "--host", "0.0.0.0");
    let interval: u64 = arg_val(&args, "--interval", "60").parse().unwrap_or(60);
    let width: i32 = arg_val(&args, "--width", "1072").parse().unwrap_or(1072);
    let height: i32 = arg_val(&args, "--height", "1448").parse().unwrap_or(1448);
    let once = has("--once");
    let web = has("--http");
    let init_view_arg = arg_val(&args, "--view", "");

    let _ = config::app(); // 预加载，缺文件早 panic
    let view0 = if init_view_arg.is_empty() { config::default_view() } else { init_view_arg };
    input::init_state(&view0);
    let trigger = input::init_trigger();
    eprintln!("Starting kkanpan (view={view0})...");
    // 编译期自检：RUSTFLAGS 的 +neon 是否透过 zigbuild 生效，gray 快慢全看它
    eprintln!("[simd] neon={}", cfg!(target_feature = "neon"));

    kindle::disable_coexist_mode();
    if config::app().dim_frontlight {
        kindle::save_and_turn_off_frontlight();
    }

    let disp = fbink::Display::open().expect("display init");

    // 字体磁盘加载与首轮网络 fetch 并行：隐藏预热延迟，首屏即缓存命中
    let warmer = std::thread::spawn(text::precache_names);
    let data = fetch::refresh_data(&view0);
    warmer.join().ok();
    config::update_data_refresh_time();
    eprintln!("Fetched {} stocks", data.len());

    if once {
        let res = render::render_gray(&data, width, height, &view0);
        do_update(&disp, &res, width, height, true, true);
        return;
    }

    if web {
        std::thread::spawn(move || server::serve(&host, port, server::Ctx { width, height }));
    }
    input::start_touch_listener(width, height);
    input::start_power_listener();

    // 优雅退出：直接 kill 会丢下共存模式/背光不恢复
    let term = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        if signal_hook::flag::register(sig, term.clone()).is_err() {
            eprintln!("Failed to register signal handler for {sig}");
        }
    }

    let view = input::view();
    let cur = input::clamp_page(layout::total_pages(&data, height, &view).max(1));
    let res = render::render_gray_page(&data, width, height, &view, cur);
    let mut last_screen = screen_id(&view, cur);
    do_update(&disp, &res, width, height, true, true);
    let mut last = data;
    let mut flash_count = 0usize;
    let every = config::app().full_flash_every.max(1) as usize;
    let mut data_ver = 1u64;

    // 懒加载分页缓存：只存单页，用户翻到哪页才渲染哪页
    // 同 key 重复触发（屏上已是该页）直接跳过，代替旧 differ 的空比对
    let mut pool: std::collections::HashMap<String, render::RenderResult> =
        std::collections::HashMap::new();
    let mut last_pushed =
        page_cache_key(&view, input::style_mode().as_str(), cur, data_ver);
    pool.insert(last_pushed.clone(), res);

    // 后台预热其它 Tab 的数据：切 Tab 只读 cached_snapshot，冷 Tab 会在 UI 线程上
    // 同步 refresh_data（数秒网络+TLC），提前拉好则切 Tab 只剩渲染+推送
    {
        let warm_views: Vec<String> = config::tab_modes()
            .into_iter()
            .filter(|v| fetch::cached_snapshot(v).is_empty())
            .collect();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            for v in warm_views {
                if !fetch::cached_snapshot(&v).is_empty() {
                    continue; // 用户已切过，数据有了
                }
                fetch::refresh_data(&v);
            }
        });
    }

    // 1 秒粒度轮询：既响应信号，又不打乱 interval 节拍
    let mut last_tick = std::time::Instant::now();
    loop {
        if term.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Signal received, restoring Kindle state...");
            break;
        }
        match trigger.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(full) => {
                // 点按只用内存里的缓存数据，不发网络请求（网络只在后台定时刷新）。
                // 切视图/风格本来就全闪；翻页等局部刷新累计 flash_count，
                // 每 every 次全闪一轮，清掉墨水屏残影。
                let t_sw = std::time::Instant::now();
                let view = input::view();
                let t_fetch0 = std::time::Instant::now();
                last = fetch::cached_snapshot(&view);
                let mut cold = false;
                if last.is_empty() {
                    // 冷 Tab：UI 线程同步走网络（数秒），正常只发生一次（后台有预热兜底）
                    cold = true;
                    last = fetch::get_data(&view); // 仅该 View 从未拉取过时 fallback
                }
                let t_fetch = t_fetch0.elapsed();
                let t_pages0 = std::time::Instant::now();
                let total = layout::total_pages(&last, height, &view).max(1);
                let cur = input::clamp_page(total);
                let t_pages = t_pages0.elapsed();
                let key = page_cache_key(&view, input::style_mode().as_str(), cur, data_ver);
                let t_render0 = std::time::Instant::now();
                let pool_hit = pool.contains_key(&key);
                if !pool_hit {
                    pool.insert(key.clone(), render::render_gray_page(&last, width, height, &view, cur));
                }
                let t_render = t_render0.elapsed();
                let do_full = if full {
                    true
                } else {
                    flash_count += 1;
                    flash_count % every == 0
                };
                let mut pushed_ms = 0u128;
                if !do_full && key == last_pushed {
                    crate::dlog!("[diff] No changes, skip update");
                } else {
                    let switched = screen_id(&view, cur) != last_screen;
                    last_screen = screen_id(&view, cur);
                    let t_push0 = std::time::Instant::now();
                    do_update(&disp, &pool[&key], width, height, do_full, switched);
                    pushed_ms = t_push0.elapsed().as_millis();
                    last_pushed = key;
                }
                // 常开一行：dlog 默认关闭时也能看到切 Tab 各环节耗时
                eprintln!(
                    "[switch] view={view} page={cur} {}{}fetch={}ms pages={}ms render={}ms({}) push={}ms total={}ms",
                    if full { "full " } else { "" },
                    if cold { "cold " } else { "" },
                    t_fetch.as_millis(),
                    t_pages.as_millis(),
                    t_render.as_millis(),
                    if pool_hit { "pool-hit" } else { "pool-miss" },
                    pushed_ms,
                    t_sw.elapsed().as_millis(),
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_tick.elapsed() < std::time::Duration::from_secs(interval) {
                    continue;
                }
                last_tick = std::time::Instant::now();
                let d = fetch::refresh_data(&input::view());
                if data_changed(&last, &d) {
                    config::update_data_refresh_time();
                    data_ver += 1;
                    pool.clear(); // 数据变了，所有视图的页面缓存统统失效
                    let view = input::view();
                    let total = layout::total_pages(&d, height, &view).max(1);
                    let cur = input::clamp_page(total);
                    // 后台更新也只渲染当前停留的一页，其余页等用户翻到再懒加载
                    let res = render::render_gray_page(&d, width, height, &view, cur);
                    flash_count += 1;
                    let switched = screen_id(&view, cur) != last_screen;
                    last_screen = screen_id(&view, cur);
                    let key =
                        page_cache_key(&view, input::style_mode().as_str(), cur, data_ver);
                    do_update(&disp, &res, width, height, flash_count % every == 0, switched);
                    pool.insert(key.clone(), res);
                    last_pushed = key;
                }
                last = d;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    kindle::enable_coexist_mode();
    kindle::restore_frontlight();
}

#[cfg(test)]
mod tests {
    use super::data_changed;
    use crate::config::StockData;

    fn snap(price: f64, last: f64) -> Vec<StockData> {
        vec![StockData {
            code: "sh600000".to_string(),
            name: "浦发".to_string(),
            group: "g".to_string(),
            price,
            change: 0.1,
            pct: 1.0,
            prev: 10.0,
            prices: vec![10.0, 10.1, last],
            timestamps: vec![1, 2, 3],
            regular_start: 0,
            regular_end: 0,
            chart_prev_close: 0.0,
        }]
    }

    #[test]
    fn detects_same_length_corrections() {
        let a = snap(10.2, 10.2);
        assert!(!data_changed(&a, &snap(10.2, 10.2)));
        // 同长度、尾点被修正：旧逻辑只比 len 会漏刷新
        assert!(data_changed(&a, &snap(10.2, 10.25)));
    }
}
