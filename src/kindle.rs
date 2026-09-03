use crate::diff::{find_dirty, should_full, DirtyRect};
use image::GrayImage;
use std::process::Command;
use std::time::{Duration, Instant};

pub const EIPS: &str = "/usr/sbin/eips";
const TMP: &str = "/tmp/kkanpan.png";

/// 非 Kindle (开发机) 跳过所有 killall/lipc/背光操作, 避免污染 host.
pub fn is_kindle() -> bool {
    std::path::Path::new(EIPS).exists() || std::path::Path::new("/dev/fb0").exists()
}

fn has_eips() -> bool {
    std::path::Path::new(EIPS).exists()
}

fn save_png(path: &str, img: &GrayImage) -> std::io::Result<()> {
    img.save(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

/// 原 ScreenDiffer.UpdateScreen, 改为显式状态机: 前一帧跟随 Eips 实例走,
/// 不再有全局单例. full=true 时清屏全刷.
pub struct Eips {
    prev: Option<Vec<u8>>,
    w: u32,
    h: u32,
}

impl Eips {
    pub fn new() -> Self {
        Self { prev: None, w: 0, h: 0 }
    }

    pub fn clear_cache(&mut self) {
        self.prev = None;
    }

    pub fn update(&mut self, img: &GrayImage, full: bool) {
        let (w, h) = (img.width(), img.height());
        if full {
            write_full(img);
            self.prev = Some(img.as_raw().to_vec());
            self.w = w;
            self.h = h;
            return;
        }
        let old = self.prev.as_ref().and_then(|p| {
            if self.w == w && self.h == h {
                Some((p.as_slice(), w, h))
            } else {
                None
            }
        });
        let rects = find_dirty(old, img.as_raw(), w, h);
        if rects.is_empty() {
            eprintln!("[diff] no changes, skip");
            return;
        }
        let dirty: u64 = rects.iter().map(|r| r.w as u64 * r.h as u64).sum();
        eprintln!(
            "[diff] {} regions, {:.1}% changed",
            rects.len(),
            dirty as f64 / (w as f64 * h as f64) * 100.0
        );
        if should_full(&rects, w, h) {
            eprintln!("[diff] fallback to full update");
            write_full(img);
        } else if has_eips() {
            for (i, r) in rects.iter().enumerate() {
                if let Err(e) = partial(img, *r, i) {
                    eprintln!("[diff] partial {i} failed ({e}), fallback full");
                    write_full(img);
                    break;
                }
            }
        } else {
            write_full(img);
        }
        self.prev = Some(img.as_raw().to_vec());
        self.w = w;
        self.h = h;
    }
}

fn write_full(img: &GrayImage) {
    if save_png(TMP, img).is_err() {
        return;
    }
    if !has_eips() {
        eprintln!("[eips] rendered {TMP} (not on Kindle)");
        return;
    }
    let _ = Command::new(EIPS).args(["-c"]).status();
    std::thread::sleep(Duration::from_millis(200));
    if Command::new(EIPS).args(["-f", "-g", TMP]).output().is_err() {
        let _ = Command::new(EIPS).args(["-g", TMP]).status();
    }
    eprintln!("[eips] full refresh done");
}

fn partial(img: &GrayImage, r: DirtyRect, idx: usize) -> std::io::Result<()> {
    let mut crop = GrayImage::new(r.w, r.h);
    for y in 0..r.h {
        for x in 0..r.w {
            crop.put_pixel(x, y, *img.get_pixel(r.x + x, r.y + y));
        }
    }
    let path = format!("/tmp/kkanpan_patch_{idx}.png");
    save_png(&path, &crop)?;
    let st = Command::new(EIPS)
        .args(["-g", &path, "-x", &r.x.to_string(), "-y", &r.y.to_string()])
        .status()?;
    if !st.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "eips partial failed"));
    }
    Ok(())
}

// ---- 背光 / 电量时间 ----

pub const FRONTLIGHT: &str = "/sys/class/backlight/max77696-bl/brightness";

pub struct Frontlight(pub Option<String>);

