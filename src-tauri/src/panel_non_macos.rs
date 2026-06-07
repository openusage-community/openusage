use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt, KeyButMask, Window as X11Window},
    rust_connection::RustConnection,
};

use crate::panel::{
    position_panel_at_logical_anchor, position_panel_at_tray_click, position_panel_from_tray,
};

const FOCUS_LOSS_GRACE_MS: u64 = 250;
const FOCUS_POLL_INTERVAL_MS: u64 = 35;
const CLICK_CATCHER_LABEL: &str = "panel-click-catcher";
const CLICK_CATCHER_URL: &str = "index.html?overlay=panel-click-catcher";
static PANEL_OPEN_SESSION: AtomicU64 = AtomicU64::new(0);
static PANEL_FOCUS_WATCH_ID: AtomicU64 = AtomicU64::new(0);
static LAST_PANEL_OPEN_TIME_MS: AtomicU64 = AtomicU64::new(0);
static PANEL_HAD_FOCUS: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct PanelRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct LogicalOverlayBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl PanelRect {
    fn contains(self, x: i32, y: i32) -> bool {
        let right = self.x.saturating_add(self.width as i32);
        let bottom = self.y.saturating_add(self.height as i32);
        x >= self.x && x < right && y >= self.y && y < bottom
    }
}

#[cfg(target_os = "linux")]
struct PointerSnapshot {
    x: i32,
    y: i32,
    is_button_down: bool,
}

#[cfg(target_os = "linux")]
struct LinuxPointerWatcher {
    connection: RustConnection,
    root_window: X11Window,
}

#[cfg(target_os = "linux")]
impl LinuxPointerWatcher {
    fn new() -> Result<Self, String> {
        let (connection, screen_num) =
            x11rb::connect(None).map_err(|error| format!("connect failed: {error}"))?;
        let root_window = connection
            .setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| format!("screen {screen_num} not found"))?
            .root;

        Ok(Self {
            connection,
            root_window,
        })
    }

    fn read(&self) -> Result<PointerSnapshot, String> {
        let reply = self
            .connection
            .query_pointer(self.root_window)
            .map_err(|error| format!("query_pointer failed: {error}"))?
            .reply()
            .map_err(|error| format!("query_pointer reply failed: {error}"))?;

        let button_mask = u16::from(KeyButMask::BUTTON1)
            | u16::from(KeyButMask::BUTTON2)
            | u16::from(KeyButMask::BUTTON3);
        let active_mask = u16::from(reply.mask);

        Ok(PointerSnapshot {
            x: i32::from(reply.root_x),
            y: i32::from(reply.root_y),
            is_button_down: active_mask & button_mask != 0,
        })
    }
}

fn run_after_panel_map(session_id: u64, task: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(120));
        if current_panel_open_session() != session_id {
            return;
        }
        task();
    });
}

fn now_millis() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(now) => now.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(_) => 0,
    }
}

pub fn register_panel_opened() -> u64 {
    let session_id = PANEL_OPEN_SESSION.fetch_add(1, Ordering::SeqCst) + 1;
    LAST_PANEL_OPEN_TIME_MS.store(now_millis(), Ordering::SeqCst);
    // Keep close-on-blur behavior stable: only close after we have observed
    // at least one focus event for this open session.
    PANEL_HAD_FOCUS.store(false, Ordering::SeqCst);
    session_id
}

pub(crate) fn current_panel_open_session() -> u64 {
    PANEL_OPEN_SESSION.load(Ordering::Acquire)
}

fn should_hide_unfocused_panel(
    is_visible: bool,
    panel_had_focus: bool,
    is_focused: bool,
    elapsed_ms: u64,
) -> bool {
    is_visible && panel_had_focus && !is_focused && elapsed_ms >= FOCUS_LOSS_GRACE_MS
}

fn should_hide_for_pointer_down_outside(
    is_visible: bool,
    is_button_down: bool,
    pointer_inside_panel: bool,
    elapsed_ms: u64,
) -> bool {
    is_visible && is_button_down && !pointer_inside_panel && elapsed_ms >= FOCUS_LOSS_GRACE_MS
}

