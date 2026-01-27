# SessionStatus Transition Design

**Date:** 2026-01-27
**Status:** Approved

## Overview

Refactor `SessionStatus` from 6 variants to 3 semantic states, expand hook event handling to all 12 Claude Code events, and simplify the domain model by eliminating `DisplayState`.

## Design Decisions

### 1. Goal: Comprehensive Refactor
- Simplify domain model (6 → 3 states)
- Support all 12 Claude Code hook events
- Defer subagent tracking to follow-up PR

### 2. Implementation Strategy: Events First
1. **Phase 1**: Expand `HookEventType` to all 12 events, update parsing
2. **Phase 2**: Refactor `SessionStatus` to 3-state model
3. **Phase 3** (future): Add subagent parent-child tracking

### 3. Hook Event Scope: All 12 Events
```rust
pub enum HookEventType {
    // Tool Execution
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,

    // User Interaction
    UserPromptSubmit,
    Stop,

    // Subagent Lifecycle (parsed but not fully handled yet)
    SubagentStart,
    SubagentStop,

    // Session Lifecycle
    SessionStart,
    SessionEnd,

    // Context Management
    PreCompact,
    Setup,

    // Notifications
    Notification,
}
```

### 4. Parsing Approach: Hybrid
- **Wire layer**: Flat `RawHookEvent` with all optional fields
- **Domain layer**: Typed `HookEvent` enum with event-specific structs
- Validation happens during conversion from Raw → Typed

### 5. Merge SessionStatus and DisplayState
- Eliminate `DisplayState` entirely
- UI uses `SessionStatus` directly
- Helper methods provide UI concerns (blinking, icons, colors)

### 6. Activity Details: Separate Struct
```rust
pub struct ActivityDetail {
    /// Tool name if running/waiting on a tool
    pub tool_name: Option<String>,
    /// When the current activity started
    pub started_at: DateTime<Utc>,
    /// Additional context (e.g., "Compacting", "Setup")
    pub context: Option<String>,
}

pub struct SessionDomain {
    pub status: SessionStatus,
    pub current_activity: Option<ActivityDetail>,
    // ... rest unchanged
}
```

### 7. Notification Handling: Full Parsing
Map notification types to status changes:
- `permission_prompt` → `AttentionNeeded`
- `idle_prompt` → `Idle`
- `elicitation_dialog` → `AttentionNeeded`
- Others → no status change (informational)

### 8. Subagent Tracking: Deferred
Will be implemented in follow-up PR with:
- `parent_session_id: Option<SessionId>` in `SessionDomain`
- Registry index for parent-child lookups

## New SessionStatus Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Nothing happening - Claude finished, waiting for user.
    /// User can take their time, no urgency.
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
```

## Hook Event → Status Mapping

| Hook Event | Condition | → SessionStatus |
|------------|-----------|-----------------|
| `PreToolUse` | Interactive tool* | `AttentionNeeded` |
| `PreToolUse` | Standard tool | `Working` |
| `PostToolUse` | — | `Working` |
| `PostToolUseFailure` | — | `Working` |
| `UserPromptSubmit` | — | `Working` |
| `Stop` | — | `Idle` |
| `SessionStart` | — | `Idle` |
| `SessionEnd` | — | *(Remove session)* |
| `PreCompact` | — | `Working` |
| `Setup` | — | `Working` |
| `Notification` | `permission_prompt` | `AttentionNeeded` |
| `Notification` | `idle_prompt` | `Idle` |
| `Notification` | `elicitation_dialog` | `AttentionNeeded` |
| `Notification` | other | *(No change)* |

*Interactive tools: `AskUserQuestion`, `EnterPlanMode`, `ExitPlanMode`

## Files to Modify

### Phase 1: Hook Event Expansion
| File | Changes |
|------|---------|
| `atm-core/src/hook.rs` | Add 7 new variants to `HookEventType` |
| `atm-protocol/src/parse.rs` | Expand `RawHookEvent` with all fields |
| `atm-protocol/src/message.rs` | Add typed `HookEvent` enum |
| `atmd/src/server/connection.rs` | Update event parsing |

### Phase 2: SessionStatus Transition
| File | Changes |
|------|---------|
| `atm-core/src/session.rs` | Replace `SessionStatus` (6→3), add `ActivityDetail`, remove `DisplayState` |
| `atm-core/src/lib.rs` | Update re-exports |
| `atmd/src/registry/actor.rs` | Update status mapping logic |
| `atmd/src/registry/commands.rs` | Update `ApplyHookEvent` |
| `atm/src/ui/*.rs` | Update UI to use new status directly |

## Migration Notes

### Backward Compatibility
- Protocol messages may need version bump if status serialization changes
- Existing clients should handle unknown status gracefully

### Testing Strategy
- Unit tests for each hook event → status mapping
- Integration tests with real Claude Code events
- UI visual tests for status display

## References

- [Hook Event Handling Design](../HOOK_EVENT_HANDLING_DESIGN.md)
- [Claude Code Hooks Reference](../CLAUDE_CODE_HOOKS_REFERENCE.md)
- [Domain Model](../DOMAIN_MODEL.md)
