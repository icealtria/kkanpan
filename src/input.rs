use std::sync::{Mutex, OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

static VIEW: RwLock<String> = RwLock::new(String::new());
static STYLE: RwLock<StyleMode> = RwLock::new(StyleMode::Normal);
static PAGE: Mutex<usize> = Mutex::new(0);
static TOUCH_ON: AtomicBool = AtomicBool::new(true);
static TRIGGER: OnceLock<mpsc::SyncSender<bool>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleMode {
    Normal,
    Large,
}

impl StyleMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            StyleMode::Normal => "normal",
            StyleMode::Large => "large",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            StyleMode::Normal => "S",
            StyleMode::Large => "L",
        }
    }

    pub fn is_large(&self) -> bool {
        *self == StyleMode::Large
    }
}

pub fn init_state(default_view: &str) {
    *VIEW.write().unwrap() = default_view.to_string();
    *STYLE.write().unwrap() = StyleMode::Normal;
}

pub fn init_trigger() -> mpsc::Receiver<bool> {
    let (tx, rx) = mpsc::sync_channel(1);
    let _ = TRIGGER.set(tx);
    rx
}

pub fn trigger_refresh() {
    if let Some(tx) = TRIGGER.get() {
        let _ = tx.try_send(true);
    }
}

pub fn trigger_page_turn() {
    if let Some(tx) = TRIGGER.get() {
        let _ = tx.try_send(false);
    }
}

pub fn view() -> String {
    VIEW.read().unwrap().clone()
}

pub fn set_view(mode: &str) {
    *VIEW.write().unwrap() = mode.to_string();
    *PAGE.lock().unwrap() = 0;
    trigger_refresh();
}

pub fn style_mode() -> StyleMode {
    *STYLE.read().unwrap()
}

pub fn set_style(m: &str) {
    let mode = match m {
        "large" => StyleMode::Large,
        "normal" => StyleMode::Normal,
        _ => return,
    };
    *STYLE.write().unwrap() = mode;
    *PAGE.lock().unwrap() = 0;
    trigger_refresh();
}

pub fn next_style() -> StyleMode {
    let m = if style_mode().is_large() { StyleMode::Normal } else { StyleMode::Large };
    *STYLE.write().unwrap() = m;
    *PAGE.lock().unwrap() = 0;
    trigger_refresh();
    m
}

pub fn style_label() -> &'static str {
    style_mode().label()
}

pub fn touch_enabled() -> bool {
    TOUCH_ON.load(Ordering::Relaxed)
}

pub fn clamp_page(total: usize) -> usize {
    let mut p = PAGE.lock().unwrap();
    if total == 0 {
        *p = 0;
        return 0;
    }
    if *p >= total {
        *p = total - 1;
    }
    *p
}

pub fn next_page(total: usize) -> bool {
    let mut p = PAGE.lock().unwrap();
    if *p + 1 < total {
        *p += 1;
        true
    } else {
        false
    }
}

pub fn prev_page() -> bool {
    let mut p = PAGE.lock().unwrap();
    if *p > 0 {
        *p -= 1;
        true
    } else {
        false
    }
}

pub fn quit_app() -> ! {
    eprintln!("Exit requested. Restoring Kindle state...");
    crate::kindle::restore_frontlight();
    crate::kindle::enable_coexist_mode();
    std::process::Command::new("lipc-set-prop")
        .args(["com.lab126.appmgrd", "show", "app://com.lab126.booklet.home"])
        .output()
        .ok();
    crate::fbink::Display::clear_auto();
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::process::exit(0);
}


#[cfg(target_os = "linux")]
fn find_input(devs: &[&str]) -> Option<String> {
    devs.iter().find(|p| std::path::Path::new(p).exists()).map(|s| s.to_string())
}

#[cfg(target_os = "linux")]
fn read_event(f: &mut std::fs::File) -> Option<(u16, u16, i32)> {
    use std::io::Read;
    let mut buf = [0u8; 16];
    f.read_exact(&mut buf).ok()?;
    let ty = u16::from_le_bytes([buf[8], buf[9]]);
    let code = u16::from_le_bytes([buf[10], buf[11]]);
    let val = i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    Some((ty, code, val))
}

