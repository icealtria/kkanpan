// 日志分级：默认只打重要日志（启动/抓取/用户操作/错误）；
// 设 KKANPAN_LOG=debug 才输出 [render]/[diff]/[font] 等高频调试日志。
use std::sync::OnceLock;

static DEBUG: OnceLock<bool> = OnceLock::new();

pub fn debug() -> bool {
    *DEBUG.get_or_init(|| {
        matches!(std::env::var("KKANPAN_LOG").as_deref(), Ok("debug") | Ok("verbose") | Ok("1"))
    })
}

#[macro_export]
macro_rules! dlog {
    ($($t:tt)*) => {
        if $crate::util::debug() {
            eprintln!($($t)*)
        }
    };
}
