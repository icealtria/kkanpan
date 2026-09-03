use crate::config::{Quote, Source, StockConfig};
use crate::quote::{parse_gtimg_rows, parse_qt};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// ureq 同步 client. proxy 为空则直连.
pub fn agent(proxy: &str) -> ureq::Agent {
    let mut b = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8)));
    if !proxy.is_empty() {
        if let Ok(p) = ureq::Proxy::new(proxy) {
            b = b.proxy(Some(p));
        }
    }
    b.build().into()
}

/// 腾讯 qt 批量接口: 一次请求拿全部分时报价 (原 fetchQT).
pub fn fetch_qt(agent: &ureq::Agent, codes: &[String]) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    if codes.is_empty() {
        return out;
    }
    let url = format!("https://qt.gtimg.cn/q={}", codes.join(","));
    let Ok(mut resp) = agent
        .get(&url)
        .header("Referer", "https://gu.qq.com/")
        .header("User-Agent", "Mozilla/5.0")
        .call()
    else {
        return out;
    };
    let Ok(body) = resp.body_mut().read_to_string() else {
        return out;
    };
    for m in split_qt(&body) {
        out.insert(m.0, m.1);
    }
    out
}

fn split_qt(body: &str) -> Vec<(String, Vec<String>)> {
    // v_sh000001="..." 逐段切, 比 Go 的正则更快且无依赖
    let mut out = Vec::new();
    for seg in body.split("v_") {
        let Some(eq) = seg.find("=\"") else { continue };
        let code = &seg[..eq];
        let rest = &seg[eq + 2..];
        let Some(end) = rest.find('"') else { continue };
        let raw = &rest[..end];
        let vals = if raw.contains('~') {
            raw.split('~').map(|s| s.to_string()).collect()
        } else {
            raw.split(',').map(|s| s.to_string()).collect()
        };
        out.push((code.to_string(), vals));
    }
    out
}

#[derive(Debug, Default)]
struct Chart {
    prices: Vec<f64>,
    timestamps: Vec<i64>,
    reg_start: i64,
    reg_end: i64,
    y_price: f64,
    y_prev: f64,
    chart_prev: f64,
}

fn fetch_yahoo(agent: &ureq::Agent, code: &str) -> Chart {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{code}?interval=1m&range=1d"
    );
    let Ok(mut resp) = agent.get(&url).header("User-Agent", "Mozilla/5.0").call() else {
        return Chart::default();
    };
    let Ok(body) = resp.body_mut().read_to_string() else {
        return Chart::default();
    };
    parse_yahoo(&body)
}

