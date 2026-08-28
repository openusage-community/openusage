use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, PhysicalPosition, Position, Size};

#[cfg(target_os = "linux")]
use gtk::prelude::*;

use crate::panel::{
    position_panel_at_logical_anchor, position_panel_at_tray_click, position_panel_from_tray,
};

#[cfg(target_os = "linux")]
static LINUX_FOCUS_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static PANEL_IS_OPEN: AtomicBool = AtomicBool::new(false);

fn register_panel_opened() {
    PANEL_IS_OPEN.store(true, Ordering::SeqCst);
}

fn register_panel_closed() {
    PANEL_IS_OPEN.store(false, Ordering::SeqCst);
}

fn should_hide_for_focus_loss(is_visible: bool, is_open: bool) -> bool {
    is_visible && is_open
}

fn register_panel_focus_loss(is_visible: bool) -> bool {
    should_hide_for_focus_loss(is_visible, PANEL_IS_OPEN.load(Ordering::Acquire))
}

#[cfg(target_os = "linux")]
fn present_gtk_window(window: &tauri::WebviewWindow) {
    if let Ok(gtk_window) = window.gtk_window() {
        gtk_window.present();
    }
}

#[cfg(not(target_os = "linux"))]
fn present_gtk_window(_window: &tauri::WebviewWindow) {}

pub(crate) fn apply_panel_position(
    app_handle: &AppHandle,
    panel_x: f64,
    panel_y: f64,
    _primary_logical_h: f64,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    log::debug!(
        "apply_panel_position: requested logical position=({:.0},{:.0})",
        panel_x,
        panel_y
    );
    if let Err(e) = window.set_position(tauri::LogicalPosition::new(panel_x, panel_y)) {
        log::warn!(
            "apply_panel_position: set_position failed (best-effort): {}",
            e
        );
    }
}

/// No NSPanel on non-macOS; the regular window is configured via tauri.conf.json.
pub fn init(app_handle: &AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "linux")]
    init_linux_focus_loss_handler(app_handle)?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn init_linux_focus_loss_handler(app_handle: &AppHandle) -> tauri::Result<()> {
    if LINUX_FOCUS_HANDLER_INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }

    let Some(window) = app_handle.get_webview_window("main") else {
        return Ok(());
    };
    let app_handle = app_handle.clone();

    window.on_window_event(move |event| {
        let tauri::WindowEvent::Focused(false) = event else {
            return;
        };
        let is_visible = app_handle
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);

        if register_panel_focus_loss(is_visible) {
            hide_panel(&app_handle);
        }
    });

    LINUX_FOCUS_HANDLER_INSTALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Show the window as a floating panel, positioned under the tray icon.
pub fn show_panel(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        present_gtk_window(&window);
        register_panel_opened();
        return;
    }

    let _ = window.set_always_on_top(true);
    position_panel_from_tray(app_handle);
    let _ = window.show();
    position_panel_from_tray(app_handle);
    let _ = window.set_focus();
    present_gtk_window(&window);
    register_panel_opened();
}

fn show_panel_at_tray_icon(
    app_handle: &AppHandle,
    click_position: PhysicalPosition<f64>,
    icon_position: Position,
    icon_size: Size,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    let _ = window.set_always_on_top(true);
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.show();
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.set_focus();
    present_gtk_window(&window);
    register_panel_opened();
}

pub fn show_panel_at_logical_anchor(
    app_handle: &AppHandle,
    center_x: f64,
    top_y: f64,
    bottom_y: f64,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    let _ = window.set_always_on_top(true);
    position_panel_at_logical_anchor(app_handle, center_x, top_y, bottom_y);
    let _ = window.show();
    position_panel_at_logical_anchor(app_handle, center_x, top_y, bottom_y);
    let _ = window.set_focus();
    present_gtk_window(&window);
    register_panel_opened();
}

/// Toggle window visibility.
pub fn toggle_panel(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        log::debug!("toggle_panel: hiding window");
        hide_panel(app_handle);
    } else {
        log::debug!("toggle_panel: showing window");
        show_panel(app_handle);
    }
}

pub fn toggle_panel_at_tray_icon(
    app_handle: &AppHandle,
    click_position: PhysicalPosition<f64>,
    icon_position: Position,
    icon_size: Size,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        log::debug!("toggle_panel_at_tray_icon: hiding window");
        hide_panel(app_handle);
    } else {
        log::debug!("toggle_panel_at_tray_icon: showing window");
        show_panel_at_tray_icon(app_handle, click_position, icon_position, icon_size);
    }
}

pub fn hide_panel(app_handle: &AppHandle) {
    register_panel_closed();
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset_panel_state_for_test() {
        PANEL_IS_OPEN.store(false, Ordering::SeqCst);
    }

    #[test]
    fn visible_open_panel_hides_on_focus_loss() {
        assert!(should_hide_for_focus_loss(true, true));
    }

    #[test]
    fn hidden_or_closed_panel_ignores_focus_loss() {
        assert!(!should_hide_for_focus_loss(false, true));
        assert!(!should_hide_for_focus_loss(true, false));
    }

    #[test]
    #[serial]
    fn open_panel_hides_on_focus_loss() {
        reset_panel_state_for_test();
        register_panel_opened();

        assert!(register_panel_focus_loss(true));
    }

    #[test]
    #[serial]
    fn repeated_internal_activity_does_not_break_later_focus_loss_close() {
        reset_panel_state_for_test();
        register_panel_opened();
        register_panel_opened();
        register_panel_opened();

        assert!(register_panel_focus_loss(true));
    }

    #[test]
    #[serial]
    fn closed_panel_does_not_hide_on_later_focus_loss() {
        reset_panel_state_for_test();
        register_panel_opened();

        register_panel_closed();

        assert!(!register_panel_focus_loss(true));
    }

    #[test]
    #[serial]
    fn reopened_panel_hides_on_focus_loss() {
        reset_panel_state_for_test();
        register_panel_opened();
        register_panel_closed();

        register_panel_opened();

        assert!(register_panel_focus_loss(true));
    }

    #[test]
    #[serial]
    fn closing_panel_resets_active_state() {
        reset_panel_state_for_test();
        register_panel_opened();

        register_panel_closed();

        assert!(!PANEL_IS_OPEN.load(Ordering::Acquire));
    }
}
