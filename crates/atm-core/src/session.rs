//! Session domain entities and value objects.

use crate::{AgentType, ContextUsage, HookEventType, Model, Money, TokenCount};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use tracing::debug;

// ============================================================================
// Type-Safe Identifiers
// ============================================================================

/// Unique identifier for a Claude Code session.
///
/// Wraps a UUID string (e.g., "8e11bfb5-7dc2-432b-9206-928fa5c35731").
/// Obtained from Claude Code's status line JSON `session_id` field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

/// Prefix used for pending session IDs (sessions discovered before their real ID is known).
pub const PENDING_SESSION_PREFIX: &str = "pending-";

impl SessionId {
    /// Creates a new SessionId from a string.
    ///
    /// Note: This does not validate UUID format. Claude Code provides
    /// the session_id, so we trust its format.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Creates a pending session ID from a process ID.
    ///
    /// Used when a Claude process is discovered but no transcript exists yet
    /// (e.g., session just started, no conversation has occurred).
    /// The pending session will be upgraded to the real session ID when
    /// it arrives via hook event or status line.
    pub fn pending_from_pid(pid: u32) -> Self {
        Self(format!("{PENDING_SESSION_PREFIX}{pid}"))
    }

    /// Checks if this is a pending session ID (not yet associated with real session).
    pub fn is_pending(&self) -> bool {
        self.0.starts_with(PENDING_SESSION_PREFIX)
    }

    /// Extracts the PID from a pending session ID.
    ///
    /// Returns `None` if this is not a pending session ID or the PID cannot be parsed.
    pub fn pending_pid(&self) -> Option<u32> {
        if !self.is_pending() {
            return None;
        }
        self.0
            .strip_prefix(PENDING_SESSION_PREFIX)
            .and_then(|s| s.parse().ok())
    }

    /// Returns the underlying string reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a shortened display form (first 8 characters).
    ///
    /// Useful for compact TUI display.
    pub fn short(&self) -> &str {
        self.0.get(..8).unwrap_or(&self.0)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Unique identifier for a tool invocation.
///
/// Format: "toolu_..." (e.g., "toolu_01ABC123XYZ")
/// Provided by Claude Code in hook events.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolUseId(String);

impl ToolUseId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolUseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ToolUseId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Path to a session's transcript JSONL file.
///
/// Example: "/home/user/.claude/projects/.../session.jsonl"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TranscriptPath(PathBuf);

impl TranscriptPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns the filename portion of the path.
    pub fn filename(&self) -> Option<&str> {
        self.0.file_name().and_then(|n| n.to_str())
    }
}

impl fmt::Display for TranscriptPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl AsRef<Path> for TranscriptPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

// ============================================================================
// Session Status
// ============================================================================

/// Current operational status of a session.
///
/// Derived from session activity patterns and hook events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is actively processing (received update within 5 seconds)
    Active,

    /// Agent is thinking/generating (between tool calls)
    Thinking,

    /// Agent is executing a tool
    RunningTool {
        /// Name of the tool being executed
        tool_name: String,
        /// When tool execution started
        #[serde(skip_serializing_if = "Option::is_none")]
        started_at: Option<DateTime<Utc>>,
    },

    /// Agent is waiting for user permission to execute a tool
    WaitingForPermission {
        /// Tool awaiting permission
        tool_name: String,
    },

    /// Session is idle (no activity for extended period)
    Idle,

    /// Session is stale (no activity for >8 hours, pending cleanup)
    Stale,
}

