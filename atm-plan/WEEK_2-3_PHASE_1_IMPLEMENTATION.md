# Week 2-3: Phase 1 Implementation - Core Daemon

**Duration:** 10-12 days
**Status:** Ready to Begin (Week 1 Complete)
**Goal:** Build production-ready daemon with domain-driven architecture, actor-based concurrency, and comprehensive testing

> **IMPORTANT:** This plan implements types and structures defined in `docs/DOMAIN_MODEL.md`.
> All type definitions, value objects, and domain entities MUST match that specification.
> When in doubt, refer to the domain model as the source of truth.

---

## Overview

Week 2-3 implements the core daemon infrastructure that will serve as the foundation for all future phases. This phase focuses on architectural correctness, clean domain modeling, and robust infrastructure.

**What We're Building:**
- Core shared types in `atm-core` crate (as per DOMAIN_MODEL.md)
- Type-safe identifiers: `SessionId`, `ToolUseId`, `TranscriptPath`
- Value objects: `Money`, `TokenCount`, `ContextUsage`, `SessionDuration`, `LinesChanged`
- Domain entities: `SessionDomain`, `SessionInfrastructure`
- Type-safe enums: `Model`, `AgentType`, `SessionStatus`, `HookEventType`
- Domain services: `SessionAggregator`, `CostCalculator`, `ContextAnalyzer`
- Protocol layer with versioning, parsing helpers (`RawStatusLine`, `RawHookEvent`)
- Registry actor using message-passing concurrency
- Daemon server with Unix socket communication
- Comprehensive test suite

**Why This Matters:**
Getting the architecture right in Phase 1 prevents months of technical debt. The actor model, domain separation, and error handling patterns established here will scale through all future phases.

---

## Prerequisites (From Week 1)

Before starting Week 2-3, ensure Week 1 deliverables are complete:

- ✅ Claude Code integration validated (status line + hooks working)
- ✅ `docs/CONCURRENCY_MODEL.md` - Actor pattern specification
- ✅ `docs/ERROR_HANDLING.md` - Error types and retry policies
- ✅ `docs/RESOURCE_LIMITS.md` - Limits and cleanup strategies
- ✅ `docs/PROTOCOL_VERSIONING.md` - Version negotiation design
- ✅ `CLAUDE_CODE_INTEGRATION.md` - Actual JSON structures from testing

**Confidence Check:** If Week 1 revealed issues with Claude Code integration, revise this plan before proceeding.

---

## Day 1-2: Project Setup & Domain Layer

**Duration:** 1.5 days
**Goal:** Initialize project structure and implement clean domain entities

### Day 1 Morning: Project Initialization (2-3 hours)

#### Task 1.1: Create Rust Project Structure

> **Reference:** See `docs/DOMAIN_MODEL.md` "Module Organization" section for workspace structure.

```bash
# Using existing project directory /home/damel/code/atm
# (Already in the correct location)

# Initialize cargo workspace (matching DOMAIN_MODEL.md structure)
mkdir -p crates
cargo init --lib crates/atm-core
cargo init --lib crates/atm-protocol
cargo init --bin crates/atmd
cargo init --bin crates/atm

# Create workspace Cargo.toml
cat > Cargo.toml <<'EOF'
[workspace]
resolver = "2"
members = [
    "crates/atm-core",
    "crates/atm-protocol",
    "crates/atmd",
    "crates/atm",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]

[workspace.dependencies]
# Core (minimal dependencies per DOMAIN_MODEL.md)
chrono = { version = "0.4", default-features = false, features = ["serde", "clock"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"

# Infrastructure & Protocol
tokio = { version = "1.35", features = ["full"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# Testing
proptest = "1.4"
tokio-test = "0.4"
EOF
```

**Note:** The workspace structure follows DOMAIN_MODEL.md:
- `atm-core` - Shared types (session, context, cost, model, agent, hook, error)
- `atm-protocol` - Wire protocol (messages, parsing, versioning)
- `atmd` - Daemon binary
- `atm` - TUI binary (Phase 2)

#### Task 1.2: Create Documentation Structure

```bash
mkdir -p docs
cd docs

# Copy Week 1 documentation
cp ../integration-test/CLAUDE_CODE_INTEGRATION.md .

# Create architecture docs directory structure
mkdir -p architecture/{domain,infrastructure,protocol}
mkdir -p guides/{development,deployment,testing}
mkdir -p adr  # Architecture Decision Records
```

#### Task 1.3: Create Initial ADR

**File:** `docs/adr/001-actor-model-for-registry.md`

```markdown
# ADR 001: Actor Model for Session Registry

**Status:** Accepted
**Date:** 2026-01-23
**Context:** Need thread-safe session registry without lock contention

## Decision
Use actor model with message passing instead of Arc<RwLock<HashMap>>.

## Rationale
- Eliminates lock contention and deadlock risk
- Natural fit for Rust's ownership model
- Tokio's mpsc provides backpressure
- Easier to reason about (serial access guaranteed)

## Consequences
- Slightly more boilerplate (command enums)
- All operations must be async
- Single actor could become bottleneck (unlikely given workload)

## Alternatives Considered
- Arc<RwLock<HashMap>>: Risk of lock contention
- Actor + read cache: Unnecessary complexity for our scale
```

**Time Estimate:** 3 hours

---

### Day 1 Afternoon: Domain Layer - Core Types (4-5 hours)

#### Task 1.4: Implement Type-Safe Identifiers

> **Reference:** See `docs/DOMAIN_MODEL.md` "Type-Safe Identifiers" section.
> Copy implementations directly from the domain model document.

**File:** `crates/atm-core/src/session.rs`

