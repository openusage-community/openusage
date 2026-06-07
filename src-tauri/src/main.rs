// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn prefer_x11_backend_for_panel_positioning() {
    if should_force_x11_backend(
        std::env::var_os("GDK_BACKEND").as_deref(),
        std::env::var_os("DISPLAY").as_deref(),
    ) {
        // Wayland compositors usually ignore absolute window positioning.
        // The tray panel needs X11/XWayland so it can open next to the tray icon.
        unsafe {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }
}

#[cfg(target_os = "linux")]
fn should_force_x11_backend(
    current_backend: Option<&std::ffi::OsStr>,
    display: Option<&std::ffi::OsStr>,
) -> bool {
    display.is_some() && current_backend.and_then(|value| value.to_str()) != Some("x11")
}

fn main() {
    #[cfg(target_os = "linux")]
    prefer_x11_backend_for_panel_positioning();

    openusage_lib::run()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn forces_x11_when_wayland_backend_is_inherited() {
        assert!(should_force_x11_backend(
            Some(OsStr::new("wayland")),
            Some(OsStr::new(":0")),
        ));
    }

    #[test]
    fn does_not_force_x11_without_display() {
        assert!(!should_force_x11_backend(Some(OsStr::new("wayland")), None,));
    }

    #[test]
    fn leaves_existing_x11_backend_alone() {
        assert!(!should_force_x11_backend(
            Some(OsStr::new("x11")),
            Some(OsStr::new(":0")),
        ));
    }
}
