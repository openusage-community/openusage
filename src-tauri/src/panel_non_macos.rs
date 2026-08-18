use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

#[cfg(target_os = "linux")]
use gtk::prelude::*;

use crate::panel::{
    position_panel_at_logical_anchor, position_panel_at_tray_click, position_panel_from_tray,
};

const CLICK_CATCHER_LABEL: &str = "panel-click-catcher";
const CLICK_CATCHER_URL: &str = "index.html?overlay=panel-click-catcher";
static PANEL_IS_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct LogicalOverlayBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn register_panel_opened() {
    PANEL_IS_OPEN.store(true, Ordering::SeqCst);
}

fn register_panel_closed() {
    PANEL_IS_OPEN.store(false, Ordering::SeqCst);
}

#[cfg(target_os = "linux")]
static LINUX_FOCUS_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

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

fn monitor_logical_bounds(monitor: &tauri::Monitor) -> LogicalOverlayBounds {
    let scale = monitor.scale_factor();
    LogicalOverlayBounds {
        x: monitor.position().x as f64 / scale,
        y: monitor.position().y as f64 / scale,
        width: monitor.size().width as f64 / scale,
        height: monitor.size().height as f64 / scale,
    }
}

fn merge_overlay_bounds(
    current: Option<LogicalOverlayBounds>,
    next: LogicalOverlayBounds,
) -> LogicalOverlayBounds {
    match current {
        Some(current) => {
            let min_x = current.x.min(next.x);
            let min_y = current.y.min(next.y);
            let max_x = (current.x + current.width).max(next.x + next.width);
            let max_y = (current.y + current.height).max(next.y + next.height);
            LogicalOverlayBounds {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        }
        None => next,
    }
}

fn click_catcher_bounds(window: &tauri::WebviewWindow) -> Option<LogicalOverlayBounds> {
    let monitors = window.available_monitors().ok()?;
    let mut bounds = None;
    for monitor in &monitors {
        bounds = Some(merge_overlay_bounds(
            bounds,
            monitor_logical_bounds(monitor),
        ));
    }
    bounds
}

fn get_or_create_click_catcher(app_handle: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app_handle.get_webview_window(CLICK_CATCHER_LABEL) {
        return Some(window);
    }

    match WebviewWindowBuilder::new(
        app_handle,
        CLICK_CATCHER_LABEL,
        WebviewUrl::App(CLICK_CATCHER_URL.into()),
    )
    .title("")
    .decorations(false)
    .transparent(true)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .visible(false)
    .focused(false)
    .focusable(false)
    .shadow(false)
    .inner_size(1.0, 1.0)
    .build()
    {
        Ok(window) => Some(window),
        Err(error) => {
            log::warn!("click catcher: failed to create overlay window: {error}");
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn should_show_click_catcher() -> bool {
    // The always-on-top click-catcher can stack above the panel on some compositors
    // (KDE/XWayland) and intercept panel clicks; dismiss via focus loss instead.
    false
}

#[cfg(not(target_os = "linux"))]
fn should_show_click_catcher() -> bool {
    true
}

fn show_click_catcher(app_handle: &AppHandle) {
    if !should_show_click_catcher() {
        return;
    }

    let Some(main_window) = app_handle.get_webview_window("main") else {
        return;
    };
    let Some(click_catcher) = get_or_create_click_catcher(app_handle) else {
        return;
    };

    if let Some(bounds) = click_catcher_bounds(&main_window) {
        let _ = click_catcher.set_position(LogicalPosition::new(bounds.x, bounds.y));
        let _ = click_catcher.set_size(LogicalSize::new(bounds.width, bounds.height));
    }

    let _ = click_catcher.set_always_on_top(true);
    let _ = click_catcher.set_focusable(false);
    let _ = click_catcher.show();
}

fn hide_click_catcher(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(CLICK_CATCHER_LABEL) {
        let _ = window.hide();
    }
}

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
    eprintln!(
        "apply_panel_position requested logical=({:.0},{:.0})",
        panel_x, panel_y
    );
    if let Err(e) = window.set_position(tauri::LogicalPosition::new(panel_x, panel_y)) {
        log::warn!(
            "apply_panel_position: set_position failed (best-effort): {}",
            e
        );
        eprintln!("apply_panel_position set_position failed: {e}");
        return;
    }
    match window.outer_position() {
        Ok(position) => {
            eprintln!(
                "apply_panel_position actual outer physical=({},{})",
                position.x, position.y
            );
        }
        Err(error) => {
            eprintln!("apply_panel_position actual outer position unavailable: {error}");
        }
    }
}

/// No NSPanel on non-macOS; the regular window is configured via tauri.conf.json.
pub fn init(app_handle: &AppHandle) -> tauri::Result<()> {
    // Dismiss on focus loss instead of the click-catcher (see should_show_click_catcher):
    // clicks inside the panel keep the toplevel focused, so interaction doesn't dismiss it.
    #[cfg(target_os = "linux")]
    init_linux_focus_loss_handler(app_handle)?;
    #[cfg(not(target_os = "linux"))]
    let _ = app_handle;
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
        show_click_catcher(app_handle);
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        present_gtk_window(&window);
        register_panel_opened();
        return;
    }

    show_click_catcher(app_handle);
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
    show_click_catcher(app_handle);
    let _ = window.set_always_on_top(true);
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.show();
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.set_focus();
    present_gtk_window(&window);
    register_panel_opened();
}

pub fn show_panel_at_logical_anchor(app_handle: &AppHandle, center_x: f64, bottom_y: f64) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    show_click_catcher(app_handle);
    let _ = window.set_always_on_top(true);
    position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
    let _ = window.show();
    position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
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
    hide_click_catcher(app_handle);
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_dismisses_via_focus_loss_not_click_catcher() {
        assert!(!should_show_click_catcher());
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
