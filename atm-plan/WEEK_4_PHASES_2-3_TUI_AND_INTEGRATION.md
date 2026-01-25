# Week 4: Phases 2-3 - TUI & Shell Integration (7-8 days)

**Goal:** Implement the user-facing components - TUI for visualization and bash scripts for Claude Code integration

**Status:** Ready to start after Weeks 1-3 completion

---

## Overview

Week 4 brings Agent Tmux Monitor to life by creating the components users actually interact with:

1. **Phase 2: Basic TUI** (Days 1-4)
   - Terminal user interface using ratatui
   - Real-time session monitoring
   - Keyboard-driven navigation
   - Daemon connection management

2. **Phase 3: Shell Components** (Days 5-7)
   - Bash scripts for Claude Code integration
   - Installation and configuration tools
   - End-to-end testing with live sessions

**Investment:** 7-8 days of implementation
**Return:** Complete, user-ready monitoring system

---

## Prerequisites

Before starting Week 4, you must have completed:

### From Week 1: Validated Integration
- ✅ Tested Claude Code status line API (confirmed JSON structure)
- ✅ Tested hooks system (confirmed event delivery)
- ✅ Documented actual integration behavior in `CLAUDE_CODE_INTEGRATION.md`
- ✅ Identified session ID strategy (env var, PID, or generated UUID)
- ✅ Validated error handling approach (timeouts, graceful degradation)

### From Weeks 2-3: Working Daemon
- ✅ Daemon runs and accepts Unix socket connections
- ✅ Session registry with actor-based concurrency
- ✅ Domain model implemented (SessionDomain, ContextUsage, AgentType)
- ✅ Protocol messages (Register, StatusUpdate, GetAllSessions)
- ✅ Event broadcasting to connected clients
- ✅ Basic error handling and logging

### Verification Commands
```bash
# 1. Daemon starts successfully
atmd start
# Expected: Daemon running at /tmp/atm.sock

# 2. Can connect to socket
echo '{"type":"ping"}' | nc -U /tmp/atm.sock
# Expected: Some response (even if error - connection works)

# 3. Logs are being written
tail -f ~/.local/state/atm/atm.log
# Expected: Log entries appear

# 4. Can stop daemon
atmd stop
# Expected: Daemon stops cleanly
```

---

## Day 1-2: TUI Foundation & Layout (2 days)

**Duration:** 2 days
**Priority:** Critical

### Objective
Set up ratatui and implement the basic application structure with layout and event handling.

### Tasks

#### 1.1 Create TUI Crate Structure

**Create:** `crates/atm-tui/`

```bash
cd /path/to/atm
cargo new --lib crates/atm-tui
cd crates/atm-tui
```

**Update:** `crates/atm-tui/Cargo.toml`
```toml
[package]
name = "atm-tui"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "atm"
path = "src/main.rs"

[dependencies]
# TUI Framework
ratatui = "0.26"
crossterm = "0.27"

# Async runtime
tokio = { version = "1.35", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Date/Time
chrono = "0.4"

# Internal dependencies
atm-domain = { path = "../atm-domain" }
atm-protocol = { path = "../atm-protocol" }

[dev-dependencies]
tempfile = "3.8"
```

**File Structure:**
```
crates/atm-tui/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point
│   ├── lib.rs            # Library exports
│   ├── app.rs            # Application state
│   ├── ui.rs             # UI rendering
│   ├── event.rs          # Event handling
│   ├── daemon.rs         # Daemon client
│   └── error.rs          # TUI-specific errors
```

#### 1.2 Define TUI Error Types