fn should_hide_for_window_focus_loss(
    is_visible: bool,
    has_open_session: bool,
    elapsed_ms: u64,
) -> bool {
    is_visible && has_open_session && elapsed_ms >= FOCUS_LOSS_GRACE_MS
}

pub fn should_hide_for_window_focus_loss_now(is_visible: bool) -> bool {
    let opened_at = LAST_PANEL_OPEN_TIME_MS.load(Ordering::Acquire);
    let elapsed_ms = if opened_at == 0 {
        FOCUS_LOSS_GRACE_MS
    } else {
        now_millis().saturating_sub(opened_at)
    };

    should_hide_for_window_focus_loss(is_visible, current_panel_open_session() > 0, elapsed_ms)
}

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

fn show_click_catcher(app_handle: &AppHandle) {
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
    let _ = click_catcher.show();
}

fn hide_click_catcher(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(CLICK_CATCHER_LABEL) {
        let _ = window.hide();
    }
}

fn current_panel_rect(window: &tauri::WebviewWindow) -> Result<PanelRect, String> {
    let position = window
        .outer_position()
        .map_err(|error| format!("outer_position failed: {error}"))?;
    let size = window
        .outer_size()
        .map_err(|error| format!("outer_size failed: {error}"))?;

    Ok(PanelRect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

fn start_focus_loss_watcher(app_handle: AppHandle, session_id: u64) {
    let watch_id = PANEL_FOCUS_WATCH_ID.fetch_add(1, Ordering::SeqCst) + 1;

    std::thread::spawn(move || {
        let session_marker = session_id;
        let started_at = Instant::now();
        #[cfg(target_os = "linux")]
        let mut pointer_watcher = match LinuxPointerWatcher::new() {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                log::warn!("panel focus watcher: Linux pointer watcher unavailable: {error}");
                None
            }
        };
        loop {
            std::thread::sleep(Duration::from_millis(FOCUS_POLL_INTERVAL_MS));

            if PANEL_FOCUS_WATCH_ID.load(Ordering::SeqCst) != watch_id {
                return;
            }

            if current_panel_open_session() != session_marker {
                return;
            }

            let Some(window) = app_handle.get_webview_window("main") else {
                return;
            };

            let is_visible = match window.is_visible() {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("panel focus watcher: failed to read visibility: {error}");
                    return;
                }
            };
            if !is_visible {
                return;
            }

            let is_focused = match window.is_focused() {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("panel focus watcher: failed to read focus: {error}");
                    return;
                }
            };
            if is_focused {
                PANEL_HAD_FOCUS.store(true, Ordering::SeqCst);
            }

            let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            if should_hide_unfocused_panel(
                is_visible,
                PANEL_HAD_FOCUS.load(Ordering::Acquire),
                is_focused,
                elapsed_ms,
            ) {
                let _ = window.hide();
                return;
            }

            #[cfg(target_os = "linux")]
            if let Some(watcher) = pointer_watcher.as_ref() {
                match watcher.read() {
                    Ok(pointer) => {
                        let pointer_inside_panel = match current_panel_rect(&window) {
                            Ok(rect) => rect.contains(pointer.x, pointer.y),
                            Err(error) => {
                                log::warn!(
                                    "panel focus watcher: failed to read panel rect: {error}"
                                );
                                false
                            }
                        };

                        if should_hide_for_pointer_down_outside(
                            is_visible,
                            pointer.is_button_down,
                            pointer_inside_panel,
                            elapsed_ms,
                        ) {
                            let _ = window.hide();
                            return;
                        }
                    }
                    Err(error) => {
                        log::warn!("panel focus watcher: disabling Linux pointer watcher: {error}");
                        pointer_watcher = LinuxPointerWatcher::new().ok();
                    }
                }
            }
        }
    });
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
pub fn init(_app_handle: &AppHandle) -> tauri::Result<()> {
    Ok(())
}

