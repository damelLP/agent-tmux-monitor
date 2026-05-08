//! Snapshot tests for ATM TUI widgets.
//!
//! Renders individual widgets into a fixed-size `TestBackend` buffer
//! and compares the textual output against committed `.snap` files.
//! Intended to catch unintended visual regressions in widget rendering.
//!
//! To regenerate snapshots after an intentional change:
//!     INSTA_UPDATE=always cargo test -p atm-tui --test ui_snapshots
//! Then visually inspect the diff before committing the new `.snap` files.

use std::sync::Mutex;

use atm_core::{SessionId, SessionStatus, SessionView};
use atm_tui::app::{App, AppState};
use atm_tui::ui::{
    detail_panel::render_detail_panel_inline,
    help_popup::render_help_popup,
    session_list::render_session_list,
    status_bar::{render_footer, render_header},
};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

/// Serializes tests that mutate process-wide environment variables.
/// Multiple integration tests share one binary and run in parallel by default;
/// without this lock, env mutation in one test races with reads in another.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Runs `f` with `TMUX` set to a fixed value, restoring the previous value after.
/// Used for widgets whose output depends on `tmux::is_in_tmux()`.
fn with_tmux<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_LOCK.lock().expect("env lock poisoned");
    let prev = std::env::var("TMUX").ok();
    std::env::set_var("TMUX", "/tmp/snapshot-test");
    let result = f();
    match prev {
        Some(v) => std::env::set_var("TMUX", v),
        None => std::env::remove_var("TMUX"),
    }
    result
}

/// Builds a fully-populated `SessionView` for snapshot fixtures.
///
/// Uses an ISO-8601 `started_at` so tree ordering is stable.
fn make_session(
    id: &str,
    project: &str,
    branch: &str,
    model: &str,
    status: SessionStatus,
    status_label: &str,
    ctx_pct: f64,
    cost: f64,
    started_at: &str,
) -> SessionView {
    SessionView {
        id: SessionId::new(id),
        id_short: id.get(..8).unwrap_or(id).to_string(),
        agent_type: "general".to_string(),
        model: model.to_string(),
        status,
        status_label: status_label.to_string(),
        activity_detail: None,
        should_blink: false,
        status_icon: ">".to_string(),
        context_percentage: ctx_pct,
        context_display: format!("{}%", ctx_pct as u32),
        context_warning: ctx_pct >= 70.0,
        context_critical: ctx_pct >= 90.0,
        cost_display: format!("${cost:.2}"),
        cost_usd: cost,
        duration_display: "5m".to_string(),
        duration_seconds: 300.0,
        lines_display: "+100 -20".to_string(),
        working_directory: Some(project.to_string()),
        needs_attention: false,
        last_activity_display: "10s ago".to_string(),
        age_display: "5m ago".to_string(),
        started_at: started_at.to_string(),
        last_activity: started_at.to_string(),
        tmux_pane: None,
        project_root: Some(project.to_string()),
        worktree_path: Some(project.to_string()),
        worktree_branch: Some(branch.to_string()),
        ..Default::default()
    }
}

fn make_app_with_sessions() -> App {
    let mut app = App::new();
    app.state = AppState::Connected;
    // started_at descending → working session listed first (newer)
    app.update_sessions(vec![
        make_session(
            "abc12345-aaaa-bbbb-cccc-000000000001",
            "/home/dev/project-alpha",
            "main",
            "Opus 4.5",
            SessionStatus::Working,
            "working",
            45.0,
            0.50,
            "2026-01-15T10:05:00Z",
        ),
        make_session(
            "def67890-aaaa-bbbb-cccc-000000000002",
            "/home/dev/project-alpha",
            "main",
            "Sonnet 4.5",
            SessionStatus::Idle,
            "idle",
            72.0,
            1.20,
            "2026-01-15T10:00:00Z",
        ),
    ]);
    app
}

fn render_to_string<F>(width: u16, height: u16, draw: F) -> String
where
    F: FnOnce(&mut ratatui::Frame, Rect),
{
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            draw(frame, area);
        })
        .expect("draw frame");
    format!("{}", terminal.backend())
}

#[test]
fn session_list_two_agents_in_one_project() {
    let app = make_app_with_sessions();
    let out = render_to_string(40, 12, |frame, area| {
        render_session_list(frame, area, &app);
    });
    insta::assert_snapshot!(out);
}

#[test]
fn detail_panel_inline_no_capture() {
    let session = make_session(
        "abc12345-aaaa-bbbb-cccc-000000000001",
        "/home/dev/project-alpha",
        "main",
        "Opus 4.5",
        SessionStatus::Working,
        "working",
        45.0,
        0.50,
        "2026-01-15T10:05:00Z",
    );
    let out = render_to_string(60, 20, |frame, area| {
        render_detail_panel_inline(frame, area, Some(&session), &[]);
    });
    insta::assert_snapshot!(out);
}

#[test]
fn detail_panel_inline_empty() {
    let out = render_to_string(60, 20, |frame, area| {
        render_detail_panel_inline(frame, area, None, &[]);
    });
    insta::assert_snapshot!(out);
}

#[test]
fn header_connected_with_sessions() {
    let app = make_app_with_sessions();
    let out = render_to_string(80, 3, |frame, area| {
        render_header(frame, area, &app);
    });
    insta::assert_snapshot!(out);
}

#[test]
fn footer_in_tmux() {
    let snap = with_tmux(|| {
        let app = App::new();
        render_to_string(80, 3, |frame, area| {
            render_footer(frame, area, &app);
        })
    });
    insta::assert_snapshot!(snap);
}

#[test]
fn help_popup_in_tmux() {
    // Larger terminal so the centered popup (60% × 70%) is big enough
    // to display every keybinding entry without clipping.
    let snap = with_tmux(|| {
        render_to_string(100, 40, |frame, area| {
            render_help_popup(frame, area);
        })
    });
    insta::assert_snapshot!(snap);
}
