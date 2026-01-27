# SessionStatus Transition Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use feature-dev:feature-dev to thoughtfully implement this plan task-by-task checking each Phase with critique.

**Goal:** Refactor SessionStatus from 6 variants to 3 semantic states (Idle/Working/AttentionNeeded), expand HookEventType to all 12 Claude Code events, and eliminate DisplayState.

**Architecture:** Events-first approach - expand hook event handling first, then refactor SessionStatus. The hybrid parsing strategy keeps wire-level parsing simple (flat RawHookEvent) while providing type safety in the domain (typed HookEvent enum).

**Tech Stack:** Rust, serde, chrono, tokio (existing stack)

---

## Phase 1: Expand HookEventType Enum

### Task 1: Add New Hook Event Variants

**Files:**
- Modify: `crates/atm-core/src/hook.rs:32-76`
- Test: `crates/atm-core/src/hook.rs` (existing test module)

**Step 1: Write the failing tests**

```rust
// Add to existing tests module in hook.rs
#[test]
fn test_hook_event_all_variants_parse() {
    // Tool events
    assert_eq!(HookEventType::from_event_name("PreToolUse"), Some(HookEventType::PreToolUse));
    assert_eq!(HookEventType::from_event_name("PostToolUse"), Some(HookEventType::PostToolUse));
    assert_eq!(HookEventType::from_event_name("PostToolUseFailure"), Some(HookEventType::PostToolUseFailure));

    // User events
    assert_eq!(HookEventType::from_event_name("UserPromptSubmit"), Some(HookEventType::UserPromptSubmit));
    assert_eq!(HookEventType::from_event_name("Stop"), Some(HookEventType::Stop));

    // Subagent events
    assert_eq!(HookEventType::from_event_name("SubagentStart"), Some(HookEventType::SubagentStart));
    assert_eq!(HookEventType::from_event_name("SubagentStop"), Some(HookEventType::SubagentStop));

    // Session events
    assert_eq!(HookEventType::from_event_name("SessionStart"), Some(HookEventType::SessionStart));
    assert_eq!(HookEventType::from_event_name("SessionEnd"), Some(HookEventType::SessionEnd));

    // Context events
    assert_eq!(HookEventType::from_event_name("PreCompact"), Some(HookEventType::PreCompact));
    assert_eq!(HookEventType::from_event_name("Setup"), Some(HookEventType::Setup));

    // Notification
    assert_eq!(HookEventType::from_event_name("Notification"), Some(HookEventType::Notification));
}

#[test]
fn test_hook_event_classification_extended() {
    // Pre-events
    assert!(HookEventType::PreToolUse.is_pre_event());
    assert!(HookEventType::SessionStart.is_pre_event());
    assert!(HookEventType::PreCompact.is_pre_event());

    // Post-events
    assert!(HookEventType::PostToolUse.is_post_event());
    assert!(HookEventType::PostToolUseFailure.is_post_event());
    assert!(HookEventType::SessionEnd.is_post_event());
    assert!(HookEventType::Stop.is_post_event());
    assert!(HookEventType::SubagentStop.is_post_event());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p atm-core hook_event_all_variants`
Expected: FAIL with "no variant named PostToolUseFailure"

**Step 3: Implement the expanded enum**

Replace the `HookEventType` enum in `hook.rs`:

