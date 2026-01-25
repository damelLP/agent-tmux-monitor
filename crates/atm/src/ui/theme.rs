//! Shared theme utilities for the ATM TUI.
//!
//! Provides consistent styling across all UI components.

use atm_core::DisplayState;
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

/// Returns the appropriate color for a display state.
///
/// Color coding:
/// - Blue: Working (active, normal operation)
/// - Magenta: Compacting (context management in progress)
/// - Yellow: NeedsInput (blocked, requires urgent attention)
/// - LightMagenta: Idle (relaxed, waiting for user prompt)
/// - DarkGray: Stale (abandoned, low priority)
///
/// # Arguments
/// * `state` - The display state to colorize
///
/// # Returns
/// The appropriate `Color` for the state
pub fn display_state_color(state: DisplayState) -> Color {
    match state {
        DisplayState::Working => Color::Blue,
        DisplayState::Compacting => Color::Magenta,
        DisplayState::NeedsInput => Color::Yellow,
        DisplayState::Idle => Color::LightMagenta,
        DisplayState::Stale => Color::DarkGray,
    }
}

/// Returns the icon for a display state, respecting blink visibility.
///
/// States that blink (NeedsInput, Stale) return empty string when blink is off.
///
/// # Arguments
/// * `state` - The display state
/// * `blink_visible` - Whether blinking elements should be visible
///
/// # Returns
/// The icon string (may be empty if blinking and blink_visible is false)
pub fn display_state_icon(state: DisplayState, blink_visible: bool) -> &'static str {
    if state.should_blink() && !blink_visible {
        " "
    } else {
        state.icon()
    }
}

/// Returns the row background color for a display state.
///
/// Only NeedsInput gets a background tint to draw urgent attention.
/// Idle does NOT get a background - it's a relaxed state.
/// Critical context gets red tint (handled separately by caller).
///
/// # Arguments
/// * `state` - The display state
///
/// # Returns
/// Optional background color
pub fn display_state_background(state: DisplayState) -> Option<Color> {
    match state {
        DisplayState::NeedsInput => Some(Color::Rgb(50, 40, 0)), // Subtle yellow/amber tint
        DisplayState::Idle => None,                              // No tint - relaxed state
        _ => None,
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
    fn test_display_state_color_working() {
        assert_eq!(display_state_color(DisplayState::Working), Color::Blue);
    }

    #[test]
    fn test_display_state_color_compacting() {
        assert_eq!(display_state_color(DisplayState::Compacting), Color::Magenta);
    }

    #[test]
    fn test_display_state_color_needs_input() {
        assert_eq!(display_state_color(DisplayState::NeedsInput), Color::Yellow);
    }

    #[test]
    fn test_display_state_color_idle() {
        assert_eq!(display_state_color(DisplayState::Idle), Color::LightMagenta);
    }

    #[test]
    fn test_display_state_color_stale() {
        assert_eq!(display_state_color(DisplayState::Stale), Color::DarkGray);
    }

    #[test]
    fn test_display_state_icon_working() {
        // Working doesn't blink, always shows icon
        assert_eq!(display_state_icon(DisplayState::Working, true), ">");
        assert_eq!(display_state_icon(DisplayState::Working, false), ">");
    }

    #[test]
    fn test_display_state_icon_needs_input_blinks() {
        // NeedsInput blinks
        assert_eq!(display_state_icon(DisplayState::NeedsInput, true), "!");
        assert_eq!(display_state_icon(DisplayState::NeedsInput, false), " ");
    }

    #[test]
    fn test_display_state_icon_idle_no_blink() {
        // Idle does NOT blink - it's chill
        assert_eq!(display_state_icon(DisplayState::Idle, true), "-");
        assert_eq!(display_state_icon(DisplayState::Idle, false), "-");
    }

    #[test]
    fn test_display_state_icon_stale_no_blink() {
        // Stale does NOT blink - it's low priority, doesn't need attention
        assert_eq!(display_state_icon(DisplayState::Stale, true), "z");
        assert_eq!(display_state_icon(DisplayState::Stale, false), "z");
    }

    #[test]
    fn test_display_state_background() {
        assert!(display_state_background(DisplayState::NeedsInput).is_some());
        assert!(display_state_background(DisplayState::Working).is_none());
        assert!(display_state_background(DisplayState::Idle).is_none()); // Idle is chill, no highlight
        assert!(display_state_background(DisplayState::Stale).is_none());
    }
}
