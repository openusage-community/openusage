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
    /// Top edge of the anchored element (tray icon). Used when the panel
    /// opens upward (tray at the bottom of the screen).
    pub(crate) top_y: f64,
    /// Bottom edge of the anchored element. Used when the panel opens downward.
    pub(crate) bottom_y: f64,
}

impl LogicalAnchor {
    /// Anchor with no known height (e.g. cursor position).
    pub(crate) fn at_point(center_x: f64, y: f64) -> Self {
        LogicalAnchor {
            center_x,
            top_y: y,
            bottom_y: y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnchorEdge {
    /// Panel sits below the anchor; arrow points up (tray at top).
    Top,
    /// Panel sits above the anchor; arrow points down (tray at bottom).
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelAnchorPosition {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) arrow_offset_px: f64,
    pub(crate) edge: AnchorEdge,
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
    let min_y = monitor.y;
    let max_y = monitor.y + (monitor.height - panel_height).max(0.0);

    let below_y = anchor.bottom_y - PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX;
    let fits_below = below_y + panel_height <= monitor.y + monitor.height;
    let (desired_y, edge) = if fits_below {
        (below_y, AnchorEdge::Top)
    } else {
        // Tray at the bottom of the screen: open upward, panel bottom at the icon top.
        (
            anchor.top_y + PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX - panel_height,
            AnchorEdge::Bottom,
        )
    };
    let y = desired_y.clamp(min_y, max_y);
    let arrow_offset_px = anchor.center_x - (x + (panel_width / 2.0));

    PanelAnchorPosition {
        x,
        y,
        arrow_offset_px,
        edge,
    }
}

pub(crate) fn fallback_anchor_for_monitor(monitor: &LogicalMonitorBounds) -> LogicalAnchor {
    LogicalAnchor::at_point(
        monitor.x + monitor.width - FALLBACK_ANCHOR_RIGHT_INSET_PX,
        monitor.y + FALLBACK_TOP_PANEL_BOTTOM_Y_PX,
    )
}

#[cfg(test)]
pub(crate) fn top_panel_anchor_at_x(
    monitor: &LogicalMonitorBounds,
    center_x: f64,
) -> LogicalAnchor {
    LogicalAnchor::at_point(center_x, monitor.y + FALLBACK_TOP_PANEL_BOTTOM_Y_PX)
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
        let anchor = LogicalAnchor::at_point(960.0, 28.0);

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 760.0);
        assert_eq!(result.y, 22.0);
        assert_eq!(result.arrow_offset_px, 0.0);
        assert_eq!(result.edge, AnchorEdge::Top);
    }

    #[test]
    fn right_edge_tray_anchor_clamps_panel_inside_monitor() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor::at_point(1900.0, 28.0);

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
        assert_eq!(result.edge, AnchorEdge::Top);
    }

    #[test]
    fn arrow_offset_stays_aligned_after_left_clamp() {
        let monitor = LogicalMonitorBounds {
            x: 100.0,
            y: 0.0,
            width: 800.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor::at_point(130.0, 28.0);

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.x, 100.0);
        assert_eq!(result.y, 22.0);
        assert_eq!(result.arrow_offset_px, -170.0);
    }

    #[test]
    fn bottom_tray_opens_panel_upward() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        // Tray icon at the bottom edge: top=1052, bottom=1076.
        let anchor = LogicalAnchor {
            center_x: 960.0,
            top_y: 1052.0,
            bottom_y: 1076.0,
        };

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        // Panel bottom (plus arrow tip inset) touches the icon top.
        assert_eq!(
            result.y,
            1052.0 + PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX - 500.0
        );
        assert_eq!(result.edge, AnchorEdge::Bottom);
        assert_eq!(result.arrow_offset_px, 0.0);
    }

    #[test]
    fn bottom_anchor_without_height_still_opens_upward() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor::at_point(960.0, 1075.0);

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);

        assert_eq!(result.edge, AnchorEdge::Bottom);
        // Desired 581 clamps to the monitor's max panel top (1080 - 500).
        assert_eq!(result.y, 580.0);
    }

    #[test]
    fn y_position_clamps_inside_monitor() {
        let monitor = LogicalMonitorBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let anchor = LogicalAnchor::at_point(960.0, 1200.0);

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
        let anchor = LogicalAnchor::at_point(1440.0, 32.0);

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
        let anchor = LogicalAnchor::at_point(1440.0, 32.0);

        let result = compute_anchor_position(&monitor, anchor, 400.0, 500.0);
        let arrow_tip_y = result.y + PANEL_WINDOW_ARROW_TIP_TOP_OFFSET_PX;
        let body_y = arrow_tip_y + PANEL_ARROW_HEIGHT_PX;

        assert_eq!(arrow_tip_y, anchor.bottom_y);
        assert!(body_y > anchor.bottom_y);
        assert_eq!(result.arrow_offset_px, 0.0);
    }
}