```rust
/// Types of hook events from Claude Code.
///
/// All 12 Claude Code hook events, based on official documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventType {
    // === Tool Execution ===
    /// Before a tool is executed
    PreToolUse,
    /// After a tool completes successfully
    PostToolUse,
    /// After a tool fails
    PostToolUseFailure,

    // === User Interaction ===
    /// User submitted a prompt
    UserPromptSubmit,
    /// Claude stopped responding (finished turn)
    Stop,

    // === Subagent Lifecycle ===
    /// A subagent was spawned
    SubagentStart,
    /// A subagent completed
    SubagentStop,

    // === Session Lifecycle ===
    /// Session started (new, resumed, or cleared)
    SessionStart,
    /// Session ended
    SessionEnd,

    // === Context Management ===
    /// Context compaction is about to occur
    PreCompact,
    /// One-time setup is running
    Setup,

    // === Notifications ===
    /// Informational notification
    Notification,
}

impl HookEventType {
    /// Returns true if this is a pre-execution event.
    pub fn is_pre_event(&self) -> bool {
        matches!(
            self,
            Self::PreToolUse | Self::SessionStart | Self::PreCompact | Self::SubagentStart | Self::Setup
        )
    }

    /// Returns true if this is a post-execution event.
    pub fn is_post_event(&self) -> bool {
        matches!(
            self,
            Self::PostToolUse
                | Self::PostToolUseFailure
                | Self::SessionEnd
                | Self::Stop
                | Self::SubagentStop
        )
    }

    /// Parses from a hook event name string.
    pub fn from_event_name(name: &str) -> Option<Self> {
        match name {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "PostToolUseFailure" => Some(Self::PostToolUseFailure),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            "Stop" => Some(Self::Stop),
            "SubagentStart" => Some(Self::SubagentStart),
            "SubagentStop" => Some(Self::SubagentStop),
            "SessionStart" => Some(Self::SessionStart),
            "SessionEnd" => Some(Self::SessionEnd),
            "PreCompact" => Some(Self::PreCompact),
            "Setup" => Some(Self::Setup),
            "Notification" => Some(Self::Notification),
            _ => None,
        }
    }
}

impl fmt::Display for HookEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreToolUse => write!(f, "PreToolUse"),
            Self::PostToolUse => write!(f, "PostToolUse"),
            Self::PostToolUseFailure => write!(f, "PostToolUseFailure"),
            Self::UserPromptSubmit => write!(f, "UserPromptSubmit"),
            Self::Stop => write!(f, "Stop"),
            Self::SubagentStart => write!(f, "SubagentStart"),
            Self::SubagentStop => write!(f, "SubagentStop"),
            Self::SessionStart => write!(f, "SessionStart"),
            Self::SessionEnd => write!(f, "SessionEnd"),
            Self::PreCompact => write!(f, "PreCompact"),
            Self::Setup => write!(f, "Setup"),
            Self::Notification => write!(f, "Notification"),
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p atm-core hook_event`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/atm-core/src/hook.rs
git commit -m "feat(hook): expand HookEventType to all 12 Claude Code events"
```

---

## Phase 2: Expand RawHookEvent Parsing

### Task 2: Add All Fields to RawHookEvent

**Files:**
- Modify: `crates/atm-protocol/src/parse.rs:161-190`
- Test: `crates/atm-protocol/src/parse.rs` (test module)

**Step 1: Write the failing tests**

```rust
// Add to tests module in parse.rs
#[test]
fn test_raw_hook_event_stop() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "Stop",
        "stop_hook_active": true
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::Stop));
    assert_eq!(event.stop_hook_active, Some(true));
}

#[test]
fn test_raw_hook_event_user_prompt() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "UserPromptSubmit",
        "prompt": "Help me write a function"
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::UserPromptSubmit));
    assert_eq!(event.prompt.as_deref(), Some("Help me write a function"));
}

#[test]
fn test_raw_hook_event_subagent_start() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "SubagentStart",
        "agent_id": "agent_456",
        "agent_type": "Explore"
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::SubagentStart));
    assert_eq!(event.agent_id.as_deref(), Some("agent_456"));
    assert_eq!(event.agent_type.as_deref(), Some("Explore"));
}

#[test]
fn test_raw_hook_event_notification() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "Notification",
        "notification_type": "permission_prompt",
        "message": "Allow tool execution?"
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::Notification));
    assert_eq!(event.notification_type.as_deref(), Some("permission_prompt"));
}

#[test]
fn test_raw_hook_event_session_start() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "SessionStart",
        "source": "resume",
        "model": "claude-opus-4-5-20251101"
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::SessionStart));
    assert_eq!(event.source.as_deref(), Some("resume"));
}

