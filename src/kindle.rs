use std::sync::{Mutex, OnceLock};

#[derive(Clone, Default)]
struct SysInfo {
    batt: String,
    charging: bool,
    updated: i64,
}

static SYS: Mutex<SysInfo> = Mutex::new(SysInfo { batt: String::new(), charging: false, updated: 0 });

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn read_first(paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|p| {
        std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    })
}

pub fn system_batt() -> (String, bool) {
    let now = now_unix();
    {
        let s = SYS.lock().unwrap();
        if s.updated > 0 && now - s.updated < 30 {
            return (s.batt.clone(), s.charging);
        }
    }
    let batt = read_first(&[
        "/sys/class/power_supply/max77696-battery/capacity",
        "/sys/class/power_supply/battery/capacity",
        "/sys/class/power_supply/mc13892_bat/capacity",
    ])
    .unwrap_or("--".to_string());
    let charging = ["max77696-battery", "battery", "mc13892_bat"].iter().any(|d| {
        std::fs::read_to_string(format!("/sys/class/power_supply/{d}/status"))
            .map(|s| matches!(s.trim(), "Charging" | "Full"))
            .unwrap_or(false)
    });
    *SYS.lock().unwrap() = SysInfo { batt: batt.clone(), charging, updated: now };
    (batt, charging)
}

pub fn format_status_bar() -> String {
    let (batt, charging) = system_batt();
    let mut s = format!("Updated: {} | BATT {batt}%", crate::config::data_refresh_time());
    if charging {
        s.push_str(" CHG");
    }
    if !crate::input::touch_enabled() {
        s.push_str(" | TOUCH OFF");
    }
    s
}

const FRONTLIGHT: &str = "/sys/class/backlight/max77696-bl/brightness";
static SAVED_BL: OnceLock<String> = OnceLock::new();

pub fn save_and_turn_off_frontlight() {
    if let Ok(b) = std::fs::read_to_string(FRONTLIGHT) {
        let v = b.trim().to_string();
        let _ = SAVED_BL.set(v.clone());
        std::fs::write(FRONTLIGHT, "0").ok();
        eprintln!("[Power] Frontlight saved={v}, off");
    }
}

pub fn restore_frontlight() {
    if let Some(v) = SAVED_BL.get() {
        std::fs::write(FRONTLIGHT, v).ok();
        eprintln!("[Power] Frontlight restored to {v}");
    }
}


fn run(prog: &str, args: &[&str]) -> bool {
    std::process::Command::new(prog).args(args).output().map(|o| o.status.success()).unwrap_or(false)
}

fn fw_version() -> String {
    std::process::Command::new("sh")
        .args(["-c", "grep '^Kindle 5' /etc/prettyversion.txt 2>/dev/null | sed -n -r 's/^(Kindle)([[:blank:]]*)([[:digit:]\\.]*)(.*?)$/\\3/p'"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn ver_ge(a: &str, b: &str) -> bool {
    fn parse(s: &str) -> i32 {
        let mut v = 0;
        for (i, part) in s.split('.').take(3).enumerate() {
            let n: i32 = part.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
            v += n * [1_000_000, 1_000, 1][i];
        }
        v
    }
    parse(a) >= parse(b)
}

pub fn disable_coexist_mode() {
    run("lipc-set-prop", &["-i", "com.lab126.powerd", "preventScreenSaver", "1"]);
    let upstart = std::path::Path::new("/etc/upstart").exists();
    let fw = fw_version();
    if upstart {
        if !fw.is_empty() && ver_ge(&fw, "5.6.5") {
            run("lipc-set-prop", &["com.lab126.pillow", "disableEnablePillow", "disable"]);
            if ver_ge(&fw, "5.7.2") && !ver_ge(&fw, "5.12.4") {
                run("killall", &["-STOP", "awesome"]);
            } else if ver_ge(&fw, "5.7.2") {
                run("killall", &["-STOP", "awesome"]);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        if std::path::Path::new("/etc/upstart/statusbar.conf").exists() {
            run("stop", &["statusbar"]);
        }
    } else {
        run("killall", &["-STOP", "cvm"]);
    }
    run("killall", &["-STOP", "volumd"]);
}

pub fn enable_coexist_mode() {
    run("killall", &["-CONT", "volumd"]);
    run("killall", &["-CONT", "awesome"]);
    run("killall", &["-CONT", "cvm"]);
    run("start", &["statusbar"]);
    run("lipc-set-prop", &["com.lab126.pillow", "disableEnablePillow", "enable"]);
    run("lipc-set-prop", &["com.lab126.appmgrd", "start", "app://com.lab126.booklet.home"]);
    run("lipc-set-prop", &["-i", "com.lab126.powerd", "preventScreenSaver", "0"]);
}
