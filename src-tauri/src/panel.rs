use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position, Size};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel,
};

use crate::panel_geometry::{
    LogicalAnchor, LogicalMonitorBounds, PanelAnchorPosition, compute_anchor_position,
    fallback_anchor_for_monitor,
};

const PANEL_ANCHOR_OFFSET_EVENT: &str = "panel:anchor-offset";
static LINUX_PANEL_ANCHOR: OnceLock<Mutex<Option<LogicalAnchor>>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn linux_panel_anchor_state() -> &'static Mutex<Option<LogicalAnchor>> {
    LINUX_PANEL_ANCHOR.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "linux")]
pub(crate) fn remember_linux_panel_anchor(center_x: f64, bottom_y: f64) {
    if !center_x.is_finite() || !bottom_y.is_finite() {
        return;
    }

    let mut anchor = linux_panel_anchor_state()
        .lock()
        .expect("linux panel anchor state poisoned");
    *anchor = Some(LogicalAnchor { center_x, bottom_y });
}

#[cfg(target_os = "linux")]
fn remembered_linux_panel_anchor() -> Option<LogicalAnchor> {
    *linux_panel_anchor_state()
        .lock()
        .expect("linux panel anchor state poisoned")
}

fn monitor_contains_physical_point(
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
    point_x: f64,
    point_y: f64,
) -> bool {
    point_x >= origin_x
        && point_x < origin_x + width
        && point_y >= origin_y
        && point_y < origin_y + height
}

fn logical_bounds_from_monitor(monitor: &tauri::Monitor) -> LogicalMonitorBounds {
    let scale = monitor.scale_factor();
    LogicalMonitorBounds {
        x: monitor.position().x as f64 / scale,
        y: monitor.position().y as f64 / scale,
        width: monitor.size().width as f64 / scale,
        height: monitor.size().height as f64 / scale,
    }
}

fn monitor_containing_physical_point<'a>(
    monitors: &'a [tauri::Monitor],
    point_x: f64,
    point_y: f64,
) -> Option<&'a tauri::Monitor> {
    monitors.iter().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        monitor_contains_physical_point(
            origin.x as f64,
            origin.y as f64,
            size.width as f64,
            size.height as f64,
            point_x,
            point_y,
        )
    })
}

fn monitor_contains_logical_point(monitor: &tauri::Monitor, point_x: f64, point_y: f64) -> bool {
    let bounds = logical_bounds_from_monitor(monitor);
    point_x >= bounds.x
        && point_x < bounds.x + bounds.width
        && point_y >= bounds.y
        && point_y < bounds.y + bounds.height
}

fn monitor_containing_logical_point<'a>(
    monitors: &'a [tauri::Monitor],
    point_x: f64,
    point_y: f64,
) -> Option<&'a tauri::Monitor> {
    monitors
        .iter()
        .find(|monitor| monitor_contains_logical_point(monitor, point_x, point_y))
}

fn emit_panel_anchor_offset(app_handle: &AppHandle, arrow_offset_px: f64) {
    if let Err(error) = app_handle.emit(PANEL_ANCHOR_OFFSET_EVENT, arrow_offset_px) {
        log::debug!("emit_panel_anchor_offset: failed: {}", error);
    }
}