#[test]
fn test_raw_hook_event_pre_compact() {
    let json = r#"{
        "session_id": "test-123",
        "hook_event_name": "PreCompact",
        "trigger": "auto"
    }"#;

    let event: RawHookEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), Some(HookEventType::PreCompact));
    assert_eq!(event.trigger.as_deref(), Some("auto"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p atm-protocol raw_hook_event_stop`
Expected: FAIL with "unknown field `stop_hook_active`"

**Step 3: Expand RawHookEvent struct**

Replace the struct in `parse.rs`:

```rust
/// Raw hook event JSON structure from Claude Code.
///
/// Flat structure with all possible fields as Option<T>.
/// Use typed conversion for domain-layer type safety.
#[derive(Debug, Clone, Deserialize)]
pub struct RawHookEvent {
    // === Common Fields (all events) ===
    pub session_id: String,
    pub hook_event_name: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,

    // === Injected by hook script ===
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub tmux_pane: Option<String>,

    // === Tool Events (PreToolUse, PostToolUse, PostToolUseFailure) ===
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_response: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_use_id: Option<String>,

    // === User Prompt (UserPromptSubmit) ===
    #[serde(default)]
    pub prompt: Option<String>,

    // === Stop Events (Stop, SubagentStop) ===
    #[serde(default)]
    pub stop_hook_active: Option<bool>,

    // === Subagent Events (SubagentStart, SubagentStop) ===
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub agent_transcript_path: Option<String>,

    // === Session Events (SessionStart, SessionEnd) ===
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub model: Option<String>,

    // === Compaction/Setup (PreCompact, Setup) ===
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub custom_instructions: Option<String>,

    // === Notification ===
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p atm-protocol raw_hook_event`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/atm-protocol/src/parse.rs
git commit -m "feat(protocol): expand RawHookEvent with all Claude Code hook fields"
```

---

## Phase 3: Add ActivityDetail Struct

### Task 3: Create ActivityDetail Value Object

**Files:**
- Modify: `crates/atm-core/src/session.rs`
- Test: `crates/atm-core/src/session.rs` (test module)

**Step 1: Write the failing test**

```rust
// Add to tests module in session.rs
#[test]
fn test_activity_detail_creation() {
    let detail = ActivityDetail::new("Bash");
    assert_eq!(detail.tool_name.as_deref(), Some("Bash"));
    assert!(detail.started_at <= Utc::now());
    assert!(detail.context.is_none());
}

#[test]
fn test_activity_detail_with_context() {
    let detail = ActivityDetail::with_context("Compacting");
    assert!(detail.tool_name.is_none());
    assert_eq!(detail.context.as_deref(), Some("Compacting"));
}

#[test]
fn test_activity_detail_display() {
    let detail = ActivityDetail::new("Read");
    assert_eq!(detail.display(), "Read");

    let context_detail = ActivityDetail::with_context("Setup");
    assert_eq!(context_detail.display(), "Setup");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p atm-core activity_detail`
Expected: FAIL with "cannot find value `ActivityDetail`"

**Step 3: Implement ActivityDetail**

Add after the `SessionStatus` impl block in `session.rs`:

```rust
// ============================================================================
// Activity Detail
// ============================================================================

/// Detailed information about current session activity.
///
/// Provides structured details alongside the simple SessionStatus enum.
/// This separates "what state are we in" from "what specifically is happening".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityDetail {
    /// Tool name if running/waiting on a tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// When the current activity started
    pub started_at: DateTime<Utc>,
    /// Additional context (e.g., "Compacting", "Setup", "Thinking")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl ActivityDetail {
    /// Creates a new ActivityDetail for a tool operation.
    pub fn new(tool_name: &str) -> Self {
        Self {
            tool_name: Some(tool_name.to_string()),
            started_at: Utc::now(),
            context: None,
        }
    }

    /// Creates an ActivityDetail with context but no specific tool.
    pub fn with_context(context: &str) -> Self {
        Self {
            tool_name: None,
            started_at: Utc::now(),
            context: Some(context.to_string()),
        }
    }

    /// Creates an ActivityDetail for "thinking" state.
    pub fn thinking() -> Self {
        Self::with_context("Thinking")
    }

    /// Returns how long this activity has been running.
    pub fn duration(&self) -> chrono::Duration {
        Utc::now().signed_duration_since(self.started_at)
    }

    /// Returns a display string for this activity.
    pub fn display(&self) -> String {
        if let Some(ref tool) = self.tool_name {
            tool.clone()
        } else if let Some(ref ctx) = self.context {
            ctx.clone()
        } else {
            "Unknown".to_string()
        }
    }
}

impl Default for ActivityDetail {
    fn default() -> Self {
        Self::thinking()
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p atm-core activity_detail`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/atm-core/src/session.rs
git commit -m "feat(session): add ActivityDetail struct for rich activity context"
```

---

## Phase 4: Transition SessionStatus to 3-State Model

### Task 4: Replace SessionStatus Enum

**Files:**
- Modify: `crates/atm-core/src/session.rs:168-262`
- Test: `crates/atm-core/src/session.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn test_new_session_status_variants() {
    // All three states should exist
    let idle = SessionStatus::Idle;
    let working = SessionStatus::Working;
    let attention = SessionStatus::AttentionNeeded;

    assert_eq!(idle.label(), "idle");
    assert_eq!(working.label(), "working");
    assert_eq!(attention.label(), "needs input");
}

#[test]
fn test_session_status_should_blink() {
    assert!(!SessionStatus::Idle.should_blink());
    assert!(!SessionStatus::Working.should_blink());
    assert!(SessionStatus::AttentionNeeded.should_blink());
}

#[test]
fn test_session_status_icons() {
    assert_eq!(SessionStatus::Idle.icon(), "-");
    assert_eq!(SessionStatus::Working.icon(), ">");
    assert_eq!(SessionStatus::AttentionNeeded.icon(), "!");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p atm-core new_session_status`
Expected: FAIL (old enum has different variants)

**Step 3: Replace SessionStatus enum**

Replace the entire `SessionStatus` enum and impl in `session.rs`:

```rust
// ============================================================================
// Session Status (3-State Model)
// ============================================================================

/// Current operational status of a session.
///
/// Three fundamental states based on user action requirements:
/// - **Idle**: Nothing happening - Claude finished, waiting for user
/// - **Working**: Claude is actively processing - user just waits
/// - **AttentionNeeded**: User must act for session to proceed
///
/// Staleness is NOT a status - it's a computed property via `is_stale()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is idle - Claude finished, waiting for user's next action.
    /// User can take their time, no urgency.
    #[default]
    Idle,

    /// Claude is actively processing - user just waits.
    /// Work is happening, no user action needed.
    Working,

    /// User must take action for the session to proceed.
    /// Something is blocked waiting for user input.
    AttentionNeeded,
}

impl SessionStatus {
    /// Returns the display label for this status.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::AttentionNeeded => "needs input",
        }
    }

    /// Returns the ASCII icon for this status.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Idle => "-",
            Self::Working => ">",
            Self::AttentionNeeded => "!",
        }
    }

    /// Returns true if this status should blink in the UI.
    pub fn should_blink(&self) -> bool {
        matches!(self, Self::AttentionNeeded)
    }

    /// Returns true if the session is actively processing.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Working)
    }

    /// Returns true if user action is needed.
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::AttentionNeeded)
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Working => write!(f, "Working"),
            Self::AttentionNeeded => write!(f, "Needs Input"),
        }
    }
}
```

**Step 4: Run tests to verify new tests pass**

Run: `cargo test -p atm-core new_session_status`
Expected: PASS

**Step 5: Commit (will have compile errors - that's expected)**

```bash
git add crates/atm-core/src/session.rs
git commit -m "feat(session): transition SessionStatus to 3-state model (WIP - compile errors expected)"
```

---

### Task 5: Update SessionDomain for New Status Model

**Files:**
- Modify: `crates/atm-core/src/session.rs` (SessionDomain struct and methods)

**Step 1: Add current_activity field to SessionDomain**

In the `SessionDomain` struct, add:

```rust
/// Current activity details (tool name, context, timing)
#[serde(skip_serializing_if = "Option::is_none")]
pub current_activity: Option<ActivityDetail>,
```

**Step 2: Update SessionDomain::new()**

```rust
pub fn new(id: SessionId, agent_type: AgentType, model: Model) -> Self {
    let now = Utc::now();
    Self {
        id,
        agent_type,
        model,
        status: SessionStatus::Idle,  // Changed from Active
        current_activity: None,       // New field
        context: ContextUsage::new(model.context_window_size()),
        cost: Money::zero(),
        duration: SessionDuration::default(),
        lines_changed: LinesChanged::default(),
        started_at: now,
        last_activity: now,
        working_directory: None,
        claude_code_version: None,
        tmux_pane: None,
    }
}
```

**Step 3: Update apply_hook_event() for new model**

```rust
/// Updates status based on a hook event.
pub fn apply_hook_event(&mut self, event_type: HookEventType, tool_name: Option<&str>) {
    self.last_activity = Utc::now();

    match event_type {
        HookEventType::PreToolUse => {
            if let Some(name) = tool_name {
                if is_interactive_tool(name) {
                    self.status = SessionStatus::AttentionNeeded;
                    self.current_activity = Some(ActivityDetail::new(name));
                } else {
                    self.status = SessionStatus::Working;
                    self.current_activity = Some(ActivityDetail::new(name));
                }
            }
        }
        HookEventType::PostToolUse | HookEventType::PostToolUseFailure => {
            self.status = SessionStatus::Working;
            self.current_activity = Some(ActivityDetail::thinking());
        }
        HookEventType::UserPromptSubmit => {
            self.status = SessionStatus::Working;
            self.current_activity = None;
        }
        HookEventType::Stop => {
            self.status = SessionStatus::Idle;
            self.current_activity = None;
        }
        HookEventType::SessionStart => {
            self.status = SessionStatus::Idle;
            self.current_activity = None;
        }
        HookEventType::SessionEnd => {
            // Session will be removed by registry
            self.status = SessionStatus::Idle;
            self.current_activity = None;
        }
        HookEventType::PreCompact => {
            self.status = SessionStatus::Working;
            self.current_activity = Some(ActivityDetail::with_context("Compacting"));
        }
        HookEventType::Setup => {
            self.status = SessionStatus::Working;
            self.current_activity = Some(ActivityDetail::with_context("Setup"));
        }
        HookEventType::Notification => {
            // Notification handling is done separately with notification_type
            // This is a fallback - don't change status
        }
        HookEventType::SubagentStart | HookEventType::SubagentStop => {
            // Subagent tracking deferred to future PR
            self.status = SessionStatus::Working;
        }
    }
}

