const PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX: f64 = 6.0;
#[cfg(test)]
const PANEL_ARROW_HEIGHT_PX: f64 = 7.0;
const FALLBACK_ANCHOR_RIGHT_INSET_PX: f64 = 48.0;
const FALLBACK_TOP_PANEL_BOTTOM_Y_PX: f64 = 32.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct LogicalMonitorBounds {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LogicalAnchor {
    pub(crate) center_x: f64,
    pub(crate) bottom_y: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelAnchorPosition {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) arrow_offset_px: f64,
}

pub(crate) fn compute_anchor_position(
    monitor: &LogicalMonitorBounds,
    anchor: LogicalAnchor,
    panel_width: f64,
    panel_height: f64,
) -> PanelAnchorPosition {
    let desired_x = anchor.center_x - (panel_width / 2.0);
    let min_x = monitor.x;
    let max_x = monitor.x + (monitor.width - panel_width).max(0.0);
    let x = desired_x.clamp(min_x, max_x);
    let desired_y = anchor.bottom_y - PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX;
    let min_y = monitor.y;
    let max_y = monitor.y + (monitor.height - panel_height).max(0.0);
    let y = desired_y.clamp(min_y, max_y);
    let arrow_offset_px = anchor.center_x - (x + (panel_width / 2.0));

    PanelAnchorPosition {
        x,
        y,
        arrow_offset_px,
    }
}

pub(crate) fn fallback_anchor_for_monitor(monitor: &LogicalMonitorBounds) -> LogicalAnchor {
    LogicalAnchor {
        center_x: monitor.x + monitor.width - FALLBACK_ANCHOR_RIGHT_INSET_PX,
        bottom_y: monitor.y + FALLBACK_TOP_PANEL_BOTTOM_Y_PX,
    }
}

#[cfg(test)]
pub(crate) fn top_panel_anchor_at_x(
    monitor: &LogicalMonitorBounds,
    center_x: f64,
) -> LogicalAnchor {
    LogicalAnchor {
        center_x,
        bottom_y: monitor.y + FALLBACK_TOP_PANEL_BOTTOM_Y_PX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_anchor_centers_panel_when_there_is_room() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 960.0,
            bottom_y: 28.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 760.0);
        assert_eq!(result.y, 22.0);
        assert_eq!(result.arrow_offset_px, 0.0);
    }

    #[test]
    fn right_edge_tray_anchor_clamps_panel_inside_monitor() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 1900.0,
            bottom_y: 28.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 1520.0);
        assert_eq!(result.y, 22.0);
        assert_eq!(result.arrow_offset_px, 180.0);
    }

    #[test]
    fn fallback_anchor_places_panel_near_top_right() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };

        let anchor = fallback_anchor_for_monitor(&monitor);
        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 1520.0);
        assert_eq!(result.y, 26.0);
        assert_eq!(result.arrow_offset_px, 152.0);
    }

    #[test]
    fn arrow_offset_stays_aligned_after_left_clamp() {
        let monitor = LogicalMonitorBounds {
            x: 100.0,
            y: 0.0,
            width: 800.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 130.0,
            bottom_y: 28.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 100.0);
        assert_eq!(result.y, 22.0);
        assert_eq!(result.arrow_offset_px, -170.0);
    }

    #[test]
    fn y_position_clamps_inside_monitor() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 960.0,
            bottom_y: 1200.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 760.0);
        assert_eq!(result.y, 580.0);
        assert_eq!(result.arrow_offset_px, 0.0);
    }

    #[test]
    fn explicit_anchor_centers_panel_under_indicator() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 1440.0,
            bottom_y: 32.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 1240.0);
        assert_eq!(result.y, 26.0);
        assert_eq!(result.arrow_offset_px, 0.0);
    }

    #[test]
    fn top_panel_anchor_uses_given_x_instead_of_right_edge() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };

        let anchor = top_panel_anchor_at_x(&monitor, 1180.0);
        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 980.0);
        assert_eq!(result.y, 26.0);
        assert_eq!(result.arrow_offset_px, 0.0);
    }

    #[test]
    fn top_panel_anchor_aligns_arrow_tip_and_keeps_body_below_anchor() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor {
            center_x: 1440.0,
            bottom_y: 32.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);
        let arrow_tip_y = result.y + PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX;
        let body_y = arrow_tip_y + PANEL_ARROW_HEIGHT_PX;

        assert_eq!(arrow_tip_y, anchor.bottom_y);
        assert!(body_y > anchor.bottom_y);
        assert_eq!(result.arrow_offset_px, 0.0);
    }
}