**Create:** `crates/atm-tui/src/error.rs`
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Failed to initialize terminal: {0}")]
    TerminalInit(#[from] std::io::Error),

    #[error("Daemon connection failed: {0}")]
    DaemonConnection(String),

    #[error("Daemon disconnected")]
    DaemonDisconnected,

    #[error("Connection timeout after {0:?}")]
    ConnectionTimeout(std::time::Duration),

    #[error("Failed to render UI: {0}")]
    RenderError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, TuiError>;
```

#### 1.3 Implement Application State

**Create:** `crates/atm-tui/src/app.rs`
```rust
use chrono::{DateTime, Utc};
use atm_domain::{SessionDomain, SessionId};
use std::collections::HashMap;

/// Application state for the TUI
pub struct App {
    /// Current application state
    pub state: AppState,

    /// All active sessions
    pub sessions: HashMap<SessionId, SessionDomain>,

    /// Currently selected session index
    pub selected_index: usize,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Last update timestamp
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// Connected to daemon and receiving updates
    Connected,

    /// Disconnected from daemon, attempting to reconnect
    Disconnected {
        since: DateTime<Utc>,
        retry_count: u32,
    },

    /// Initial connection in progress
    Connecting,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::Connecting,
            sessions: HashMap::new(),
            selected_index: 0,
            should_quit: false,
            last_update: Utc::now(),
        }
    }

    /// Update sessions from daemon
    pub fn update_sessions(&mut self, sessions: Vec<SessionDomain>) {
        self.sessions.clear();
        for session in sessions {
            self.sessions.insert(session.id.clone(), session);
        }
        self.last_update = Utc::now();

        // Mark as connected
        if !matches!(self.state, AppState::Connected) {
            self.state = AppState::Connected;
        }
    }

    /// Mark daemon as disconnected
    pub fn mark_disconnected(&mut self) {
        if let AppState::Disconnected { retry_count, .. } = self.state {
            self.state = AppState::Disconnected {
                since: Utc::now(),
                retry_count: retry_count + 1,
            };
        } else {
            self.state = AppState::Disconnected {
                since: Utc::now(),
                retry_count: 0,
            };
        }
    }

    /// Get sessions as sorted vector for display
    pub fn sessions_sorted(&self) -> Vec<&SessionDomain> {
        let mut sessions: Vec<_> = self.sessions.values().collect();
        // Sort by start time, newest first
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        sessions
    }

    /// Get currently selected session
    pub fn selected_session(&self) -> Option<&SessionDomain> {
        let sessions = self.sessions_sorted();
        sessions.get(self.selected_index).copied()
    }

    /// Navigate selection down
    pub fn select_next(&mut self) {
        let session_count = self.sessions.len();
        if session_count > 0 {
            self.selected_index = (self.selected_index + 1) % session_count;
        }
    }

    /// Navigate selection up
    pub fn select_previous(&mut self) {
        let session_count = self.sessions.len();
        if session_count > 0 {
            if self.selected_index == 0 {
                self.selected_index = session_count - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Request quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 1.4 Define UI Layout

**Create:** `crates/atm-tui/src/ui.rs`
```rust
use crate::app::{App, AppState};
use chrono::Utc;
use atm_domain::SessionDomain;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render the complete UI
pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Footer
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_main_content(f, chunks[1], app);
    render_footer(f, chunks[2], app);
}

/// Render header with connection status
fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let (status_text, status_style) = match &app.state {
        AppState::Connected => {
            ("Connected", Style::default().fg(Color::Green))
        }
        AppState::Connecting => {
            ("Connecting...", Style::default().fg(Color::Yellow))
        }
        AppState::Disconnected { retry_count, .. } => {
            (
                &format!("Disconnected (retry {})", retry_count) as &str,
                Style::default().fg(Color::Red),
            )
        }
    };

    let header_text = vec![
        Line::from(vec![
            Span::styled("Agent Tmux Monitor ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("- Claude Code Session Monitor | Status: "),
            Span::styled(status_text, status_style),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_style(
            if matches!(app.state, AppState::Connected) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            },
        ));

    f.render_widget(header, area);
}

/// Render main content area with session list
fn render_main_content(f: &mut Frame, area: Rect, app: &App) {
    if app.sessions.is_empty() {
        render_empty_state(f, area, &app.state);
        return;
    }

    let sessions = app.sessions_sorted();

    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(idx, session)| create_session_list_item(session, idx == app.selected_index))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Sessions ({}) ", app.sessions.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(list, area);
}

/// Create a list item for a session
fn create_session_list_item(session: &SessionDomain, is_selected: bool) -> ListItem {
    let context_pct = session.context.used_percentage;
    let context_color = if context_pct >= 90.0 {
        Color::Red
    } else if context_pct >= 70.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let elapsed = Utc::now()
        .signed_duration_since(session.started_at)
        .num_minutes();

    let line = Line::from(vec![
        Span::styled(
            if is_selected { "> " } else { "  " },
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(format!("{:12} ", session.agent_type.to_string())),
        Span::styled(
            format!("{:5.1}% ", context_pct),
            Style::default().fg(context_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "{:>6}k in / {:>6}k out ",
            session.context.total_input_tokens / 1000,
            session.context.total_output_tokens / 1000,
        )),
        Span::styled(
            format!("${:.2} ", session.cost.total_cost_usd),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(format!("{}m ago", elapsed)),
    ]);

    ListItem::new(line)
}

/// Render empty state when no sessions
fn render_empty_state(f: &mut Frame, area: Rect, state: &AppState) {
    let message = match state {
        AppState::Connected => {
            vec![
                Line::from("No active Claude Code sessions"),
                Line::from(""),
                Line::from("Start a Claude Code session to see it here."),
            ]
        }
        AppState::Connecting => {
            vec![
                Line::from("Connecting to daemon..."),
                Line::from(""),
                Line::from("Make sure atmd is running: atmd start"),
            ]
        }
        AppState::Disconnected { .. } => {
            vec![
                Line::from(Span::styled(
                    "Disconnected from daemon",
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from("Attempting to reconnect..."),
                Line::from(""),
                Line::from("Check daemon status: atmd status"),
            ]
        }
    };

    let paragraph = Paragraph::new(message)
        .block(Block::default().borders(Borders::ALL).title(" Status "))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(paragraph, area);
}

/// Render footer with keybindings
fn render_footer(f: &mut Frame, area: Rect, _app: &App) {
    let help_text = vec![Line::from(vec![
        Span::raw(" "),
        Span::styled("↑/k", Style::default().fg(Color::Cyan)),
        Span::raw(" up  "),
        Span::styled("↓/j", Style::default().fg(Color::Cyan)),
        Span::raw(" down  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::raw(" jump  "),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw(" refresh  "),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::raw(" quit "),
    ])];

    let footer = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}
```

#### 1.5 Implement Event Handling

**Create:** `crates/atm-tui/src/event.rs`
```rust
use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

/// Events that can occur in the TUI
#[derive(Debug, Clone)]
pub enum Event {
    /// Key press event
    Key(KeyEvent),

    /// Terminal resize event
    Resize(u16, u16),

    /// Daemon sent new session data
    SessionUpdate(Vec<atm_domain::SessionDomain>),

    /// Daemon disconnected
    DaemonDisconnected,

    /// Tick event for regular updates
    Tick,
}

/// Event handler that merges terminal events and daemon events
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(daemon_rx: mpsc::UnboundedReceiver<Event>) -> Self {
        Self { rx: daemon_rx }
    }

    /// Get next event (blocking)
    pub async fn next(&mut self) -> Option<Event> {
        // Poll for terminal events with timeout
        let terminal_event = tokio::time::timeout(
            Duration::from_millis(100),
            tokio::task::spawn_blocking(|| {
                if event::poll(Duration::from_millis(100)).ok()? {
                    event::read().ok()
                } else {
                    None
                }
            }),
        )
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();

        // Check for daemon events (non-blocking)
        if let Ok(daemon_event) = self.rx.try_recv() {
            return Some(daemon_event);
        }

        // Process terminal event
        if let Some(event) = terminal_event {
            return match event {
                CrosstermEvent::Key(key) => Some(Event::Key(key)),
                CrosstermEvent::Resize(w, h) => Some(Event::Resize(w, h)),
                _ => None,
            };
        }

        // Tick event if nothing else
        Some(Event::Tick)
    }
}

/// Handle key events
pub fn handle_key_event(key: KeyEvent, app: &mut crate::app::App) {
    use KeyCode::*;

    match key.code {
        // Quit
        Char('q') | Char('Q') | Esc => {
            app.quit();
        }

        // Navigation
        Down | Char('j') => {
            app.select_next();
        }
        Up | Char('k') => {
            app.select_previous();
        }

        // Jump to session (placeholder for later)
        Enter => {
            if let Some(session) = app.selected_session() {
                tracing::info!("Jump to session: {}", session.id);
                // TODO: Implement tmux jump in Phase 3
            }
        }

        // Refresh (placeholder)
        Char('r') | Char('R') => {
            tracing::info!("Manual refresh requested");
            // Daemon automatically pushes updates, so this is mainly for UX
        }

        _ => {}
    }
}
```

### Day 1-2 Success Criteria

- ✅ TUI crate created with proper structure
- ✅ Error types defined
- ✅ Application state management implemented
- ✅ UI layout renders correctly (header, content, footer)
- ✅ Event handling framework in place
- ✅ Keyboard navigation works (up/down/quit)
- ✅ Can compile: `cargo build -p atm-tui`

**Test manually:**
```bash
cargo run -p atm-tui
# Should show UI with "Connecting to daemon..." message
# Press q to quit
```

---

## Day 3: Daemon Client & Connection Management (1 day)

**Duration:** 1 day
**Priority:** Critical

### Objective
Implement the client that connects to the daemon, handles reconnection, and propagates session updates to the UI.

### Tasks

#### 3.1 Implement Daemon Client

**Create:** `crates/atm-tui/src/daemon.rs`
```rust
use crate::error::{Result, TuiError};
use crate::event::Event;
use atm_domain::SessionDomain;
use atm_protocol::{ClientMessage, DaemonMessage};
use std::path::PathBuf;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::mpsc,
    time::sleep,
};
use tracing::{debug, error, info, warn};

/// Configuration for daemon connection
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub retry_initial_delay: Duration,
    pub retry_max_delay: Duration,
    pub retry_multiplier: f64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/tmp/atm.sock"),
            retry_initial_delay: Duration::from_secs(1),
            retry_max_delay: Duration::from_secs(30),
            retry_multiplier: 2.0,
        }
    }
}

/// Daemon client that manages connection and event streaming
pub struct DaemonClient {
    config: DaemonConfig,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl DaemonClient {
    /// Create a new daemon client
    pub fn new(config: DaemonConfig, event_tx: mpsc::UnboundedSender<Event>) -> Self {
        Self { config, event_tx }
    }

    /// Run the client (connects and maintains connection)
    pub async fn run(&self) {
        info!("Starting daemon client");

        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    info!("Daemon connection closed normally");
                }
                Err(e) => {
                    error!("Daemon connection error: {}", e);
                }
            }

            // Notify UI of disconnection
            let _ = self.event_tx.send(Event::DaemonDisconnected);

            // Wait before retry
            let delay = self.config.retry_initial_delay;
            warn!("Reconnecting to daemon in {:?}", delay);
            sleep(delay).await;
        }
    }

    /// Connect to daemon and stream events
    async fn connect_and_stream(&self) -> Result<()> {
        // Exponential backoff connection
        let stream = self.connect_with_retry().await?;

        info!("Connected to daemon at {:?}", self.config.socket_path);

        // Split stream for reading and writing
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send initial subscription message
        let subscribe = ClientMessage::Subscribe {
            client_type: "tui".to_string(),
        };
        self.send_message(&mut writer, &subscribe).await?;

        // Read messages from daemon
        let mut line = String::new();
        loop {
            line.clear();

            let bytes_read = reader.read_line(&mut line).await
                .map_err(|e| TuiError::DaemonConnection(e.to_string()))?;

            if bytes_read == 0 {
                // Connection closed
                return Err(TuiError::DaemonDisconnected);
            }

            // Parse message
            match serde_json::from_str::<DaemonMessage>(&line) {
                Ok(msg) => {
                    self.handle_daemon_message(msg).await;
                }
                Err(e) => {
                    error!("Failed to parse daemon message: {}, line: {}", e, line);
                }
            }
        }
    }

    /// Connect with exponential backoff retry
    async fn connect_with_retry(&self) -> Result<UnixStream> {
        let mut delay = self.config.retry_initial_delay;

        loop {
            match UnixStream::connect(&self.config.socket_path).await {
                Ok(stream) => {
                    info!("Connected to daemon");
                    return Ok(stream);
                }
                Err(e) => {
                    warn!(
                        "Failed to connect to daemon at {:?}: {}, retrying in {:?}",
                        self.config.socket_path, e, delay
                    );

                    sleep(delay).await;

                    // Exponential backoff
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * self.config.retry_multiplier)
                            .min(self.config.retry_max_delay.as_secs_f64()),
                    );
                }
            }
        }
    }

    /// Send a message to daemon
    async fn send_message(
        &self,
        writer: &mut tokio::net::unix::OwnedWriteHalf,
        msg: &ClientMessage,
    ) -> Result<()> {
        let json = serde_json::to_string(msg)?;
        writer.write_all(json.as_bytes()).await
            .map_err(|e| TuiError::DaemonConnection(e.to_string()))?;
        writer.write_all(b"\n").await
            .map_err(|e| TuiError::DaemonConnection(e.to_string()))?;
        Ok(())
    }

    /// Handle a message from daemon
    async fn handle_daemon_message(&self, msg: DaemonMessage) {
        match msg {
            DaemonMessage::SessionList { sessions } => {
                debug!("Received {} sessions from daemon", sessions.len());
                let _ = self.event_tx.send(Event::SessionUpdate(sessions));
            }
            DaemonMessage::SessionUpdate { session } => {
                debug!("Received session update: {}", session.id);
                // For simplicity, request full list on any update
                // TODO: In production, merge individual updates
                let _ = self.event_tx.send(Event::SessionUpdate(vec![session]));
            }
            DaemonMessage::Error { message } => {
                error!("Daemon error: {}", message);
            }
            _ => {
                debug!("Unhandled daemon message: {:?}", msg);
            }
        }
    }
}
```

#### 3.2 Integrate Daemon Client with Main Loop

**Create:** `crates/atm-tui/src/main.rs`
```rust
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use atm_tui::{
    app::App,
    daemon::{DaemonClient, DaemonConfig},
    event::{Event, EventHandler, handle_key_event},
    ui,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atm_tui=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(io::stderr))
        .init();

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create event channel
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    // Spawn daemon client
    let daemon_config = DaemonConfig::default();
    let daemon_client = DaemonClient::new(daemon_config, event_tx);
    tokio::spawn(async move {
        daemon_client.run().await;
    });

    // Run TUI
    let mut app = App::new();
    let mut event_handler = EventHandler::new(event_rx);

    loop {
        // Render UI
        terminal.draw(|f| {
            ui::render(f, &app);
        })?;

        // Handle events
        if let Some(event) = event_handler.next().await {
            match event {
                Event::Key(key) => {
                    handle_key_event(key, &mut app);
                }
                Event::SessionUpdate(sessions) => {
                    app.update_sessions(sessions);
                }
                Event::DaemonDisconnected => {
                    app.mark_disconnected();
                }
                Event::Tick => {
                    // Regular tick for animations, etc.
                }
                Event::Resize(_, _) => {
                    // Terminal resized, will re-render automatically
                }
            }
        }

        // Check if should quit
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
```

**Create:** `crates/atm-tui/src/lib.rs`
```rust
pub mod app;
pub mod daemon;
pub mod error;
pub mod event;
pub mod ui;
```

### Day 3 Success Criteria

- ✅ Daemon client connects to Unix socket
- ✅ Exponential backoff retry on connection failure
- ✅ Receives and parses daemon messages
- ✅ Propagates session updates to UI
- ✅ Handles disconnection gracefully
- ✅ UI shows "Connected" when daemon is reachable
- ✅ UI shows "Disconnected (retrying)" when daemon is down

**Test:**
```bash
# Start daemon
atmd start

# Run TUI
cargo run -p atm-tui

# Should show "Connected" status
# Stop daemon (in another terminal)
atmd stop

# TUI should show "Disconnected", keep retrying
# Restart daemon
atmd start

# TUI should reconnect automatically
```

---

## Day 4: Polish & Error Handling (1 day)

**Duration:** 1 day
**Priority:** High

### Objective
Add polish, improve error messages, and handle edge cases.

### Tasks

#### 4.1 Add Session Detail View (Modal/Popup)

**Update:** `crates/atm-tui/src/ui.rs`

Add a detail view that shows full session information when 'd' is pressed:

```rust
// Add to App struct in app.rs
pub show_detail: bool,

// Add key handler in event.rs
Char('d') | Char('D') => {
    app.show_detail = !app.show_detail;
}

// Add to ui.rs
pub fn render_session_detail(f: &mut Frame, area: Rect, session: &SessionDomain) {
    // Create a centered popup
    let popup_area = centered_rect(80, 80, area);

    let details = vec![
        Line::from(format!("Session ID: {}", session.id)),
        Line::from(format!("Agent Type: {}", session.agent_type)),
        Line::from(format!("Started: {}", session.started_at)),
        Line::from(""),
        Line::from(format!("Context Usage: {:.1}%", session.context.used_percentage)),
        Line::from(format!("  Input Tokens: {}", session.context.total_input_tokens)),
        Line::from(format!("  Output Tokens: {}", session.context.total_output_tokens)),
        Line::from(format!("  Max Tokens: {}", session.context.max_tokens)),
        Line::from(""),
        Line::from(format!("Cost: ${:.2}", session.cost.total_cost_usd)),
        Line::from(format!("  Input Cost: ${:.2}", session.cost.input_cost)),
        Line::from(format!("  Output Cost: ${:.2}", session.cost.output_cost)),
        Line::from(""),
        Line::from(format!("Model: {}", session.model_id)),
        Line::from(format!("PID: {}", session.infrastructure.pid)),
        Line::from(""),
        Line::from(Span::styled("Press 'd' to close", Style::default().fg(Color::Gray))),
    ];

    let block = Block::default()
        .title(" Session Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(details)
        .block(block);

    f.render_widget(ratatui::widgets::Clear, popup_area); // Clear background
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Update main render function
pub fn render(f: &mut Frame, app: &App) {
    // ... existing code ...

    // Show detail popup if requested
    if app.show_detail {
        if let Some(session) = app.selected_session() {
            render_session_detail(f, f.area(), session);
        }
    }
}
```

#### 4.2 Add Better Error Messages

**Update:** `crates/atm-tui/src/ui.rs`

Improve error message display:

```rust
fn render_empty_state(f: &mut Frame, area: Rect, state: &AppState) {
    let (title, message, help) = match state {
        AppState::Connected => (
            " No Sessions ",
            vec![
                Line::from(Span::styled(
                    "No active Claude Code sessions detected",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(""),
                Line::from("To get started:"),
                Line::from("  1. Open a terminal"),
                Line::from("  2. Run: claude"),
                Line::from("  3. Session will appear here automatically"),
            ],
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Tip: Configure Claude Code with atm-status.sh",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                )),
            ],
        ),
        AppState::Connecting => (
            " Connecting ",
            vec![
                Line::from("Connecting to ATM daemon..."),
                Line::from(""),
                Line::from("This usually takes 1-2 seconds."),
            ],
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "If this persists, check: atmd status",
                    Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                )),
            ],
        ),
        AppState::Disconnected { retry_count, .. } => (
            " Disconnected ",
            vec![
                Line::from(Span::styled(
                    "Lost connection to daemon",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("Retry attempt: {}", retry_count)),
                Line::from(""),
                Line::from("Troubleshooting:"),
                Line::from("  1. Check daemon: atmd status"),
                Line::from("  2. View logs: tail ~/.local/state/atm/atm.log"),
                Line::from("  3. Restart daemon: atmd restart"),
            ],
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'q' to quit",
                    Style::default().fg(Color::Gray),
                )),
            ],
        ),
    };

    let mut all_lines = message;
    all_lines.extend(help);

    let paragraph = Paragraph::new(all_lines)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(paragraph, area);
}
```

#### 4.3 Add Color-Coded Context Usage

**Update:** `crates/atm-tui/src/ui.rs`

Add visual indicator for context pressure:

```rust
fn create_session_list_item(session: &SessionDomain, is_selected: bool) -> ListItem {
    let context_pct = session.context.used_percentage;

    // Color code by pressure
    let (context_color, context_symbol) = if context_pct >= 95.0 {
        (Color::Red, "●") // Critical
    } else if context_pct >= 90.0 {
        (Color::LightRed, "●") // High
    } else if context_pct >= 70.0 {
        (Color::Yellow, "●") // Medium
    } else if context_pct >= 50.0 {
        (Color::Green, "●") // Low
    } else {
        (Color::DarkGray, "○") // Minimal
    };

    let elapsed = Utc::now()
        .signed_duration_since(session.started_at)
        .num_minutes();

    let elapsed_display = if elapsed < 60 {
        format!("{}m", elapsed)
    } else {
        format!("{}h", elapsed / 60)
    };

    let line = Line::from(vec![
        Span::styled(
            if is_selected { "> " } else { "  " },
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ", context_symbol),
            Style::default().fg(context_color),
        ),
        Span::styled(
            format!("{:5.1}% ", context_pct),
            Style::default().fg(context_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{:12} ", session.agent_type.to_string())),
        Span::raw(format!(
            "{:>5}k/{:<5}k ",
            session.context.total_input_tokens / 1000,
            session.context.total_output_tokens / 1000,
        )),
        Span::styled(
            format!("${:>5.2} ", session.cost.total_cost_usd),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            format!("{:>4} ", elapsed_display),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    ListItem::new(line)
}
```

### Day 4 Success Criteria

- ✅ Detail view shows complete session information
- ✅ Error messages are helpful and actionable
- ✅ Color-coded context usage makes pressure obvious
- ✅ UI feels polished and professional
- ✅ Edge cases handled (empty sessions, disconnection, etc.)

---

## Day 5: Shell Components - Status Line Script (1 day)

**Duration:** 1 day
**Priority:** Critical

### Objective
Implement the bash script that integrates with Claude Code's status line API, using the validation results from Week 1.

### Tasks

#### 5.1 Create Status Line Script

**Create:** `scripts/atm-status.sh`

```bash
#!/bin/bash
#
# Agent Tmux Monitor Status Line Integration for Claude Code
#
# This script is called by Claude Code's status line API.
# It receives JSON on stdin containing context and cost information,
# parses it, and sends updates to the ATM daemon.
#
# Configuration:
#   - Socket: /tmp/atm.sock
#   - Protocol: JSON over Unix socket
#   - Timeout: 100ms (non-blocking)
#
# Exit codes:
#   0 - Success (or graceful degradation)
#   Non-zero exits would break Claude Code, so we always exit 0
#

set -euo pipefail

# Configuration
SOCKET="/tmp/atm.sock"
TIMEOUT="0.1"  # 100ms timeout
LOG_FILE="/tmp/atm-status.log"
DEBUG="${ATM_DEBUG:-0}"

# Session identification
# Try environment variable first, fall back to PID
SESSION_ID="${CLAUDE_SESSION_ID:-$(uuidgen)}"
SESSION_PID="$$"

# Log debug info if enabled
debug_log() {
    if [ "$DEBUG" = "1" ]; then
        echo "$(date '+%Y-%m-%d %H:%M:%S') [DEBUG] $*" >> "$LOG_FILE"
    fi
}

# Check if daemon is running
if [ ! -S "$SOCKET" ]; then
    debug_log "Daemon socket not found at $SOCKET"
    exit 0  # Exit gracefully
fi

# Register session on first run
# Use a marker file to track if we've registered
SESSION_MARKER="/tmp/atm-session-$SESSION_ID"
if [ ! -f "$SESSION_MARKER" ]; then
    debug_log "Registering new session: $SESSION_ID"

    # Get working directory
    WORKDIR="$(pwd)"

    # Create registration message
    REGISTER_MSG=$(cat <<EOF
{
  "type": "register",
  "session_id": "$SESSION_ID",
  "pid": $SESSION_PID,
  "agent_type": "general-purpose",
  "working_dir": "$WORKDIR",
  "started_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)

    debug_log "Sending registration: $REGISTER_MSG"

    # Send registration (with timeout)
    echo "$REGISTER_MSG" | timeout "$TIMEOUT" nc -U "$SOCKET" 2>/dev/null || {
        debug_log "Failed to register session"
        exit 0
    }

    # Mark session as registered
    touch "$SESSION_MARKER"
    debug_log "Session registered successfully"
fi

# Read status line JSON from stdin
while IFS= read -r line; do
    debug_log "Received status line: $line"

    # Parse JSON fields using jq
    # Based on Week 1 validation, we expect:
    # {
    #   "context_window": {
    #     "used_percentage": 45.2,
    #     "total_input_tokens": 12000,
    #     "total_output_tokens": 8000,
    #     "max_tokens": 200000
    #   },
    #   "cost": {
    #     "total_cost_usd": 0.45,
    #     "input_cost": 0.30,
    #     "output_cost": 0.15
    #   },
    #   "model": {
    #     "id": "claude-sonnet-4-5-20250929"
    #   }
    # }

    USED_PCT=$(echo "$line" | jq -r '.context_window.used_percentage // 0')
    INPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_input_tokens // 0')
    OUTPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_output_tokens // 0')
    MAX_TOKENS=$(echo "$line" | jq -r '.context_window.max_tokens // 200000')

    TOTAL_COST=$(echo "$line" | jq -r '.cost.total_cost_usd // 0')
    INPUT_COST=$(echo "$line" | jq -r '.cost.input_cost // 0')
    OUTPUT_COST=$(echo "$line" | jq -r '.cost.output_cost // 0')

    MODEL_ID=$(echo "$line" | jq -r '.model.id // "unknown"')

    debug_log "Parsed: $USED_PCT% ($INPUT_TOKENS/$OUTPUT_TOKENS tokens), \$$TOTAL_COST, $MODEL_ID"

    # Create status update message
    UPDATE_MSG=$(cat <<EOF
{
  "type": "status_update",
  "session_id": "$SESSION_ID",
  "context": {
    "used_percentage": $USED_PCT,
    "total_input_tokens": $INPUT_TOKENS,
    "total_output_tokens": $OUTPUT_TOKENS,
    "max_tokens": $MAX_TOKENS
  },
  "cost": {
    "total_cost_usd": $TOTAL_COST,
    "input_cost": $INPUT_COST,
    "output_cost": $OUTPUT_COST
  },
  "model_id": "$MODEL_ID",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)

    # Send update to daemon (with timeout)
    echo "$UPDATE_MSG" | timeout "$TIMEOUT" nc -U "$SOCKET" 2>/dev/null || {
        debug_log "Failed to send status update"
        # Don't exit - continue processing
    }
done

# Always exit 0 to avoid breaking Claude Code
exit 0
```

**Make executable:**
```bash
chmod +x scripts/atm-status.sh
```

#### 5.2 Create Hooks Script

**Create:** `scripts/atm-hooks.sh`

```bash
#!/bin/bash
#
# Agent Tmux Monitor Hooks Integration for Claude Code
#
# This script is called by Claude Code's hooks system.
# It receives hook events on stdin and forwards them to the daemon.
#
# Supported hooks:
#   - PermissionRequest
#   - PreToolUse
#   - PostToolUse
#

set -euo pipefail

# Configuration
SOCKET="/tmp/atm.sock"
TIMEOUT="0.1"
LOG_FILE="/tmp/atm-hooks.log"
DEBUG="${ATM_DEBUG:-0}"

SESSION_ID="${CLAUDE_SESSION_ID:-unknown}"

debug_log() {
    if [ "$DEBUG" = "1" ]; then
        echo "$(date '+%Y-%m-%d %H:%M:%S') [DEBUG] $*" >> "$LOG_FILE"
    fi
}

# Check if daemon is running
if [ ! -S "$SOCKET" ]; then
    debug_log "Daemon socket not found"
    exit 0
fi

# Read hook event from stdin
IFS= read -r line
debug_log "Received hook event: $line"

# Parse hook event
HOOK_EVENT=$(echo "$line" | jq -r '.hook_event_name // "unknown"')
debug_log "Hook type: $HOOK_EVENT"

# Create hook message for daemon
HOOK_MSG=$(cat <<EOF
{
  "type": "hook_event",
  "session_id": "$SESSION_ID",
  "hook_event": $line,
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
)

# Send to daemon
echo "$HOOK_MSG" | timeout "$TIMEOUT" nc -U "$SOCKET" 2>/dev/null || {
    debug_log "Failed to send hook event"
}

exit 0
```

**Make executable:**
```bash
chmod +x scripts/atm-hooks.sh
```

### Day 5 Success Criteria

- ✅ Status line script parses Claude Code JSON correctly
- ✅ Session registration works on first run
- ✅ Status updates sent to daemon every ~300ms
- ✅ Script exits gracefully when daemon is unavailable
- ✅ Debug logging works when ATM_DEBUG=1
- ✅ No errors break Claude Code operation

**Test:**
```bash
# Enable debug logging
export ATM_DEBUG=1

# Simulate status line input
echo '{"context_window":{"used_percentage":45.2,"total_input_tokens":12000,"total_output_tokens":8000,"max_tokens":200000},"cost":{"total_cost_usd":0.45,"input_cost":0.30,"output_cost":0.15},"model":{"id":"claude-sonnet-4-5-20250929"}}' | ./scripts/atm-status.sh

# Check logs
cat /tmp/atm-status.log
```

---

## Day 6: Installation & Configuration (1 day)

**Duration:** 1 day
**Priority:** High

### Objective
Create installation scripts and configuration examples for easy setup.

### Tasks

#### 6.1 Create Installation Script

**Create:** `scripts/install.sh`

```bash
#!/bin/bash
#
# Agent Tmux Monitor Installation Script
#
# Installs ATM daemon and TUI, sets up Claude Code integration
#

set -e

echo "=== Agent Tmux Monitor Installation ==="
echo ""

# Check prerequisites
echo "Checking prerequisites..."

# Check for jq
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed."
    echo "Install with: sudo apt install jq  (or brew install jq on macOS)"
    exit 1
fi

# Check for nc (netcat)
if ! command -v nc &> /dev/null; then
    echo "Error: nc (netcat) is required but not installed."
    echo "Install with: sudo apt install netcat  (or brew install netcat on macOS)"
    exit 1
fi

echo "✓ Prerequisites satisfied"
echo ""

# Install binaries
echo "Installing Agent Tmux Monitor..."
cargo install --path crates/atm-daemon
cargo install --path crates/atm-tui

echo "✓ Binaries installed"
echo ""

# Create directories
echo "Creating directories..."
mkdir -p ~/.local/state/atm
mkdir -p ~/.local/bin

# Copy scripts
echo "Installing scripts..."
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp "$SCRIPT_DIR/atm-status.sh" ~/.local/bin/
cp "$SCRIPT_DIR/atm-hooks.sh" ~/.local/bin/
chmod +x ~/.local/bin/atm-status.sh
chmod +x ~/.local/bin/atm-hooks.sh

echo "✓ Scripts installed to ~/.local/bin/"
echo ""

# Configure Claude Code
echo "Configuring Claude Code..."

# Check if .claude exists
CLAUDE_CONFIG_DIR="$HOME/.claude"
if [ ! -d "$CLAUDE_CONFIG_DIR" ]; then
    echo "Creating Claude Code config directory: $CLAUDE_CONFIG_DIR"
    mkdir -p "$CLAUDE_CONFIG_DIR"
fi

# Backup existing config
if [ -f "$CLAUDE_CONFIG_DIR/settings.json" ]; then
    echo "Backing up existing settings.json..."
    cp "$CLAUDE_CONFIG_DIR/settings.json" "$CLAUDE_CONFIG_DIR/settings.json.backup.$(date +%s)"
fi

# Generate settings.json
cat > "$CLAUDE_CONFIG_DIR/settings.json" <<EOF
{
  "statusLine": {
    "type": "command",
    "command": "$HOME/.local/bin/atm-status.sh"
  }
}
EOF

echo "✓ Claude Code configured with status line integration"
echo ""

# Generate hooks.json example
cat > "$CLAUDE_CONFIG_DIR/hooks.json.example" <<EOF
{
  "hooks": {
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ]
  }
}
EOF

echo "✓ Example hooks configuration created: $CLAUDE_CONFIG_DIR/hooks.json.example"
echo "  (Hooks are optional - uncomment to enable detailed tracking)"
echo ""

# Start daemon
echo "Starting ATM daemon..."
atmd start

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Next steps:"
echo "  1. Open a new terminal"
echo "  2. Start a Claude Code session: claude"
echo "  3. In another terminal, run: atm"
echo "  4. You should see your session appear!"
echo ""
echo "Troubleshooting:"
echo "  - Check daemon status: atmd status"
echo "  - View logs: tail -f ~/.local/state/atm/atm.log"
echo "  - Enable debug mode: export ATM_DEBUG=1"
echo ""
echo "To uninstall:"
echo "  - Remove binaries: cargo uninstall atmd atm"
echo "  - Remove scripts: rm ~/.local/bin/atm-*.sh"
echo "  - Remove config: rm ~/.claude/settings.json"
echo ""
```

**Make executable:**
```bash
chmod +x scripts/install.sh
```

#### 6.2 Create Configuration Examples

**Create:** `docs/CONFIGURATION.md`

```markdown
# Agent Tmux Monitor Configuration Guide

## Claude Code Integration

### Status Line (Required)

The status line integration sends context and cost information to Agent Tmux Monitor.

**File:** `~/.claude/settings.json`
```json
{
  "statusLine": {
    "type": "command",
    "command": "/home/username/.local/bin/atm-status.sh"
  }
}
```

### Hooks (Optional)

Hooks provide detailed event tracking (tool usage, permissions).

**File:** `~/.claude/hooks.json`
```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/home/username/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/home/username/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/home/username/.local/bin/atm-hooks.sh"
          }
        ]
      }
    ]
  }
}
```

## Daemon Configuration

**Socket Path:** `/tmp/atm.sock` (default)

**Logs:** `~/.local/state/atm/atm.log`

**Max Sessions:** 100 (configured in code)

### Custom Socket Path

Set environment variable before starting daemon:

```bash
export ATM_SOCKET=/custom/path/atm.sock
atmd start
```

Also update scripts:
```bash
export ATM_SOCKET=/custom/path/atm.sock
```

## TUI Configuration

The TUI automatically connects to the daemon socket.

### Keyboard Shortcuts

- `↑/k` - Move selection up
- `↓/j` - Move selection down
- `d` - Show session details
- `Enter` - Jump to session (requires tmux)
- `r` - Refresh (automatic, but available manually)
- `q` - Quit

## Debug Mode

Enable verbose logging:

```bash
export ATM_DEBUG=1
```

Logs will appear in:
- Daemon: `~/.local/state/atm/atm.log`
- Status script: `/tmp/atm-status.log`
- Hooks script: `/tmp/atm-hooks.log`

## Troubleshooting

### TUI shows "Connecting..." forever

**Cause:** Daemon not running

**Fix:**
```bash
atmd start
```

### No sessions appear in TUI

**Cause:** Status line not configured

**Fix:** Check `~/.claude/settings.json` has status line config

### Scripts fail with "jq not found"

**Cause:** Missing dependency

**Fix:**
```bash
# Ubuntu/Debian
sudo apt install jq netcat

