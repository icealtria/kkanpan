use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StockConfig {
    pub code: String,
    pub name: String,
    pub group: String,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Tencent,
    Yahoo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutoRule {
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub weekdays: Vec<u8>, // 0=Sun..6=Sat, 空=每天
    #[serde(default)]
    pub start: String, // "09:00"
    #[serde(default)]
    pub end: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub proxy: String,
    #[serde(default = "default_ttl")]
    pub cache_ttl: i64,
    #[serde(default)]
    pub auto_rules: Vec<AutoRule>,
    #[serde(default)]
    pub default_view: String,
    #[serde(default)]
    pub dim_frontlight: bool,
}

fn default_ttl() -> i64 {
    55
}

#[derive(Debug, Clone, Serialize)]
pub struct Quote {
    pub code: String,
    pub name: String,
    pub group: String,
    pub price: f64,
    pub prev: f64,
    pub change: f64,
    pub pct: f64,
    pub prices: Vec<f64>,
    pub timestamps: Vec<i64>,
    pub regular_start: i64,
    pub regular_end: i64,
    pub chart_prev_close: f64,
}

pub const CONFIG_PATHS: &[&str] = &[
    "app.json",
    "/mnt/us/extensions/kkanpan/app.json",
    "/mnt/us/kkanpan/app.json",
];
pub const STOCKS_PATHS: &[&str] = &[
    "stocks.json",
    "/mnt/us/extensions/kkanpan/stocks.json",
    "/mnt/us/kkanpan/stocks.json",
];

pub fn load_app_config() -> Result<AppConfig, String> {
    for p in CONFIG_PATHS {
        if let Ok(data) = fs::read(p) {
            if let Ok(mut cfg) = serde_json::from_slice::<AppConfig>(&data) {
                if cfg.cache_ttl == 0 {
                    cfg.cache_ttl = 55;
                }
                eprintln!("[cfg] loaded {p} (proxy={:?})", cfg.proxy);
                return Ok(cfg);
            }
        }
    }
    Err("app.json not found or invalid".into())
}

pub fn load_stocks() -> Result<Vec<StockConfig>, String> {
    for p in STOCKS_PATHS {
        if let Ok(data) = fs::read(p) {
            if let Ok(mut cfg) = serde_json::from_slice::<Vec<StockConfig>>(&data) {
                // 去重: 修复 stocks.json 里 sz002624 重复项这类脏数据
                let mut seen = std::collections::HashSet::new();
                cfg.retain(|s| seen.insert(s.code.clone()));
                if !cfg.is_empty() {
                    eprintln!("[cfg] loaded {p} ({} items)", cfg.len());
                    return Ok(cfg);
                }
            }
        }
    }
    Err("stocks.json not found or empty".into())
}

// ---- 纯函数: 分组/tab/图表横轴 (原 config.go) ----

pub fn all_groups(stocks: &[StockConfig]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for s in stocks {
        if seen.insert(s.group.clone()) {
            out.push(s.group.clone());
        }
    }
    out
}

pub fn tab_modes(stocks: &[StockConfig], auto_rules: &[AutoRule]) -> Vec<String> {
    let mut tabs = Vec::new();
    if !auto_rules.is_empty() {
        tabs.push("AUTO".to_string());
    }
    tabs.extend(all_groups(stocks));
    tabs.push("ALL".to_string());
    tabs
}

pub fn default_view(cfg: &AppConfig, stocks: &[StockConfig]) -> String {
    if !cfg.default_view.is_empty() {
        return cfg.default_view.clone();
    }
    if !cfg.auto_rules.is_empty() {
        return "AUTO".into();
    }
    if let Some(g) = all_groups(stocks).first() {
        return g.clone();
    }
    "ALL".into()
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

/// 实际点数超过横轴时以实际为准 (A股盘后交易等)
pub fn chart_total(code: &str, n: usize) -> usize {
    trading_minutes(code).max(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_and_groups() {
        let stocks = vec![
            StockConfig {
                code: "sz002624".into(),
                name: "a".into(),
                group: "g".into(),
                source: Source::Tencent,
            },
            StockConfig {
                code: "sz002624".into(),
                name: "a".into(),
                group: "g".into(),
                source: Source::Tencent,
            },
        ];
        let mut seen = std::collections::HashSet::new();
        let mut v = stocks.clone();
        v.retain(|s| seen.insert(s.code.clone()));
        assert_eq!(v.len(), 1);
        assert_eq!(all_groups(&v), vec!["g".to_string()]);
    }

    #[test]
    fn chart_total_uses_actual_when_larger() {
        assert_eq!(chart_total("sh000001", 300), 300);
        assert_eq!(chart_total("sh000001", 100), 240);
    }
}
