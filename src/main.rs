mod config;
mod fbink;
mod fetch;
mod input;
mod kindle;
mod render;
mod server;
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

fn pool_key(view: &str, style: &str) -> String {
    // touch 开关显示在状态栏，进 key 保证电源键切换立即重绘；
    // 拉取时间进 key，数据更新后旧缓存自动失效
    format!("{view}|{style}|{}|{}", crate::input::touch_enabled(), crate::fetch::last_fetch_unix(view))
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

    kindle::disable_coexist_mode();
    if config::app().dim_frontlight {
        kindle::save_and_turn_off_frontlight();
    }

    let disp = fbink::Display::open().expect("display init");
    let mut differ = fbink::ScreenDiffer::new();

    let data = fetch::refresh_data(&view0);
    config::update_data_refresh_time();
    eprintln!("Fetched {} stocks", data.len());

    if once {
        let gray = render::render_gray(&data, width, height, &view0);
        differ.update(&disp, &gray, width, height, true).unwrap();
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

    let gray = render::render_gray(&data, width, height, &input::view());
    differ.update(&disp, &gray, width, height, true).unwrap();
    let mut last = data;
    let mut count = 0usize;
    let mut pool: std::collections::HashMap<String, Vec<Vec<u8>>> = std::collections::HashMap::new();
    pool.insert(
        pool_key(&input::view(), &input::style_mode()),
        render::render_all_pages(&last, width, height, &input::view()),
    );
    let mut fast_count = 0usize;

    // 1 秒粒度轮询：既响应信号，又不打乱 interval 节拍
    let mut last_tick = std::time::Instant::now();
    loop {
        if term.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Signal received, restoring Kindle state...");
            break;
        }
        match trigger.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(full) => {
                // 切 Tab 时覆盖本地缺失的代码；切视图/风格走 GC16，同视图翻页走 DU，
                // 每 10 次快刷全闪一次去鬼影
                let view = input::view();
                last = fetch::get_data(&view);
                let key = pool_key(&view, &input::style_mode());
                if !pool.contains_key(&key) {
                    pool.insert(key.clone(), render::render_all_pages(&last, width, height, &view));
                }
                let pages = &pool[&key];
                let total = pages.len().max(1);
                let cur = input::clamp_page(total);
                let do_full = if full {
                    true
                } else {
                    fast_count += 1;
                    fast_count % 10 == 0
                };
                differ.update(&disp, &pages[cur], width, height, do_full).unwrap();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_tick.elapsed() < std::time::Duration::from_secs(interval) {
                    continue;
                }
                last_tick = std::time::Instant::now();
                count += 1;
                let d = fetch::refresh_data(&input::view());
                if data_changed(&last, &d) {
                    config::update_data_refresh_time();
                    pool.clear(); // 数据变了，所有视图缓存失效
                    let view = input::view();
                    let pages = render::render_all_pages(&d, width, height, &view);
                    let total = pages.len().max(1);
                    let cur = input::clamp_page(total);
                    differ.update(&disp, &pages[cur], width, height, count % 5 == 0).unwrap();
                    pool.insert(pool_key(&view, &input::style_mode()), pages);
                }
                last = d;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    kindle::enable_coexist_mode();
    kindle::restore_frontlight();
}
