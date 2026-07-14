// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn prefer_x11_backend_for_panel_positioning() {
    if should_force_x11_backend(
        std::env::var_os("GDK_BACKEND").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("XDG_SESSION_TYPE").as_deref(),
        std::env::var_os("DISPLAY").as_deref(),
    ) {
        // Wayland ignores client window positioning; route through XWayland so the tray
        // panel can be placed under the tray icon.
        unsafe {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }
}

#[cfg(target_os = "linux")]
fn should_force_x11_backend(
    current_backend: Option<&std::ffi::OsStr>,
    wayland_display: Option<&std::ffi::OsStr>,
    session_type: Option<&std::ffi::OsStr>,
    display: Option<&std::ffi::OsStr>,
) -> bool {
    // Never override an explicit GDK_BACKEND.
    if current_backend.is_some() {
        return false;
    }
    // The x11 backend needs a reachable X server.
    if display.is_none() {
        return false;
    }
    // Wayland detection: DISPLAY is also set under XWayland, so it can't distinguish X11
    // from Wayland. Key off WAYLAND_DISPLAY (GDK's own signal), then XDG_SESSION_TYPE.
    wayland_display.is_some() || session_type.and_then(|value| value.to_str()) == Some("wayland")
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

    fn os(value: &str) -> Option<&OsStr> {
        Some(OsStr::new(value))
    }

    #[test]
    fn forces_x11_on_wayland_session_with_xwayland() {
        // unset backend, WAYLAND_DISPLAY + DISPLAY set.
        assert!(should_force_x11_backend(
            None,
            os("wayland-0"),
            os("wayland"),
            os(":0"),
        ));
    }

    #[test]
    fn forces_x11_when_only_session_type_signals_wayland() {
        // no WAYLAND_DISPLAY, session_type=wayland, DISPLAY set.
        assert!(should_force_x11_backend(None, None, os("wayland"), os(":0")));
    }

    #[test]
    fn does_not_force_on_pure_x11_session() {
        // no Wayland signals, DISPLAY set: already X11.
        assert!(!should_force_x11_backend(None, None, os("x11"), os(":0")));
    }

    #[test]
    fn does_not_force_without_xwayland() {
        // Wayland session, no DISPLAY: stay on Wayland.
        assert!(!should_force_x11_backend(None, os("wayland-0"), os("wayland"), None));
    }

    #[test]
    fn respects_explicit_wayland_backend() {
        // explicit GDK_BACKEND=wayland.
        assert!(!should_force_x11_backend(
            os("wayland"),
            os("wayland-0"),
            os("wayland"),
            os(":0"),
        ));
    }

    #[test]
    fn leaves_existing_x11_backend_alone() {
        assert!(!should_force_x11_backend(
            os("x11"),
            os("wayland-0"),
            os("wayland"),
            os(":0"),
        ));
    }
}
