# Week 1: Planning & Validation (3-4 days)

**Goal:** Resolve critical architectural uncertainties and complete specification gaps before implementation

**Status:** 🔴 BLOCKING - Must complete before Week 2

---

## Overview

Week 1 addresses all **CRITICAL** and **HIGH** priority issues identified in the critique. These are blocking issues that could derail implementation if not resolved upfront.

**Investment:** 3-4 days of planning work
**Return:** Prevents weeks of rework and production issues

---

## Day 1: Claude Code Integration Validation

**Duration:** 1-2 days
**Priority:** 🔴 CRITICAL (Could block Phase 3 entirely)

### Objective
Validate that Claude Code's Status Line API and Hooks system work as assumed in the implementation plan.

### Tasks

#### 1.1 Set Up Test Environment
```bash
# Create test directory in project root
mkdir -p integration-test
cd integration-test

# Create test Claude Code configuration
mkdir -p .claude
```

#### 1.2 Test Status Line Component

**Create test script:** `test-status-line.sh`
```bash
#!/bin/bash
# Test status line component

LOG_FILE="/tmp/atm-status-test.log"

echo "$(date): Received status line update" >> "$LOG_FILE"

# Read JSON from stdin
while IFS= read -r line; do
    echo "$(date): $line" >> "$LOG_FILE"

    # Parse JSON fields (requires jq)
    USED_PCT=$(echo "$line" | jq -r '.context_window.used_percentage // "N/A"')
    INPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_input_tokens // "N/A"')
    OUTPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_output_tokens // "N/A"')
    COST=$(echo "$line" | jq -r '.cost.total_cost_usd // "N/A"')
    MODEL=$(echo "$line" | jq -r '.model.id // "N/A"')

    echo "$(date): Parsed - Used: $USED_PCT%, In: $INPUT_TOKENS, Out: $OUTPUT_TOKENS, Cost: \$$COST, Model: $MODEL" >> "$LOG_FILE"
done
```

**Configure in `.claude/settings.json`:**
```json
{
  "statusLine": {
    "type": "command",
    "command": "./test-status-line.sh"
  }
}
```

**Test:**
1. Start Claude Code in test directory
2. Have conversation to generate status updates
3. Verify `/tmp/atm-status-test.log` receives JSON every ~300ms
4. Validate JSON structure matches plan specification

**Expected Outcome:**
- ✅ Status line command receives JSON on stdin
- ✅ Updates arrive approximately every 300ms
- ✅ JSON structure matches specification (context_window, cost, model fields present)

**If Failed:**
- Document actual JSON structure received
- Identify missing fields
- Revise protocol specification accordingly

#### 1.3 Test Hooks System

**Create test script:** `test-hooks.sh`
```bash
#!/bin/bash
# Test hooks handler

LOG_FILE="/tmp/atm-hooks-test.log"

echo "$(date): Hook triggered" >> "$LOG_FILE"

# Read hook event JSON from stdin
IFS= read -r line
echo "$(date): $line" >> "$LOG_FILE"

# Parse hook event
HOOK_EVENT=$(echo "$line" | jq -r '.hook_event_name // "N/A"')
SESSION_ID=$(echo "$line" | jq -r '.session_id // "N/A"')

echo "$(date): Hook: $HOOK_EVENT, Session: $SESSION_ID" >> "$LOG_FILE"

# Check for PermissionRequest specifically
if [ "$HOOK_EVENT" = "PermissionRequest" ]; then
    TOOL=$(echo "$line" | jq -r '.tool_name // "N/A"')
    echo "$(date): Permission requested for tool: $TOOL" >> "$LOG_FILE"
fi
```

**Configure in `.claude/hooks.json`:**
```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./test-hooks.sh"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./test-hooks.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./test-hooks.sh"
          }
        ]
      }
    ]
  }
}
```

**Test:**
1. Trigger permission request (ask Claude to edit a file)
2. Trigger tool execution (ask Claude to run bash command)
3. Verify `/tmp/atm-hooks-test.log` receives events
4. Validate JSON structure