/// Retrieve the tray icon rect and position the panel beneath it.
/// No-ops gracefully if the tray icon or its rect is unavailable.
pub(crate) fn position_panel_from_tray(app_handle: &AppHandle) {
    let Some(tray) = app_handle.tray_by_id("tray") else {
        log::debug!("position_panel_from_tray: tray icon not found");
        #[cfg(not(target_os = "macos"))]
        position_panel_at_fallback_anchor(app_handle);
        return;
    };
    match tray.rect() {
        Ok(Some(rect)) => {
            log::debug!(
                "position_panel_from_tray: tray rect position={:?} size={:?}",
                rect.position,
                rect.size
            );
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        Ok(None) => {
            log::debug!("position_panel_from_tray: tray rect not available; using fallback anchor");
            #[cfg(not(target_os = "macos"))]
            position_panel_at_fallback_anchor(app_handle);
        }
        Err(e) => {
            log::warn!(
                "position_panel_from_tray: failed to get tray rect: {}; using fallback anchor",
                e
            );
            #[cfg(not(target_os = "macos"))]
            position_panel_at_fallback_anchor(app_handle);
        }
    }
}

/// Compute the desired logical top-left of the panel given the tray icon rect.
/// Returns `(panel_x, panel_y, primary_logical_height)` or `None` if geometry
/// can't be resolved. The math is platform-independent; only the final apply
/// step differs per OS (macOS uses flipped/bottom-left coordinates).
fn compute_panel_position(
    app_handle: &AppHandle,
    icon_position: Position,
    icon_size: Size,
) -> Option<(PanelAnchorPosition, f64)> {
    let window = app_handle.get_webview_window("main")?;

    let (icon_phys_x, icon_phys_y) = match &icon_position {
        Position::Physical(pos) => (pos.x as f64, pos.y as f64),
        Position::Logical(pos) => (pos.x, pos.y),
    };
    let (icon_phys_w, icon_phys_h) = match &icon_size {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    };

    let monitors = window.available_monitors().ok()?;
    let primary_logical_h = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.size().height as f64 / m.scale_factor())
        .unwrap_or(0.0);

    let (anchor_phys_x, anchor_phys_y) =
        tray_icon_anchor_position(icon_phys_x, icon_phys_y, icon_phys_w, icon_phys_h);

    let found_monitor = monitor_containing_physical_point(&monitors, anchor_phys_x, anchor_phys_y);

    let monitor = match found_monitor {
        Some(m) => m.clone(),
        None => {
            log::warn!(
                "No monitor found for tray rect center at ({:.0}, {:.0}), using primary",
                anchor_phys_x,
                anchor_phys_y
            );
            window.primary_monitor().ok().flatten()?
        }
    };

    let monitor_bounds = logical_bounds_from_monitor(&monitor);

    let target_scale = monitor.scale_factor();
    let mon_phys_x = monitor.position().x as f64;
    let mon_phys_y = monitor.position().y as f64;
    let anchor_logical_x = monitor_bounds.x + (anchor_phys_x - mon_phys_x) / target_scale;
    let icon_logical_y = monitor_bounds.y + (icon_phys_y - mon_phys_y) / target_scale;
    let icon_logical_h = icon_phys_h / target_scale;

    // Read panel width from the window, converted to logical points.
    // outer_size() returns physical pixels at the window's current scale factor.
    // If the window isn't available yet, parse the configured width from tauri.conf.json
    // (embedded at compile time) so it stays in sync automatically.
    let (panel_width, panel_height) = panel_size_from_window_or_config(&window);

    let anchor = LogicalAnchor {
        center_x: anchor_logical_x,
        bottom_y: icon_logical_y + icon_logical_h,
    };
    let position = compute_anchor_position(&monitor_bounds, anchor, panel_width, panel_height);
    log::debug!(
        "compute_panel_position: icon=({:.0},{:.0} {:.0}x{:.0}) anchor=({:.0},{:.0}) monitor=({:.0},{:.0} {:.0}x{:.0} scale={:.2}) panel_size=({:.0}x{:.0}) panel=({:.0},{:.0}) arrow_offset={:.0}",
        icon_phys_x,
        icon_phys_y,
        icon_phys_w,
        icon_phys_h,
        anchor_phys_x,
        anchor_phys_y,
        monitor_bounds.x,
        monitor_bounds.y,
        monitor_bounds.width,
        monitor_bounds.height,
        target_scale,
        panel_width,
        panel_height,
        position.x,
        position.y,
        position.arrow_offset_px
    );

    Some((position, primary_logical_h))
}

fn tray_icon_anchor_position(
    icon_phys_x: f64,
    icon_phys_y: f64,
    icon_phys_w: f64,
    icon_phys_h: f64,
) -> (f64, f64) {
    (icon_phys_x + (icon_phys_w / 2.0), icon_phys_y + icon_phys_h)
}

#[cfg(not(target_os = "macos"))]
fn compute_fallback_panel_position(app_handle: &AppHandle) -> Option<PanelAnchorPosition> {
    let window = app_handle.get_webview_window("main")?;
    let monitor = window.primary_monitor().ok().flatten()?;
    let monitor_bounds = logical_bounds_from_monitor(&monitor);
    let anchor = fallback_anchor_for_monitor(&monitor_bounds);
    let (panel_width, panel_height) = panel_size_from_window_or_config(&window);
    log::debug!(
        "compute_fallback_panel_position: monitor=({:.0},{:.0} {:.0}x{:.0}) anchor=({:.0},{:.0}) panel_size=({:.0}x{:.0})",
        monitor_bounds.x,
        monitor_bounds.y,
        monitor_bounds.width,
        monitor_bounds.height,
        anchor.center_x,
        anchor.bottom_y,
        panel_width,
        panel_height
    );

    Some(compute_anchor_position(
        &monitor_bounds,
        anchor,
        panel_width,
        panel_height,
    ))
}

