//! Keyboard input handling for the ATM TUI.
//!
//! This module provides event types and handlers for keyboard input,
//! terminal resizing, and other TUI events.
//!
//! All code follows the panic-free policy: no `.unwrap()`, `.expect()`,
//! `panic!()`, `unreachable!()`, `todo!()`, or direct indexing `[i]`.

use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use atm_core::SessionView;

// ============================================================================
// Event Types
// ============================================================================

/// Events that the TUI can receive and process.
///
/// These events drive the main event loop and include both user input
/// and system-generated events.
#[derive(Debug, Clone)]
pub enum Event {
    /// Keyboard input from the user.
    Key(KeyEvent),

    /// Terminal window resize event.
    Resize(u16, u16),

    /// Session data update received from the daemon (merge with existing).
    SessionUpdate(Vec<SessionView>),

    /// Full session list received from daemon (replace all sessions).
    SessionListReplace(Vec<SessionView>),

    /// Connection to the daemon was lost.
    DaemonDisconnected,

    /// A session was removed from the daemon.
    SessionRemoved(String),

    /// Discovery operation completed.
    DiscoveryComplete {
        /// Number of sessions discovered.
        discovered: u32,
        /// Number of failures during discovery.
        failed: u32,
    },
}

// ============================================================================
// Client Commands
// ============================================================================

/// Commands that can be sent to the daemon client from the main loop.
#[derive(Debug, Clone)]
pub enum ClientCommand {
    /// Request session discovery from the daemon.
    Discover,
}

// ============================================================================
// Action Types
// ============================================================================

/// Actions that can result from user input.
///
/// These actions are returned by the input handler to signal what
/// the main loop should do in response to user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// No action required.
    None,

    /// Quit the application.
    Quit,

    /// Refresh the session data from the daemon.
    Refresh,

    /// Jump to a specific session (e.g., open in editor or terminal).
    JumpToSession(String),
}

// ============================================================================
// Input Handler
// ============================================================================

