//! Pure geometry and capture-protection policy for the meeting notes window.

/// Return the centered, top-offset meeting window geometry in logical units.
pub fn meeting_window_geometry(
    monitor_x: f64,
    monitor_y: f64,
    monitor_w: f64,
) -> (f64, f64, f64, f64) {
    (
        monitor_x + (monitor_w - 540.0) / 2.0,
        monitor_y + 24.0,
        540.0,
        520.0,
    )
}

/// Whether the notes window should be excluded from screen capture.
pub fn content_protected(setting: bool) -> bool {
    setting
}

#[cfg(test)]
mod tests {
    use super::{content_protected, meeting_window_geometry};

    #[test]
    fn panel_is_centered_under_the_top_edge() {
        assert_eq!(
            meeting_window_geometry(0.0, 0.0, 1920.0),
            (690.0, 24.0, 540.0, 520.0)
        );
    }

    #[test]
    fn panel_respects_a_secondary_monitor_offset() {
        assert_eq!(
            meeting_window_geometry(1920.0, 0.0, 2560.0),
            (1920.0 + 1010.0, 24.0, 540.0, 520.0)
        );
    }

    #[test]
    fn protection_follows_the_setting() {
        assert!(content_protected(true));
        assert!(!content_protected(false));
    }
}