#[cfg(target_os = "linux")]
fn compute_remembered_panel_position(app_handle: &AppHandle) -> Option<PanelAnchorPosition> {
    let anchor = remembered_linux_panel_anchor()?;
    let window = app_handle.get_webview_window("main")?;
    let monitors = window.available_monitors().ok()?;
    let monitor = monitor_containing_logical_point(&monitors, anchor.center_x, anchor.bottom_y)
        .cloned()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    let monitor_bounds = logical_bounds_from_monitor(&monitor);
    let (panel_width, panel_height) = panel_size_from_window_or_config(&window);
    let position = compute_anchor_position(&monitor_bounds, anchor, panel_width, panel_height);
    log::debug!(
        "compute_remembered_panel_position: anchor=({:.0},{:.0}) panel=({:.0},{:.0})",
        anchor.center_x,
        anchor.bottom_y,
        position.x,
        position.y
    );
    Some(position)
}

#[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
fn compute_remembered_panel_position(_app_handle: &AppHandle) -> Option<PanelAnchorPosition> {
    None
}

fn compute_logical_anchor_panel_position(
    app_handle: &AppHandle,
    center_x: f64,
    bottom_y: f64,
) -> Option<PanelAnchorPosition> {
    let window = app_handle.get_webview_window("main")?;
    let monitors = window.available_monitors().ok()?;
    let monitor = monitor_containing_logical_point(&monitors, center_x, bottom_y)
        .cloned()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    let monitor_bounds = logical_bounds_from_monitor(&monitor);
    let (panel_width, panel_height) = panel_size_from_window_or_config(&window);
    let anchor = LogicalAnchor { center_x, bottom_y };
    let position = compute_anchor_position(&monitor_bounds, anchor, panel_width, panel_height);

    log::debug!(
        "compute_logical_anchor_panel_position: anchor=({:.0},{:.0}) monitor=({:.0},{:.0} {:.0}x{:.0}) panel_size=({:.0}x{:.0}) panel=({:.0},{:.0}) arrow_offset={:.0}",
        center_x,
        bottom_y,
        monitor_bounds.x,
        monitor_bounds.y,
        monitor_bounds.width,
        monitor_bounds.height,
        panel_width,
        panel_height,
        position.x,
        position.y,
        position.arrow_offset_px
    );

    Some(position)
}

fn configured_panel_size() -> (f64, f64) {
    let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
        .expect("tauri.conf.json must be valid JSON");
    let window = &conf["app"]["windows"][0];
    (
        window["width"]
            .as_f64()
            .expect("width must be set in tauri.conf.json"),
        window["height"]
            .as_f64()
            .expect("height must be set in tauri.conf.json"),
    )
}

fn panel_size_from_window_or_config(window: &tauri::WebviewWindow) -> (f64, f64) {
    let configured = configured_panel_size();
    let Ok(s) = window.outer_size() else {
        return configured;
    };
    let Ok(win_scale) = window.scale_factor() else {
        return configured;
    };
    let width = s.width as f64 / win_scale;
    let height = s.height as f64 / win_scale;

    if width > 0.0 && height > 0.0 {
        (width, height)
    } else {
        configured
    }
}

/// Position the panel directly beneath the tray icon (best-effort).
/// On Wayland the compositor may ignore the requested position; the panel is
/// still shown, just not pinned to the tray icon.
pub fn position_panel_at_tray_icon(
    app_handle: &AppHandle,
    icon_position: Position,
    icon_size: Size,
) {
    let Some((position, primary_logical_h)) =
        compute_panel_position(app_handle, icon_position, icon_size)
    else {
        return;
    };
    apply_panel_position(app_handle, position.x, position.y, primary_logical_h);
    emit_panel_anchor_offset(app_handle, position.arrow_offset_px);
}

pub fn position_panel_at_tray_click(
    app_handle: &AppHandle,
    _click_position: PhysicalPosition<f64>,
    icon_position: Position,
    icon_size: Size,
) {
    let Some((position, primary_logical_h)) =
        compute_panel_position(app_handle, icon_position, icon_size)
    else {
        return;
    };
    apply_panel_position(app_handle, position.x, position.y, primary_logical_h);
    emit_panel_anchor_offset(app_handle, position.arrow_offset_px);
}