# macOS
brew install jq netcat
```

### High CPU usage

**Cause:** Too many sessions or fast updates

**Fix:** Restart daemon to cleanup stale sessions:
```bash
atmd restart
```
```

#### 6.3 Create README

**Create:** `README.md`

```markdown
# Agent Tmux Monitor

Real-time monitoring and management for Claude Code sessions.

## What is Agent Tmux Monitor?

Agent Tmux Monitor gives you visibility into your Claude Code sessions:
- Real-time context window usage
- Cost tracking across all sessions
- Session management and navigation
- tmux integration for quick access

## Features

- **Real-Time Monitoring:** See context usage, token counts, and costs as they update
- **Multi-Session Management:** Track multiple Claude Code sessions simultaneously
- **Context Pressure Alerts:** Visual indicators when approaching context limits
- **Cost Tracking:** Per-session and aggregate cost monitoring
- **tmux Integration:** Jump directly to sessions from the TUI

## Installation

### Prerequisites

- Rust (1.70+)
- jq
- netcat

### Quick Install

```bash
git clone https://github.com/yourusername/atm.git
cd atm
./scripts/install.sh
```

The installer will:
1. Build and install the daemon and TUI
2. Set up bash scripts for Claude Code integration
3. Configure Claude Code automatically
4. Start the daemon

## Usage

### Start the daemon

```bash
atmd start
```

### Launch the TUI

```bash
atm
```

### Start Claude Code sessions

In any terminal, start Claude Code as normal:
```bash
claude
```

Your session will automatically appear in the ATM TUI!

## Architecture

Agent Tmux Monitor consists of three components:

1. **Daemon (atmd):** Background service that maintains session registry
2. **TUI (atm):** Terminal UI for viewing and managing sessions
3. **Shell Scripts:** Bash scripts that integrate with Claude Code

```
┌─────────────────┐
│  Claude Code    │
│  (Session 1)    │
└────────┬────────┘
         │ status line
         │ (JSON via bash)
         ↓
