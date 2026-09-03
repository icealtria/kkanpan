use crate::config::AutoRule;
use chrono::{DateTime, FixedOffset};

pub fn cst_now() -> DateTime<FixedOffset> {
    chrono::Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
}

pub fn parse_hm(s: &str) -> i32 {
    let mut it = s.split(':');
    let h: i32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let m: i32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    h * 60 + m
}

/// 单条规则是否命中 (纯函数, 可单测; 原 touch.go matchRule)
pub fn match_rule(weekday: u32, hm: i32, r: &AutoRule) -> bool {
    // chrono weekday: Mon=1..Sun=7 -> Go 0=Sun..6=Sat
    let go_wd = (weekday % 7) as u8;
    if !r.weekdays.is_empty() && !r.weekdays.contains(&go_wd) {
        return false;
    }
    let start = parse_hm(&r.start);
    let end = parse_hm(&r.end);
    if start <= end {
        hm >= start && hm <= end
    } else {
        hm >= start || hm <= end
    }
}

fn now_wd_hm(now: &DateTime<FixedOffset>) -> (u32, i32) {
    use chrono::Datelike;
    use chrono::Timelike;
    (
        now.weekday().number_from_monday(),
        now.hour() as i32 * 60 + now.minute() as i32,
    )
}

/// 返回 (effective_group, is_auto); 原 GetEffectiveGroup
pub fn effective_group(mode: &str, rules: &[AutoRule], now: &DateTime<FixedOffset>) -> (String, bool) {
    if mode != "AUTO" && !mode.is_empty() {
        return (mode.to_string(), false);
    }
    if rules.is_empty() {
        return (String::new(), true);
    }
    let (wd, hm) = now_wd_hm(now);
    for r in rules {
        if match_rule(wd, hm, r) {
            return (r.group.clone(), true);
        }
    }
    (String::new(), true)
}

/// 原 GetMatchingAutoGroups: AUTO 下同时命中的所有组 (去重保序)
pub fn matching_groups(rules: &[AutoRule], now: &DateTime<FixedOffset>) -> Vec<String> {
    let (wd, hm) = now_wd_hm(now);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in rules {
        if match_rule(wd, hm, r) && seen.insert(r.group.clone()) {
            out.push(r.group.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn cst(y: i32, mo: u32, d: u32, h: u32, mi: u32, wd_check: bool) -> DateTime<FixedOffset> {
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        let dt = tz.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
        if wd_check {
            let _ = dt;
        }
        dt
    }

    #[test]
    fn weekday_and_window() {
        // 2026-09-03 是周四 (Go wd=4)
        let now = cst(2026, 9, 3, 10, 0, true);
        let r = AutoRule {
            group: "a-share".into(),
            weekdays: vec![1, 2, 3, 4, 5],
            start: "09:00".into(),
            end: "15:30".into(),
        };
        assert!(match_rule(4, 600, &r));
        assert!(!match_rule(4, 8 * 60, &r));
        assert!(!match_rule(6, 600, &r)); // 周六不命中
        let (g, auto) = effective_group("AUTO", &[r], &now);
        assert_eq!((g.as_str(), auto), ("a-share", true));
        // 非 AUTO 直接透传
        let (g, auto) = effective_group("US", &[], &now);
        assert_eq!((g.as_str(), auto), ("US", false));
    }

    #[test]
    fn overnight_window() {
        let r = AutoRule {
            group: "US".into(),
            weekdays: vec![],
            start: "22:00".into(),
            end: "02:00".into(),
        };
        assert!(match_rule(4, 23 * 60, &r));
        assert!(match_rule(4, 60, &r));
        assert!(!match_rule(4, 12 * 60, &r));
    }
}
