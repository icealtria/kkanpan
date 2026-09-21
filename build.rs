// 只在 Linux 目标上链接 FBInk 静态库（本机 macOS 调试不链接，走 PNG stub）。
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("linux") {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        for dir in ["fbinklib"] {
            let p = std::path::Path::new(&manifest).join(dir);
            if p.join("libfbink.a").exists() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
        println!("cargo:rustc-link-lib=static=fbink");
    }
}