┌─────────────────┐      Unix Socket      ┌─────────────────┐
│   atmd    │ ←──────────────────→  │   atm     │
│   (daemon)      │                        │   (TUI)         │
│                 │                        │                 │
│  Session        │                        │  • View sessions│
│  Registry       │                        │  • Monitor usage│
└────────┬────────┘                        │  • Track costs  │
         ↑                                 └─────────────────┘
         │
         │
┌────────┴────────┐
│  Claude Code    │
│  (Session 2)    │
└─────────────────┘
```

## Configuration

Configuration files are located in `~/.claude/`:

- **settings.json:** Status line integration (required)
- **hooks.json:** Event hooks (optional)

See [CONFIGURATION.md](docs/CONFIGURATION.md) for details.

## Development

### Build from source

```bash
cargo build --release
```

### Run tests

```bash
cargo test
```

### Project structure

```
atm/
├── crates/
│   ├── atm-daemon/    # Background daemon
│   ├── atm-tui/       # Terminal UI
│   ├── atm-domain/    # Domain model
│   └── atm-protocol/  # Wire protocol
├── scripts/
│   ├── atm-status.sh  # Status line integration
│   ├── atm-hooks.sh   # Hooks integration
│   └── install.sh           # Installation script
└── docs/
    ├── ARCHITECTURE.md
    ├── CONFIGURATION.md
    └── PROTOCOL.md