**Expected Outcome:**
- ✅ Hooks execute when events occur
- ✅ JSON structure includes hook_event_name, session_id, tool information
- ✅ Can detect PermissionRequest events specifically

**If Failed:**
- Document actual hook mechanism
- Explore alternative integration methods
- Consider fallback approaches (polling Claude Code logs, etc.)

#### 1.4 Verify Session ID Availability

**Question:** Can we uniquely identify Claude Code sessions?

**Options to test:**
1. **Environment variable:** Check if `$CLAUDE_SESSION_ID` exists
2. **Process PID:** Use `$$` (bash PID) or parent PID
3. **Generate UUID:** Use `uuidgen` or similar

**Test script additions:**
```bash
# In test-status-line.sh
echo "CLAUDE_SESSION_ID: ${CLAUDE_SESSION_ID:-not_set}" >> "$LOG_FILE"
echo "BASH_PID: $$" >> "$LOG_FILE"
echo "PPID: $PPID" >> "$LOG_FILE"
```

**Decision:** Choose session ID strategy based on what's available

#### 1.5 Document Findings

**Create:** `CLAUDE_CODE_INTEGRATION.md`

Document:
- ✅ Confirmed integration methods
- ⚠️ Deviations from plan assumptions
- 🔧 Required protocol adjustments
- 📋 Example JSON structures (actual data from tests)

---

## Day 2: Architecture Specifications

**Duration:** 1 day
**Priority:** 🔴 CRITICAL

### Objective
Complete missing architectural specifications that could cause production issues.

### Tasks

#### 2.1 Define Concurrency Model

**Create:** `docs/CONCURRENCY_MODEL.md`

```markdown
# Agent Tmux Monitor Concurrency Model

## Overview
ATM daemon uses an **Actor Model** for session registry to ensure data consistency without explicit locking.

## Architecture

### Registry Actor
- Single tokio task owns the `SessionRegistry`
- All mutations go through message passing
- Guarantees serial access to registry (no race conditions)

### Message Types
\`\`\`rust
pub enum RegistryCommand {
    Register {
        session: SessionDomain,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },
    UpdateContext {
        session_id: SessionId,
        context: ContextUsage,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },
    GetSession {
        session_id: SessionId,
        respond_to: oneshot::Sender<Option<SessionView>>,
    },
    GetAllSessions {
        respond_to: oneshot::Sender<Vec<SessionView>>,
    },
    Unregister {
        session_id: SessionId,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },
}
\`\`\`

### Implementation Pattern
\`\`\`rust
pub struct RegistryActor {
    receiver: mpsc::Receiver<RegistryCommand>,
    registry: HashMap<SessionId, SessionDomain>,
    infrastructure: HashMap<SessionId, SessionInfrastructure>,
    event_publisher: broadcast::Sender<SessionEvent>,
}

impl RegistryActor {
    pub async fn run(mut self) {
        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                RegistryCommand::Register { session, respond_to } => {
                    let result = self.handle_register(session);
                    let _ = respond_to.send(result);
                }
                // ... handle other commands
            }
        }
    }

    fn handle_register(&mut self, session: SessionDomain) -> Result<(), RegistryError> {
        if self.registry.len() >= MAX_SESSIONS {
            return Err(RegistryError::RegistryFull);
        }

        self.registry.insert(session.id.clone(), session.clone());

        // Publish event
        let _ = self.event_publisher.send(SessionEvent::Registered {
            session_id: session.id.clone(),
            agent_type: session.agent_type,
            timestamp: Utc::now(),
        });

        Ok(())
    }
}
\`\`\`

### Client Interface
\`\`\`rust
#[derive(Clone)]
pub struct RegistryHandle {
    sender: mpsc::Sender<RegistryCommand>,
}

impl RegistryHandle {
    pub async fn register(&self, session: SessionDomain) -> Result<(), RegistryError> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(RegistryCommand::Register {
            session,
            respond_to: tx,
        }).await?;
        rx.await?
    }

    pub async fn get_all_sessions(&self) -> Vec<SessionView> {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(RegistryCommand::GetAllSessions {
            respond_to: tx,
        }).await;
        rx.await.unwrap_or_default()
    }
}
\`\`\`

## Why Actor Model?

**Pros:**
- Zero lock contention (no Arc<Mutex> or Arc<RwLock>)
- Impossible to have deadlocks
- Easy to reason about (single-threaded access)
- Natural fit for Rust's ownership model
- Message queue provides backpressure automatically

**Cons:**
- Slightly more boilerplate (command enums, oneshot channels)
- All operations are async (must await even for reads)

## Alternative: RwLock (Not Chosen)
\`\`\`rust
// We could use this, but decided against it:
pub struct SessionRegistry {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
}

// Why not:
// - Risk of lock contention under load
// - Easy to accidentally hold locks across await points (deadlock risk)
// - Requires careful lock hierarchy management
\`\`\`

## Async Boundaries

### Where await is allowed:
- Inside tokio tasks
- In async functions
- After acquiring oneshot responses

### Where await is NOT allowed:
- Inside sync callbacks
- In Drop implementations
- While holding locks (if we used locks)

## Testing Strategy
- Unit test RegistryActor with mock commands
- Integration test with multiple concurrent clients
- Stress test with 100+ sessions and 1000+ messages/sec
```