/// Updates status based on a notification event.
pub fn apply_notification(&mut self, notification_type: Option<&str>) {
    self.last_activity = Utc::now();

    match notification_type {
        Some("permission_prompt") => {
            self.status = SessionStatus::AttentionNeeded;
            self.current_activity = Some(ActivityDetail::with_context("Permission"));
        }
        Some("idle_prompt") => {
            self.status = SessionStatus::Idle;
            self.current_activity = None;
        }
        Some("elicitation_dialog") => {
            self.status = SessionStatus::AttentionNeeded;
            self.current_activity = Some(ActivityDetail::with_context("MCP Input"));
        }
        _ => {
            // Informational notification - no status change
        }
    }
}
```

**Step 4: Update update_from_status_line()**

```rust
pub fn update_from_status_line(/* ... existing params ... */) {
    // ... existing field updates ...

    self.last_activity = Utc::now();

    // Status line update means Claude is working
    // Don't override AttentionNeeded (permission wait)
    if self.status != SessionStatus::AttentionNeeded {
        self.status = SessionStatus::Working;
    }
}
```

**Step 5: Remove set_waiting_for_permission() - no longer needed**

Delete this method as it's replaced by apply_hook_event().

**Step 6: Run cargo check**

Run: `cargo check -p atm-core`
Expected: Should compile (may have warnings)

**Step 7: Commit**

```bash
git add crates/atm-core/src/session.rs
git commit -m "feat(session): update SessionDomain for 3-state model with ActivityDetail"
```

---

### Task 6: Remove DisplayState Enum

**Files:**
- Modify: `crates/atm-core/src/session.rs` (remove DisplayState)
- Modify: `crates/atm-core/src/lib.rs` (remove re-export)

**Step 1: Remove DisplayState from session.rs**

Delete the entire `DisplayState` enum (lines ~264-463) and all its impl blocks.

**Step 2: Update SessionView**

Replace `display_state: DisplayState` field with direct status usage:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionView {
    // ... existing fields ...

    /// Current status
    pub status: SessionStatus,

    /// Status label for display
    pub status_label: String,

    /// Activity detail (tool name or context)
    pub activity_detail: Option<String>,

    /// Whether this status should blink
    pub should_blink: bool,

    /// Status icon
    pub status_icon: String,

    // Remove: pub display_state: DisplayState,
}
```