```

## License

MIT

## Contributing

Contributions welcome! See CONTRIBUTING.md for guidelines.
```

### Day 6 Success Criteria

- ✅ Installation script works end-to-end
- ✅ Configuration examples are complete and tested
- ✅ README provides clear setup instructions
- ✅ Documentation is comprehensive

**Test:**
```bash
# Clean slate test
rm -rf ~/.local/state/atm ~/.local/bin/atm-*.sh
cargo uninstall atmd atm || true

# Run installer
./scripts/install.sh

# Should complete without errors and start daemon
```

---

## Day 7: End-to-End Testing & tmux Integration (1 day)

**Duration:** 1 day
**Priority:** High

### Objective
Test the complete system with real Claude Code sessions and implement tmux jump functionality.

### Tasks

#### 7.1 Implement tmux Jump Feature

**Update:** `crates/atm-tui/src/event.rs`

Add tmux integration:

```rust
// Add to event handler
Enter => {
    if let Some(session) = app.selected_session() {
        if let Err(e) = jump_to_tmux_session(&session.id) {
            tracing::error!("Failed to jump to session: {}", e);
            // TODO: Show error in UI
        }
    }
}

/// Jump to tmux session for a Claude Code session
fn jump_to_tmux_session(session_id: &atm_domain::SessionId) -> anyhow::Result<()> {
    use std::process::Command;

    // Check if in tmux
    let in_tmux = std::env::var("TMUX").is_ok();

    if !in_tmux {
        anyhow::bail!("Not in a tmux session");
    }

    // Find tmux pane with matching PID
    // (This requires we store PID in session metadata)
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"])
        .output()?;

    let panes = String::from_utf8_lossy(&output.stdout);

    // TODO: Match PID from session to pane
    // For now, just demonstrate the concept

    // Switch to pane
    Command::new("tmux")
        .args(["select-pane", "-t", "PANE_ID"])
        .status()?;

    Ok(())
}
```

