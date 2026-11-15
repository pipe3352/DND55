//! Embeds the application icon and version info into DND55.exe when building for Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        // Cross-compiling from Linux uses MinGW's windres; on Windows the SDK's rc.exe is found automatically.
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") && cfg!(not(windows)) {
            res.set_toolkit_path("/usr/bin");
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        if let Err(e) = res.compile() {
            // Never fail the build over an icon.
            println!("cargo:warning=could not embed Windows icon: {e}");
        }
    }
}
