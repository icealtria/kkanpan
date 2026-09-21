//! Build script: link the vendored libfbink.a static library.
//!
//! The pre-built ARM static library lives at ../../vendor-fbink/fbinklib/libfbink.a
//! relative to the kkanpan repo root. For a standalone pocketjs checkout, place
//! the library at a path set by the FBINK_LIB_DIR env var.

fn main() {
    // Try the kkanpan vendor path first, then env override
    let lib_paths = [
        "../../vendor-fbink/fbinklib",
        "../vendor-fbink/fbinklib",
    ];

    let lib_dir = std::env::var("FBINK_LIB_DIR").ok().or_else(|| {
        for p in &lib_paths {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
        None
    });

    if let Some(dir) = lib_dir {
        println!("cargo:rustc-link-search=native={}", dir);
        println!("cargo:rustc-link-lib=static=fbink");
        // FBInk needs these system libraries
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=m");
    } else {
        // Fallback: try system libfbink
        println!("cargo:rustc-link-lib=fbink");
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=m");
    }

    // Re-run if the library changes
    println!("cargo:rerun-if-changed=../../vendor-fbink/fbinklib/libfbink.a");
}