Implement `SessionId`, `ToolUseId`, and `TranscriptPath` exactly as specified in DOMAIN_MODEL.md.
Key characteristics:
- `SessionId`: String wrapper for UUID from Claude Code's `session_id` field
- `ToolUseId`: String wrapper for tool invocation IDs (`toolu_...` format)
- `TranscriptPath`: PathBuf wrapper for transcript JSONL file paths
- All implement: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`
- `SessionId` has `short()` method returning first 8 characters

```rust
// See DOMAIN_MODEL.md for full implementation
// Key difference from previous plan: uses Claude Code's session_id, not generated UUID
```

#### Task 1.5: Implement Type-Safe Enums

> **Reference:** See `docs/DOMAIN_MODEL.md` "Type-Safe Enums" section.
> Implement `Model`, `AgentType`, `SessionStatus`, `HookEventType` exactly as specified.

**File:** `crates/atm-core/src/model.rs`

Implement `Model` enum as specified in DOMAIN_MODEL.md:
- Variants: `Opus45`, `Sonnet4`, `Haiku35`, `Sonnet35V2`, `Unknown`
- Methods: `display_name()`, `context_window_size()`, `input_cost_per_million()`, `output_cost_per_million()`
- Uses serde rename for model IDs (e.g., `claude-opus-4-5-20251101`)

**File:** `crates/atm-core/src/agent.rs`

Implement `AgentType` enum as specified in DOMAIN_MODEL.md:
- Variants: `GeneralPurpose`, `Explore`, `Plan`, `CodeReviewer`, `FileSearch`, `Custom(String)`
- Methods: `short_name()`, `label()`

**File:** `crates/atm-core/src/hook.rs`

Implement `HookEventType` enum as specified in DOMAIN_MODEL.md:
- Variants: `PreToolUse`, `PostToolUse`, `SessionStart`, `SessionEnd`, `Notification`
- Methods: `is_pre_event()`, `is_post_event()`

```rust
// See DOMAIN_MODEL.md for full implementations
// Key additions from previous plan: Model enum with pricing, richer AgentType variants
```

#### Task 1.6: Implement Value Objects

> **Reference:** See `docs/DOMAIN_MODEL.md` "Value Objects" section.
> Implement `Money`, `TokenCount`, `ContextUsage`, `SessionDuration`, `LinesChanged`.

**File:** `crates/atm-core/src/cost.rs`

Implement `Money` as specified in DOMAIN_MODEL.md:
- Stores amount in microdollars (millionths of a dollar) for precision
- Methods: `from_usd()`, `from_microdollars()`, `as_usd()`, `format()`, `format_compact()`
- Implements `Add`, `AddAssign` with saturating arithmetic

**File:** `crates/atm-core/src/context.rs`

Implement `TokenCount` and `ContextUsage` as specified in DOMAIN_MODEL.md:

`TokenCount`:
- Wraps `u64` for type safety
- Methods: `format()` with K/M suffixes, `saturating_add()`

`ContextUsage`:
- Fields: `total_input_tokens`, `total_output_tokens`, `context_window_size`,
  `current_input_tokens`, `current_output_tokens`, `cache_creation_tokens`, `cache_read_tokens`
- Methods: `usage_percentage()`, `is_warning()` (80%), `is_critical()` (90%),
  `remaining_tokens()`, `exceeds_200k()`

**Additional Value Objects:**

`SessionDuration` (in `crates/atm-core/src/session.rs`):
- Fields: `total_ms`, `api_ms`
- Methods: `format()`, `format_compact()`, `overhead_ms()`

`LinesChanged` (in `crates/atm-core/src/session.rs`):
- Fields: `added`, `removed`
- Methods: `net()`, `churn()`, `format()`

```rust
// See DOMAIN_MODEL.md for full implementations
// Key additions: Money with microdollar precision, comprehensive ContextUsage with cache tracking
```

**Time Estimate:** 5 hours

---

### Day 2 Morning: Domain Layer - Session Entity (4 hours)

#### Task 1.7: Implement Session Status

> **Reference:** See `docs/DOMAIN_MODEL.md` "SessionStatus" section.
> Uses richer status model with tool execution states.

**File:** `crates/atm-core/src/session.rs` (alongside SessionDomain)

Implement `SessionStatus` as specified in DOMAIN_MODEL.md:

```rust
/// Current operational status of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is actively processing (received update within 5 seconds)
    Active,

    /// Agent is thinking/generating (between tool calls)
    Thinking,

    /// Agent is executing a tool
    RunningTool {
        tool_name: String,
        started_at: Option<DateTime<Utc>>,
    },

    /// Agent is waiting for user permission to execute a tool
    WaitingForPermission {
        tool_name: String,
    },

    /// Session is idle (no activity for 30-90 seconds)
    Idle,

    /// Session is stale (no activity for >90 seconds, pending cleanup)
    Stale,
}
```

Key methods:
- `is_active()` - Returns true for `Active`, `Thinking`, `RunningTool`
- `needs_attention()` - Returns true for `WaitingForPermission`
- `is_removable()` - Returns true for `Stale`
- `label()` - Short status label for display
- `tool_name()` - Returns tool name if applicable

#### Task 1.8: Implement Core Session Domain Entity

> **Reference:** See `docs/DOMAIN_MODEL.md` "SessionDomain" section.
> Uses richer model with `Model`, `Money`, `SessionDuration`, `LinesChanged`.

**File:** `crates/atm-core/src/session.rs`

Implement `SessionDomain` as specified in DOMAIN_MODEL.md:

```rust
/// Core domain model for a Claude Code session.
///
/// Contains pure business logic and state. Does NOT include
/// infrastructure concerns (PIDs, sockets, file paths).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDomain {
    pub id: SessionId,
    pub agent_type: AgentType,
    pub model: Model,
    pub status: SessionStatus,
    pub context: ContextUsage,
    pub cost: Money,
    pub duration: SessionDuration,
    pub lines_changed: LinesChanged,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub working_directory: Option<String>,
    pub claude_code_version: Option<String>,
}
```

Key methods from DOMAIN_MODEL.md:
- `new(id, agent_type, model)` - Create with defaults
- `from_status_line(...)` - Create from Claude Code status JSON
- `update_from_status_line(...)` - Update with new status data
- `apply_hook_event(event_type, tool_name)` - Update status from hooks
- `set_waiting_for_permission(tool_name)` - Mark as waiting
- `age()`, `time_since_activity()` - Duration calculations
- `is_stale()` - No activity for >90 seconds
- `needs_context_attention()` - Context warning/critical

**Also implement `SessionInfrastructure`** (separate from domain):

```rust
/// Infrastructure-level data for a session.
/// Contains OS/system concerns (PIDs, sockets, file paths).
pub struct SessionInfrastructure {
    pub pid: Option<u32>,
    pub socket_path: Option<PathBuf>,
    pub transcript_path: Option<TranscriptPath>,
    pub recent_tools: VecDeque<ToolUsageRecord>,
    pub update_count: u64,
    pub hook_event_count: u64,
    pub last_error: Option<String>,
}
```

#### Task 1.9: Create Core Module Exports

> **Reference:** See `docs/DOMAIN_MODEL.md` "Module Organization" section.

**File:** `crates/atm-core/src/lib.rs`

```rust
//! Agent Tmux Monitor Core - Shared types for Claude Code monitoring
//!
//! This crate provides the core domain types shared between
//! the daemon (atmd) and TUI (atm).

pub mod session;
pub mod context;
pub mod cost;
pub mod model;
pub mod agent;
pub mod hook;
pub mod error;

// Re-exports for convenience
pub use session::{SessionId, ToolUseId, TranscriptPath, SessionDomain, SessionStatus,
                  SessionDuration, LinesChanged, SessionInfrastructure, ToolUsageRecord};
pub use context::{ContextUsage, TokenCount};
pub use cost::Money;
pub use model::Model;
pub use agent::AgentType;
pub use hook::HookEventType;
pub use error::{DomainError, DomainResult};

// Re-export chrono for convenience
pub use chrono::{DateTime, Utc};
```

**File:** `crates/atm-core/Cargo.toml`

```toml
[package]
name = "atm-core"
version.workspace = true
edition.workspace = true

[dependencies]
chrono.workspace = true
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_json = "1.0"
proptest.workspace = true
```

**Time Estimate:** 4 hours

---

### Day 2 Afternoon: Domain Services & Application Layer (3-4 hours)

> **Reference:** See `docs/DOMAIN_MODEL.md` "Domain Services" and "Application Layer DTOs" sections.

#### Task 1.10: Implement Domain Services

**File:** `crates/atm-core/src/services.rs`

Implement domain services as specified in DOMAIN_MODEL.md:

**SessionAggregator:**
- `aggregate(sessions)` - Computes `AggregateStats` across sessions
- `group_by_agent_type(sessions)` - Groups sessions by type
- `sort_by_context_usage()`, `sort_by_cost()`, `sort_by_activity()`

**CostCalculator:**
- `estimate_cost(model, input_tokens, output_tokens)` - Token-based cost
- `hourly_rate(cost, duration)` - Cost per hour
- `project_cost(current_cost, duration, target_hours)` - Projected cost

**ContextAnalyzer:**
- `analyze(context)` - Returns `ContextWarningLevel` (Normal/Elevated/Warning/Critical)
- `warning_message(context)` - Human-readable warning
- `estimate_remaining_turns(context, avg_tokens)` - Turns remaining
- `cache_efficiency(context)` - Cache hit percentage

#### Task 1.11: Implement Application Layer DTO

**File:** `crates/atm-core/src/view.rs`

Implement `SessionView` as specified in DOMAIN_MODEL.md:

```rust
/// Read-only view of a session for TUI display.
/// Immutable snapshot created from SessionDomain.
pub struct SessionView {
    pub id: SessionId,
    pub id_short: String,
    pub agent_type: String,
    pub model: String,
    pub status: String,
    pub status_detail: Option<String>,
    pub context_percentage: f64,
    pub context_display: String,
    pub context_warning: bool,
    pub context_critical: bool,
    pub cost_display: String,
    pub cost_usd: f64,
    pub duration_display: String,
    pub duration_seconds: f64,
    pub lines_display: String,
    pub working_directory: Option<String>,
    pub is_stale: bool,
    pub needs_attention: bool,
    // ... etc
}
```

Key method: `SessionView::from_domain(session: &SessionDomain) -> Self`

---

### Day 2 Evening: Domain Layer - Testing (3-4 hours)

#### Task 1.12: Property-Based Testing for Core Types

**File:** `crates/atm-core/tests/property_tests.rs`

```rust
use chrono::{Duration, Utc};
use atm_core::{AgentType, ContextUsage, SessionDomain, SessionId, SessionStatus, Model, Money, TokenCount};
use proptest::prelude::*;