#[cfg(not(target_os = "macos"))]
fn choose_fallback_panel_position(
    remembered: Option<PanelAnchorPosition>,
    safe_fallback: Option<PanelAnchorPosition>,
) -> Option<PanelAnchorPosition> {
    remembered.or(safe_fallback)
}

#[cfg(not(target_os = "macos"))]
fn position_panel_at_fallback_anchor(app_handle: &AppHandle) {
    let Some(position) = choose_fallback_panel_position(
        compute_remembered_panel_position(app_handle),
        compute_fallback_panel_position(app_handle),
    ) else {
        return;
    };
    apply_panel_position(app_handle, position.x, position.y, 0.0);
    emit_panel_anchor_offset(app_handle, position.arrow_offset_px);
}

pub fn position_panel_at_logical_anchor(app_handle: &AppHandle, center_x: f64, bottom_y: f64) {
    let Some(position) = compute_logical_anchor_panel_position(app_handle, center_x, bottom_y)
    else {
        return;
    };
    apply_panel_position(app_handle, position.x, position.y, 0.0);
    emit_panel_anchor_offset(app_handle, position.arrow_offset_px);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_click_anchor_uses_icon_bottom_under_icon() {
        let icon_position = Position::Physical(tauri::PhysicalPosition::new(100, 0));
        let icon_size = Size::Physical(tauri::PhysicalSize::new(40, 24));

        let (icon_phys_x, icon_phys_y) = match icon_position {
            Position::Physical(pos) => (pos.x as f64, pos.y as f64),
            Position::Logical(pos) => (pos.x, pos.y),
        };
        let (icon_phys_w, icon_phys_h) = match icon_size {
            Size::Physical(size) => (size.width as f64, size.height as f64),
            Size::Logical(size) => (size.width, size.height),
        };
        let anchor = tray_icon_anchor_position(icon_phys_x, icon_phys_y, icon_phys_w, icon_phys_h);

        assert_eq!(anchor.0, 120.0);
        assert_eq!(anchor.1, 24.0);
    }

    #[test]
    fn fallback_position_does_not_use_cursor_anchor() {
        let remembered = PanelAnchorPosition {
            x: 10.0,
            y: 20.0,
            arrow_offset_px: 0.0,
        };
        let safe = PanelAnchorPosition {
            x: 30.0,
            y: 40.0,
            arrow_offset_px: 0.0,
        };

        let chosen = choose_fallback_panel_position(None, Some(safe)).expect("safe fallback");
        assert_eq!(chosen.x, 30.0);

        let chosen =
            choose_fallback_panel_position(Some(remembered), Some(safe)).expect("remembered");
        assert_eq!(chosen.x, 10.0);
    }
}

