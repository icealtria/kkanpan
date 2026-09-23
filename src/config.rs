use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};


#[derive(Debug, Clone, Deserialize)]
pub struct StockConfig {
    pub code: String,
    pub name: String,
    pub group: String,
    pub source: String, // "tencent" | "yahoo"
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StockData {
    pub code: String,
    pub name: String,
    pub group: String,
    pub price: f64,
    pub change: f64,
    pub pct: f64,
    pub prev: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prices: Vec<f64>,
    #[serde(skip)]
    pub timestamps: Vec<i64>,
    #[serde(skip)]
    pub regular_start: i64,
    #[serde(skip)]
    pub regular_end: i64,
    #[serde(skip)]
    pub chart_prev_close: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutoRule {
    pub group: String,
    #[serde(default)]
    pub weekdays: Vec<i32>, // 0=Sun..6=Sat，空=每天
    pub start: String,      // "09:00"
    pub end: String,        // "15:30"
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub proxy: String,
    #[serde(default = "default_ttl")]
    pub cache_ttl: i64,
    #[serde(default, rename = "autoRules")]
    pub auto_rules: Vec<AutoRule>,
    #[serde(default, rename = "defaultView")]
    pub default_view: String,
    #[serde(default, rename = "dimFrontlight")]
    pub dim_frontlight: bool,
    #[serde(default = "default_full_flash_every", rename = "fullFlashEvery")]
    pub full_flash_every: u64,
}

fn default_ttl() -> i64 {
    55
}

fn default_full_flash_every() -> u64 {
    10
}


static APP: OnceLock<AppConfig> = OnceLock::new();
static STOCKS: OnceLock<Vec<StockConfig>> = OnceLock::new();

pub fn app() -> &'static AppConfig {
    APP.get_or_init(|| load_app_config())
}

fn load_app_config() -> AppConfig {
    for p in [
        "app.json",
        "/mnt/us/extensions/kkanpan/app.json",
        "/mnt/us/kkanpan/app.json",
    ] {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(mut cfg) = serde_json::from_slice::<AppConfig>(&data) {
                if cfg.cache_ttl == 0 {
                    cfg.cache_ttl = 55;
                }
                eprintln!("Loaded app config from {p}");
                return cfg;
            }
        }
    }
    panic!("app.json not found or invalid");
}

pub fn stocks() -> &'static [StockConfig] {
    STOCKS
        .get_or_init(|| {
            for p in [
                "stocks.json",
                "/mnt/us/extensions/kkanpan/stocks.json",
                "/mnt/us/kkanpan/stocks.json",
            ] {
                if let Ok(data) = std::fs::read(p) {
                    if let Ok(cfg) = serde_json::from_slice::<Vec<StockConfig>>(&data) {
                        if !cfg.is_empty() {
                            eprintln!("Loaded stocks from {p} ({} items)", cfg.len());
                            return cfg;
                        }
                    }
                }
            }
            panic!("stocks.json not found or empty");
        })
        .as_slice()
}

pub fn trading_minutes(code: &str) -> usize {
    if code.starts_with("sh") || code.starts_with("sz") {
        240
    } else if code.starts_with("us") {
        390
    } else if code.starts_with("hk") {
        330
    } else {
        0
    }
}

pub fn chart_total(code: &str, n: usize) -> usize {
    let t = trading_minutes(code);
    if t >= n { t } else { n }
}

pub fn all_groups() -> Vec<String> {
    let mut out = Vec::new();
    for s in stocks() {
        if !out.iter().any(|g| g == &s.group) {
            out.push(s.group.clone());
        }
    }
    out
}

pub fn tab_modes() -> Vec<String> {
    let mut tabs = Vec::new();
    if !app().auto_rules.is_empty() {
        tabs.push("AUTO".to_string());
    }
    tabs.extend(all_groups());
    tabs.push("ALL".to_string());
    tabs
}

pub fn default_view() -> String {
    if !app().default_view.is_empty() {
        return app().default_view.clone();
    }
    if !app().auto_rules.is_empty() {
        return "AUTO".to_string();
    }
    all_groups().into_iter().next().unwrap_or("ALL".to_string())
}

// 不用 chrono，CST 直接 +8h 算

fn cst_now() -> (i32, i32) {
    // weekday 0=Sun, minutes since midnight, Asia/Shanghai
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 8 * 3600;
    let days = secs.div_euclid(86400);
    let wd = ((days + 4) % 7) as i32; // 1970-01-01 周四
    let mins = (secs.rem_euclid(86400) / 60) as i32;
    (wd, mins)
}

fn parse_hm(s: &str) -> i32 {
    let mut it = s.split(':');
    let h: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let m: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    h * 60 + m
}

fn match_rule(wd: i32, mins: i32, r: &AutoRule) -> bool {
    if !r.weekdays.is_empty() && !r.weekdays.contains(&wd) {
        return false;
    }
    let (s, e) = (parse_hm(&r.start), parse_hm(&r.end));
    if s <= e {
        mins >= s && mins <= e
    } else {
        mins >= s || mins <= e
    }
}

pub fn effective_group(mode: &str) -> (String, bool) {
    if mode != "AUTO" && !mode.is_empty() {
        return (mode.to_string(), false);
    }
    if app().auto_rules.is_empty() {
        return (String::new(), true);
    }
    let (wd, mins) = cst_now();
    for r in &app().auto_rules {
        if match_rule(wd, mins, r) {
            return (r.group.clone(), true);
        }
    }
    (String::new(), true)
}

pub fn matching_auto_groups() -> Vec<String> {
    let mut out = Vec::new();
    let (wd, mins) = cst_now();
    for r in &app().auto_rules {
        if match_rule(wd, mins, r) && !out.contains(&r.group) {
            out.push(r.group.clone());
        }
    }
    out
}


static REFRESH_TIME: RwLock<String> = RwLock::new(String::new());

pub fn update_data_refresh_time() {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 8 * 3600;
    let s = secs.rem_euclid(86400);
    *REFRESH_TIME.write().unwrap() = format!("{:02}:{:02}", s / 3600, (s % 3600) / 60);
}

pub fn data_refresh_time() -> String {
    REFRESH_TIME.read().unwrap().clone()
}
