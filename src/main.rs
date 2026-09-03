mod app;
mod auto;
mod config;
mod diff;
mod fetch;
mod font;
mod kindle;
mod layout;
mod quote;
mod render;
mod server;
mod touch;

use app::{App, Event};
use config::{load_app_config, load_stocks};
use kindle::{Coexist, Frontlight};
use std::io::Read;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use touch::{classify_swipe, hit_test, parse_event, Swipe, TapAction};

struct Args {
    port: u16,
    host: String,
    interval: u64,
    width: u32,
    height: u32,
    eips: bool,
    once: bool,
    http: bool,
    view: String,
}

fn parse_args() -> Args {
    let mut a = Args {
        port: 8000,
        host: "0.0.0.0".into(),
        interval: 60,
        width: 1072,
        height: 1448,
        eips: true,
        once: false,
        http: false,
        view: String::new(),
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        let arg = &raw[i];
        let (k, inline) = match arg.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (arg.clone(), None),
        };
        // 非 bool 选项的值: 优先 = 内联, 否则下一个 arg
        let mut take_val = || -> String {
            if let Some(v) = inline.clone() {
                v
            } else {
                i += 1;
                raw.get(i).cloned().unwrap_or_default()
            }
        };
        match k.trim_start_matches('-') {
            "port" => a.port = take_val().parse().unwrap_or(8000),
            "host" => a.host = take_val(),
            "interval" => a.interval = take_val().parse().unwrap_or(60),
            "width" => a.width = take_val().parse().unwrap_or(1072),
            "height" => a.height = take_val().parse().unwrap_or(1448),
            "eips" => {
                let s = inline.clone().unwrap_or_default();
                a.eips = s.is_empty() || s == "true" || s == "1";
            }
            "once" => a.once = true,
            "http" => a.http = true,
            "view" => a.view = take_val(),
            _ => {}
        }
        i += 1;
    }
    a
}

const EVIOCGRAB: u32 = 0x40044590;

fn find_input(cands: &[&str]) -> Option<String> {
    cands.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string())
}

fn touch_thread(app: Arc<App>) {
    let Some(dev) = find_input(&["/dev/input/event1", "/dev/input/event0", "/dev/input/event2"])
    else {
        eprintln!("[touch] no device, skip");
        return;
    };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(&dev).or_else(|_| std::fs::File::open(&dev));
    let Ok(mut file) = file else {
        eprintln!("[touch] cannot open {dev}");
        return;
    };
    use std::os::fd::AsRawFd;
    let r = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB as _, 1) };
    if r != 0 {
        eprintln!("[touch] EVIOCGRAB grab failed (non-fatal)");
    }
    if let Ok(dup) = file.try_clone() {
        app.set_touch_file(dup);
    }
    app.grab_touch(true);
    eprintln!("[touch] listening on {dev}");

    let (w, h) = (app.width as i32, app.height as i32);
    let mut buf = [0u8; 24];
    let (mut cur_x, mut cur_y) = (0i32, 0i32);
    let (mut sx, mut sy) = (0i32, 0i32);
    let mut t0: Option<Instant> = None;
    let mut touching = false;

    loop {
        let n = file.read(&mut buf).unwrap_or(0);
        if n != 16 && n != 24 {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        let Some((t, c, v)) = parse_event(&buf[..n]) else { continue };
        if !app.touch_on() {
            continue; // 仍消费事件, 防内核缓冲满
        }
        match (t, c) {
            (3, 0x00) | (3, 0x53) => cur_x = v,
            (3, 0x01) | (3, 0x54) => cur_y = v,
            (1, 0x14a) if v == 1 => {
                touching = true;
                (sx, sy) = (cur_x, cur_y);
                t0 = Some(Instant::now());
            }
            (1, 0x14a) if v == 0 && touching => {
                touching = false;
                let (ex, ey) = if cur_x == 0 && cur_y == 0 { (sx, sy) } else { (cur_x, cur_y) };
                let ms = t0.map(|t| t.elapsed().as_millis() as i64).unwrap_or(0);
                t0 = None;
                if classify_swipe(sx, sy, ex, ey, ms).is_some_and(|s| handle_swipe(&app, s)) {
                    (sx, sy) = (0, 0);
                    continue;
                }
                if ex > 0 && ey > 0 {
                    let (ax, ay) = (ex, ey);
                    let app2 = app.clone();
                    std::thread::spawn(move || handle_tap(&app2, ax, ay));
                }
                (sx, sy) = (0, 0);
            }
            _ => {}
        }
        let _ = (w, h);
    }
}