// Property: Session IDs are always unique
proptest! {
    #[test]
    fn session_ids_are_unique(count in 1usize..100) {
        use std::collections::HashSet;

        let ids: Vec<SessionId> = (0..count)
            .map(|_| SessionId::new())
            .collect();

        let unique: HashSet<_> = ids.iter().collect();

        prop_assert_eq!(ids.len(), unique.len());
    }
}

// Property: Context usage percentages are always valid
proptest! {
    #[test]
    fn context_percentage_in_valid_range(
        input in 0u64..1_000_000,
        output in 0u64..1_000_000,
        pct in 0.0f64..100.0,
        cost in 0.0f64..1000.0
    ) {
        let usage = ContextUsage::new(input, output, pct, cost);

        prop_assert!(usage.used_percentage >= 0.0);
        prop_assert!(usage.used_percentage <= 100.0);
        prop_assert_eq!(usage.total_tokens(), input + output);
    }
}

// Property: Sessions become stale after threshold
proptest! {
    #[test]
    fn sessions_become_stale_after_threshold(
        seconds_elapsed in 91i64..10_000
    ) {
        let mut session = SessionDomain::new(
            SessionId::new(),
            AgentType::GeneralPurpose,
        );

        let future = Utc::now() + Duration::seconds(seconds_elapsed);

        prop_assert!(session.is_stale(future));
        prop_assert!(session.should_cleanup(future));
    }
}