/// Handles a keyboard event and updates application state accordingly.
///
/// Returns an `Action` indicating what the main loop should do in response.
///
/// # Key Bindings
///
/// | Key          | Action                              |
/// |--------------|-------------------------------------|
/// | `q`, `Q`     | Quit the application                |
/// | `Esc`        | Quit the application                |
/// | `Ctrl+C`     | Quit the application                |
/// | `j`, `Down`  | Select the next session             |
/// | `k`, `Up`    | Select the previous session         |
/// | `Enter`      | Jump to the selected session        |
/// | `r`, `R`     | Refresh session data                |
///
/// # Arguments
///
/// * `key` - The keyboard event to handle.
/// * `app` - Mutable reference to the application state.
///
/// # Returns
///
/// An `Action` indicating what further action the main loop should take.
#[must_use]
pub fn handle_key_event(key: KeyEvent, app: &mut App) -> Action {
    // Handle Ctrl+C specially as an unconditional quit
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit();
        return Action::Quit;
    }

    // Match on key code for standard navigation and actions
    match key.code {
        // Quit keys
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
            app.quit();
            Action::Quit
        }

        // Navigation: next session
        KeyCode::Char('j') | KeyCode::Down => {
            app.select_next();
            Action::None
        }

        // Navigation: previous session
        KeyCode::Char('k') | KeyCode::Up => {
            app.select_previous();
            Action::None
        }

        // Jump to selected session (future: open in terminal/editor)
        KeyCode::Enter => {
            if let Some(session) = app.selected_session() {
                Action::JumpToSession(session.id.to_string())
            } else {
                Action::None
            }
        }

        // Refresh session data
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Refresh,

        // Unhandled keys
        _ => Action::None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use atm_core::{SessionId, SessionView};

    /// Creates a test KeyEvent with no modifiers.
    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Creates a test KeyEvent with specified modifiers.
    fn key_event_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    /// Creates a test session for use in tests.
    fn create_test_session(id: &str, started_at: &str) -> SessionView {
        SessionView {
            id: SessionId::new(id),
            id_short: id.get(..8).unwrap_or(id).to_string(),
            agent_type: "general".to_string(),
            model: "Opus 4.5".to_string(),
            status: "active".to_string(),
            status_detail: None,
            context_percentage: 25.0,
            context_display: "25%".to_string(),
            context_warning: false,
            context_critical: false,
            cost_display: "$0.50".to_string(),
            cost_usd: 0.50,
            duration_display: "5m".to_string(),
            duration_seconds: 300.0,
            lines_display: "+100 -20".to_string(),
            working_directory: Some("/home/user/project".to_string()),
            is_stale: false,
            needs_attention: false,
            last_activity_display: "10s ago".to_string(),
            age_display: "5m ago".to_string(),
            started_at: started_at.to_string(),
            last_activity: "2024-01-15T10:05:00Z".to_string(),
            tmux_pane: None,
            display_state: atm_core::DisplayState::Working,
        }
    }

    // ------------------------------------------------------------------------
    // Quit key tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_q_quits() {
        let mut app = App::new();
        let action = handle_key_event(key_event(KeyCode::Char('q')), &mut app);
        assert_eq!(action, Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_uppercase_q_quits() {
        let mut app = App::new();
        let action = handle_key_event(key_event(KeyCode::Char('Q')), &mut app);
        assert_eq!(action, Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_escape_quits() {
        let mut app = App::new();
        let action = handle_key_event(key_event(KeyCode::Esc), &mut app);
        assert_eq!(action, Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c_quits() {
        let mut app = App::new();
        let action = handle_key_event(
            key_event_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
        );
        assert_eq!(action, Action::Quit);
        assert!(app.should_quit);
    }

    // ------------------------------------------------------------------------
    // Navigation key tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_j_selects_next() {
        let mut app = App::new();
        app.update_sessions(vec![
            create_test_session("session-1", "2024-01-15T10:00:00Z"),
            create_test_session("session-2", "2024-01-15T10:01:00Z"),
        ]);
        assert_eq!(app.selected_index, 0);

        let action = handle_key_event(key_event(KeyCode::Char('j')), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_down_arrow_selects_next() {
        let mut app = App::new();
        app.update_sessions(vec![
            create_test_session("session-1", "2024-01-15T10:00:00Z"),
            create_test_session("session-2", "2024-01-15T10:01:00Z"),
        ]);
        assert_eq!(app.selected_index, 0);

        let action = handle_key_event(key_event(KeyCode::Down), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected_index, 1);
    }

    #[test]
    fn test_k_selects_previous() {
        let mut app = App::new();
        app.update_sessions(vec![
            create_test_session("session-1", "2024-01-15T10:00:00Z"),
            create_test_session("session-2", "2024-01-15T10:01:00Z"),
        ]);
        app.selected_index = 1;

        let action = handle_key_event(key_event(KeyCode::Char('k')), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_up_arrow_selects_previous() {
        let mut app = App::new();
        app.update_sessions(vec![
            create_test_session("session-1", "2024-01-15T10:00:00Z"),
            create_test_session("session-2", "2024-01-15T10:01:00Z"),
        ]);
        app.selected_index = 1;

        let action = handle_key_event(key_event(KeyCode::Up), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected_index, 0);
    }

    // ------------------------------------------------------------------------
    // Jump to session tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_enter_jumps_to_selected_session() {
        let mut app = App::new();
        app.update_sessions(vec![
            create_test_session("session-1", "2024-01-15T10:00:00Z"),
            create_test_session("session-2", "2024-01-15T10:01:00Z"),
        ]);
        // session-2 is first (newest) at index 0
        app.selected_index = 0;

        let action = handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(action, Action::JumpToSession("session-2".to_string()));
    }

    #[test]
    fn test_enter_with_no_sessions_returns_none() {
        let mut app = App::new();

        let action = handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(action, Action::None);
    }

    // ------------------------------------------------------------------------
    // Refresh tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_r_returns_refresh() {
        let mut app = App::new();

        let action = handle_key_event(key_event(KeyCode::Char('r')), &mut app);
        assert_eq!(action, Action::Refresh);
    }

    #[test]
    fn test_uppercase_r_returns_refresh() {
        let mut app = App::new();

        let action = handle_key_event(key_event(KeyCode::Char('R')), &mut app);
        assert_eq!(action, Action::Refresh);
    }

    // ------------------------------------------------------------------------
    // Unhandled key tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_unhandled_key_returns_none() {
        let mut app = App::new();

        let action = handle_key_event(key_event(KeyCode::Char('x')), &mut app);
        assert_eq!(action, Action::None);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_function_key_returns_none() {
        let mut app = App::new();

        let action = handle_key_event(key_event(KeyCode::F(1)), &mut app);
        assert_eq!(action, Action::None);
    }

    // ------------------------------------------------------------------------
    // Event type tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_event_key_variant() {
        let key = key_event(KeyCode::Enter);
        let event = Event::Key(key);
        match event {
            Event::Key(k) => assert_eq!(k.code, KeyCode::Enter),
            _ => panic!("Expected Key event"),
        }
    }

    #[test]
    fn test_event_resize_variant() {
        let event = Event::Resize(80, 24);
        match event {
            Event::Resize(w, h) => {
                assert_eq!(w, 80);
                assert_eq!(h, 24);
            }
            _ => panic!("Expected Resize event"),
        }
    }

    #[test]
    fn test_event_session_update_variant() {
        let sessions = vec![create_test_session("session-1", "2024-01-15T10:00:00Z")];
        let event = Event::SessionUpdate(sessions);
        match event {
            Event::SessionUpdate(s) => assert_eq!(s.len(), 1),
            _ => panic!("Expected SessionUpdate event"),
        }
    }

    #[test]
    fn test_event_daemon_disconnected_variant() {
        let event = Event::DaemonDisconnected;
        assert!(matches!(event, Event::DaemonDisconnected));
    }

    // ------------------------------------------------------------------------
    // Action type tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_action_equality() {
        assert_eq!(Action::None, Action::None);
        assert_eq!(Action::Quit, Action::Quit);
        assert_eq!(Action::Refresh, Action::Refresh);
        assert_eq!(
            Action::JumpToSession("test".to_string()),
            Action::JumpToSession("test".to_string())
        );
        assert_ne!(
            Action::JumpToSession("a".to_string()),
            Action::JumpToSession("b".to_string())
        );
    }

    #[test]
    fn test_action_debug() {
        let action = Action::JumpToSession("session-123".to_string());
        let debug_str = format!("{:?}", action);
        assert!(debug_str.contains("JumpToSession"));
        assert!(debug_str.contains("session-123"));
    }
}