fn handle_swipe(app: &App, s: Swipe) -> bool {
    let total = app.total_pages(&app.last()).max(1);
    if total <= 1 {
        return false;
    }
    match s {
        Swipe::Up => app.next_page(total),
        Swipe::Down => app.prev_page(),
    }
}

fn handle_tap(app: &App, x: i32, y: i32) {
    // w/h 从 app 取, 避免与 render 的 tab 几何脱节
    let (w, h) = (app.width as i32, app.height as i32);
    match hit_test(x, y, w, h, app.tabs().len()) {
        TapAction::Close => app.quit(),
        TapAction::Style => {
            app.next_style();
        }
        TapAction::Tab(i) => {
            if let Some(name) = app.tabs().get(i).cloned() {
                app.set_view(&name);
            }
        }
        TapAction::PrevPage => {
            app.prev_page();
        }
        TapAction::NextPage => {
            let total = app.total_pages(&app.last()).max(1);
            if !app.next_page(total) {
                app.kick(); // 末页右下角 = 立即刷新 (原逻辑)
            }
        }
        TapAction::Refresh => app.kick(),
        TapAction::None => {}
    }
}

fn power_thread(app: Arc<App>) {
    let Some(dev) = find_input(&["/dev/input/event0", "/dev/input/event1", "/dev/input/event2"])
    else {
        return;
    };
    let Ok(mut f) = std::fs::File::open(&dev) else { return };
    eprintln!("[power] listening on {dev}");
    let mut buf = [0u8; 24];
    loop {
        let n = f.read(&mut buf).unwrap_or(0);
        if n != 16 && n != 24 {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        // KEY_POWER: type=1 code=116 value=1
        if let Some((1, 116, 1)) = parse_event(&buf[..n]) {
            app.toggle_touch();
            eprintln!("[power] touch on={}", app.touch_on());
        }
    }
}

fn quit_cleanup(co: &mut Coexist, fl: &mut Frontlight) {
    use std::process::Command;
    fl.restore();
    co.enable();
    if !kindle::is_kindle() {
        return;
    }
    let _ = Command::new("lipc-set-prop")
        .args(["com.lab126.appmgrd", "show", "app://com.lab126.booklet.home"])
        .status();
    let _ = Command::new("/usr/sbin/eips").args(["-c"]).status();
    std::thread::sleep(Duration::from_millis(300));
}

fn main() {
    let args = parse_args();
    let cfg = load_app_config().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let stocks = load_stocks().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    let (tx, rx) = mpsc::sync_channel::<Event>(1);
    let app = Arc::new(App::new(cfg, stocks, args.width, args.height, tx));
    if !args.view.is_empty() {
        app.set_view(&args.view);
        // 构造时的默认 kick 无人消费, 排空
        while rx.try_recv().is_ok() {}
    }
    eprintln!("[main] view={}", app.view());

    let mut coexist = Coexist::default();
    coexist.disable();
    let mut frontlight = if app.cfg.dim_frontlight { Frontlight::turn_off() } else { Frontlight(None) };

    let data = app.refresh();
    eprintln!("[main] fetched {} quotes", data.len());

    if args.once {
        app.show(&data, true);
        quit_cleanup(&mut coexist, &mut frontlight);
        return;
    }

    if args.http {
        let (h, p, a) = (args.host.clone(), args.port, app.clone());
        std::thread::spawn(move || server::serve(a, h, p));
    }
    {
        let a = app.clone();
        std::thread::spawn(move || touch_thread(a));
        let a = app.clone();
        std::thread::spawn(move || power_thread(a));
    }

    if !args.eips {
        // --eips=false: 只跑 http+touch (原 select{} 改为事件循环, 可正常退出)
        loop {
            match rx.recv() {
                Ok(Event::Quit) => break,
                Ok(Event::Kick) => {
                    let d = app.data();
                    app.set_last(&d);
                }
                Err(_) => break,
            }
        }
        quit_cleanup(&mut coexist, &mut frontlight);
        return;
    }

    app.show(&data, true);
    let (mut last, mut last_view) = (data, app.view());
    let mut count = 0u64;
    let interval = Duration::from_secs(args.interval.max(5));
    loop {
        match rx.recv_timeout(interval) {
            Ok(Event::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Event::Kick) => {
                app.clear_diff();
                let d = app.data();
                app.show(&d, false);
                last = d;
                last_view = app.view();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                count += 1;
                let d = app.refresh();
                let v = app.view();
                if App::same_quotes(&last, &d) && v == last_view {
                    eprintln!("[skip] unchanged, skip render");
                    continue;
                }
                last = d.clone();
                last_view = v;
                app.show(&d, count % 5 == 0);
            }
        }
    }
    quit_cleanup(&mut coexist, &mut frontlight);
}