impl SessionStatus {
    /// Returns true if the session is in an active state.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Active | Self::Thinking | Self::RunningTool { .. }
        )
    }

    /// Returns true if the session is waiting for user input.
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::WaitingForPermission { .. })
    }

    /// Returns true if the session may be cleaned up.
    pub fn is_removable(&self) -> bool {
        matches!(self, Self::Stale)
    }

    /// Returns a short status label for display.
    pub fn label(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Thinking => "thinking",
            Self::RunningTool { .. } => "running",
            Self::WaitingForPermission { .. } => "waiting",
            Self::Idle => "idle",
            Self::Stale => "stale",
        }
    }

    /// Returns the tool name if applicable.
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::RunningTool { tool_name, .. } => Some(tool_name.as_str()),
            Self::WaitingForPermission { tool_name } => Some(tool_name.as_str()),
            _ => None,
        }
    }
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Thinking => write!(f, "Thinking..."),
            Self::RunningTool { tool_name, .. } => write!(f, "Running: {tool_name}"),
            Self::WaitingForPermission { tool_name } => {
                write!(f, "Permission: {tool_name}")
            }
            Self::Idle => write!(f, "Idle"),
            Self::Stale => write!(f, "Stale"),
        }
    }
}

// ============================================================================
// Display State (UI Layer)
// ============================================================================

/// Visual display state for a session in the TUI.
///
/// Represents the simplified user-facing state of a session, derived from
/// the underlying `SessionStatus` and activity timing. This provides clear
/// visual feedback about whether the user needs to take action.
///
/// ## State Definitions
///
/// - **Working**: Claude is actively processing (recent activity within 5s)
/// - **Compacting**: Working after significant context reduction
/// - **NeedsInput**: User's turn - either permission needed or awaiting next message
/// - **Stale**: No activity for extended period (>8 hours)
///
/// ## Key Insight
///
/// If Claude isn't actively generating or running a tool with recent activity,
/// the session needs user input. Tool execution without updates likely means
/// Claude is waiting for user confirmation ("Do you want to proceed?").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayState {
    /// Actively processing with recent activity
    #[default]
    Working,

    /// Working after context reduction (compaction detected)
    Compacting,

    /// Waiting for user input or permission (blocked, urgent)
    NeedsInput,

    /// Idle - no recent activity but not stale (relaxed, non-urgent)
    Idle,

    /// No activity for extended period (>8 hours)
    Stale,
}

impl DisplayState {
    /// Activity threshold in seconds - below this is considered "recent".
    const ACTIVITY_THRESHOLD_SECS: i64 = 5;

    /// Stale threshold in seconds (8 hours) - matches SessionDomain::is_stale().
    const STALE_THRESHOLD_SECS: i64 = 8 * 3600;

    /// Compaction detection: previous context must have been at least this high.
    const COMPACTION_HIGH_THRESHOLD: f64 = 70.0;

    /// Compaction detection: context must drop by at least this much.
    const COMPACTION_DROP_THRESHOLD: f64 = 20.0;