**Step 3: Update SessionView::from_domain()**

```rust
impl SessionView {
    pub fn from_domain(session: &SessionDomain) -> Self {
        let now = Utc::now();
        let since_activity = now.signed_duration_since(session.last_activity);
        let age = now.signed_duration_since(session.started_at);

        Self {
            id: session.id.clone(),
            id_short: session.id.short().to_string(),
            agent_type: session.agent_type.short_name().to_string(),
            model: session.model.display_name().to_string(),
            status: session.status,
            status_label: session.status.label().to_string(),
            activity_detail: session.current_activity.as_ref().map(|a| a.display()),
            should_blink: session.status.should_blink(),
            status_icon: session.status.icon().to_string(),
            context_percentage: session.context.usage_percentage(),
            context_display: session.context.format(),
            context_warning: session.context.is_warning(),
            context_critical: session.context.is_critical(),
            cost_display: session.cost.format(),
            cost_usd: session.cost.as_usd(),
            duration_display: session.duration.format(),
            duration_seconds: session.duration.total_seconds(),
            lines_display: session.lines_changed.format(),
            working_directory: session.working_directory.clone().map(|p| {
                if p.len() > 30 {
                    format!("...{}", &p[p.len().saturating_sub(27)..])
                } else {
                    p
                }
            }),
            is_stale: session.is_stale(),
            needs_attention: session.status.needs_attention() || session.needs_context_attention(),
            last_activity_display: format_duration(since_activity),
            age_display: format_duration(age),
            started_at: session.started_at.to_rfc3339(),
            last_activity: session.last_activity.to_rfc3339(),
            tmux_pane: session.tmux_pane.clone(),
        }
    }
}
```

