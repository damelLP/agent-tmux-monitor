//! Shared theme utilities for the ATM TUI.
//!
//! Provides consistent styling across all UI components.

use atm_core::SessionStatus;
use ratatui::style::Color;

/// Returns the appropriate color for context usage display.
///
/// Color coding follows a traffic-light pattern:
/// - Green (< 50%): Normal usage, plenty of context remaining
/// - Yellow (50-89%): Elevated usage, may need attention soon
/// - Red (>= 90% or critical flag): Critical usage, intervention needed
///
/// # Arguments
/// * `percentage` - Context usage percentage (0.0 - 100.0+)
/// * `is_critical` - Whether the session is in an explicitly critical state
///
/// # Returns
/// The appropriate `Color` for the context indicator
///
/// # Example
/// ```ignore
/// let color = context_color(75.0, false);
/// assert_eq!(color, Color::Yellow);
/// ```
pub fn context_color(percentage: f64, is_critical: bool) -> Color {
    if is_critical || percentage >= 90.0 {
        Color::Red
    } else if percentage >= 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Returns the appropriate color for a session status.
///
/// Color coding:
/// - Blue: Working (active, normal operation)
/// - Yellow: AttentionNeeded (blocked, requires urgent attention)
/// - LightMagenta: Idle (relaxed, waiting for user prompt)
///
/// # Arguments
/// * `status` - The session status to colorize
///
/// # Returns
/// The appropriate `Color` for the status
pub fn status_color(status: SessionStatus) -> Color {
    match status {
        SessionStatus::Working => Color::Blue,
        SessionStatus::AttentionNeeded => Color::Yellow,
        SessionStatus::Idle => Color::LightMagenta,
    }
}

/// Returns the icon for a session status, respecting blink visibility.
///
/// AttentionNeeded blinks - returns empty string when blink is off.
///
/// # Arguments
/// * `status` - The session status
/// * `blink_visible` - Whether blinking elements should be visible
///
/// # Returns
/// The icon string (may be empty if blinking and blink_visible is false)
pub fn status_icon(status: SessionStatus, blink_visible: bool) -> &'static str {
    if status.should_blink() && !blink_visible {
        " "
    } else {
        status.icon()
    }
}

/// Returns the row background color for a session status.
///
/// Only AttentionNeeded gets a background tint to draw urgent attention.
/// Idle does NOT get a background - it's a relaxed state.
/// Critical context gets red tint (handled separately by caller).
///
/// # Arguments
/// * `status` - The session status
///
/// # Returns
/// Optional background color
pub fn status_background(status: SessionStatus) -> Option<Color> {
    match status {
        SessionStatus::AttentionNeeded => Some(Color::Rgb(50, 40, 0)), // Subtle yellow/amber tint
        SessionStatus::Idle => None,                                   // No tint - relaxed state
        SessionStatus::Working => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_color_normal() {
        assert_eq!(context_color(0.0, false), Color::Green);
        assert_eq!(context_color(25.0, false), Color::Green);
        assert_eq!(context_color(49.9, false), Color::Green);
    }

    #[test]
    fn test_context_color_warning() {
        assert_eq!(context_color(50.0, false), Color::Yellow);
        assert_eq!(context_color(75.0, false), Color::Yellow);
        assert_eq!(context_color(89.9, false), Color::Yellow);
    }

    #[test]
    fn test_context_color_critical() {
        assert_eq!(context_color(90.0, false), Color::Red);
        assert_eq!(context_color(95.0, false), Color::Red);
        assert_eq!(context_color(100.0, false), Color::Red);
    }

    #[test]
    fn test_context_color_critical_flag_overrides() {
        // Critical flag should force red regardless of percentage
        assert_eq!(context_color(0.0, true), Color::Red);
        assert_eq!(context_color(30.0, true), Color::Red);
        assert_eq!(context_color(50.0, true), Color::Red);
    }

    #[test]
    fn test_status_color_working() {
        assert_eq!(status_color(SessionStatus::Working), Color::Blue);
    }

    #[test]
    fn test_status_color_attention_needed() {
        assert_eq!(status_color(SessionStatus::AttentionNeeded), Color::Yellow);
    }

    #[test]
    fn test_status_color_idle() {
        assert_eq!(status_color(SessionStatus::Idle), Color::LightMagenta);
    }

    #[test]
    fn test_status_icon_working() {
        // Working doesn't blink, always shows icon
        assert_eq!(status_icon(SessionStatus::Working, true), ">");
        assert_eq!(status_icon(SessionStatus::Working, false), ">");
    }

    #[test]
    fn test_status_icon_attention_needed_blinks() {
        // AttentionNeeded blinks
        assert_eq!(status_icon(SessionStatus::AttentionNeeded, true), "!");
        assert_eq!(status_icon(SessionStatus::AttentionNeeded, false), " ");
    }

    #[test]
    fn test_status_icon_idle_no_blink() {
        // Idle does NOT blink - it's chill
        assert_eq!(status_icon(SessionStatus::Idle, true), "-");
        assert_eq!(status_icon(SessionStatus::Idle, false), "-");
    }

    #[test]
    fn test_status_background() {
        assert!(status_background(SessionStatus::AttentionNeeded).is_some());
        assert!(status_background(SessionStatus::Working).is_none());
        assert!(status_background(SessionStatus::Idle).is_none()); // Idle is chill, no highlight
    }
}