#### 2.2 Define Error Handling Strategy

**Create:** `docs/ERROR_HANDLING.md`

```markdown
# Agent Tmux Monitor Error Handling Strategy

## Principles
1. **Fail fast in development, graceful degradation in production**
2. **Never break Claude Code** - bash scripts always exit 0
3. **Observable failures** - log all errors with context
4. **Typed errors** - use thiserror for clear error types

## Error Types by Layer

### Daemon Errors
\`\`\`rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Session registry is full (max: {max})")]
    RegistryFull { max: usize },

    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    #[error("Invalid message format: {0}")]
    InvalidFormat(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
\`\`\`

### TUI Errors
\`\`\`rust
#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Daemon disconnected")]
    DaemonDisconnected,

    #[error("Daemon connection timeout")]
    ConnectionTimeout,

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("Invalid terminal state")]
    InvalidState,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
\`\`\`

### Shell Script Exit Codes
\`\`\`bash
# Exit code semantics:
# 0  = Success (or graceful degradation)
# 1  = Daemon unavailable (silent, don't log)
# 2  = Invalid message format
# 3  = Timeout

# CRITICAL: Always exit 0 to avoid breaking Claude Code
# Log errors to file, but don't propagate failures
\`\`\`

## Retry Policies

### Bash Scripts (Status Line & Hooks)
\`\`\`bash
# Non-blocking socket write with timeout
# If fails, just exit cleanly

SOCKET="/tmp/atm.sock"
TIMEOUT=0.1  # 100ms

if [ ! -S "$SOCKET" ]; then
    # Daemon not running - silently exit
    exit 0
fi

# Send with timeout
echo "$message" | timeout $TIMEOUT nc -U "$SOCKET" 2>/dev/null || {
    # Failed to send - log but don't break
    echo "$(date): Failed to send to daemon" >> /tmp/atm-client.log
    exit 0  # Exit 0 anyway!
}

exit 0
\`\`\`

### TUI Daemon Connection
\`\`\`rust
use tokio::time::{sleep, Duration};

pub struct DaemonClient {
    socket_path: PathBuf,
    retry_config: RetryConfig,
}

pub struct RetryConfig {
    initial_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl DaemonClient {
    pub async fn connect_with_retry(&self) -> Result<UnixStream, TuiError> {
        let mut delay = self.retry_config.initial_delay;

        loop {
            match UnixStream::connect(&self.socket_path).await {
                Ok(stream) => {
                    tracing::info!("Connected to daemon");
                    return Ok(stream);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to daemon: {}, retrying in {:?}", e, delay);
                    sleep(delay).await;

                    // Exponential backoff
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * self.retry_config.multiplier)
                            .min(self.retry_config.max_delay.as_secs_f64())
                    );
                }
            }
        }
    }
}
\`\`\`

### Daemon Error Recovery
\`\`\`rust
// Daemon continues operation even if individual requests fail

async fn handle_connection(stream: UnixStream, registry: RegistryHandle) {
    let (reader, writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<ClientMessage>(&line) {
            Ok(msg) => {
                // Process message
                if let Err(e) = handle_message(msg, &registry).await {
                    tracing::error!("Failed to handle message: {}", e);
                    // Log error but keep connection alive
                }
            }
            Err(e) => {
                tracing::error!("Failed to parse message: {}, line: {}", e, line);
                // Log malformed message but continue
            }
        }
    }

    tracing::info!("Client disconnected");
}
\`\`\`

## Graceful Degradation

### Bash Scripts: Daemon Unavailable
**Behavior:** Silently fail, exit 0, don't break Claude Code
\`\`\`bash
# Quick check before attempting connection
if [ ! -S "$SOCKET" ]; then
    # Don't even try - just exit
    exit 0
fi
\`\`\`

### TUI: Daemon Disconnected
**Behavior:** Show banner, keep trying to reconnect, allow exit
\`\`\`rust
pub enum AppState {
    Connected,
    Disconnected { since: DateTime<Utc> },
}

// In UI rendering:
if matches!(app.state, AppState::Disconnected { .. }) {
    let banner = Paragraph::new("⚠️  Daemon Disconnected - Reconnecting...")
        .style(Style::default().bg(Color::Red).fg(Color::White));
    f.render_widget(banner, top_area);
}
\`\`\`

### Daemon: Resource Exhaustion
**Behavior:** Reject new connections, log error, keep serving existing
\`\`\`rust
if active_clients.len() >= MAX_CLIENTS {
    tracing::warn!("Max clients reached, rejecting connection");
    // Send error message and close
    let error = DaemonMessage::Error {
        message: "Server at capacity".to_string(),
    };
    let _ = write_message(&mut stream, &error).await;
    return;
}
\`\`\`

## Logging Strategy

### Structured Logging with tracing
\`\`\`rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(registry))]
async fn handle_register(
    session: SessionDomain,
    registry: &RegistryHandle,
) -> Result<(), DaemonError> {
    debug!("Registering session: {:?}", session.id);

    match registry.register(session.clone()).await {
        Ok(()) => {
            info!(
                session_id = %session.id,
                agent_type = ?session.agent_type,
                "Session registered successfully"
            );
            Ok(())
        }
        Err(e) => {
            error!(
                session_id = %session.id,
                error = %e,
                "Failed to register session"
            );
            Err(e.into())
        }
    }
}
\`\`\`

### Log Levels
- **ERROR:** Unexpected failures that need investigation
- **WARN:** Degraded operation (disconnections, retries, limits hit)
- **INFO:** Normal operation (sessions registered, clients connected)
- **DEBUG:** Detailed flow (messages received, state changes)
- **TRACE:** Very verbose (every message content)

### Log Destinations
- **Daemon:** `~/.local/state/atm/atm.log` (with rotation)
- **TUI:** stderr (user's terminal)
- **Bash scripts:** `/tmp/atm-client.log` (only on errors)

## User-Facing Error Messages

### TUI Messages
\`\`\`rust
pub fn error_message(error: &TuiError) -> String {
    match error {
        TuiError::DaemonDisconnected => {
            "Daemon disconnected. Reconnecting...".to_string()
        }
        TuiError::ConnectionTimeout => {
            "Connection timeout. Is atmd running? (Try: atmd start)".to_string()
        }
        TuiError::RenderError(e) => {
            format!("Display error: {}. Try resizing terminal.", e)
        }
        TuiError::InvalidState => {
            "Invalid state. Please restart atm.".to_string()
        }
        TuiError::IoError(e) => {
            format!("IO error: {}", e)
        }
    }
}
\`\`\`

## Testing Error Handling

### Unit Tests
\`\`\`rust
#[tokio::test]
async fn test_registry_full() {
    let registry = create_test_registry(max_sessions: 2);

    // Fill registry
    registry.register(session1).await.unwrap();
    registry.register(session2).await.unwrap();

    // This should fail
    let result = registry.register(session3).await;
    assert!(matches!(result, Err(DaemonError::RegistryFull { .. })));
}
\`\`\`

### Integration Tests
\`\`\`rust
#[tokio::test]
async fn test_daemon_survives_malformed_messages() {
    let daemon = start_test_daemon().await;
    let mut client = connect_to_daemon().await;

    // Send malformed JSON
    client.write_all(b"{ invalid json }\n").await.unwrap();

    // Daemon should still be responsive
    let sessions = client.get_all_sessions().await.unwrap();
    assert_eq!(sessions.len(), 0);
}
\`\`\`

### Manual Tests
1. Kill daemon while TUI running - verify reconnection
2. Fill registry to max - verify new sessions rejected gracefully
3. Send invalid JSON - verify daemon logs error and continues
4. Network partition (close socket) - verify bash scripts don't hang
```