**Step 4: Update lib.rs re-exports**

Remove `DisplayState` from the re-exports in `crates/atm-core/src/lib.rs`.

**Step 5: Run cargo check**

Run: `cargo check -p atm-core`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/atm-core/src/session.rs crates/atm-core/src/lib.rs
git commit -m "refactor(session): remove DisplayState, use SessionStatus directly"
```

---

## Phase 5: Update Registry and UI

### Task 7: Update Registry Actor

**Files:**
- Modify: `crates/atmd/src/registry/actor.rs`
- Modify: `crates/atmd/src/registry/commands.rs`

**Step 1: Update ApplyHookEvent command to include notification_type**

In `commands.rs`, update the command:

```rust
ApplyHookEvent {
    session_id: SessionId,
    event_type: HookEventType,
    tool_name: Option<String>,
    notification_type: Option<String>,  // Add this
    pid: Option<u32>,
    tmux_pane: Option<String>,
    respond_to: oneshot::Sender<Result<(), RegistryError>>,
},
```

**Step 2: Update handle_apply_hook_event in actor.rs**

```rust
fn handle_apply_hook_event(
    &mut self,
    session_id: SessionId,
    event_type: HookEventType,
    tool_name: Option<String>,
    notification_type: Option<String>,  // Add this
    pid: Option<u32>,
    tmux_pane: Option<String>,
) -> Result<(), RegistryError> {
    // ... existing session lookup code ...

    // Apply the hook event
    if event_type == HookEventType::Notification {
        session.apply_notification(notification_type.as_deref());
    } else {
        session.apply_hook_event(event_type, tool_name.as_deref());
    }

    // ... rest of method ...
}
```

**Step 3: Run cargo check on atmd**

Run: `cargo check -p atmd`
Expected: Should compile

**Step 4: Commit**

```bash
git add crates/atmd/src/registry/
git commit -m "feat(registry): update hook event handling for new status model"
```

---

### Task 8: Update TUI for New Status Model

**Files:**
- Modify: `crates/atm/src/ui/session_list.rs`
- Modify: `crates/atm/src/ui/detail_panel.rs`

**Step 1: Update session_list.rs status display**

Replace DisplayState references with SessionStatus:

```rust
// In the render function, replace display_state usage:
let status_style = match session.status {
    SessionStatus::Working => Style::default().fg(Color::Green),
    SessionStatus::AttentionNeeded => Style::default().fg(Color::Yellow),
    SessionStatus::Idle => Style::default().fg(Color::Gray),
};