pub fn start_power_listener() {
    #[cfg(target_os = "linux")]
    std::thread::spawn(|| {
        let Some(path) = find_input(&["/dev/input/event0", "/dev/input/event1", "/dev/input/event2"]) else {
            eprintln!("[Power] No input device, disabled");
            return;
        };
        let mut f = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[Power] Cannot open {path}: {e}");
                return;
            }
        };
        eprintln!("[Power] listener on {path}");
        loop {
            let Some((ty, code, val)) = read_event(&mut f) else {
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            };
            if ty == 0x01 && code == 116 && val == 1 {
                TOUCH_ON.store(!TOUCH_ON.load(Ordering::Relaxed), Ordering::Relaxed);
                eprintln!("[Touch] enabled={}", TOUCH_ON.load(Ordering::Relaxed));
                trigger_refresh();
            }
        }
    });
}

fn handle_tap(x: i32, y: i32, sw: i32, sh: i32) {
    if y <= 65 {
        if x >= sw - 95 && x <= sw - 10 {
            quit_app();
        }
        if x >= sw - 185 && x <= sw - 105 {
            eprintln!("Style -> {}", next_style().as_str());
            return;
        }
    }
    if (60..=135).contains(&y) {
        let modes = crate::config::tab_modes();
        let tab_w = (sw - 60) / modes.len().max(1) as i32;
        let idx = (x - 30) / tab_w.max(1);
        if idx >= 0 && (idx as usize) < modes.len() {
            eprintln!("View -> {}", modes[idx as usize]);
            set_view(&modes[idx as usize].clone());
        }
        return;
    }
    if y >= sh - 100 {
        if x < sw / 3 {
            if prev_page() {
                trigger_page_turn();
            }
            return;
        }
        if x > sw * 2 / 3 {
            let total = crate::layout::total_pages(&crate::fetch::cached_snapshot(&view()), sh, &view()).max(1);
            if total > 1 && next_page(total) {
                trigger_page_turn();
            }
            return;
        }
        trigger_refresh();
    }
}

pub fn start_touch_listener(sw: i32, sh: i32) {
    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        use std::os::unix::io::AsRawFd;
        let Some(path) = find_input(&["/dev/input/event1", "/dev/input/event0", "/dev/input/event2"]) else {
            eprintln!("Touch device not found, skip");
            return;
        };
        let f = std::fs::OpenOptions::new().read(true).write(true).open(&path).or_else(|_| std::fs::File::open(&path));
        let mut f = match f {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Cannot open {path}: {e}");
                return;
            }
        };
        const EVIOCGRAB: libc::c_ulong = 0x40044590;
        // SAFETY: EVIOCGRAB ioctl，arg=1 独占
        #[cfg(not(target_env = "musl"))]
        let rc = unsafe { libc::ioctl(f.as_raw_fd(), EVIOCGRAB, 1) };
        // musl 上 request 参数是 c_int 而非 c_ulong（值 < 2^31，直接转）
        #[cfg(target_env = "musl")]
        let rc = unsafe { libc::ioctl(f.as_raw_fd(), EVIOCGRAB as libc::c_int, 1) };
        if rc != 0 {
            eprintln!("EVIOCGRAB grab rc={rc} (might be non-fatal)");
        }
        eprintln!("Touch listener on {path} ({sw}x{sh})");
        let (mut cur_x, mut cur_y, mut start_x) = (0, 0, 0);
        let mut touching = false;
        loop {
            let Some((ty, code, val)) = read_event(&mut f) else {
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            };
            if !TOUCH_ON.load(Ordering::Relaxed) {
                continue;
            }
            if ty == 0x03 {
                if code == 0x35 || code == 0x00 {
                    cur_x = val;
                } else if code == 0x36 || code == 0x01 {
                    cur_y = val;
                }
            } else if ty == 0x01 && code == 0x14a {
                if val == 1 {
                    touching = true;
                    start_x = cur_x;
                } else if val == 0 && touching {
                    touching = false;
                    let (mut x, y) = (cur_x, cur_y);
                    if x == 0 && y == 0 {
                        x = start_x;
                    }
                    if x > 0 && y > 0 {
                        handle_tap(x, y, sw, sh);
                    }
                    start_x = 0;
                }
            }
        }
    });
    #[cfg(not(target_os = "linux"))]
    let _ = (sw, sh);
}