**Note:** tmux integration requires storing the terminal PID and matching it to tmux panes. This is complex and may be Phase 4 work. For Week 4, implement a basic version or stub it out.

#### 7.2 Create End-to-End Test Plan

**Create:** `docs/E2E_TEST_PLAN.md`

```markdown
# End-to-End Test Plan

## Test Environment

- Fresh installation
- Claude Code installed and configured
- tmux session running

## Test Scenarios

### Scenario 1: Clean Installation

**Steps:**
1. Run `./scripts/install.sh`
2. Verify daemon starts: `atmd status`
3. Verify TUI starts: `atm`
4. Verify Claude Code config: `cat ~/.claude/settings.json`

**Expected:**
- ✅ Installation completes without errors
- ✅ Daemon is running
- ✅ TUI shows "Connected" status
- ✅ settings.json contains status line config

### Scenario 2: Single Session Tracking

**Steps:**
1. Start daemon: `atmd start`
2. Start TUI: `atm`
3. In another terminal, start Claude Code: `claude`
4. Have a conversation with Claude (ask it to do something)
5. Observe TUI

**Expected:**
- ✅ Session appears in TUI within 1-2 seconds
- ✅ Context percentage updates in real-time
- ✅ Token counts increase with conversation
- ✅ Cost accumulates
- ✅ Session details are accurate

### Scenario 3: Multiple Sessions

**Steps:**
1. Start 3 Claude Code sessions in different terminals
2. Have different conversations in each
3. Observe TUI

**Expected:**
- ✅ All 3 sessions appear in TUI
- ✅ Can navigate between sessions with ↑/↓
- ✅ Each session shows independent context/cost
- ✅ Selection works correctly

### Scenario 4: Daemon Restart

**Steps:**
1. Start daemon and TUI
2. Start Claude Code session
3. Stop daemon: `atmd stop`
4. Observe TUI (should show disconnected)
5. Restart daemon: `atmd start`
6. Continue Claude Code conversation

**Expected:**
- ✅ TUI shows "Disconnected" when daemon stops
- ✅ TUI reconnects automatically when daemon restarts
- ✅ Session re-appears in TUI
- ✅ Context/cost continue to update

### Scenario 5: Session Cleanup

**Steps:**
1. Start daemon and TUI
2. Start Claude Code session
3. Exit Claude Code (Ctrl+D)
4. Wait 90 seconds
5. Observe TUI

**Expected:**
- ✅ Session marked as stale after 90s
- ✅ Session removed from TUI after cleanup cycle

### Scenario 6: Error Handling

**Steps:**
1. Start TUI without daemon running
2. Observe error messages
3. Start daemon
4. Verify TUI recovers

**Expected:**
- ✅ TUI shows helpful error message
- ✅ Suggests running `atmd start`
- ✅ Reconnects automatically when daemon starts

### Scenario 7: High Load

**Steps:**
1. Start daemon
2. Start 10 Claude Code sessions simultaneously
3. Have active conversations in all
4. Monitor daemon logs and TUI performance

**Expected:**
- ✅ All sessions appear in TUI
- ✅ TUI remains responsive
- ✅ No errors in daemon logs
- ✅ Memory usage reasonable (<50MB)

## Performance Benchmarks

| Metric | Target | Measured |
|--------|--------|----------|
| Session registration latency | <500ms | |
| Status update latency | <100ms | |
| TUI refresh rate | 10 fps | |
| Daemon memory | <10MB | |
| TUI memory | <20MB | |

## Bug Tracking

| Bug # | Description | Status | Fix |
|-------|-------------|--------|-----|
| 1 | | | |
```

