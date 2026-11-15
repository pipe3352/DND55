//! Platform integration: data folders, portable mode, crash reports, log file, window icon.
//!
//! Windows 10/11 is the primary platform:
//! * installed: saves/settings/logs in `%APPDATA%\DND55\` (survives uninstall and updates);
//! * portable: if a file named `portable.txt` sits next to `DND55.exe`, everything is stored in a
//!   `data\` folder beside the executable instead (for USB sticks / zip releases).

use std::io::Write;
use std::path::PathBuf;

pub const APP_DIR: &str = "DND55";

/// Folder that holds `campaigns.db`, `settings.json` and `logs\`.
pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DND55_DATA_DIR") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    {
        if exe_dir.join("portable.txt").exists() {
            return exe_dir.join("data");
        }
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join(APP_DIR);
    }
    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library/Application Support")
            .join(APP_DIR);
    }
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(x).join(APP_DIR);
    }
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local/share").join(APP_DIR))
        .unwrap_or_else(|_| PathBuf::from("DND55-data"))
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Shows a native error dialog on Windows; prints to stderr elsewhere.
pub fn error_dialog(title: &str, message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let (t, m) = (wide(title), wide(message));
        // SAFETY: both buffers are valid NUL-terminated UTF-16 strings that outlive the call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                m.as_ptr(),
                t.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    eprintln!("{title}: {message}");
}

/// Human explanation for common fatal errors (shown in the crash dialog).
pub fn explain(panic_text: &str) -> &'static str {
    let t = panic_text.to_lowercase();
    if t.contains("adapter")
        || t.contains("gpu")
        || t.contains("wgpu")
        || t.contains("surface")
        || t.contains("vulkan")
        || t.contains("dx12")
    {
        "DND55 could not start the graphics system.\n\n\
         • Update your graphics driver (NVIDIA / AMD / Intel website).\n\
         • Open Settings ▶ Graphics backend and try another option (DirectX 12 / Vulkan / OpenGL),\n\
           or start DND55 with the environment variable WGPU_BACKEND=dx12 (or vulkan, gl).\n\
         • Remote desktop and some virtual machines have no 3D acceleration."
    } else if t.contains("address") && t.contains("use") {
        "The network port is already in use. Close other DND55 windows or choose another port."
    } else {
        "DND55 hit an unexpected error."
    }
}

/// Probes the default audio output on a throwaway thread. Some systems (no output device, broken
/// drivers, remote sessions, Wine) make the audio library panic during device enumeration; in that
/// case DND55 starts without sound instead of crashing.
pub fn audio_available() -> bool {
    std::thread::Builder::new()
        .name("audio-probe".into())
        .spawn(|| {
            std::panic::catch_unwind(|| {
                use cpal::traits::{DeviceTrait, HostTrait};
                cpal::default_host()
                    .default_output_device()
                    .map(|d| d.default_output_config().is_ok())
                    .unwrap_or(false)
            })
            .unwrap_or(false)
        })
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or(false)
}

/// After a graphics-initialization failure, relaunch DND55 with the next backend
/// (automatic ▶ Vulkan ▶ OpenGL). Returns false when there is nothing left to try.
fn relaunch_with_next_backend() -> bool {
    let next = match std::env::var("WGPU_BACKEND").ok().as_deref() {
        None | Some("") | Some("dx12") => "vulkan",
        Some("vulkan") => "gl",
        _ => return false,
    };
    if std::env::var("DND55_NO_GPU_FALLBACK").is_ok() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env("WGPU_BACKEND", next)
        .spawn()
        .is_ok()
}

/// Installs a panic hook that writes `logs\crash-<time>.txt` and shows a dialog.
pub fn install_crash_handler() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        if std::thread::current().name() == Some("audio-probe") {
            return;
        }
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dir = logs_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("crash-{secs}.txt"));
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = writeln!(
                f,
                "DND55 {} crash report\nOS: {} {}\nThread: {:?}\nLocation: {location}\nMessage: {msg}\n\nBacktrace:\n{}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
                std::thread::current().name(),
                std::backtrace::Backtrace::force_capture()
            );
        }
        if std::thread::current().name() == Some("audio-probe") {
            return; // expected on systems without audio; handled by audio_available()
        }
        // Graphics failed: silently retry with another backend before bothering the user.
        if std::thread::current().name() == Some("main")
            && explain(&msg).contains("graphics")
            && relaunch_with_next_backend()
        {
            std::process::exit(0);
        }
        // Only the main thread shows a dialog (network threads may panic on shutdown harmlessly).
        if std::thread::current().name() == Some("main") {
            error_dialog(
                "DND55 — error",
                &format!(
                    "{}\n\nDetails: {msg}\n\nA crash report was saved to:\n{}",
                    explain(&msg),
                    path.display()
                ),
            );
        }
        default(info);
    }));
}

/// Log file layer for Bevy's LogPlugin (`logs\dnd55.log`, overwritten each launch).
pub fn file_log_layer(_app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::{self, Layer};
    let dir = logs_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let file = std::fs::File::create(dir.join("dnd55.log")).ok()?;
    Some(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .boxed(),
    )
}

/// The window/taskbar icon (the .exe icon itself is embedded by `build.rs`).
pub fn set_window_icon(
    _marker: bevy::ecs::system::NonSendMarker,
    mut done: bevy::prelude::Local<u8>,
) {
    if *done > 3 {
        return;
    }
    let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon-64.png")) else {
        return;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) else {
        return;
    };
    bevy::winit::WINIT_WINDOWS.with_borrow(|ww| {
        for window in ww.windows.values() {
            window.set_window_icon(Some(icon.clone()));
            *done += 1;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_errors_get_a_helpful_explanation() {
        assert!(
            explain("Unable to find a GPU! Make sure you have installed required drivers!")
                .contains("graphics")
        );
        assert!(explain("something else").contains("unexpected"));
    }

    #[test]
    fn data_dir_override() {
        // SAFETY: tests in this module don't read this variable concurrently.
        unsafe { std::env::set_var("DND55_DATA_DIR", "/tmp/dnd55-test-dir") };
        assert_eq!(data_dir(), PathBuf::from("/tmp/dnd55-test-dir"));
        unsafe { std::env::remove_var("DND55_DATA_DIR") };
    }
}
