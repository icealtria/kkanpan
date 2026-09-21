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