    /// Returns the display label for this state.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Compacting => "compacting",
            Self::NeedsInput => "needs input",
            Self::Idle => "idle",
            Self::Stale => "stale",
        }
    }

    /// Returns the ASCII icon for this state.
    ///
    /// Uses ASCII characters for terminal compatibility:
    /// - `>` Working (active indicator)
    /// - `~` Compacting (context shrinking)
    /// - `!` NeedsInput (attention needed)
    /// - `-` Idle (relaxed, waiting)
    /// - `z` Stale (sleeping)
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Working => ">",
            Self::Compacting => "~",
            Self::NeedsInput => "!",
            Self::Idle => "-",
            Self::Stale => "z",
        }
    }

    /// Returns a description of this state.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Working => "Session is actively processing",
            Self::Compacting => "Working after context reduction",
            Self::NeedsInput => "Waiting for user input or permission",
            Self::Idle => "Session idle, awaiting user prompt",
            Self::Stale => "No activity for extended period",
        }
    }

    /// Returns true if this state should blink in the UI.
    ///
    /// Only truly urgent/blocked states blink:
    /// - NeedsInput: Blocked, waiting for permission (urgent)
    ///
    /// States that do NOT blink:
    /// - Idle: Relaxed "waiting for user" state
    /// - Stale: Old session, low priority, doesn't need attention
    pub fn should_blink(&self) -> bool {
        matches!(self, Self::NeedsInput)
    }

    /// Determines display state from session data.
    ///
    /// # Algorithm
    ///
    /// 1. Stale check: >8 hours idle → Stale
    /// 2. Permission check: waiting for permission → NeedsInput (urgent, blocked)
    /// 3. Explicit Thinking/RunningTool status → Working (trust these regardless of activity)
    /// 4. Generic Active status with recent activity (<5s) → Working/Compacting
    /// 5. Generic Active status without recent activity → Idle (relaxed, non-urgent)
    /// 6. Otherwise → Idle
    ///
    /// # Arguments
    ///
    /// * `time_since_activity_secs` - Seconds since last activity
    /// * `status` - Current session status
    /// * `context_percentage` - Current context usage percentage
    /// * `previous_context_percentage` - Previous context percentage for compaction detection
    pub fn from_session(
        time_since_activity_secs: i64,
        status: &SessionStatus,
        context_percentage: f64,
        previous_context_percentage: Option<f64>,
    ) -> Self {
        // Priority 1: Stale (no activity for 8+ hours OR explicit Stale status)
        if time_since_activity_secs > Self::STALE_THRESHOLD_SECS {
            return Self::Stale;
        }

        // Priority 2: Explicit Idle/Stale status from domain
        // (handles cases where status is set directly, e.g., during testing)
        match status {
            SessionStatus::Stale => return Self::Stale,
            SessionStatus::Idle => return Self::Idle,
            _ => {}
        }

        // Priority 3: Waiting for permission = needs input
        if status.needs_attention() {
            return Self::NeedsInput;
        }

        // Priority 4: Explicit Thinking/RunningTool status = Working
        // Trust these statuses regardless of activity timeout - they indicate
        // Claude is actively processing (either generating response or running tool)
        match status {
            SessionStatus::Thinking | SessionStatus::RunningTool { .. } => {
                // Check for compaction even during thinking
                if let Some(prev_pct) = previous_context_percentage {
                    let dropped = prev_pct - context_percentage;
                    if prev_pct >= Self::COMPACTION_HIGH_THRESHOLD
                        && dropped >= Self::COMPACTION_DROP_THRESHOLD
                    {
                        return Self::Compacting;
                    }
                }
                return Self::Working;
            }
            _ => {}
        }

        // Check if session has recent activity
        let has_recent_activity = time_since_activity_secs < Self::ACTIVITY_THRESHOLD_SECS;

        // Generic Active status without recent activity = idle, waiting for user
        // (relaxed state, not urgent - user can take their time)
        if !has_recent_activity {
            return Self::Idle;
        }

        // Recent activity + active status = Working (or Compacting)
        if status.is_active() {
            // Check for compaction: context was high and dropped significantly
            if let Some(prev_pct) = previous_context_percentage {
                let dropped = prev_pct - context_percentage;
                if prev_pct >= Self::COMPACTION_HIGH_THRESHOLD
                    && dropped >= Self::COMPACTION_DROP_THRESHOLD
                {
                    return Self::Compacting;
                }
            }
            return Self::Working;
        }

        // Default: idle (waiting for user)
        Self::Idle
    }
}

impl fmt::Display for DisplayState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ============================================================================
// Value Objects
// ============================================================================

/// Duration tracking for a session.
///
/// Based on Claude Code status line `cost.total_duration_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionDuration {
    /// Total duration in milliseconds
    total_ms: u64,
    /// API call duration in milliseconds (time spent waiting for Claude)
    api_ms: u64,
}

impl SessionDuration {
    /// Creates a new SessionDuration.
    pub fn new(total_ms: u64, api_ms: u64) -> Self {
        Self { total_ms, api_ms }
    }

    /// Creates from total duration only.
    pub fn from_total_ms(total_ms: u64) -> Self {
        Self { total_ms, api_ms: 0 }
    }

    /// Returns total duration in milliseconds.
    pub fn total_ms(&self) -> u64 {
        self.total_ms
    }

    /// Returns API duration in milliseconds.
    pub fn api_ms(&self) -> u64 {
        self.api_ms
    }

    /// Returns total duration as seconds (float).
    pub fn total_seconds(&self) -> f64 {
        self.total_ms as f64 / 1000.0
    }

    /// Returns the overhead time (total - API).
    pub fn overhead_ms(&self) -> u64 {
        self.total_ms.saturating_sub(self.api_ms)
    }

