use std::path::Path;
use std::process::Command;

fn main() {
    // whisper.cpp's Metal backend uses Objective-C `@available` checks. When hams targets
    // a macOS older than the build machine those compile to a call into clang's runtime
    // library, which rustc does not link by default.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        if let Ok(out) = Command::new("clang").arg("--print-file-name=libclang_rt.osx.a").output() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if Path::new(&path).is_absolute() && Path::new(&path).exists() {
                println!("cargo:rustc-link-arg={path}");
            }
        }
    }
    tauri_build::build()
}