impl Frontlight {
    pub fn turn_off() -> Self {
        if !is_kindle() {
            return Self(None);
        }
        let saved = std::fs::read_to_string(FRONTLIGHT).ok().map(|s| s.trim().to_string());
        if saved.is_some() {
            let _ = std::fs::write(FRONTLIGHT, "0");
        }
        Self(saved)
    }
    pub fn restore(&mut self) {
        if let Some(v) = self.0.take() {
            let _ = std::fs::write(FRONTLIGHT, v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct SysInfo {
    pub time: String,
    pub batt: String,
    pub charging: bool,
}

impl SysInfo {
    pub fn read() -> Self {
        let time = chrono::Local::now().format("%H:%M").to_string();
        let batt = ["max77696-battery", "battery", "mc13892_bat"]
            .iter()
            .find_map(|n| {
                std::fs::read_to_string(format!("/sys/class/power_supply/{n}/capacity"))
                    .ok()
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or("--".into());
        let charging = ["max77696-battery", "battery", "mc13892_bat"]
            .iter()
            .find_map(|n| {
                std::fs::read_to_string(format!("/sys/class/power_supply/{n}/status")).ok()
            })
            .map(|s| {
                let t = s.trim();
                t == "Charging" || t == "Full"
            })
            .unwrap_or(false);
        Self { time, batt, charging }
    }

    pub fn status(&self) -> String {
        if self.charging {
            format!("{} | BATT {}% CHG", self.time, self.batt)
        } else {
            format!("{} | BATT {}%", self.time, self.batt)
        }
    }
}

pub struct SysCache {
    info: SysInfo,
    at: Option<Instant>,
}

impl SysCache {
    pub fn new() -> Self {
        Self { info: SysInfo { time: "--:--".into(), batt: "--".into(), charging: false }, at: None }
    }
    /// 30s 缓存, lipc 耗电避免频繁唤醒
    pub fn get(&mut self) -> SysInfo {
        let fresh = self.at.map(|t| t.elapsed() < Duration::from_secs(30)).unwrap_or(false);
        if fresh {
            return self.info.clone();
        }
        self.info = SysInfo::read();
        self.at = Some(Instant::now());
        self.info.clone()
    }
}

// ---- 共存模式 (不杀 framework, 对齐 KOReader, 退出不重启) ----

fn sh(cmd: &str) {
    let _ = Command::new("sh").args(["-c", cmd]).status();
}

fn init_type() -> &'static str {
    if std::path::Path::new("/etc/upstart").exists() {
        "upstart"
    } else {
        "sysv"
    }
}

fn fw_version() -> String {
    std::fs::read_to_string("/etc/prettyversion.txt")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("Kindle 5"))
        .and_then(|l| {
            l.trim_start_matches("Kindle")
                .trim_start()
                .split_whitespace()
                .next()
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

/// 复刻 koreader.sh version() awk: %d%03d%03d 比较
pub fn version_ge(a: &str, b: &str) -> bool {
    fn num(s: &str) -> u32 {
        let mut v = 0u32;
        for (i, part) in s.split('.').take(3).enumerate() {
            let n: u32 = part
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            v += match i {
                0 => n * 1_000_000,
                1 => n * 1_000,
                _ => n,
            };
        }
        v
    }
    num(a) >= num(b)
}

#[derive(Default)]
pub struct Coexist {
    pillow: bool,
    awesome: bool,
    cvm: bool,
    volumd: bool,
    statusbar: bool,
}

impl Coexist {
    pub fn disable(&mut self) {
        if !is_kindle() {
            return;
        }
        let _ =
            Command::new("lipc-set-prop").args(["-i", "com.lab126.powerd", "preventScreenSaver", "1"]).status();
        if init_type() == "upstart" {
            sh("cat /dev/fb0 > /var/tmp/kkanpan-fb.dump 2>/dev/null");
            let fw = fw_version();
            if !fw.is_empty() && version_ge(&fw, "5.6.5") {
                let _ = Command::new("lipc-set-prop")
                    .args(["com.lab126.pillow", "disableEnablePillow", "disable"])
                    .status();
                self.pillow = true;
                if version_ge(&fw, "5.7.2") {
                    if !version_ge(&fw, "5.12.4") {
                        // wmctrl 1px 标题栏 (5.12.4+ 有 softlock 风险则跳过)
                        sh("wmctrl -r :titleBar_ID: -e 0,-1,-1,-1,1 2>/dev/null");
                    }
                    if Command::new("killall").args(["-STOP", "awesome"]).status().map(|s| s.success()).unwrap_or(false) {
                        self.awesome = true;
                    } else if Command::new("killall").args(["-STOP", "cvm"]).status().map(|s| s.success()).unwrap_or(false) {
                        self.cvm = true;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
            } else {
                let _ = Command::new("lipc-set-prop")
                    .args(["com.lab126.pillow", "interrogatePillow", r#"{"pillowId": "default_status_bar", "function": "nativeBridge.hideMe();"#])
                    .status();
            }
            if std::path::Path::new("/etc/upstart/statusbar.conf").exists()
                && Command::new("stop").arg("statusbar").status().map(|s| s.success()).unwrap_or(false)
            {
                self.statusbar = true;
            }
        } else if Command::new("killall").args(["-STOP", "cvm"]).status().map(|s| s.success()).unwrap_or(false) {
            self.cvm = true;
        }
        if Command::new("killall").args(["-STOP", "volumd"]).status().map(|s| s.success()).unwrap_or(false) {
            self.volumd = true;
        }
    }

    pub fn enable(&mut self) {
        if !is_kindle() {
            return;
        }
        let run = |a: &str, b: &str| {
            let _ = Command::new("killall").args([a, b]).status();
        };
        if self.volumd {
            run("-CONT", "volumd");
            self.volumd = false;
        }
        if init_type() == "sysv" && self.cvm {
            run("-CONT", "cvm");
            self.cvm = false;
            sh("echo 'send 139' > /proc/keypad 2>/dev/null");
        }
        if init_type() == "upstart" {
            if self.statusbar {
                let _ = Command::new("start").arg("statusbar").status();
                self.statusbar = false;
            }
            if self.awesome {
                run("-CONT", "awesome");
                self.awesome = false;
            } else if self.cvm {
                run("-CONT", "cvm");
                self.cvm = false;
            }
            if self.pillow {
                sh("cat /var/tmp/kkanpan-fb.dump > /dev/fb0 2>/dev/null; rm -f /var/tmp/kkanpan-fb.dump");
                let _ = Command::new("lipc-set-prop")
                    .args(["com.lab126.pillow", "disableEnablePillow", "enable"])
                    .status();
                let _ = Command::new("lipc-set-prop")
                    .args(["com.lab126.appmgrd", "start", "app://com.lab126.booklet.home"])
                    .status();
                self.pillow = false;
            }
        }
        let _ =
            Command::new("lipc-set-prop").args(["-i", "com.lab126.powerd", "preventScreenSaver", "0"]).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_cmp() {
        assert!(version_ge("5.16.2", "5.6.5"));
        assert!(!version_ge("5.6.4", "5.6.5"));
        assert!(version_ge("5.6.5", "5.6.5"));
    }
}