    /// Formats duration for display.
    ///
    /// Returns format like "35s", "2m 15s", "1h 30m"
    pub fn format(&self) -> String {
        let secs = self.total_ms / 1000;
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            if remaining_secs == 0 {
                format!("{mins}m")
            } else {
                format!("{mins}m {remaining_secs}s")
            }
        } else {
            let hours = secs / 3600;
            let remaining_mins = (secs % 3600) / 60;
            if remaining_mins == 0 {
                format!("{hours}h")
            } else {
                format!("{hours}h {remaining_mins}m")
            }
        }
    }

    /// Formats duration compactly.
    pub fn format_compact(&self) -> String {
        let secs = self.total_ms / 1000;
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            let mins = secs / 60;
            format!("{mins}m")
        } else {
            let hours = secs / 3600;
            format!("{hours}h")
        }
    }
}

impl fmt::Display for SessionDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Tracks lines added and removed in a session.
///
/// Based on Claude Code status line `cost.total_lines_added/removed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LinesChanged {
    /// Lines added
    pub added: u64,
    /// Lines removed
    pub removed: u64,
}

impl LinesChanged {
    /// Creates new LinesChanged.
    pub fn new(added: u64, removed: u64) -> Self {
        Self { added, removed }
    }

    /// Returns net change (added - removed).
    pub fn net(&self) -> i64 {
        self.added as i64 - self.removed as i64
    }

    /// Returns total churn (added + removed).
    pub fn churn(&self) -> u64 {
        self.added.saturating_add(self.removed)
    }

    /// Returns true if no changes have been made.
    pub fn is_empty(&self) -> bool {
        self.added == 0 && self.removed == 0
    }

    /// Formats for display (e.g., "+150 -30").
    pub fn format(&self) -> String {
        format!("+{} -{}", self.added, self.removed)
    }

    /// Formats net change with sign.
    pub fn format_net(&self) -> String {
        let net = self.net();
        if net >= 0 {
            format!("+{net}")
        } else {
            format!("{net}")
        }
    }
}

impl fmt::Display for LinesChanged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

// ============================================================================
// Domain Entity
// ============================================================================

/// Core domain model for a Claude Code session.
///
/// Contains pure business logic and state. Does NOT include
/// infrastructure concerns (PIDs, sockets, file paths).
///
/// Consistent with CONCURRENCY_MODEL.md RegistryActor ownership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDomain {
    /// Unique session identifier
    pub id: SessionId,

    /// Type of agent (main, subagent, etc.)
    pub agent_type: AgentType,

    /// Claude model being used
    pub model: Model,

    /// Current session status
    pub status: SessionStatus,

    /// Context window usage
    pub context: ContextUsage,

    /// Accumulated cost
    pub cost: Money,

    /// Session duration tracking
    pub duration: SessionDuration,

    /// Lines of code changed
    pub lines_changed: LinesChanged,

    /// When the session started
    pub started_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Working directory (project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// Claude Code version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_version: Option<String>,

    /// Tmux pane ID (e.g., "%5") if session is running in tmux
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_pane: Option<String>,
}

