use crate::config::{self, StockConfig, StockData};
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};


static CACHED: std::sync::LazyLock<RwLock<HashMap<String, ViewCache>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));
static PRICE_HIST: std::sync::LazyLock<Mutex<HashMap<String, Vec<f64>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct ViewCache {
    data: Vec<StockData>,
    at: i64,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        let mut b = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(8)));
        let proxy = &config::app().proxy;
        if !proxy.is_empty() {
            if let Ok(p) = ureq::Proxy::new(proxy) {
                b = b.proxy(Some(p));
            }
        }
        b.build().new_agent()
    })
}

fn http_get(url: &str, referer: &str) -> Option<Vec<u8>> {
    let mut req = agent().get(url).header("User-Agent", "Mozilla/5.0");
    if !referer.is_empty() {
        req = req.header("Referer", referer);
    }
    req.call().ok()?.body_mut().read_to_vec().ok()
}


fn needed_stocks(view: &str) -> Vec<StockConfig> {
    let all = config::stocks();
    let (eff, is_auto) = config::effective_group(view);
    if eff == "ALL" {
        return all.to_vec();
    }
    if !is_auto {
        if eff.is_empty() {
            return vec![];
        }
        return all.iter().filter(|c| c.group == eff).cloned().collect();
    }
    let matching = config::matching_auto_groups();
    if matching.is_empty() {
        return vec![];
    }
    all.iter()
        .filter(|c| matching.contains(&c.group))
        .cloned()
        .collect()
}

fn fetch_qt(codes: &[String]) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    if codes.is_empty() {
        return out;
    }
    let url = format!("https://qt.gtimg.cn/q={}", codes.join(","));
    let body = match http_get(&url, "https://gu.qq.com/") {
        Some(b) => String::from_utf8_lossy(&b).into_owned(),
        None => return out,
    };
    for seg in body.split("v_").skip(1) {
        let Some(eq) = seg.find("=\"") else { continue };
        let code = &seg[..eq];
        let rest = &seg[eq + 2..];
        let Some(end) = rest.find('"') else { continue };
        let raw = &rest[..end];
        let vals: Vec<String> = if raw.contains('~') {
            raw.split('~').map(|s| s.to_string()).collect()
        } else {
            raw.split(',').map(|s| s.to_string()).collect()
        };
        out.insert(code.to_string(), vals);
    }
    out
}

struct ChartResult {
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    reg_start: i64,
    reg_end: i64,
    chart_prev: f64,
    y_price: f64,
    y_prev: f64,
}

fn fetch_yahoo(code: &str) -> ChartResult {
    let mut r = ChartResult {
        prices: vec![],
        timestamps: vec![],
        reg_start: 0,
        reg_end: 0,
        chart_prev: 0.0,
        y_price: 0.0,
        y_prev: 0.0,
    };
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1m&range=1d",
        code
    );
    let body = match http_get(&url, "") {
        Some(b) => b,
        None => return r,
    };
    let v: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return r,
    };
    let res = match v.pointer("/chart/result/0") {
        Some(v) => v,
        None => return r,
    };
    r.y_price = res.pointer("/meta/regularMarketPrice").and_then(|x| x.as_f64()).unwrap_or(0.0);
    r.y_prev = res.pointer("/meta/previousClose").and_then(|x| x.as_f64()).unwrap_or(0.0);
    r.chart_prev = res.pointer("/meta/chartPreviousClose").and_then(|x| x.as_f64()).unwrap_or(0.0);
    if r.y_prev == 0.0 {
        r.y_prev = r.chart_prev;
    }
    r.reg_start = res.pointer("/meta/currentTradingPeriod/regular/start").and_then(|x| x.as_i64()).unwrap_or(0);
    r.reg_end = res.pointer("/meta/currentTradingPeriod/regular/end").and_then(|x| x.as_i64()).unwrap_or(0);
    let empty = vec![];
    let stamps = res.pointer("/timestamp").and_then(|x| x.as_array()).unwrap_or(&empty);
    let closes = res.pointer("/indicators/quote/0/close").and_then(|x| x.as_array());
    if let Some(cl) = closes {
        for (i, c) in cl.iter().enumerate() {
            if let Some(p) = c.as_f64() {
                r.prices.push(p);
                if let Some(t) = stamps.get(i).and_then(|x| x.as_i64()) {
                    r.timestamps.push(t);
                }
            }
        }
    }
    if r.y_price == 0.0 {
        if let Some(&last) = r.prices.last() {
            r.y_price = last;
        }
    }
    r
}

fn parse_gtimg_rows(rows: &[&str]) -> Option<Vec<f64>> {
    let mut prices = Vec::with_capacity(rows.len());
    for row in rows {
        let mut it = row.split_whitespace();
        it.next()?;
        let p: f64 = it.next()?.parse().ok()?;
        prices.push(p);
    }
    if prices.len() < 2 { None } else { Some(prices) }
}

fn fetch_gtimg_minute(code: &str) -> Option<Vec<f64>> {
    let is_us = code.starts_with("us");
    let url = if is_us {
        format!("https://web.ifzq.gtimg.cn/appstock/app/UsMinute/query?code={code}")
    } else {
        format!("https://web.ifzq.gtimg.cn/appstock/app/minute/query?code={code}")
    };
    let body = http_get(&url, if is_us { "https://gu.qq.com/" } else { "" })?;
    let v: serde_json::Value = serde_json::from_slice(&body).ok()?;
    if is_us && v.pointer("/code").and_then(|x| x.as_f64()).unwrap_or(0.0) != 0.0 {
        return None;
    }
    let rows = v.pointer(&format!("/data/{code}/data/data"))?.as_array()?;
    let strs: Vec<&str> = rows.iter().filter_map(|x| x.as_str()).collect();
    let mut prices = parse_gtimg_rows(&strs)?;
    if is_us {
        prices.retain(|&p| p > 0.0);
        if prices.len() < 2 {
            return None;
        }
    }
    Some(prices)
}