let status_text = if let Some(ref detail) = session.activity_detail {
    format!("{} ({})", session.status_label, detail)
} else {
    session.status_label.clone()
};
```

**Step 2: Update detail_panel.rs**

Update any references to `display_state` to use the new fields.

**Step 3: Run cargo check on atm**

Run: `cargo check -p atm`
Expected: Should compile

**Step 4: Commit**

```bash
git add crates/atm/src/ui/
git commit -m "feat(ui): update TUI for 3-state SessionStatus model"
```

---

### Task 9: Update Server Connection

**Files:**
- Modify: `crates/atmd/src/server/connection.rs`

**Step 1: Update hook event extraction**

Ensure the connection handler extracts `notification_type` from RawHookEvent and passes it to the registry command.

**Step 2: Run cargo check**

Run: `cargo check -p atmd`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/atmd/src/server/connection.rs
git commit -m "feat(server): extract notification_type from hook events"
```

---

## Phase 6: Final Integration

### Task 10: Fix All Tests

**Files:**
- All test files that reference old SessionStatus variants

**Step 1: Run all tests**

Run: `cargo test --workspace`
Expected: Some failures from old status variant references

**Step 2: Update failing tests**

Replace references to old variants:
- `SessionStatus::Active` → `SessionStatus::Working` or `SessionStatus::Idle`
- `SessionStatus::Thinking` → `SessionStatus::Working`
- `SessionStatus::RunningTool { .. }` → `SessionStatus::Working`
- `SessionStatus::WaitingForPermission { .. }` → `SessionStatus::AttentionNeeded`
- `SessionStatus::Stale` → check `is_stale()` instead

**Step 3: Run tests again**

Run: `cargo test --workspace`
Expected: PASS

**Step 4: Commit**

```bash
git add .
git commit -m "test: update all tests for 3-state SessionStatus model"
```

---

### Task 11: Run Clippy and Format

**Step 1: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No errors

**Step 2: Run rustfmt**

Run: `cargo fmt --all`
Expected: Formatted

**Step 3: Commit if changes**

```bash
git add .
git commit -m "chore: clippy and fmt fixes"
```

---

### Task 12: Integration Test

**Step 1: Build release binaries**

Run: `cargo build --release`
Expected: PASS

**Step 2: Manual test with real Claude Code session**

1. Start atmd daemon
2. Start a Claude Code session in tmux
3. Verify session appears in atm TUI
4. Trigger different states (run tool, ask question, stop)
5. Verify status transitions correctly

**Step 3: Final commit**

```bash
git add .
git commit -m "feat: complete SessionStatus transition to 3-state model

- Expanded HookEventType to all 12 Claude Code events
- Added ActivityDetail struct for rich activity context
- Transitioned SessionStatus from 6 to 3 variants (Idle/Working/AttentionNeeded)
- Removed DisplayState (merged into SessionStatus)
- Updated registry, server, and TUI for new model
- Full notification event handling"
```

---

## Summary

| Phase | Tasks | Key Changes |
|-------|-------|-------------|
| 1 | 1 | Expand HookEventType: 5 → 12 variants |
| 2 | 2 | Expand RawHookEvent with all fields |
| 3 | 3 | Add ActivityDetail struct |
| 4 | 4-6 | Transition SessionStatus: 6 → 3 variants, remove DisplayState |
| 5 | 7-9 | Update registry, TUI, server |
| 6 | 10-12 | Tests, clippy, integration test |

**Total: 12 bite-sized tasks**

---

Plan complete and saved to `docs/plans/2026-01-27-session-status-transition-impl.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session in worktree with executing-plans, batch execution with checkpoints

Which approach?