impl SessionDomain {
    /// Creates a new SessionDomain with required fields.
    pub fn new(id: SessionId, agent_type: AgentType, model: Model) -> Self {
        let now = Utc::now();
        Self {
            id,
            agent_type,
            model,
            status: SessionStatus::Active,
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

    /// Creates a SessionDomain from Claude Code status line data.
    #[allow(clippy::too_many_arguments)]
    pub fn from_status_line(
        session_id: &str,
        model_id: &str,
        cost_usd: f64,
        total_duration_ms: u64,
        api_duration_ms: u64,
        lines_added: u64,
        lines_removed: u64,
        total_input_tokens: u64,
        total_output_tokens: u64,
        context_window_size: u32,
        current_input_tokens: u64,
        current_output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
        cwd: Option<&str>,
        version: Option<&str>,
    ) -> Self {
        let model = Model::from_id(model_id);

        let mut session = Self::new(
            SessionId::new(session_id),
            AgentType::GeneralPurpose, // Default, may be updated by hook events
            model,
        );

        session.cost = Money::from_usd(cost_usd);
        session.duration = SessionDuration::new(total_duration_ms, api_duration_ms);
        session.lines_changed = LinesChanged::new(lines_added, lines_removed);
        session.context = ContextUsage {
            total_input_tokens: TokenCount::new(total_input_tokens),
            total_output_tokens: TokenCount::new(total_output_tokens),
            context_window_size,
            current_input_tokens: TokenCount::new(current_input_tokens),
            current_output_tokens: TokenCount::new(current_output_tokens),
            cache_creation_tokens: TokenCount::new(cache_creation_tokens),
            cache_read_tokens: TokenCount::new(cache_read_tokens),
        };
        session.working_directory = cwd.map(|s| s.to_string());
        session.claude_code_version = version.map(|s| s.to_string());
        session.last_activity = Utc::now();

        session
    }

    /// Updates the session with new status line data.
    ///
    /// When `current_usage` is null in Claude's status line, all current_* values
    /// will be 0, which correctly resets context percentage to 0%.
    #[allow(clippy::too_many_arguments)]
    pub fn update_from_status_line(
        &mut self,
        cost_usd: f64,
        total_duration_ms: u64,
        api_duration_ms: u64,
        lines_added: u64,
        lines_removed: u64,
        total_input_tokens: u64,
        total_output_tokens: u64,
        current_input_tokens: u64,
        current_output_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) {
        self.cost = Money::from_usd(cost_usd);
        self.duration = SessionDuration::new(total_duration_ms, api_duration_ms);
        self.lines_changed = LinesChanged::new(lines_added, lines_removed);
        self.context.total_input_tokens = TokenCount::new(total_input_tokens);
        self.context.total_output_tokens = TokenCount::new(total_output_tokens);
        self.context.current_input_tokens = TokenCount::new(current_input_tokens);
        self.context.current_output_tokens = TokenCount::new(current_output_tokens);
        self.context.cache_creation_tokens = TokenCount::new(cache_creation_tokens);
        self.context.cache_read_tokens = TokenCount::new(cache_read_tokens);
        self.last_activity = Utc::now();

        // Update status based on activity
        if !matches!(self.status, SessionStatus::WaitingForPermission { .. }) {
            self.status = SessionStatus::Active;
        }
    }

    /// Updates status based on a hook event.
    pub fn apply_hook_event(&mut self, event_type: HookEventType, tool_name: Option<&str>) {
        self.last_activity = Utc::now();

        match event_type {
            HookEventType::PreToolUse => {
                if let Some(name) = tool_name {
                    self.status = SessionStatus::RunningTool {
                        tool_name: name.to_string(),
                        started_at: Some(Utc::now()),
                    };
                }
            }
            HookEventType::PostToolUse => {
                self.status = SessionStatus::Thinking;
            }
            _ => {}
        }
    }

    /// Marks the session as waiting for permission.
    pub fn set_waiting_for_permission(&mut self, tool_name: &str) {
        self.status = SessionStatus::WaitingForPermission {
            tool_name: tool_name.to_string(),
        };
        self.last_activity = Utc::now();
    }

    /// Returns the session age (time since started).
    pub fn age(&self) -> chrono::Duration {
        Utc::now().signed_duration_since(self.started_at)
    }

    /// Returns time since last activity.
    pub fn time_since_activity(&self) -> chrono::Duration {
        Utc::now().signed_duration_since(self.last_activity)
    }

    /// Returns true if the session should be considered stale.
    ///
    /// A session is stale if no activity for 8 hours.
    pub fn is_stale(&self) -> bool {
        self.time_since_activity() > chrono::Duration::hours(8)
    }

    /// Returns true if context usage needs attention.
    pub fn needs_context_attention(&self) -> bool {
        self.context.is_warning() || self.context.is_critical()
    }
}

impl Default for SessionDomain {
    fn default() -> Self {
        Self::new(
            SessionId::new("unknown"),
            AgentType::default(),
            Model::default(),
        )
    }
}

// ============================================================================
// Infrastructure Entity
// ============================================================================

/// Record of a tool invocation.
#[derive(Debug, Clone)]
pub struct ToolUsageRecord {
    /// Name of the tool (e.g., "Bash", "Read", "Write")
    pub tool_name: String,
    /// Unique ID for this tool invocation
    pub tool_use_id: Option<ToolUseId>,
    /// When the tool was invoked
    pub timestamp: DateTime<Utc>,
}

/// Infrastructure-level data for a session.
///
/// Contains OS/system concerns that don't belong in the domain model.
/// Owned by RegistryActor alongside SessionDomain.
#[derive(Debug, Clone)]
pub struct SessionInfrastructure {
    /// Process ID of the Claude Code process (if known)
    pub pid: Option<u32>,

    /// Process start time in clock ticks (from /proc/{pid}/stat field 22).
    /// Used to detect PID reuse - if the start time changes, it's a different process.
    pub process_start_time: Option<u64>,

    /// Path to the Unix socket for this session (if applicable)
    pub socket_path: Option<PathBuf>,

    /// Path to the transcript JSONL file
    pub transcript_path: Option<TranscriptPath>,

    /// Recent tool usage history (bounded FIFO queue)
    pub recent_tools: VecDeque<ToolUsageRecord>,

    /// Number of status updates received
    pub update_count: u64,

    /// Number of hook events received
    pub hook_event_count: u64,

    /// Last error encountered (for debugging)
    pub last_error: Option<String>,
}

impl SessionInfrastructure {
    /// Maximum number of tool records to keep.
    const MAX_TOOL_HISTORY: usize = 50;

    /// Creates new SessionInfrastructure.
    pub fn new() -> Self {
        Self {
            pid: None,
            process_start_time: None,
            socket_path: None,
            transcript_path: None,
            recent_tools: VecDeque::with_capacity(Self::MAX_TOOL_HISTORY),
            update_count: 0,
            hook_event_count: 0,
            last_error: None,
        }
    }

    /// Sets the process ID and captures the process start time for PID reuse detection.
    ///
    /// The start time is read from `/proc/{pid}/stat` field 22 (starttime in clock ticks).
    /// If the PID is already set with the same value, this is a no-op.
    ///
    /// # Validation
    ///
    /// The PID is only stored if:
    /// - It's non-zero (PID 0 is invalid)
    /// - We can successfully read its start time from `/proc/{pid}/stat`
    ///
    /// This prevents storing invalid PIDs that would cause incorrect liveness checks.
    pub fn set_pid(&mut self, pid: u32) {
        // PID 0 is invalid
        if pid == 0 {
            return;
        }

        // Only update if PID changed or wasn't set
        if self.pid == Some(pid) {
            return;
        }

        // Only store PID if we can read and validate its start time
        // This ensures the PID is valid and gives us PID reuse protection
        if let Some(start_time) = read_process_start_time(pid) {
            self.pid = Some(pid);
            self.process_start_time = Some(start_time);
        } else {
            debug!(
                pid = pid,
                "PID validation failed - process may have exited or is inaccessible"
            );
        }
    }

    /// Checks if the tracked process is still alive.
    ///
    /// Returns `true` if:
    /// - No PID is tracked (can't determine liveness)
    /// - The process exists and has the same start time
    ///
    /// Returns `false` if:
    /// - The process no longer exists
    /// - The PID has been reused by a different process (start time mismatch)
    pub fn is_process_alive(&self) -> bool {
        let Some(pid) = self.pid else {
            // No PID tracked - assume alive (can't determine)
            debug!(pid = ?self.pid, "is_process_alive: no PID tracked, assuming alive");
            return true;
        };

        let Some(expected_start_time) = self.process_start_time else {
            // No start time recorded - just check if process exists via procfs
            let exists = procfs::process::Process::new(pid as i32).is_ok();
            debug!(pid, exists, "is_process_alive: no start_time, checking procfs only");
            return exists;
        };

        // Check if process exists and has same start time
        match read_process_start_time(pid) {
            Some(current_start_time) => {
                let alive = current_start_time == expected_start_time;
                if !alive {
                    debug!(
                        pid,
                        expected_start_time,
                        current_start_time,
                        "is_process_alive: start time MISMATCH - PID reused?"
                    );
                }
                alive
            }
            None => {
                debug!(pid, expected_start_time, "is_process_alive: process NOT FOUND in /proc");
                false
            }
        }
    }

    /// Records a tool usage.
    pub fn record_tool_use(&mut self, tool_name: &str, tool_use_id: Option<ToolUseId>) {
        let record = ToolUsageRecord {
            tool_name: tool_name.to_string(),
            tool_use_id,
            timestamp: Utc::now(),
        };

        self.recent_tools.push_back(record);

        // Maintain bounded size using safe VecDeque operations
        while self.recent_tools.len() > Self::MAX_TOOL_HISTORY {
            self.recent_tools.pop_front();
        }

        self.hook_event_count += 1;
    }

    /// Increments the update count.
    pub fn record_update(&mut self) {
        self.update_count += 1;
    }

    /// Records an error.
    pub fn record_error(&mut self, error: &str) {
        self.last_error = Some(error.to_string());
    }

    /// Returns the most recent tool used.
    pub fn last_tool(&self) -> Option<&ToolUsageRecord> {
        self.recent_tools.back()
    }

    /// Returns recent tools (most recent first).
    pub fn recent_tools_iter(&self) -> impl Iterator<Item = &ToolUsageRecord> {
        self.recent_tools.iter().rev()
    }
}

/// Reads the process start time using the procfs crate.
///
/// The start time (in clock ticks since boot) is stable for the lifetime
/// of a process and unique enough to detect PID reuse.
///
/// Returns `None` if the process doesn't exist or can't be read.
fn read_process_start_time(pid: u32) -> Option<u64> {
    let process = procfs::process::Process::new(pid as i32).ok()?;
    let stat = process.stat().ok()?;
    Some(stat.starttime)
}

impl Default for SessionInfrastructure {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Application Layer DTO
// ============================================================================

/// Read-only view of a session for TUI display.
///
/// Immutable snapshot created from SessionDomain.
/// Implements Clone for easy distribution to multiple UI components.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionView {
    /// Session identifier
    pub id: SessionId,

    /// Short ID for display (first 8 chars)
    pub id_short: String,

    /// Agent type label
    pub agent_type: String,

    /// Model display name
    pub model: String,

    /// Status label
    pub status: String,

    /// Status detail (tool name if applicable)
    pub status_detail: Option<String>,

    /// Context usage percentage
    pub context_percentage: f64,

    /// Context usage formatted string
    pub context_display: String,

    /// Whether context is in warning state
    pub context_warning: bool,

    /// Whether context is in critical state
    pub context_critical: bool,

    /// Cost formatted string
    pub cost_display: String,

    /// Cost in USD (for sorting)
    pub cost_usd: f64,

    /// Duration formatted string
    pub duration_display: String,

    /// Duration in seconds (for sorting)
    pub duration_seconds: f64,

    /// Lines changed formatted string
    pub lines_display: String,

    /// Working directory (shortened for display)
    pub working_directory: Option<String>,

    /// Whether session is stale
    pub is_stale: bool,

    /// Whether session needs attention (permission wait, high context)
    pub needs_attention: bool,

    /// Time since last activity (formatted)
    pub last_activity_display: String,

    /// Session age (formatted)
    pub age_display: String,

    /// Session start time (ISO 8601)
    pub started_at: String,

    /// Last activity time (ISO 8601)
    pub last_activity: String,

    /// Tmux pane ID (e.g., "%5") if session is running in tmux
    pub tmux_pane: Option<String>,

    /// Display state for UI visualization (working/needs_input/stale/compacting)
    pub display_state: DisplayState,
}

impl SessionView {
    /// Creates a SessionView from a SessionDomain.
    pub fn from_domain(session: &SessionDomain) -> Self {
        let now = Utc::now();
        let since_activity = now.signed_duration_since(session.last_activity);
        let age = now.signed_duration_since(session.started_at);

        Self {
            id: session.id.clone(),
            id_short: session.id.short().to_string(),
            agent_type: session.agent_type.short_name().to_string(),
            model: session.model.display_name().to_string(),
            status: session.status.label().to_string(),
            status_detail: session.status.tool_name().map(|s| s.to_string()),
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
                // Shorten path for display
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
            display_state: DisplayState::from_session(
                since_activity.num_seconds(),
                &session.status,
                session.context.usage_percentage(),
                None, // TODO: Track previous context % for compaction detection
            ),
        }
    }
}

impl From<&SessionDomain> for SessionView {
    fn from(session: &SessionDomain) -> Self {
        Self::from_domain(session)
    }
}

/// Formats a duration for human-readable display.
fn format_duration(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 0 {
        return "now".to_string();
    }
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{mins}m ago")
    } else if secs < 86400 {
        let hours = secs / 3600;
        format!("{hours}h ago")
    } else {
        let days = secs / 86400;
        format!("{days}d ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_short() {
        let id = SessionId::new("8e11bfb5-7dc2-432b-9206-928fa5c35731");
        assert_eq!(id.short(), "8e11bfb5");
    }

    #[test]
    fn test_session_id_short_short_id() {
        let id = SessionId::new("abc");
        assert_eq!(id.short(), "abc");
    }

    #[test]
    fn test_session_status_display() {
        let status = SessionStatus::RunningTool {
            tool_name: "Bash".to_string(),
            started_at: None,
        };
        assert_eq!(format!("{}", status), "Running: Bash");
    }

    #[test]
    fn test_session_domain_creation() {
        let session = SessionDomain::new(
            SessionId::new("test-123"),
            AgentType::GeneralPurpose,
            Model::Opus45,
        );
        assert_eq!(session.id.as_str(), "test-123");
        assert_eq!(session.model, Model::Opus45);
        assert!(session.cost.is_zero());
    }

    #[test]
    fn test_session_view_from_domain() {
        let session = SessionDomain::new(
            SessionId::new("8e11bfb5-7dc2-432b-9206-928fa5c35731"),
            AgentType::Explore,
            Model::Sonnet4,
        );
        let view = SessionView::from_domain(&session);

        assert_eq!(view.id_short, "8e11bfb5");
        assert_eq!(view.agent_type, "explore");
        assert_eq!(view.model, "Sonnet 4");
    }

    #[test]
    fn test_lines_changed() {
        let lines = LinesChanged::new(150, 30);
        assert_eq!(lines.net(), 120);
        assert_eq!(lines.churn(), 180);
        assert_eq!(lines.format(), "+150 -30");
        assert_eq!(lines.format_net(), "+120");
    }

    #[test]
    fn test_session_duration_formatting() {
        assert_eq!(SessionDuration::from_total_ms(35_000).format(), "35s");
        assert_eq!(SessionDuration::from_total_ms(135_000).format(), "2m 15s");
        assert_eq!(SessionDuration::from_total_ms(5_400_000).format(), "1h 30m");
    }

    #[test]
    fn test_session_id_pending_from_pid() {
        let id = SessionId::pending_from_pid(12345);
        assert_eq!(id.as_str(), "pending-12345");
        assert!(id.is_pending());
        assert_eq!(id.pending_pid(), Some(12345));
    }

    #[test]
    fn test_session_id_is_pending_true() {
        let id = SessionId::new("pending-99999");
        assert!(id.is_pending());
    }

    #[test]
    fn test_session_id_is_pending_false() {
        let id = SessionId::new("8e11bfb5-7dc2-432b-9206-928fa5c35731");
        assert!(!id.is_pending());
    }

    #[test]
    fn test_session_id_pending_pid_returns_none_for_regular_id() {
        let id = SessionId::new("8e11bfb5-7dc2-432b-9206-928fa5c35731");
        assert_eq!(id.pending_pid(), None);
    }

    #[test]
    fn test_session_id_pending_pid_returns_none_for_invalid_pid() {
        let id = SessionId::new("pending-not-a-number");
        assert_eq!(id.pending_pid(), None);
    }
}