#### 7.3 Run Full Integration Tests

Execute all test scenarios from E2E_TEST_PLAN.md and document results.

**Create a test log:**
```bash
# Run tests and capture output
./scripts/run_e2e_tests.sh > e2e_test_results.log 2>&1
```

#### 7.4 Fix Critical Bugs

Address any critical issues found during testing:
- Session not appearing
- Incorrect context calculations
- Crashes or hangs
- Connection issues

### Day 7 Success Criteria

- ✅ End-to-end test plan executed
- ✅ All critical scenarios pass
- ✅ No blocking bugs remain
- ✅ Performance meets targets
- ✅ System ready for real-world use

---

## Day 8: Polish & Documentation (1 day)

**Duration:** 1 day (optional buffer)
**Priority:** Medium

### Objective
Final polish, documentation updates, and preparation for release.

### Tasks

#### 8.1 Add Usage Examples to README

Add screenshots (ASCII art or actual screenshots) showing:
- TUI with multiple sessions
- Context pressure visualization
- Cost tracking

#### 8.2 Write Troubleshooting Guide

**Create:** `docs/TROUBLESHOOTING.md`

Common issues and solutions:
- Daemon won't start
- Sessions not appearing
- High CPU/memory usage
- Claude Code integration not working

#### 8.3 Create Release Checklist