#[derive(serde::Deserialize)]
struct YChart {
    chart: YChartInner,
}
#[derive(serde::Deserialize)]
struct YChartInner {
    result: Option<Vec<YResult>>,
}
#[derive(serde::Deserialize)]
struct YResult {
    meta: YMeta,
    timestamp: Option<Vec<i64>>,
    indicators: YInd,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct YMeta {
    #[serde(default)]
    regular_market_price: f64,
    #[serde(default)]
    previous_close: f64,
    #[serde(default)]
    chart_previous_close: f64,
    #[serde(default)]
    current_trading_period: YCtp,
}
#[derive(serde::Deserialize, Default)]
struct YCtp {
    #[serde(default)]
    regular: YReg,
}
#[derive(serde::Deserialize, Default)]
struct YReg {
    #[serde(default)]
    start: i64,
    #[serde(default)]
    end: i64,
}
#[derive(serde::Deserialize)]
struct YInd {
    quote: Option<Vec<YQuote>>,
}
#[derive(serde::Deserialize)]
struct YQuote {
    close: Option<Vec<Option<f64>>>,
}

fn parse_yahoo(body: &str) -> Chart {
    let Ok(y) = serde_json::from_str::<YChart>(body) else {
        return Chart::default();
    };
    let Some(res) = y.chart.result.and_then(|mut v| v.pop()) else {
        return Chart::default();
    };
    let mut c = Chart {
        y_price: res.meta.regular_market_price,
        y_prev: res.meta.previous_close,
        chart_prev: res.meta.chart_previous_close,
        reg_start: res.meta.current_trading_period.regular.start,
        reg_end: res.meta.current_trading_period.regular.end,
        ..Chart::default()
    };
    if c.y_prev == 0.0 {
        c.y_prev = c.chart_prev;
    }
    let closes: Vec<Option<f64>> = res
        .indicators
        .quote
        .and_then(|mut q| q.pop())
        .and_then(|q| q.close)
        .unwrap_or_default();
    let ts = res.timestamp.unwrap_or_default();
    if ts.is_empty() {
        c.prices = closes.into_iter().flatten().collect();
    } else {
        for (i, v) in closes.into_iter().enumerate() {
            if i < ts.len() {
                if let Some(p) = v {
                    c.prices.push(p);
                    c.timestamps.push(ts[i]);
                }
            }
        }
    }
    if c.y_price == 0.0 {
        if let Some(&last) = c.prices.last() {
            c.y_price = last;
        }
    }
    c
}

fn fetch_gtimg_minute(agent: &ureq::Agent, code: &str) -> Option<Vec<f64>> {
    let is_us = code.starts_with("us");
    let url = if is_us {
        format!("https://web.ifzq.gtimg.cn/appstock/app/UsMinute/query?code={code}")
    } else {
        format!("https://web.ifzq.gtimg.cn/appstock/app/minute/query?code={code}")
    };
    let Ok(mut resp) = agent
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Referer", "https://gu.qq.com/")
        .call()
    else {
        return None;
    };
    let Ok(body) = resp.body_mut().read_to_string() else {
        return None;
    };
    parse_gtimg_minute_json(code, &body)
}

fn parse_gtimg_minute_json(code: &str, body: &str) -> Option<Vec<f64>> {
    let j: serde_json::Value = serde_json::from_str(body).ok()?;
    if code.starts_with("us") && j.get("code")?.as_f64()? != 0.0 {
        return None;
    }
    let rows = j
        .pointer(&format!("/data/{code}/data/data"))?
        .as_array()?;
    let strs: Vec<String> = rows.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
    let mut prices = parse_gtimg_rows(&strs)?;
    if code.starts_with("us") {
        prices.retain(|&p| p > 0.0);
        if prices.len() < 2 {
            return None;
        }
    }
    Some(prices)
}

pub struct Store {
    pub configs: Vec<StockConfig>,
    pub proxy: String,
    pub ttl: i64,
    // ponytail: 全局 priceHist 改为 Store 内 Mutex, 跟随实例释放, 上限 1440 点/股
    hist: Mutex<HashMap<String, Vec<f64>>>,
    cache: Mutex<(Vec<Quote>, Option<Instant>)>,
}

impl Store {
    pub fn new(configs: Vec<StockConfig>, proxy: String, ttl: i64) -> Self {
        Self {
            configs,
            proxy,
            ttl,
            hist: Mutex::new(HashMap::new()),
            cache: Mutex::new((Vec::new(), None)),
        }
    }