/// Show the window as a floating panel, positioned under the tray icon.
pub fn show_panel(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let session_id = register_panel_opened();
        show_click_catcher(app_handle);
        let _ = window.set_focus();
        start_focus_loss_watcher(app_handle.clone(), session_id);
        return;
    }

    let session_id = register_panel_opened();
    show_click_catcher(app_handle);
    let _ = window.set_always_on_top(true);
    position_panel_from_tray(app_handle);
    let _ = window.show();
    position_panel_from_tray(app_handle);
    let _ = window.set_focus();
    start_focus_loss_watcher(app_handle.clone(), session_id);

    let app_handle = app_handle.clone();
    run_after_panel_map(session_id, move || {
        if current_panel_open_session() != session_id {
            return;
        }
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_focus();
            position_panel_from_tray(&app_handle);
        }
    });
}

fn show_panel_at_tray_icon(
    app_handle: &AppHandle,
    click_position: PhysicalPosition<f64>,
    icon_position: Position,
    icon_size: Size,
) {
    let session_id = register_panel_opened();
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    show_click_catcher(app_handle);
    let _ = window.set_always_on_top(true);
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.show();
    position_panel_at_tray_click(app_handle, click_position, icon_position, icon_size);
    let _ = window.set_focus();
    start_focus_loss_watcher(app_handle.clone(), session_id);

    let app_handle = app_handle.clone();
    run_after_panel_map(session_id, move || {
        if current_panel_open_session() != session_id {
            return;
        }
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_focus();
            position_panel_at_tray_click(&app_handle, click_position, icon_position, icon_size);
        }
    });
}

pub fn show_panel_at_logical_anchor(app_handle: &AppHandle, center_x: f64, bottom_y: f64) {
    let session_id = register_panel_opened();
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    show_click_catcher(app_handle);
    let _ = window.set_always_on_top(true);
    position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
    let _ = window.show();
    position_panel_at_logical_anchor(app_handle, center_x, bottom_y);
    let _ = window.set_focus();
    start_focus_loss_watcher(app_handle.clone(), session_id);

    let app_handle = app_handle.clone();
    run_after_panel_map(session_id, move || {
        if current_panel_open_session() != session_id {
            return;
        }
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_focus();
            position_panel_at_logical_anchor(&app_handle, center_x, bottom_y);
        }
    });
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
    hide_click_catcher(app_handle);
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfocused_visible_panel_does_not_hide_before_focus_observed() {
        assert!(!should_hide_unfocused_panel(true, false, false, 260));
    }

    #[test]
    fn visible_focused_panel_stays_open() {
        assert!(!should_hide_unfocused_panel(true, false, true, 0));
    }

    #[test]
    fn unfocused_visible_panel_stays_open_during_grace_period() {
        assert!(!should_hide_unfocused_panel(true, true, false, 249));
    }

    #[test]
    fn focused_or_hidden_panel_stays_open() {
        assert!(!should_hide_unfocused_panel(true, true, true, 260));
        assert!(!should_hide_unfocused_panel(false, true, false, 260));
    }

    #[test]
    fn outside_pointer_down_hides_visible_panel_after_grace_period() {
        assert!(should_hide_for_pointer_down_outside(true, true, false, 260));
    }

    #[test]
    fn pointer_down_does_not_hide_inside_panel_or_before_grace_period() {
        assert!(!should_hide_for_pointer_down_outside(true, true, true, 260));
        assert!(!should_hide_for_pointer_down_outside(
            true, true, false, 249
        ));
        assert!(!should_hide_for_pointer_down_outside(
            false, true, false, 260
        ));
        assert!(!should_hide_for_pointer_down_outside(
            true, false, false, 260
        ));
    }

    #[test]
    fn window_focus_loss_hides_after_grace_even_before_focus_observed() {
        assert!(should_hide_for_window_focus_loss(true, true, 260));
    }

    #[test]
    fn window_focus_loss_does_not_hide_before_grace_or_without_session() {
        assert!(!should_hide_for_window_focus_loss(true, true, 249));
        assert!(!should_hide_for_window_focus_loss(true, false, 260));
        assert!(!should_hide_for_window_focus_loss(false, true, 260));
    }
}