fn parse_qt(code: &str, vals: &[String]) -> (f64, f64, f64, f64) {
    if code == "^VIX" {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let f = |i: usize| vals.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    if code.starts_with("hf_") {
        let (price, change) = (f(0), f(1));
        let prev = price - change;
        let pct = if prev != 0.0 { change / prev * 100.0 } else { 0.0 };
        return (price, change, pct, prev);
    }
    let price = f(3);
    let mut prev = f(4);
    if prev == 0.0 {
        prev = price;
    }
    let pct = if vals.len() > 32 { f(32) } else { 0.0 };
    let change = if vals.len() > 31 { f(31) } else { price - prev };
    (price, change, pct, prev)
}

pub fn refresh_data(view: &str) -> Vec<StockData> {
    let configs = needed_stocks(view);
    let tencent: Vec<String> = configs
        .iter()
        .filter(|c| c.source == "tencent")
        .map(|c| c.code.clone())
        .collect();
    let qt = fetch_qt(&tencent);

    // std::thread::scope 并行，无额外依赖；分块限流，Kindle 弱 CPU/RAM 扛不住 N 线程 + N 路 TLS
    let charts: HashMap<String, ChartResult> = std::thread::scope(|s| {
        let mut results = HashMap::new();
        for chunk in configs.chunks(5) {
            let handles: Vec<_> = chunk
                .iter()
                .map(|c| {
                    s.spawn(|| {
                        let mut r = ChartResult {
                            prices: vec![],
                            timestamps: vec![],
                            reg_start: 0,
                            reg_end: 0,
                            chart_prev: 0.0,
                            y_price: 0.0,
                            y_prev: 0.0,
                        };
                        if c.source == "tencent" {
                            r.prices = fetch_gtimg_minute(&c.code).unwrap_or_default();
                        } else if c.source == "yahoo" {
                            r = fetch_yahoo(&c.code);
                        }
                        (c.code.clone(), r)
                    })
                })
                .collect();
            for h in handles {
                let (code, r) = h.join().unwrap();
                results.insert(code, r);
            }
        }
        results
    });

    let mut items = Vec::with_capacity(configs.len());
    for c in &configs {
        let cr = charts.get(&c.code);
        let (price, change, pct, prev, mut prices) = if c.source == "tencent" {
            let mut vals = qt.get(&c.code).cloned().unwrap_or_default();
            if vals.is_empty() {
                let clean = c.code.replace("sh", "").replace("sz", "");
                vals = qt.get(&clean).cloned().unwrap_or_default();
            }
            let (p, ch, pc, pr) = parse_qt(&c.code, &vals);
            (p, ch, pc, pr, cr.map(|r| r.prices.clone()).unwrap_or_default())
        } else {
            let (yp, ypr) = cr.map(|r| (r.y_price, r.y_prev)).unwrap_or((0.0, 0.0));
            let (ch, pc) = if ypr != 0.0 {
                (yp - ypr, (yp - ypr) / ypr * 100.0)
            } else {
                (0.0, 0.0)
            };
            let (p, ch, pc, pr) = if c.code == "^VIX" && yp == 0.0 {
                (0.0, 0.0, 0.0, 0.0)
            } else {
                (yp, ch, pc, ypr)
            };
            (p, ch, pc, pr, cr.map(|r| r.prices.clone()).unwrap_or_default())
        };

        let is_chart = if c.source == "tencent" {
            c.code.starts_with("sh") || c.code.starts_with("sz") || c.code.starts_with("us")
        } else {
            prices.len() >= 2 || price > 0.0
        };
        if !is_chart {
            prices.clear();
        } else {
            let mut hist = PRICE_HIST.lock().unwrap();
            let h = hist.entry(c.code.clone()).or_default();
            if price > 0.0 {
                h.push(price);
                if h.len() > 1440 {
                    let n = h.len() - 1440;
                    h.drain(..n);
                }
            }
            if prices.len() < 2 && h.len() > 2 {
                prices = h.clone();
            }
        }

        items.push(StockData {
            code: c.code.clone(),
            name: c.name.clone(),
            group: c.group.clone(),
            price,
            change,
            pct,
            prev,
            prices,
            timestamps: cr.map(|r| r.timestamps.clone()).unwrap_or_default(),
            regular_start: cr.map(|r| r.reg_start).unwrap_or(0),
            regular_end: cr.map(|r| r.reg_end).unwrap_or(0),
            chart_prev_close: cr.map(|r| r.chart_prev).unwrap_or(0.0),
        });
    }
    let at = now_unix();
    CACHED.write().unwrap().insert(view.to_string(), ViewCache { data: items.clone(), at });
    items
}

pub fn get_data(view: &str) -> Vec<StockData> {
    let needed = needed_stocks(view);
    if needed.is_empty() {
        return vec![];
    }
    let ttl = config::app().cache_ttl;
    {
        let cached = CACHED.read().unwrap();
        if let Some(vc) = cached.get(view) {
            if now_unix() - vc.at <= ttl && !vc.data.is_empty() {
                let have: std::collections::HashSet<&str> =
                    vc.data.iter().map(|d| d.code.as_str()).collect();
                if needed.iter().all(|c| have.contains(c.code.as_str())) {
                    return vc.data.clone();
                }
            }
        }
    }
    refresh_data(view)
}

pub fn cached_snapshot(view: &str) -> Vec<StockData> {
    CACHED.read().unwrap().get(view).map(|vc| vc.data.clone()).unwrap_or_default()
}