- [ ] All tests passing
- [ ] Documentation complete
- [ ] Installation script tested
- [ ] README has clear instructions
- [ ] Version numbers updated
- [ ] Git tags created

#### 8.4 Performance Optimization

If time permits:
- Profile daemon for bottlenecks
- Optimize UI rendering
- Reduce memory allocations
- Tune event loop timing

### Day 8 Success Criteria

- ✅ Documentation is comprehensive
- ✅ Known issues documented
- ✅ Release checklist complete
- ✅ Ready for beta users

---

## Week 4 Success Criteria

By the end of Week 4, you should have:

### Core Functionality
- ✅ **TUI runs and connects to daemon**
- ✅ **Sessions appear in real-time** when Claude Code starts
- ✅ **Context usage updates live** (every ~300ms)
- ✅ **Keyboard navigation works** (up/down/quit/detail)
- ✅ **Reconnection handles daemon restarts** gracefully
- ✅ **Color-coded pressure indicators** show context state
- ✅ **Cost tracking accumulates** correctly per session

### Shell Integration
- ✅ **Status line script parses** Claude Code JSON
- ✅ **Session registration** works on first run
- ✅ **Non-blocking operation** (never hangs Claude Code)
- ✅ **Graceful degradation** when daemon unavailable
- ✅ **Error handling exits cleanly** (always exit 0)

### Installation & Configuration
- ✅ **Installation script works** end-to-end
- ✅ **Claude Code configured** automatically
- ✅ **Documentation complete** with examples
- ✅ **README provides** clear setup instructions

### Testing & Quality
- ✅ **End-to-end tests pass** all scenarios
- ✅ **No critical bugs** remain
- ✅ **Performance meets** targets (<10MB daemon, <20MB TUI)
- ✅ **Logs are helpful** for debugging

### User Experience
- ✅ **First-run experience** is smooth
- ✅ **Error messages are** actionable
- ✅ **UI feels polished** and professional
- ✅ **Help text is** clear and visible

---

## Known Limitations (Week 4)

These are acceptable for initial release:

1. **tmux Integration:** Basic or stubbed - full implementation may be Phase 4
2. **Session History:** Not persistent - only shows active sessions
3. **Filtering/Search:** Not implemented - all sessions shown
4. **Custom Themes:** Not implemented - single color scheme
5. **Export/Reports:** Not implemented - no data export

These can be addressed in future phases based on user feedback.

---

## Risk Mitigation

### Risk: Claude Code Integration Differs from Week 1 Validation

**Mitigation:**
- Week 1 validation provides ground truth
- Bash scripts designed to be flexible
- Add debug logging to diagnose issues quickly

### Risk: TUI Performance Issues

**Mitigation:**
- ratatui is battle-tested and performant
- Event loop designed for efficiency
- Can optimize rendering if needed

### Risk: Connection Instability

**Mitigation:**
- Exponential backoff prevents connection storms
- Graceful degradation keeps system usable
- Clear error messages guide troubleshooting

### Risk: Installation Complexity

**Mitigation:**
- Single install script handles everything
- Prerequisites checked upfront
- Backup existing configs before modifying

---

## After Week 4

Once Week 4 is complete, you'll have a working, usable system. Next steps:

### Phase 4 (Future)
- Session history and persistence
- Advanced tmux integration
- Filtering and search
- Export and reporting
- Custom themes and configuration

### Beta Testing
- Share with early users
- Collect feedback
- Iterate on UX
- Fix bugs

### Production Hardening
- Stress testing
- Security review
- Performance optimization
- Monitoring and observability

---

## Summary

Week 4 transforms Agent Tmux Monitor from a backend daemon into a complete, user-facing system:

- **Days 1-2:** Build TUI foundation (layout, state, events)
- **Day 3:** Connect TUI to daemon (client, reconnection)
- **Day 4:** Polish UI (details, errors, colors)
- **Day 5:** Shell scripts (status line, hooks)
- **Day 6:** Installation (script, config, docs)
- **Day 7:** E2E testing (integration, tmux, bugs)
- **Day 8:** Final polish (docs, optimization)

**Outcome:** A production-ready monitoring tool for Claude Code sessions, installable with a single command, providing real-time visibility into context usage and costs.

**Confidence:** HIGH - Building on validated integrations from Week 1 and working daemon from Weeks 2-3.