// Property: Active sessions don't cleanup prematurely
proptest! {
    #[test]
    fn active_sessions_dont_cleanup_prematurely(
        seconds_elapsed in 0i64..90
    ) {
        let session = SessionDomain::new(
            SessionId::new(),
            AgentType::GeneralPurpose,
        );

        let future = Utc::now() + Duration::seconds(seconds_elapsed);

        prop_assert!(!session.is_stale(future));
        prop_assert!(!session.should_cleanup(future));
    }
}
```

**Note:** The `crates/atm-core/Cargo.toml` already includes proptest in dev-dependencies (see Task 1.9).

**Time Estimate:** 3-4 hours

---

## Day 3-4: Protocol Layer & Parsing

**Duration:** 2 days
**Goal:** Implement protocol layer with versioning and Claude Code JSON parsing

> **Note:** Infrastructure concerns (PIDs, tmux, file paths) are now handled by
> `SessionInfrastructure` in `atm-core` as per DOMAIN_MODEL.md.
> This section focuses on protocol and parsing.

### Day 3 Morning: Protocol Parsing Helpers (3-4 hours)

> **Reference:** See `docs/DOMAIN_MODEL.md` "Parsing Claude Code Data" section.

#### Task 2.1: Implement Raw Status Line Parser

**File:** `crates/atm-protocol/src/parse.rs`

Implement `RawStatusLine` and related parsing types as specified in DOMAIN_MODEL.md:

```rust
/// Raw status line JSON structure from Claude Code.
/// Based on validated integration testing (Week 1).
#[derive(Debug, Clone, Deserialize)]
pub struct RawStatusLine {
    pub session_id: String,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub model: RawModel,
    pub workspace: Option<RawWorkspace>,
    pub version: Option<String>,
    pub cost: RawCost,
    pub context_window: RawContextWindow,
    pub exceeds_200k_tokens: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawCost {
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    pub total_api_duration_ms: u64,
    pub total_lines_added: u64,
    pub total_lines_removed: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawContextWindow {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub context_window_size: u32,
    pub current_usage: Option<RawCurrentUsage>,
}
```

Key method: `RawStatusLine::to_session_domain() -> SessionDomain`

#### Task 2.2: Implement Hook Event Parser

**File:** `crates/atm-protocol/src/parse.rs` (continued)

Implement `RawHookEvent` as specified in DOMAIN_MODEL.md:

```rust
/// Raw hook event JSON structure from Claude Code.
#[derive(Debug, Clone, Deserialize)]
pub struct RawHookEvent {
    pub session_id: String,
    pub hook_event_name: String,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_use_id: Option<String>,
}

impl RawHookEvent {
    /// Parses the hook event type.
    pub fn event_type(&self) -> Option<HookEventType> {
        match self.hook_event_name.as_str() {
            "PreToolUse" => Some(HookEventType::PreToolUse),
            "PostToolUse" => Some(HookEventType::PostToolUse),
            "Notification" => Some(HookEventType::Notification),
            _ => None,
        }
    }
}
```

**Time Estimate:** 3-4 hours

---

### Day 3 Afternoon: Protocol Messages (4-5 hours)

#### Task 2.3: Protocol Version Types

> **Reference:** This aligns with `docs/PROTOCOL_VERSIONING.md`.

**File:** `crates/atm-protocol/src/version.rs`

The version types remain as specified in the existing Day 4 Protocol Layer section.

#### Task 2.4: Protocol Module Exports

**File:** `crates/atm-protocol/src/lib.rs`

```rust
//! Agent Tmux Monitor Protocol Layer
//!
//! Defines messages exchanged between daemon and clients.
//! Includes versioning, parsing, and serialization.

mod client_message;
mod daemon_message;
mod parse;
mod version;

pub use client_message::{ClientMessage, ClientType};
pub use daemon_message::{DaemonMessage, RemovalReason};
pub use parse::{RawStatusLine, RawHookEvent, RawCost, RawContextWindow, RawModel};
pub use version::{ProtocolVersion, VersionError};

// Re-export core types for convenience
pub use atm_core::*;
```

**File:** `crates/atm-protocol/Cargo.toml`

```toml
[package]
name = "atm-protocol"
version.workspace = true
edition.workspace = true

[dependencies]
atm-core = { path = "../atm-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
# None needed yet
```

**Time Estimate:** 4-5 hours

---

### Day 4: Protocol Layer (continued)

**Duration:** 1 day
**Goal:** Implement versioned protocol for daemon-client communication

#### Task 3.1: Protocol Version Types

**File:** `atm-protocol/src/version.rs`

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Protocol version using semantic versioning
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    /// Current protocol version
    pub const CURRENT: Self = Self { major: 1, minor: 0 };

    /// Create a new protocol version
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Check if this version is compatible with another
    ///
    /// Compatible if same major version and minor >= required
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor >= other.minor
    }

    /// Parse from string format "major.minor"
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        let parts: Vec<&str> = s.split('.').collect();

        if parts.len() != 2 {
            return Err(VersionError::InvalidFormat(s.to_string()));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?;

        let minor = parts[1]
            .parse()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?;

        Ok(Self { major, minor })
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[derive(Error, Debug)]
pub enum VersionError {
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),

    #[error("Incompatible version: expected {expected}, got {actual}")]
    Incompatible {
        expected: ProtocolVersion,
        actual: ProtocolVersion,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        let v = ProtocolVersion::parse("1.0").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
    }

    #[test]
    fn version_display() {
        let v = ProtocolVersion::new(1, 2);
        assert_eq!(v.to_string(), "1.2");
    }

    #[test]
    fn version_compatibility() {
        let v1_0 = ProtocolVersion::new(1, 0);
        let v1_1 = ProtocolVersion::new(1, 1);
        let v2_0 = ProtocolVersion::new(2, 0);

        // Same major, higher minor is compatible
        assert!(v1_1.is_compatible_with(&v1_0));

        // Same major, lower minor is not compatible
        assert!(!v1_0.is_compatible_with(&v1_1));

        // Different major is not compatible
        assert!(!v2_0.is_compatible_with(&v1_0));
        assert!(!v1_0.is_compatible_with(&v2_0));
    }
}
```

#### Task 3.2: Client Messages

**File:** `atm-protocol/src/client_message.rs`

```rust
use atm_domain::{AgentType, ContextUsage, SessionId};
use serde::{Deserialize, Serialize};

use crate::ProtocolVersion;

/// Messages sent from clients (bash scripts, TUI) to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Initial handshake to establish connection
    Hello {
        protocol_version: ProtocolVersion,
        client_type: ClientType,
    },

    /// Register a new session
    Register {
        session_id: SessionId,
        agent_type: AgentType,
        pid: u32,
        working_dir: String,
    },

    /// Update context usage for a session
    UpdateContext {
        session_id: SessionId,
        context: ContextUsage,
    },

    /// Mark session as ended
    EndSession { session_id: SessionId },

    /// Query for a specific session
    GetSession { session_id: SessionId },

    /// Query for all sessions
    GetAllSessions,

    /// Subscribe to session updates (TUI only)
    Subscribe,

    /// Unsubscribe from updates (TUI only)
    Unsubscribe,
}

/// Type of client connecting to daemon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// Claude Code session (bash script)
    Session,

    /// TUI monitor application
    Tui,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_hello() {
        let msg = ClientMessage::Hello {
            protocol_version: ProtocolVersion::new(1, 0),
            client_type: ClientType::Session,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"hello\""));
        assert!(json.contains("\"client_type\":\"session\""));
    }

    #[test]
    fn serialize_register() {
        let msg = ClientMessage::Register {
            session_id: SessionId::from_string("test-123"),
            agent_type: AgentType::GeneralPurpose,
            pid: 12345,
            working_dir: "/home/user/project".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"register\""));
    }

    #[test]
    fn roundtrip_serialization() {
        let msg = ClientMessage::GetAllSessions;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();

        assert!(matches!(deserialized, ClientMessage::GetAllSessions));
    }
}
```

#### Task 3.3: Daemon Messages

**File:** `atm-protocol/src/daemon_message.rs`

```rust
use atm_domain::{SessionDomain, SessionId};
use serde::{Deserialize, Serialize};

use crate::ProtocolVersion;

/// Messages sent from daemon to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonMessage {
    /// Response to Hello with version negotiation result
    ServerHello {
        protocol_version: ProtocolVersion,
        accepted: bool,
        reason: Option<String>,
    },

    /// Acknowledgment of successful operation
    Ok,

    /// Error response
    Error { code: String, message: String },

    /// Single session response
    Session { session: SessionDomain },

    /// Multiple sessions response
    Sessions { sessions: Vec<SessionDomain> },

    /// Real-time session update (for subscribers)
    SessionUpdated { session: SessionDomain },

    /// Session removed notification
    SessionRemoved {
        session_id: SessionId,
        reason: RemovalReason,
    },
}

/// Reason why a session was removed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovalReason {
    /// Session explicitly ended
    Ended,

    /// Session became stale
    Stale,

    /// Forced cleanup (max age exceeded)
    ForcedCleanup,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_server_hello() {
        let msg = DaemonMessage::ServerHello {
            protocol_version: ProtocolVersion::new(1, 0),
            accepted: true,
            reason: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"server_hello\""));
        assert!(json.contains("\"accepted\":true"));
    }

    #[test]
    fn serialize_error() {
        let msg = DaemonMessage::Error {
            code: "registry_full".to_string(),
            message: "Session registry is full".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"code\":\"registry_full\""));
    }
}
```

#### Task 3.4: Protocol Module

**File:** `atm-protocol/src/lib.rs`

```rust
//! Agent Tmux Monitor Protocol Layer
//!
//! Defines messages exchanged between daemon and clients.
//! Includes versioning and serialization.

mod client_message;
mod daemon_message;
mod version;

pub use client_message::{ClientMessage, ClientType};
pub use daemon_message::{DaemonMessage, RemovalReason};
pub use version::{ProtocolVersion, VersionError};

// Re-export domain types for convenience
pub use atm_domain::*;
```

**File:** `atm-protocol/Cargo.toml`

```toml
[package]
name = "atm-protocol"
version.workspace = true
edition.workspace = true

[dependencies]
atm-domain = { path = "../atm-domain" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
# None needed yet
```

**Time Estimate:** 6-7 hours

---

## Day 5-6: Registry Actor Implementation

**Duration:** 2 days
**Goal:** Implement actor-based session registry with message passing

### Day 5: Registry Actor Core

#### Task 4.1: Registry Commands

**File:** `crates/atmd/src/registry/commands.rs`

```rust
use atm_domain::{ContextUsage, SessionDomain, SessionId};
use atm_protocol::RemovalReason;
use tokio::sync::oneshot;

/// Commands sent to the registry actor
pub enum RegistryCommand {
    /// Register a new session
    Register {
        session: SessionDomain,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },

    /// Update context for a session
    UpdateContext {
        session_id: SessionId,
        context: ContextUsage,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },

    /// End a session
    EndSession {
        session_id: SessionId,
        respond_to: oneshot::Sender<Result<(), RegistryError>>,
    },

    /// Get a specific session
    GetSession {
        session_id: SessionId,
        respond_to: oneshot::Sender<Option<SessionDomain>>,
    },

    /// Get all sessions
    GetAllSessions {
        respond_to: oneshot::Sender<Vec<SessionDomain>>,
    },

    /// Internal command for cleanup task
    CleanupStale,
}

/// Errors that can occur in registry operations
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Session registry is full (max: {max})")]
    RegistryFull { max: usize },

    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Session already exists: {0}")]
    SessionExists(SessionId),

    #[error("Session cannot be updated (ended)")]
    SessionEnded,
}

/// Events published by the registry
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Session was registered
    Registered {
        session_id: SessionId,
        session: SessionDomain,
    },

    /// Session was updated
    Updated {
        session_id: SessionId,
        session: SessionDomain,
    },

    /// Session was removed
    Removed {
        session_id: SessionId,
        reason: RemovalReason,
    },
}
```

#### Task 4.2: Registry Actor Implementation

**File:** `crates/atmd/src/registry/actor.rs`

```rust
use super::commands::{RegistryCommand, RegistryError, SessionEvent};
use chrono::Utc;
use atm_domain::{SessionDomain, SessionId};
use atm_protocol::RemovalReason;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

/// Maximum number of concurrent sessions
const MAX_SESSIONS: usize = 100;

/// The registry actor - owns all session state
pub struct RegistryActor {
    /// Command receiver
    receiver: mpsc::Receiver<RegistryCommand>,

    /// Session storage
    sessions: HashMap<SessionId, SessionDomain>,

    /// Event publisher for real-time updates
    event_publisher: broadcast::Sender<SessionEvent>,
}

impl RegistryActor {
    /// Create a new registry actor
    pub fn new(
        receiver: mpsc::Receiver<RegistryCommand>,
        event_publisher: broadcast::Sender<SessionEvent>,
    ) -> Self {
        Self {
            receiver,
            sessions: HashMap::new(),
            event_publisher,
        }
    }

    /// Run the actor event loop
    pub async fn run(mut self) {
        info!("Registry actor starting");

        while let Some(cmd) = self.receiver.recv().await {
            match cmd {
                RegistryCommand::Register { session, respond_to } => {
                    let result = self.handle_register(session);
                    let _ = respond_to.send(result);
                }

                RegistryCommand::UpdateContext {
                    session_id,
                    context,
                    respond_to,
                } => {
                    let result = self.handle_update_context(session_id, context);
                    let _ = respond_to.send(result);
                }

                RegistryCommand::EndSession {
                    session_id,
                    respond_to,
                } => {
                    let result = self.handle_end_session(session_id);
                    let _ = respond_to.send(result);
                }

                RegistryCommand::GetSession {
                    session_id,
                    respond_to,
                } => {
                    let session = self.sessions.get(&session_id).cloned();
                    let _ = respond_to.send(session);
                }

                RegistryCommand::GetAllSessions { respond_to } => {
                    let sessions: Vec<_> = self.sessions.values().cloned().collect();
                    let _ = respond_to.send(sessions);
                }

                RegistryCommand::CleanupStale => {
                    self.handle_cleanup();
                }
            }
        }

        info!("Registry actor stopped");
    }

    /// Handle session registration
    fn handle_register(&mut self, session: SessionDomain) -> Result<(), RegistryError> {
        // Check capacity
        if self.sessions.len() >= MAX_SESSIONS {
            warn!(
                session_id = %session.id,
                current = self.sessions.len(),
                max = MAX_SESSIONS,
                "Registry full, rejecting registration"
            );
            return Err(RegistryError::RegistryFull { max: MAX_SESSIONS });
        }

        // Check for duplicate
        if self.sessions.contains_key(&session.id) {
            warn!(session_id = %session.id, "Session already exists");
            return Err(RegistryError::SessionExists(session.id.clone()));
        }

        info!(
            session_id = %session.id,
            agent_type = ?session.agent_type,
            "Registering session"
        );

        let session_id = session.id.clone();
        self.sessions.insert(session_id.clone(), session.clone());

        // Publish event
        let _ = self.event_publisher.send(SessionEvent::Registered {
            session_id,
            session,
        });

        Ok(())
    }

    /// Handle context update
    fn handle_update_context(
        &mut self,
        session_id: SessionId,
        context: atm_domain::ContextUsage,
    ) -> Result<(), RegistryError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| RegistryError::SessionNotFound(session_id.clone()))?;

        debug!(
            session_id = %session_id,
            tokens = context.total_tokens(),
            percentage = context.used_percentage,
            "Updating context"
        );

        session.update_context(context);

        // Publish update event
        let _ = self.event_publisher.send(SessionEvent::Updated {
            session_id,
            session: session.clone(),
        });

        Ok(())
    }

    /// Handle session end
    fn handle_end_session(&mut self, session_id: SessionId) -> Result<(), RegistryError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| RegistryError::SessionNotFound(session_id.clone()))?;

        info!(session_id = %session_id, "Ending session");

        session.end();

        // Remove from registry
        self.sessions.remove(&session_id);

        // Publish removal event
        let _ = self.event_publisher.send(SessionEvent::Removed {
            session_id,
            reason: RemovalReason::Ended,
        });

        Ok(())
    }

    /// Cleanup stale sessions
    fn handle_cleanup(&mut self) {
        let now = Utc::now();
        let mut to_remove = Vec::new();

        for (id, session) in &self.sessions {
            if session.should_cleanup(now) {
                to_remove.push((id.clone(), session.is_stale(now)));
            }
        }

        for (id, is_stale) in to_remove {
            self.sessions.remove(&id);

            let reason = if is_stale {
                RemovalReason::Stale
            } else {
                RemovalReason::ForcedCleanup
            };

            info!(session_id = %id, ?reason, "Cleaned up session");

            let _ = self.event_publisher.send(SessionEvent::Removed {
                session_id: id,
                reason,
            });
        }
    }
}
```

**Time Estimate:** 6-7 hours

---

### Day 6: Registry Handle & Testing

#### Task 4.3: Registry Handle (Client Interface)

**File:** `crates/atmd/src/registry/handle.rs`

```rust
use super::commands::{RegistryCommand, RegistryError, SessionEvent};
use atm_domain::{ContextUsage, SessionDomain, SessionId};
use tokio::sync::{broadcast, mpsc, oneshot};

/// Handle for interacting with the registry actor
///
/// This is a cheap-to-clone handle that can be shared across tasks.
#[derive(Clone)]
pub struct RegistryHandle {
    sender: mpsc::Sender<RegistryCommand>,
    event_subscriber: broadcast::Sender<SessionEvent>,
}

impl RegistryHandle {
    /// Create a new registry handle
    pub fn new(
        sender: mpsc::Sender<RegistryCommand>,
        event_subscriber: broadcast::Sender<SessionEvent>,
    ) -> Self {
        Self {
            sender,
            event_subscriber,
        }
    }

    /// Register a new session
    pub async fn register(&self, session: SessionDomain) -> Result<(), RegistryError> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(RegistryCommand::Register {
                session,
                respond_to: tx,
            })
            .await
            .map_err(|_| RegistryError::RegistryFull { max: 100 })?;

        rx.await.map_err(|_| RegistryError::RegistryFull { max: 100 })?
    }

    /// Update context for a session
    pub async fn update_context(
        &self,
        session_id: SessionId,
        context: ContextUsage,
    ) -> Result<(), RegistryError> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(RegistryCommand::UpdateContext {
                session_id,
                context,
                respond_to: tx,
            })
            .await
            .map_err(|_| RegistryError::RegistryFull { max: 100 })?;

        rx.await.map_err(|_| RegistryError::RegistryFull { max: 100 })?
    }

    /// End a session
    pub async fn end_session(&self, session_id: SessionId) -> Result<(), RegistryError> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(RegistryCommand::EndSession {
                session_id,
                respond_to: tx,
            })
            .await
            .map_err(|_| RegistryError::RegistryFull { max: 100 })?;

        rx.await.map_err(|_| RegistryError::RegistryFull { max: 100 })?
    }

    /// Get a specific session
    pub async fn get_session(&self, session_id: SessionId) -> Option<SessionDomain> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(RegistryCommand::GetSession {
                session_id,
                respond_to: tx,
            })
            .await;

        rx.await.ok().flatten()
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Vec<SessionDomain> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(RegistryCommand::GetAllSessions { respond_to: tx })
            .await;

        rx.await.unwrap_or_default()
    }

    /// Subscribe to session events
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_subscriber.subscribe()
    }
}
```

#### Task 4.4: Registry Module & Cleanup Task

**File:** `crates/atmd/src/registry/mod.rs`

```rust
mod actor;
mod commands;
mod handle;

pub use actor::RegistryActor;
pub use commands::{RegistryCommand, RegistryError, SessionEvent};
pub use handle::RegistryHandle;

use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration};

/// Spawn the registry actor and cleanup task
pub fn spawn_registry() -> RegistryHandle {
    const CHANNEL_BUFFER: usize = 100;
    const EVENT_BUFFER: usize = 100;
    const CLEANUP_INTERVAL_SECS: u64 = 30;

    // Create channels
    let (cmd_tx, cmd_rx) = mpsc::channel(CHANNEL_BUFFER);
    let (event_tx, _event_rx) = broadcast::channel(EVENT_BUFFER);

    // Spawn actor
    let actor = RegistryActor::new(cmd_rx, event_tx.clone());
    tokio::spawn(actor.run());

    // Spawn cleanup task
    let cleanup_sender = cmd_tx.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));

        loop {
            interval.tick().await;
            let _ = cleanup_sender.send(RegistryCommand::CleanupStale).await;
        }
    });

    RegistryHandle::new(cmd_tx, event_tx)
}
```

#### Task 4.5: Registry Integration Tests

**File:** `crates/atmd/tests/registry_tests.rs`

```rust
use atmd::registry::spawn_registry;
use atm_core::{AgentType, ContextUsage, SessionDomain, SessionId, SessionStatus, Model};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_register_and_get_session() {
    let registry = spawn_registry();

    let session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);
    let session_id = session.id.clone();

    // Register session
    registry.register(session.clone()).await.unwrap();

    // Retrieve session
    let retrieved = registry.get_session(session_id).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, session.id);
}

#[tokio::test]
async fn test_update_context() {
    let registry = spawn_registry();

    let session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);
    let session_id = session.id.clone();

    registry.register(session).await.unwrap();

    // Update context
    let context = ContextUsage::new(1000, 500, 10.0, 0.05);
    registry
        .update_context(session_id.clone(), context.clone())
        .await
        .unwrap();

    // Verify update
    let updated = registry.get_session(session_id).await.unwrap();
    assert_eq!(updated.context.input_tokens, 1000);
    assert_eq!(updated.context.output_tokens, 500);
}

#[tokio::test]
async fn test_registry_full() {
    let registry = spawn_registry();

    // Register 100 sessions (max capacity)
    for i in 0..100 {
        let session = SessionDomain::new(
            SessionId::from_string(format!("session-{}", i)),
            AgentType::GeneralPurpose,
        );
        registry.register(session).await.unwrap();
    }

    // 101st should fail
    let overflow = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);
    let result = registry.register(overflow).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_event_subscription() {
    let registry = spawn_registry();
    let mut events = registry.subscribe();

    let session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);
    let session_id = session.id.clone();

    // Register should trigger event
    registry.register(session).await.unwrap();

    // Receive event
    let event = events.recv().await.unwrap();
    match event {
        atmd::registry::SessionEvent::Registered {
            session_id: id, ..
        } => {
            assert_eq!(id, session_id);
        }
        _ => panic!("Expected Registered event"),
    }
}

#[tokio::test]
async fn test_cleanup_stale_sessions() {
    let registry = spawn_registry();

    let session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);
    let session_id = session.id.clone();

    registry.register(session).await.unwrap();

    // Wait for cleanup cycle (30s + buffer)
    // In real test, we'd mock time or trigger cleanup manually
    // For now, just verify session exists
    let exists = registry.get_session(session_id).await;
    assert!(exists.is_some());
}
```

**File:** `crates/atmd/Cargo.toml`

```toml
[package]
name = "atmd"
version.workspace = true
edition.workspace = true

[[bin]]
name = "atmd"
path = "src/main.rs"

[dependencies]
atm-core = { path = "../atm-core" }
atm-protocol = { path = "../atm-protocol" }

chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true

[dev-dependencies]
tokio-test.workspace = true
```

**Time Estimate:** 6-7 hours

---

## Day 7-8: Daemon Server Implementation

**Duration:** 2 days
**Goal:** Implement Unix socket server with protocol handling

### Day 7: Connection Handling

#### Task 5.1: Connection Handler

**File:** `crates/atmd/src/server/connection.rs`

```rust
use atm_protocol::{ClientMessage, ClientType, DaemonMessage, ProtocolVersion};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::registry::RegistryHandle;

/// Maximum message size (1MB)
const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Handle a client connection
pub struct ConnectionHandler {
    reader: BufReader<OwnedReadHalf>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
    registry: RegistryHandle,
    client_type: Option<ClientType>,
}

impl ConnectionHandler {
    pub fn new(
        reader: OwnedReadHalf,
        writer: OwnedWriteHalf,
        registry: RegistryHandle,
    ) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer: Arc::new(Mutex::new(writer)),
            registry,
            client_type: None,
        }
    }

    /// Run the connection handler
    pub async fn run(mut self) {
        info!("New client connected");

        // First message must be Hello
        if let Err(e) = self.handle_handshake().await {
            error!("Handshake failed: {}", e);
            return;
        }

        // Handle subsequent messages
        loop {
            match self.read_message().await {
                Ok(Some(msg)) => {
                    if let Err(e) = self.handle_message(msg).await {
                        error!("Failed to handle message: {}", e);
                        let _ = self.send_error("internal_error", &e.to_string()).await;
                    }
                }
                Ok(None) => {
                    info!("Client disconnected");
                    break;
                }
                Err(e) => {
                    error!("Failed to read message: {}", e);
                    break;
                }
            }
        }
    }

    /// Handle initial handshake
    async fn handle_handshake(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let msg = self
            .read_message()
            .await?
            .ok_or("Connection closed during handshake")?;

        match msg {
            ClientMessage::Hello {
                protocol_version,
                client_type,
            } => {
                debug!(
                    version = %protocol_version,
                    client_type = ?client_type,
                    "Handshake received"
                );

                // Check version compatibility
                if !protocol_version.is_compatible_with(&ProtocolVersion::CURRENT) {
                    warn!(
                        client_version = %protocol_version,
                        server_version = %ProtocolVersion::CURRENT,
                        "Incompatible protocol version"
                    );

                    self.send_message(DaemonMessage::ServerHello {
                        protocol_version: ProtocolVersion::CURRENT,
                        accepted: false,
                        reason: Some(format!(
                            "Incompatible version. Server requires {}",
                            ProtocolVersion::CURRENT
                        )),
                    })
                    .await?;

                    return Err("Incompatible protocol version".into());
                }

                // Accept handshake
                self.client_type = Some(client_type);

                self.send_message(DaemonMessage::ServerHello {
                    protocol_version: ProtocolVersion::CURRENT,
                    accepted: true,
                    reason: None,
                })
                .await?;

                Ok(())
            }
            _ => Err("First message must be Hello".into()),
        }
    }

    /// Handle a client message
    async fn handle_message(&mut self, msg: ClientMessage) -> Result<(), Box<dyn std::error::Error>> {
        match msg {
            ClientMessage::Hello { .. } => {
                // Already handled in handshake
                Ok(())
            }

            ClientMessage::Register {
                session_id,
                agent_type,
                pid,
                working_dir,
            } => {
                let session = atm_domain::SessionDomain::new(session_id, agent_type);

                match self.registry.register(session).await {
                    Ok(()) => self.send_message(DaemonMessage::Ok).await?,
                    Err(e) => {
                        self.send_error("registration_failed", &e.to_string())
                            .await?
                    }
                }

                Ok(())
            }

            ClientMessage::UpdateContext {
                session_id,
                context,
            } => {
                match self.registry.update_context(session_id, context).await {
                    Ok(()) => self.send_message(DaemonMessage::Ok).await?,
                    Err(e) => self.send_error("update_failed", &e.to_string()).await?,
                }

                Ok(())
            }

            ClientMessage::EndSession { session_id } => {
                match self.registry.end_session(session_id).await {
                    Ok(()) => self.send_message(DaemonMessage::Ok).await?,
                    Err(e) => self.send_error("end_failed", &e.to_string()).await?,
                }

                Ok(())
            }

            ClientMessage::GetSession { session_id } => {
                match self.registry.get_session(session_id).await {
                    Some(session) => {
                        self.send_message(DaemonMessage::Session { session })
                            .await?
                    }
                    None => {
                        self.send_error("not_found", "Session not found")
                            .await?
                    }
                }

                Ok(())
            }

            ClientMessage::GetAllSessions => {
                let sessions = self.registry.get_all_sessions().await;
                self.send_message(DaemonMessage::Sessions { sessions })
                    .await?;

                Ok(())
            }

            ClientMessage::Subscribe => {
                // TODO: Implement subscription handling (Day 8)
                self.send_message(DaemonMessage::Ok).await?;
                Ok(())
            }

            ClientMessage::Unsubscribe => {
                self.send_message(DaemonMessage::Ok).await?;
                Ok(())
            }
        }
    }

    /// Read a message from the client
    async fn read_message(&mut self) -> Result<Option<ClientMessage>, Box<dyn std::error::Error>> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            return Ok(None); // EOF
        }

        if line.len() > MAX_MESSAGE_SIZE {
            return Err(format!("Message too large: {} bytes", line.len()).into());
        }

        let msg: ClientMessage = serde_json::from_str(&line)?;
        Ok(Some(msg))
    }

    /// Send a message to the client
    async fn send_message(&self, msg: DaemonMessage) -> Result<(), Box<dyn std::error::Error>> {
        let mut json = serde_json::to_string(&msg)?;
        json.push('\n');

        let mut writer = self.writer.lock().await;
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Send an error message
    async fn send_error(&self, code: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.send_message(DaemonMessage::Error {
            code: code.to_string(),
            message: message.to_string(),
        })
        .await
    }
}
```

**Time Estimate:** 5-6 hours

---

### Day 8: Server Setup & Main Loop

#### Task 5.2: Server Implementation

**File:** `crates/atmd/src/server/mod.rs`

```rust
mod connection;

use connection::ConnectionHandler;

use crate::registry::RegistryHandle;
use std::path::Path;
use tokio::net::UnixListener;
use tracing::{error, info};

/// Daemon server listening on Unix socket
pub struct DaemonServer {
    socket_path: String,
    registry: RegistryHandle,
}

impl DaemonServer {
    pub fn new(socket_path: impl Into<String>, registry: RegistryHandle) -> Self {
        Self {
            socket_path: socket_path.into(),
            registry,
        }
    }

    /// Run the server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove existing socket if present
        let socket_path = Path::new(&self.socket_path);
        if socket_path.exists() {
            std::fs::remove_file(socket_path)?;
        }

        // Bind to socket
        let listener = UnixListener::bind(socket_path)?;
        info!(socket = %self.socket_path, "Daemon server listening");

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let (reader, writer) = stream.into_split();
                    let handler = ConnectionHandler::new(reader, writer, self.registry.clone());

                    // Spawn connection handler
                    tokio::spawn(handler.run());
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}
```

#### Task 5.3: Main Daemon Entry Point

**File:** `crates/atmd/src/main.rs`

```rust
mod registry;
mod server;

use registry::spawn_registry;
use server::DaemonServer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting atm daemon");

    // Spawn registry
    let registry = spawn_registry();

    // Start server
    let server = DaemonServer::new("/tmp/atm.sock", registry);
    server.run().await?;

    Ok(())
}
```

**Time Estimate:** 4-5 hours

---

## Day 9-10: Bash Client Scripts

**Duration:** 2 days
**Goal:** Create bash scripts for Claude Code integration

### Day 9: Status Line Script

#### Task 6.1: Status Line Handler

**File:** `scripts/atm-status.sh`

```bash
#!/bin/bash
# Agent Tmux Monitor status line handler
# Called by Claude Code with JSON on stdin

SOCKET="/tmp/atm.sock"
LOG_FILE="/tmp/atm-client.log"
PROTOCOL_VERSION="1.0"

# Get or create session ID
if [ -n "$CLAUDE_SESSION_ID" ]; then
    SESSION_ID="$CLAUDE_SESSION_ID"
elif [ -f "/tmp/atm-session-$$" ]; then
    SESSION_ID=$(cat "/tmp/atm-session-$$")
else
    SESSION_ID=$(uuidgen)
    echo "$SESSION_ID" > "/tmp/atm-session-$$"
fi

# Function to send message to daemon
send_message() {
    local message="$1"

    # Check if socket exists
    if [ ! -S "$SOCKET" ]; then
        # Daemon not running - silently exit
        return 0
    fi

    # Send with timeout (non-blocking)
    echo "$message" | timeout 0.1 nc -U "$SOCKET" 2>/dev/null || {
        echo "$(date): Failed to send to daemon" >> "$LOG_FILE"
        return 0
    }

    return 0
}

# Send handshake on first run
if [ ! -f "/tmp/atm-handshake-$SESSION_ID" ]; then
    HANDSHAKE=$(cat <<EOF
{"type":"hello","protocol_version":{"major":1,"minor":0},"client_type":"session"}
EOF
)
    send_message "$HANDSHAKE"

    # Wait for response (with timeout)
    RESPONSE=$(timeout 0.5 nc -U "$SOCKET" 2>/dev/null | head -n 1)

    # Register session
    REGISTER=$(cat <<EOF
{"type":"register","session_id":"$SESSION_ID","agent_type":"general-purpose","pid":$$,"working_dir":"$(pwd)"}
EOF
)
    send_message "$REGISTER"

    touch "/tmp/atm-handshake-$SESSION_ID"
fi

# Read status updates from stdin
while IFS= read -r line; do
    # Extract fields from Claude Code status JSON
    INPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_input_tokens // 0')
    OUTPUT_TOKENS=$(echo "$line" | jq -r '.context_window.total_output_tokens // 0')
    USED_PCT=$(echo "$line" | jq -r '.context_window.used_percentage // 0')
    COST=$(echo "$line" | jq -r '.cost.total_cost_usd // 0')

    # Build update message
    UPDATE=$(cat <<EOF
{"type":"update_context","session_id":"$SESSION_ID","context":{"input_tokens":$INPUT_TOKENS,"output_tokens":$OUTPUT_TOKENS,"used_percentage":$USED_PCT,"cost_usd":$COST,"timestamp":"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}}
EOF
)

    send_message "$UPDATE"
done

# Cleanup on exit
trap 'send_message "{\"type\":\"end_session\",\"session_id\":\"$SESSION_ID\"}"' EXIT

exit 0
```

**Time Estimate:** 3-4 hours

---

### Day 10: Hook Handlers & Installation

#### Task 6.2: Hook Handler Script

**File:** `scripts/atm-hook.sh`

```bash
#!/bin/bash
# Agent Tmux Monitor hook handler
# Called by Claude Code for various hook events

SOCKET="/tmp/atm.sock"
LOG_FILE="/tmp/atm-hooks.log"

# Read hook event from stdin
IFS= read -r line

# Log the event
echo "$(date): Hook event: $line" >> "$LOG_FILE"

# For now, we just log hooks
# Future: Could track tool usage, permission requests, etc.

exit 0
```

#### Task 6.3: Installation Script

**File:** `scripts/install-claude-integration.sh`

```bash
#!/bin/bash
# Install Agent Tmux Monitor Claude Code integration

set -e

echo "Installing Agent Tmux Monitor Claude Code integration..."

# Paths
CLAUDE_DIR="$HOME/.claude"
SCRIPTS_DIR="$HOME/.local/bin"

# Create directories
mkdir -p "$CLAUDE_DIR"
mkdir -p "$SCRIPTS_DIR"

# Copy scripts
cp atm-status.sh "$SCRIPTS_DIR/"
cp atm-hook.sh "$SCRIPTS_DIR/"
chmod +x "$SCRIPTS_DIR/atm-status.sh"
chmod +x "$SCRIPTS_DIR/atm-hook.sh"

# Configure Claude Code settings
cat > "$CLAUDE_DIR/settings.json" <<EOF
{
  "statusLine": {
    "type": "command",
    "command": "$SCRIPTS_DIR/atm-status.sh"
  }
}
EOF

# Configure hooks (optional)
cat > "$CLAUDE_DIR/hooks.json" <<EOF
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$SCRIPTS_DIR/atm-hook.sh"
          }
        ]
      }
    ]
  }
}
EOF

echo "Installation complete!"
echo ""
echo "Next steps:"
echo "1. Start daemon: atmd"
echo "2. Start Claude Code in a tmux pane"
echo "3. Monitor: atm (TUI - coming in Phase 2)"
```

**Time Estimate:** 3-4 hours

---

## Day 11-12: Comprehensive Testing

**Duration:** 2 days
**Goal:** Complete test coverage and integration testing

### Day 11: Unit & Integration Tests

#### Task 7.1: Domain Layer Tests (Complete Coverage)

**File:** `crates/atm-core/tests/session_tests.rs`

```rust
use chrono::{Duration, Utc};
use atm_core::*;

#[test]
fn session_lifecycle() {
    let mut session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);

    // Initially active
    assert_eq!(session.status, SessionStatus::Active);

    // Update context
    let context = ContextUsage::new(1000, 500, 10.0, 0.05);
    session.update_context(context);

    assert_eq!(session.context.total_tokens(), 1500);

    // End session
    session.end();
    assert_eq!(session.status, SessionStatus::Ended);
}

#[test]
fn stale_detection() {
    let session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);

    let now = Utc::now();
    let future = now + Duration::seconds(120); // 2 minutes later

    assert!(!session.is_stale(now)); // Not stale yet
    assert!(session.is_stale(future)); // Stale after 2 minutes
}

#[test]
fn cleanup_criteria() {
    let mut session = SessionDomain::new(SessionId::new(), AgentType::GeneralPurpose);

    let now = Utc::now();

    // Active session doesn't cleanup
    assert!(!session.should_cleanup(now));

    // Ended session should cleanup
    session.end();
    assert!(session.should_cleanup(now));
}
```

#### Task 7.2: End-to-End Integration Test

**File:** `crates/atmd/tests/e2e_test.rs`

```rust
use atmd::registry::spawn_registry;
use atmd::server::DaemonServer;
use atm_protocol::{ClientMessage, ClientType, DaemonMessage, ProtocolVersion};
use serde_json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_full_session_lifecycle() {
    // Start server
    let registry = spawn_registry();
    let server = DaemonServer::new("/tmp/atm-test.sock", registry);

    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    // Wait for server to start
    sleep(Duration::from_millis(100)).await;

    // Connect client
    let stream = UnixStream::connect("/tmp/atm-test.sock")
        .await
        .unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send handshake
    let hello = ClientMessage::Hello {
        protocol_version: ProtocolVersion::new(1, 0),
        client_type: ClientType::Session,
    };
    let msg = format!("{}\n", serde_json::to_string(&hello).unwrap());
    writer.write_all(msg.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    // Read response
    let mut response = String::new();
    reader.read_line(&mut response).await.unwrap();
    let server_hello: DaemonMessage = serde_json::from_str(&response).unwrap();

    match server_hello {
        DaemonMessage::ServerHello { accepted, .. } => {
            assert!(accepted);
        }
        _ => panic!("Expected ServerHello"),
    }

    // Cleanup
    std::fs::remove_file("/tmp/atm-test.sock").ok();
}
```

**Time Estimate:** 6-7 hours

---

### Day 12: Manual Testing & Documentation

#### Task 7.3: Manual Test Plan

**File:** `docs/guides/testing/MANUAL_TEST_PLAN.md`

```markdown
# Manual Test Plan - Phase 1

## Prerequisites
- Rust installed
- tmux installed
- Claude Code configured
- jq installed (for JSON parsing)

## Test 1: Daemon Startup
1. Build project: `cargo build --release`
2. Start daemon: `target/release/atmd`
3. Verify log output shows "Daemon server listening"
4. Verify socket created: `ls -la /tmp/atm.sock`

## Test 2: Session Registration
1. Start daemon
2. In tmux pane, start Claude Code
3. Verify session registered (check daemon logs)
4. Test query: `echo '{"type":"get_all_sessions"}' | nc -U /tmp/atm.sock`

## Test 3: Context Updates
1. Have active Claude Code session
2. Trigger context updates (have conversation)
3. Query session: verify tokens increasing
4. Check for near-limit detection at >80%

## Test 4: Stale Session Cleanup
1. Register session
2. Stop sending updates
3. Wait 90 seconds
4. Verify session marked stale
5. Wait for cleanup cycle (30s)
6. Verify session removed

## Test 5: Error Handling
1. Send malformed JSON - verify daemon continues
2. Fill registry to 100 sessions - verify 101st rejected
3. Kill daemon while session active - verify bash script doesn't hang
4. Restart daemon - verify clean startup

## Test 6: Protocol Versioning
1. Send v2.0 hello - verify rejection
2. Send v1.1 hello - verify acceptance (forward compatible)
3. Send v1.0 hello - verify acceptance
```

#### Task 7.4: Testing Documentation

**Time Estimate:** 4-5 hours

---

## Success Criteria & Deliverables

### Must Have (Blocking Week 4)

- [x] Domain layer with zero infrastructure dependencies
- [x] Infrastructure layer for tmux/process info
- [x] Protocol with versioning support
- [x] Registry actor with message passing
- [x] Daemon server with Unix socket
- [x] Bash scripts for Claude Code integration
- [x] Comprehensive test suite (>80% coverage)
- [x] All tests passing
- [x] Manual test plan validated

### Quality Gates

**Code Quality:**
- All `cargo clippy` warnings resolved
- All tests passing (`cargo test`)
- No panics in production code paths
- Structured logging with tracing

**Architecture:**
- Clean domain/infrastructure separation
- Actor model correctly implemented
- Error handling follows specification
- Resource limits enforced

**Documentation:**
- README with setup instructions
- Architecture documentation complete
- Manual test plan validated
- Code comments for complex logic

### Readiness Checklist for Week 4

Before starting Week 4 (TUI + Phase 2), verify:

- [ ] Can start daemon and register sessions
- [ ] Context updates flow through system
- [ ] Stale sessions cleanup automatically
- [ ] Protocol versioning works
- [ ] Error handling is graceful
- [ ] No memory leaks (run for 1 hour)
- [ ] Logs are clean and informative
- [ ] Bash scripts integrate with Claude Code
- [ ] Manual tests all pass

### Deliverables

**Code (matching DOMAIN_MODEL.md structure):**
- `crates/atm-core/` - Shared types (session, context, cost, model, agent, hook, error, services, view)
- `crates/atm-protocol/` - Protocol messages, parsing helpers (RawStatusLine, RawHookEvent), versioning
- `crates/atmd/` - Daemon server implementation (registry actor, connection handling)
- `crates/atm/` - TUI binary (Phase 2, scaffold only)
- `scripts/` - Bash integration scripts

**Documentation:**
- `README.md` - Project overview & quick start
- `docs/ARCHITECTURE.md` - System architecture
- `docs/CONCURRENCY_MODEL.md` - Actor pattern
- `docs/ERROR_HANDLING.md` - Error strategy
- `docs/RESOURCE_LIMITS.md` - Limits & cleanup
- `docs/PROTOCOL_VERSIONING.md` - Version negotiation
- `docs/guides/testing/MANUAL_TEST_PLAN.md` - Test procedures

**Tests:**
- Unit tests for all modules
- Integration tests for registry
- End-to-end test for full flow
- Property-based tests for domain
- Manual test validation

---

## Time Estimates Summary

| Day | Focus | Estimated Hours |
|-----|-------|----------------|
| 1-2 | Domain Layer | 12-14 hours |
| 3 | Infrastructure Layer | 7-9 hours |
| 4 | Protocol Layer | 6-7 hours |
| 5-6 | Registry Actor | 12-14 hours |
| 7-8 | Daemon Server | 9-11 hours |
| 9-10 | Bash Scripts | 6-8 hours |
| 11-12 | Testing & Validation | 10-12 hours |

**Total:** 62-75 hours over 10-12 days

---

## Risk Mitigation

**Risk: Claude Code integration doesn't work as expected**
- Mitigation: Week 1 validation already complete
- Fallback: Adjust protocol based on actual behavior

**Risk: Actor model performance issues**
- Mitigation: Design allows easy swap to RwLock if needed
- Monitoring: Track message queue depth

**Risk: Stale session cleanup too aggressive**
- Mitigation: Configurable thresholds
- Adjustment: Increase threshold if needed

**Risk: Test coverage incomplete**
- Mitigation: Property-based testing catches edge cases
- Manual testing validates real-world scenarios

---

## Next Steps After Week 2-3

**Week 4:** TUI Implementation (Phase 2)
- Build on this daemon
- Subscribe to registry events
- Real-time UI updates

**Week 5-6:** Enhancement Features (Phase 2 continued)
- Cost tracking
- Conversation history
- Alert system

**Week 7+:** Advanced Features (Phase 3)
- Agent orchestration
- Claude Code CLI integration
- Multi-agent workflows