// ===========================================================================
// macOS: floating NSPanel pinned under the menubar tray icon.
// ===========================================================================
#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    unsafe fn set_panel_frame_top_left(panel: &tauri_nspanel::NSPanel, x: f64, y: f64) {
        let point = tauri_nspanel::NSPoint::new(x, y);
        let _: () = objc2::msg_send![panel, setFrameTopLeftPoint: point];
    }

    pub(super) fn apply_panel_position(
        app_handle: &AppHandle,
        panel_x: f64,
        panel_y: f64,
        primary_logical_h: f64,
    ) {
        let Some(window) = app_handle.get_webview_window("main") else {
            return;
        };
        let Ok(panel_handle) = app_handle.get_webview_panel("main") else {
            return;
        };

        // macOS uses a bottom-left origin, so flip the y coordinate.
        let target_x = panel_x;
        let target_y = primary_logical_h - panel_y;

        if objc2_foundation::MainThreadMarker::new().is_some() {
            unsafe {
                set_panel_frame_top_left(panel_handle.as_panel(), target_x, target_y);
            }
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let panel_handle = panel_handle.clone();

        if let Err(error) = window.run_on_main_thread(move || {
            unsafe {
                set_panel_frame_top_left(panel_handle.as_panel(), target_x, target_y);
            }
            let _ = tx.send(());
        }) {
            log::warn!("Failed to position panel on main thread: {}", error);
            return;
        }

        if rx.recv().is_err() {
            log::warn!("Failed waiting for panel position on main thread");
        }
    }

    /// Get existing panel or initialize it if needed.
    macro_rules! get_or_init_panel {
        ($app_handle:expr) => {
            match $app_handle.get_webview_panel("main") {
                Ok(panel) => Some(panel),
                Err(_) => {
                    if let Err(err) = init($app_handle) {
                        log::error!("Failed to init panel: {}", err);
                        None
                    } else {
                        match $app_handle.get_webview_panel("main") {
                            Ok(panel) => Some(panel),
                            Err(err) => {
                                log::error!("Panel missing after init: {:?}", err);
                                None
                            }
                        }
                    }
                }
            }
        };
    }

    // Define our panel class and event handler together
    tauri_panel! {
        panel!(OpenUsagePanel {
            config: {
                can_become_key_window: true,
                is_floating_panel: true
            }
        })

        panel_event!(OpenUsagePanelEventHandler {
            window_did_resign_key(notification: &NSNotification) -> ()
        })
    }

    pub fn init(app_handle: &AppHandle) -> tauri::Result<()> {
        if app_handle.get_webview_panel("main").is_ok() {
            return Ok(());
        }

        let window = app_handle.get_webview_window("main").unwrap();

        let panel = window.to_panel::<OpenUsagePanel>()?;

        // Disable native shadow - it causes gray border on transparent windows
        // Let CSS handle shadow via shadow-xl class
        panel.set_has_shadow(false);
        panel.set_opaque(false);

        // Configure panel behavior
        panel.set_level(PanelLevel::MainMenu.value() + 1);

        panel.set_collection_behavior(
            CollectionBehavior::new()
                .move_to_active_space()
                .full_screen_auxiliary()
                .value(),
        );

        panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());

        // Set up event handler to hide panel when it loses focus
        let event_handler = OpenUsagePanelEventHandler::new();

        let handle = app_handle.clone();
        event_handler.window_did_resign_key(move |_notification| {
            if let Ok(panel) = handle.get_webview_panel("main") {
                panel.hide();
            }
        });

        panel.set_event_handler(Some(event_handler.as_ref()));

        Ok(())
    }

    /// Show the panel (initializing if needed), positioned under the tray icon.
    pub fn show_panel(app_handle: &AppHandle) {
        if let Some(panel) = get_or_init_panel!(app_handle) {
            panel.show_and_make_key();
            position_panel_from_tray(app_handle);
        }
    }

    pub fn show_panel_at_logical_anchor(app_handle: &AppHandle, center_x: f64, bottom_y: f64) {
        if let Some(panel) = get_or_init_panel!(app_handle) {
            position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
            panel.show_and_make_key();
            position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
        }
    }

    /// Toggle panel visibility. If visible, hide it. If hidden, show it.
    pub fn toggle_panel(app_handle: &AppHandle) {
        let Some(panel) = get_or_init_panel!(app_handle) else {
            return;
        };

        if panel.is_visible() {
            log::debug!("toggle_panel: hiding panel");
            panel.hide();
        } else {
            log::debug!("toggle_panel: showing panel");
            panel.show_and_make_key();
            position_panel_from_tray(app_handle);
        }
    }

    pub fn toggle_panel_at_tray_icon(
        app_handle: &AppHandle,
        _click_position: PhysicalPosition<f64>,
        icon_position: Position,
        icon_size: Size,
    ) {
        let Some(panel) = get_or_init_panel!(app_handle) else {
            return;
        };

        if panel.is_visible() {
            log::debug!("toggle_panel_at_tray_icon: hiding panel");
            panel.hide();
        } else {
            log::debug!("toggle_panel_at_tray_icon: showing panel");
            panel.show_and_make_key();
            position_panel_at_tray_icon(app_handle, icon_position, icon_size);
        }
    }

    pub fn hide_panel(app_handle: &AppHandle) {
        if let Ok(panel) = app_handle.get_webview_panel("main") {
            panel.hide();
        }
    }

    pub fn should_hide_for_window_focus_loss_now(_is_visible: bool) -> bool {
        true
    }
}

#[cfg(not(target_os = "macos"))]
use crate::panel_non_macos as platform;

use platform::apply_panel_position;
pub use platform::{
    hide_panel, init, should_hide_for_window_focus_loss_now, show_panel,
    show_panel_at_logical_anchor, toggle_panel, toggle_panel_at_tray_icon,
};