#### 2.3 Define Resource Limits

**Create:** `docs/RESOURCE_LIMITS.md`

```markdown
# Agent Tmux Monitor Resource Limits

## Session Registry Limits

### Maximum Active Sessions
\`\`\`rust
pub const MAX_SESSIONS: usize = 100;
\`\`\`

**Rationale:** 100 concurrent Claude Code sessions is far beyond typical use (expect 5-10). This provides plenty of headroom while preventing unbounded growth.

**Behavior when limit reached:**
- New registration requests return `DaemonError::RegistryFull`
- Oldest stale sessions are cleaned up
- If still full, reject with error

### Session Retention Policy
\`\`\`rust
pub const STALE_THRESHOLD: Duration = Duration::from_secs(90);
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(30);
pub const MAX_SESSION_AGE: Duration = Duration::from_hours(24);
\`\`\`

**State Machine:**
```
Registration
     ↓
  Active (receiving updates)
     ↓ (90s no updates)
  Stale
     ↓ (cleanup cycle)
  Removed
```

**Cleanup Logic:**
\`\`\`rust
impl RegistryActor {
    async fn cleanup_task(mut self) {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);

        loop {
            interval.tick().await;
            self.cleanup_stale_sessions();
        }
    }

    fn cleanup_stale_sessions(&mut self) {
        let now = Utc::now();
        let mut to_remove = Vec::new();

        for (id, session) in &self.registry {
            // Remove if stale
            if session.is_stale(now) {
                to_remove.push(id.clone());
            }
            // Remove if too old (even if active)
            else if now.signed_duration_since(session.started_at) > MAX_SESSION_AGE {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            self.registry.remove(&id);
            self.infrastructure.remove(&id);

            tracing::info!(session_id = %id, "Cleaned up stale session");

            let _ = self.event_publisher.send(SessionEvent::Removed {
                session_id: id,
                reason: RemovalReason::Stale,
            });
        }
    }
}
\`\`\`

## Client Connection Limits

### Maximum Concurrent TUI Clients
\`\`\`rust
pub const MAX_CLIENTS: usize = 10;
\`\`\```

**Rationale:** Typical use is 1-2 TUI instances. 10 provides headroom for multiple monitors, testing, etc.

**Behavior when limit reached:**
- New connections rejected with error message
- Existing clients continue unaffected

**Implementation:**
\`\`\`rust
pub struct DaemonServer {
    active_clients: Arc<Mutex<HashMap<ClientId, ClientInfo>>>,
}

impl DaemonServer {
    async fn accept_connection(&self, stream: UnixStream) -> Result<()> {
        let active_count = self.active_clients.lock().await.len();

        if active_count >= MAX_CLIENTS {
            // Send error and close
            let error = DaemonMessage::Error {
                code: "max_clients_reached",
                message: format!("Server at capacity ({}/{})", active_count, MAX_CLIENTS),
            };

            let _ = write_message(&mut stream, &error).await;
            return Err(DaemonError::TooManyClients);
        }

        // Accept client...
        Ok(())
    }
}
\`\`\`

### Per-Client Message Buffer
\`\`\`rust
pub const CLIENT_BUFFER_SIZE: usize = 100;
\`\`\`

**Rationale:** Daemon broadcasts session updates to all clients. If a client is slow to consume, buffer prevents blocking other clients.

**Behavior when buffer full:**
- Drop oldest messages for that client
- Log warning
- Client may see stale data but won't block system

**Implementation:**
\`\`\`rust
// Use bounded channel for each client
let (tx, rx) = mpsc::channel::<DaemonMessage>(CLIENT_BUFFER_SIZE);

// In broadcast loop:
for (client_id, client_tx) in &clients {
    if let Err(mpsc::error::SendError(_)) = client_tx.try_send(msg.clone()) {
        // Buffer full - client is too slow
        tracing::warn!(client_id = %client_id, "Client buffer full, dropping message");
    }
}
\`\`\`

## Message Size Limits

### Maximum Message Size
\`\`\```rust
pub const MAX_MESSAGE_SIZE: usize = 1_048_576; // 1MB
\`\`\`

**Rationale:** Session objects are ~1-2KB. 1MB allows for 500+ sessions in a single message with headroom.

**Behavior when exceeded:**
- Parse error logged
- Connection closed
- Client must reconnect

**Implementation:**
\`\`\`rust
async fn read_message<T: DeserializeOwned>(reader: &mut BufReader<OwnedReadHalf>) -> Result<T> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    if line.len() > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge {
            size: line.len(),
            max: MAX_MESSAGE_SIZE,
        }.into());
    }

    Ok(serde_json::from_str(&line)?)
}
\`\`\`

### Maximum Broadcast Rate
\`\`\`rust
pub const MAX_BROADCAST_RATE: f64 = 10.0; // Hz (broadcasts per second)
\`\`\`

**Rationale:** 10 broadcasts/sec is more than enough for real-time feel. Higher rates waste CPU.

**Implementation:**
\`\`\`rust
use tokio::time::{interval, Duration};

async fn broadcast_loop(mut rx: broadcast::Receiver<SessionEvent>, clients: ClientMap) {
    let mut interval = interval(Duration::from_secs_f64(1.0 / MAX_BROADCAST_RATE));
    let mut pending_updates: HashMap<SessionId, SessionView> = HashMap::new();

    loop {
        tokio::select! {
            // Collect updates
            Ok(event) = rx.recv() => {
                if let SessionEvent::Updated { session_id, session } = event {
                    pending_updates.insert(session_id, session);
                }
            }

            // Broadcast accumulated updates at max rate
            _ = interval.tick() => {
                if !pending_updates.is_empty() {
                    let msg = DaemonMessage::BatchUpdate {
                        sessions: pending_updates.drain().collect(),
                    };

                    broadcast_to_clients(&clients, msg).await;
                }
            }
        }
    }
}
\`\`\`

## Memory Limits

### Estimated Memory Usage

**Per Session:**
- SessionDomain: ~200 bytes
- SessionInfrastructure: ~100 bytes
- Total per session: ~300 bytes

**Total at max capacity:**
- 100 sessions × 300 bytes = 30KB (negligible)
- Daemon overhead: ~5MB (tokio runtime, buffers)
- **Total daemon memory: ~5-10MB**

### Memory Monitoring
\`\`\`rust
use sysinfo::{System, SystemExt};

pub struct DaemonMetrics {
    system: System,
}

impl DaemonMetrics {
    pub fn memory_usage_mb(&mut self) -> u64 {
        self.system.refresh_process(sysinfo::get_current_pid().unwrap());

        if let Some(process) = self.system.process(sysinfo::get_current_pid().unwrap()) {
            process.memory() / 1024 / 1024
        } else {
            0
        }
    }
}

// Log memory usage periodically
async fn metrics_task(mut metrics: DaemonMetrics) {
    let mut interval = interval(Duration::from_secs(60));

    loop {
        interval.tick().await;
        let mem_mb = metrics.memory_usage_mb();

        tracing::info!(memory_mb = mem_mb, "Daemon memory usage");

        if mem_mb > 100 {
            tracing::warn!(memory_mb = mem_mb, "High memory usage detected");
        }
    }
}
\`\`\`

## Log File Limits

### Log Rotation
\`\`\`rust
// Using tracing-appender for log rotation

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;

pub fn setup_logging() {
    let log_dir = PathBuf::from(env::var("HOME").unwrap())
        .join(".local/state/atm");

    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,        // Rotate daily
        log_dir,
        "atm.log"
    );

    fmt()
        .with_writer(file_appender)
        .with_max_level(tracing::Level::INFO)
        .init();
}
\`\`\`

**Policy:**
- Rotate daily
- Keep last 7 days
- Max 10MB per file

### Log Cleanup
\`\`\`bash
# In atmd start script

# Remove logs older than 7 days
find ~/.local/state/atm -name "atm.log.*" -mtime +7 -delete
\`\`\`

## Summary Table

| Resource | Limit | Behavior When Exceeded |
|----------|-------|------------------------|
| Active Sessions | 100 | Reject new, cleanup stale |
| TUI Clients | 10 | Reject new connections |
| Message Size | 1MB | Close connection |
| Broadcast Rate | 10 Hz | Queue and batch |
| Per-Client Buffer | 100 msgs | Drop oldest |
| Session Age | 24 hours | Force cleanup |
| Stale Threshold | 90 seconds | Mark for cleanup |
| Log Files | 7 days | Auto-delete |
```

