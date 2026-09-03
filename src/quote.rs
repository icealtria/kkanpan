//! 行情解析纯函数 (原 fetch.go parseQT / parseGtimgRows + render.go 文案).
//! 全部无 I/O, 可单测. 失败返回 0 而不是 recover (Go 原版用 recover 吞 panic).

/// 腾讯 qt 行解析 -> (price, change, pct, prev)
pub fn parse_qt(code: &str, vals: &[String]) -> (f64, f64, f64, f64) {
    let num = |i: usize| -> f64 { vals.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0) };
    if code.starts_with("hf_") {
        let price = num(0);
        let change = num(1);
        let prev = price - change;
        let pct = if prev != 0.0 { change / prev * 100.0 } else { 0.0 };
        return (price, change, pct, prev);
    }
    let mut price = num(3);
    let mut prev = num(4);
    if code == "^VIX" {
        return (0.0, 0.0, 0.0, 0.0);
    }
    if prev == 0.0 {
        prev = price;
    }
    let pct = num(32);
    let change = if vals.len() > 31 {
        let c = num(31);
        if c == 0.0 { price - prev } else { c }
    } else {
        price - prev
    };
    let _ = &mut price;
    (price, change, pct, prev)
}

/// gtimg 分时 "09:30 12.34 ..." 行 -> 价格序列 (<2 点视为无效)
pub fn parse_gtimg_rows(rows: &[String]) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut it = row.split_whitespace();
        it.next();
        if let Some(p) = it.next().and_then(|s| s.parse::<f64>().ok()) {
            out.push(p);
        }
    }
    if out.len() < 2 {
        return None;
    }
    Some(out)
}

/// 价格/涨跌文案 (原 stockStrings). svg=true 时用 ^/v ASCII 箭头 (eink 位图字体无 ▲▼).
pub fn stock_strings(price: f64, change: f64, pct: f64, svg: bool) -> (String, String) {
    let price_str = if price > 0.0 {
        format!("{price:.2}")
    } else {
        "--".into()
    };
    let arrow = if change > 0.0 {
        if svg {
            "^"
        } else {
            "▲"
        }
    } else if change < 0.0 {
        if svg {
            "v"
        } else {
            "▼"
        }
    } else if svg {
        ""
    } else {
        " "
    };
    (price_str, format!("{arrow} {change:+.2} ({pct:+.2}%)"))
}

/// sparkline 归一化: 返回 (min, max, range). 原 sparklinePoints 的纯计算部分.
pub fn sparkline_range(prices: &[f64]) -> Option<(f64, f64, f64)> {
    if prices.len() < 2 {
        return None;
    }
    let mut min = prices[0];
    let mut max = prices[0];
    for &p in &prices[1..] {
        if p < min {
            min = p;
        }
        if p > max {
            max = p;
        }
    }
    let mut rng = max - min;
    if rng == 0.0 {
        rng = 1.0;
    }
    Some((min, max, rng))
}

/// 昨收基准线取值: Yahoo 用 chartPrevClose, 腾讯用 prev.
/// large 风格含参考价扩展 min/max; normal 风格越界则不画 (返回 None).
pub fn ref_line(prev: f64, chart_prev: f64, is_yahoo: bool, min: f64, max: f64, large: bool) -> Option<f64> {
    let mut refv = if is_yahoo { chart_prev } else { prev };
    if refv == 0.0 {
        refv = (min + max) / 2.0;
    }
    if large {
        Some(refv)
    } else if refv < min || refv > max {
        None
    } else {
        Some(refv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_normal() {
        let mut vals = vec!["".to_string(); 33];
        vals[3] = "10.0".into();
        vals[4] = "9.0".into();
        vals[31] = "1.0".into();
        vals[32] = "11.11".into();
        assert_eq!(parse_qt("sh600000", &vals), (10.0, 1.0, 11.11, 9.0));
    }

    #[test]
    fn qt_short_does_not_panic() {
        assert_eq!(parse_qt("sh600000", &[]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn gtimg_rows() {
        let rows = vec!["0930 10.0 x".to_string(), "0931 10.5 y".to_string()];
        assert_eq!(parse_gtimg_rows(&rows), Some(vec![10.0, 10.5]));
        assert_eq!(parse_gtimg_rows(&["bad".to_string()]), None);
    }

    #[test]
    fn ref_line_normal_clips() {
        assert_eq!(ref_line(5.0, 0.0, false, 10.0, 12.0, false), None);
        assert_eq!(ref_line(5.0, 0.0, false, 10.0, 12.0, true), Some(5.0));
    }
}