    /// 需要抓的股票 (view 过滤后的子集). 调用方传入 effective/is_auto/matching.
    pub fn needed<'a>(
        &'a self,
        effective: &str,
        is_auto: bool,
        matching: &[String],
    ) -> Vec<&'a StockConfig> {
        if effective == "ALL" {
            return self.configs.iter().collect();
        }
        if !is_auto {
            if effective.is_empty() {
                return vec![];
            }
            return self.configs.iter().filter(|c| c.group == effective).collect();
        }
        if matching.is_empty() {
            return vec![];
        }
        self.configs.iter().filter(|c| matching.contains(&c.group)).collect()
    }

    /// 全量刷新: qt 一次 + 分时并发 (std::thread::scope, 无 async 运行时).
    pub fn refresh(&self, effective: &str, is_auto: bool, matching: &[String]) -> Vec<Quote> {
        let needed = self.needed(effective, is_auto, matching);
        let agent = agent(&self.proxy);
        let tencent: Vec<String> = needed
            .iter()
            .filter(|c| c.source == Source::Tencent)
            .map(|c| c.code.clone())
            .collect();
        let qt = fetch_qt(&agent, &tencent);

        let mut charts: HashMap<String, Chart> = HashMap::new();
        let mu = Mutex::new(&mut charts);
        std::thread::scope(|s| {
            for c in needed.iter() {
                s.spawn(|| {
                    let ch = match c.source {
                        Source::Tencent => Chart {
                            prices: fetch_gtimg_minute(&agent, &c.code).unwrap_or_default(),
                            ..Chart::default()
                        },
                        Source::Yahoo => fetch_yahoo(&agent, &c.code),
                    };
                    mu.lock().unwrap().insert(c.code.clone(), ch);
                });
            }
        });

        let mut items = Vec::with_capacity(needed.len());
        for c in needed {
            let ch = charts.remove(&c.code).unwrap_or_default();
            let (price, change, pct, prev, mut prices) = match c.source {
                Source::Tencent => {
                    let mut vals = qt.get(&c.code).cloned().unwrap_or_default();
                    if vals.is_empty() {
                        let clean: String = c
                            .code
                            .strip_prefix("sh")
                            .or_else(|| c.code.strip_prefix("sz"))
                            .unwrap_or(&c.code)
                            .to_string();
                        vals = qt.get(&clean).cloned().unwrap_or_default();
                    }
                    let (p, chg, pct, prv) = parse_qt(&c.code, &vals);
                    (p, chg, pct, prv, ch.prices)
                }
                Source::Yahoo => {
                    let (mut p, mut prv) = (ch.y_price, ch.y_prev);
                    let (mut chg, mut pct) = (0.0, 0.0);
                    if prv != 0.0 {
                        chg = p - prv;
                        pct = chg / prv * 100.0;
                    }
                    if c.code == "^VIX" && p == 0.0 {
                        p = 0.0;
                        prv = 0.0;
                    }
                    (p, chg, pct, prv, ch.prices)
                }
            };
            // is_chart + hist 兜底 (原逻辑保留, 但收敛到 Store 内)
            let chart_ok = match c.source {
                Source::Tencent => {
                    c.code.starts_with("sh") || c.code.starts_with("sz") || c.code.starts_with("us")
                }
                Source::Yahoo => prices.len() >= 2 || price > 0.0,
            };
            if !chart_ok {
                prices.clear();
            } else if prices.len() < 2 {
                let mut hist = self.hist.lock().unwrap();
                let h = hist.entry(c.code.clone()).or_default();
                if price > 0.0 {
                    h.push(price);
                    if h.len() > 1440 {
                        let cut = h.len() - 1440;
                        h.drain(..cut);
                    }
                }
                if h.len() > 2 {
                    prices = h.clone();
                }
            } else if price > 0.0 {
                let mut hist = self.hist.lock().unwrap();
                let h = hist.entry(c.code.clone()).or_default();
                h.push(price);
                if h.len() > 1440 {
                    let cut = h.len() - 1440;
                    h.drain(..cut);
                }
            }
            items.push(Quote {
                code: c.code.clone(),
                name: c.name.clone(),
                group: c.group.clone(),
                price,
                prev,
                change,
                pct,
                prices,
                timestamps: ch.timestamps,
                regular_start: ch.reg_start,
                regular_end: ch.reg_end,
                chart_prev_close: ch.chart_prev,
            });
        }
        *self.cache.lock().unwrap() = (items.clone(), Some(Instant::now()));
        items
    }

    /// TTL 缓存命中则直接返回, 否则 refresh.
    pub fn get(&self, effective: &str, is_auto: bool, matching: &[String]) -> Vec<Quote> {
        let need: Vec<&StockConfig> = self.needed(effective, is_auto, matching);
        if need.is_empty() {
            return vec![];
        }
        {
            let guard = self.cache.lock().unwrap();
            if let (cached, Some(t)) = (&guard.0, guard.1) {
                if t.elapsed().as_secs() <= self.ttl.max(0) as u64 && !cached.is_empty() {
                    let have: std::collections::HashSet<&str> =
                        cached.iter().map(|q| q.code.as_str()).collect();
                    if need.iter().all(|c| have.contains(c.code.as_str())) {
                        return cached.clone();
                    }
                }
            }
        }
        self.refresh(effective, is_auto, matching)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_qt_tilde_and_comma() {
        let body = r#"v_sh000001="1,2,3,4";v_hf_GC="100,1.5~x";"#;
        let m = split_qt(body);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn yahoo_bad_json_empty() {
        let c = parse_yahoo("garbage");
        assert!(c.prices.is_empty());
    }
}