#### 2.4 Add Protocol Versioning

**Update original plan:** Add versioning to all protocol message examples

**Create:** `docs/PROTOCOL_VERSIONING.md`

```markdown
# Agent Tmux Monitor Protocol Versioning

## Version Format

**Semantic Versioning:** `MAJOR.MINOR`
- **MAJOR:** Breaking changes (incompatible message formats)
- **MINOR:** Backward-compatible additions (new optional fields)

**Current Version:** `1.0`

## Message Format

### All Messages Include Version
\`\`\`json
{
  "protocol_version": "1.0",
  "type": "register",
  ...
}
\`\`\`

### Version in First Message Only (Optimization)
For status updates (sent every 300ms), version overhead is expensive. Instead:

**Handshake:** First message includes version
\`\`\`json
{
  "protocol_version": "1.0",
  "type": "register",
  "session_id": "abc123",
  ...
}
\`\`\```

**Subsequent Messages:** Version omitted (assumed same as handshake)
\`\`\`json
{
  "type": "status_update",
  "session_id": "abc123",
  ...
}
\`\`\`

## Version Negotiation

### Client → Daemon
\`\`\```rust
pub struct ClientHello {
    pub protocol_version: String,
    pub client_type: ClientType,
}

pub enum ClientType {
    Session,  // Claude Code session (bash script)
    Tui,      // TUI client
}
\`\`\`

### Daemon Response
\`\`\`rust
pub struct ServerHello {
    pub protocol_version: String,
    pub accepted: bool,
    pub reason: Option<String>,
}

// If accepted=false, connection closes after this message
\`\`\```

### Compatibility Rules

**1.0 daemon:**
- Accepts: 1.0, 1.1, 1.x (same major)
- Rejects: 2.0+ (major version mismatch)

**2.0 daemon:**
- Accepts: 2.0, 2.x
- Rejects: 1.x (legacy not supported)
- **Note:** Migration period should support 1.x for 6 months

## Backward Compatibility Strategy

### Adding Optional Fields (Minor Version Bump)

**Example:** Adding `memory_pressure` in v1.1

**v1.0 Session struct:**
\`\`\`rust
pub struct Session {
    pub session_id: SessionId,
    pub status: SessionStatus,
    pub context: ContextUsage,
    // ...
}
\`\`\`

**v1.1 Session struct:**
\`\`\`rust
pub struct Session {
    pub session_id: SessionId,
    pub status: SessionStatus,
    pub context: ContextUsage,

    // New in v1.1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_pressure: Option<f64>,
}
\`\`\```

**v1.0 clients:** Ignore unknown field (serde default)
**v1.1 clients:** Use new field if present

### Breaking Changes (Major Version Bump)

**Example:** Renaming field in v2.0

**v1.x:**
\`\`\`json
{"agent_type": "general-purpose"}
\`\`\`

**v2.0:**
\`\`\```json
{"agent_kind": "general-purpose"}
\`\`\`

**Migration:** Run both protocols during transition

\`\`\```rust
pub enum ProtocolVersion {
    V1_0,
    V2_0,
}

impl DaemonServer {
    async fn handle_message(&self, version: ProtocolVersion, line: String) -> Result<()> {
        match version {
            ProtocolVersion::V1_0 => {
                let msg: V1ClientMessage = serde_json::from_str(&line)?;
                self.handle_v1(msg).await
            }
            ProtocolVersion::V2_0 => {
                let msg: V2ClientMessage = serde_json::from_str(&line)?;
                self.handle_v2(msg).await
            }
        }
    }
}
\`\`\`

## Version Detection in Bash Scripts

**atm-status.sh:**
\`\`\```bash
#!/bin/bash

PROTOCOL_VERSION="1.0"
SESSION_ID="${CLAUDE_SESSION_ID:-$(uuidgen)}"
SOCKET="/tmp/atm.sock"

# On first run, send handshake
if [ ! -f /tmp/atm-session-$SESSION_ID ]; then
    HANDSHAKE=$(cat <<EOF
{
  "protocol_version": "$PROTOCOL_VERSION",
  "type": "register",
  "session_id": "$SESSION_ID",
  "pid": $$,
  ...
}
EOF
)
    echo "$HANDSHAKE" | timeout 0.1 nc -U "$SOCKET" 2>/dev/null || exit 0
    touch /tmp/atm-session-$SESSION_ID
fi

# Subsequent messages omit version
while IFS= read -r line; do
    STATUS_UPDATE=$(cat <<EOF
{
  "type": "status_update",
  "session_id": "$SESSION_ID",
  "context": $(echo "$line" | jq '.context_window'),
  ...
}
EOF
)
    echo "$STATUS_UPDATE" | timeout 0.1 nc -U "$SOCKET" 2>/dev/null || exit 0
done
\`\`\```

## Future Version Planning

### v1.1 (Planned Features)
- Add `memory_pressure: Option<f64>` to Session
- Add `event_history: Vec<SessionEvent>` to GetSession response
- Add `filter` parameter to GetAllSessions request

### v2.0 (Breaking Changes - Distant Future)
- Change agent_type from String to enum in JSON
- Rename fields for consistency
- Switch to binary protocol (msgpack/bincode) for performance

## Version Mismatch Error Messages

### For Users (TUI)
```
Error: Protocol version mismatch
Daemon version: 2.0
Client version: 1.0

Please upgrade atm TUI:
  cargo install atm --force
```

### For Developers (Logs)
```
WARN atmd::protocol: Version mismatch from client
  client_version="1.0"
  daemon_version="2.0"
  client_addr="/tmp/atm.sock"
  action="rejected"
```
```

---

## Day 3: Create Refactored Domain Model Specification

**Duration:** 4-6 hours
**Priority:** 🟡 HIGH

### Objective
Design clean domain model following DDD principles, separating domain logic from infrastructure concerns.

### Tasks

**Create:** `docs/DOMAIN_MODEL.md`

This is a large file - will continue in next response due to length...

---

## Week 1 Success Criteria

By end of Week 1, you should have:

- ✅ **Validated Claude Code integration** - Confirmed status line and hooks work
- ✅ **Documented integration details** - Actual JSON structures, timing, limitations
- ✅ **Concurrency model specified** - Actor pattern with message passing
- ✅ **Error handling strategy documented** - Error types, retry policies, graceful degradation
- ✅ **Resource limits defined** - Max sessions, clients, memory, logs
- ✅ **Protocol versioning added** - All messages include version field
- ✅ **Domain model designed** - Clean separation of domain/infrastructure

**Confidence Level:** If all tasks complete successfully, proceed to Week 2 with **HIGH** confidence.

**If Issues Found:** Adjust plan based on learnings before starting implementation.
