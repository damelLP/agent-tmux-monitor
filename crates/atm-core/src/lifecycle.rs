//! Vendor-neutral agent lifecycle events.
//!
//! `LifecycleEvent` is the abstraction that crosses the daemon boundary —
//! every vendor adapter (Claude Code, pi, future) translates its native
//! event vocabulary into this enum. The daemon's session state machine
//! only ever pattern-matches on `LifecycleEvent`, never on a raw vendor
//! event type.
//!
//! ## Design
//!
//! Three properties drove the shape:
//!
//! 1. **No vendor escape hatch.** Earlier drafts had a `VendorSpecific`
//!    variant; that turned out to leak vendor knowledge into every
//!    consumer. Instead, every event maps to a generic concept (e.g.
//!    Claude `SubagentStart` → `ChildSessionStart`).
//! 2. **`NeedsInput` is adapter-synthesized, not raw.** Pi has no
//!    permission-prompt event — its extension synthesizes one inside a
//!    `tool_call` handler. Claude only signals it via `PreToolUse` of an
//!    interactive tool. So this variant is emitted by the translation
//!    layer, never directly by a vendor.
//! 3. **Provider/model are session metadata, not per-event.** Pi is
//!    provider-agnostic — one session can switch from Anthropic to
//!    OpenAI mid-stream — so changes ride a dedicated
//!    `ProviderModelChange` variant rather than annotating every event.

use serde::{Deserialize, Serialize};

/// Why a session is awaiting user input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum NeedsInputReason {
    /// Claude `PreToolUse` for an interactive tool
    /// (`AskUserQuestion`, `EnterPlanMode`, `ExitPlanMode`).
    InteractiveTool { tool_name: String },
    /// Pi extension-mediated `tool_call` permission gate.
    PermissionGate { tool_name: String },
    /// Generic notification-driven prompt
    /// (Claude `Notification(permission_prompt|elicitation_dialog)`).
    Notification { kind: String },
}

/// Vendor-neutral lifecycle event.
///
/// Adapters translate native vendor events into these variants; the
/// daemon dispatches on them to update session state.
///
/// `Eq` is intentionally not derived: `ContextUpdate` carries an
/// `Option<f64>` cost, which is not `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifecycleEvent {
    /// Session opened.
    SessionStart,

    /// Session closed. `reason` is vendor-supplied when available
    /// (pi `session_shutdown.reason`); `None` for vendors that don't
    /// distinguish exit reasons.
    SessionEnd { reason: Option<String> },

    /// Agent began a working block.
    /// (Pi `agent_start`; Claude has no direct analog — synthesized
    /// from `UserPromptSubmit` or first `PreToolUse`.)
    WorkingStart,

    /// Agent finished a working block.
    /// (Pi `agent_end`; Claude `Stop`.)
    WorkingEnd,

    /// Explicit idle signal. Distinct from `WorkingEnd` because pi can
    /// transition idle → idle (no work happened) via
    /// `ctx.isIdle() && !ctx.hasPendingMessages()`.
    Idle,

    /// User submitted a prompt.
    PromptSubmit { prompt: Option<String> },

    /// Session is waiting on user input.
    NeedsInput { reason: NeedsInputReason },

    /// A tool started executing.
    ToolCallStart { name: String },

    /// A tool finished executing.
    ToolCallEnd { name: String, is_error: bool },

    /// Context compaction is starting.
    ContextCompactStart,

    /// Periodic context-usage update (tokens used, accumulated cost).
    /// Either field may be `None` if the vendor doesn't expose it.
    ContextUpdate {
        tokens: Option<u64>,
        cost_usd: Option<f64>,
    },

    /// Provider or model changed mid-session (pi `model_select`).
    ProviderModelChange {
        provider: Option<String>,
        model: Option<String>,
    },

    /// Free-form notification surfaced to the user. `kind` carries an
    /// optional sub-type the UI can render specially (e.g. `"setup"`,
    /// `"info"`).
    Notification {
        message: Option<String>,
        kind: Option<String>,
    },

    /// A child session (subagent / task / fork) started.
    /// `id` is a vendor-supplied correlation id; `role` is a free-form
    /// tag (e.g. Claude subagent role: `"explore"`, `"plan"`).
    ChildSessionStart {
        id: Option<String>,
        role: Option<String>,
    },

    /// A child session finished.
    ChildSessionEnd { id: Option<String> },
}

impl LifecycleEvent {
    /// True if this event ends/clears the session's working state.
    #[must_use]
    pub fn is_terminal_for_turn(&self) -> bool {
        matches!(self, Self::WorkingEnd | Self::Idle | Self::SessionEnd { .. })
    }

    /// True if this event opens the session's working state.
    #[must_use]
    pub fn is_starting(&self) -> bool {
        matches!(
            self,
            Self::SessionStart | Self::WorkingStart | Self::PromptSubmit { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_event_serde_roundtrip() {
        let cases = vec![
            LifecycleEvent::SessionStart,
            LifecycleEvent::SessionEnd {
                reason: Some("quit".into()),
            },
            LifecycleEvent::WorkingStart,
            LifecycleEvent::WorkingEnd,
            LifecycleEvent::Idle,
            LifecycleEvent::PromptSubmit {
                prompt: Some("hello".into()),
            },
            LifecycleEvent::NeedsInput {
                reason: NeedsInputReason::InteractiveTool {
                    tool_name: "AskUserQuestion".into(),
                },
            },
            LifecycleEvent::NeedsInput {
                reason: NeedsInputReason::PermissionGate {
                    tool_name: "bash".into(),
                },
            },
            LifecycleEvent::NeedsInput {
                reason: NeedsInputReason::Notification {
                    kind: "permission_prompt".into(),
                },
            },
            LifecycleEvent::ToolCallStart { name: "Bash".into() },
            LifecycleEvent::ToolCallEnd {
                name: "Bash".into(),
                is_error: false,
            },
            LifecycleEvent::ContextCompactStart,
            LifecycleEvent::ContextUpdate {
                tokens: Some(1024),
                cost_usd: Some(0.05),
            },
            LifecycleEvent::ProviderModelChange {
                provider: Some("openai-codex".into()),
                model: Some("gpt-5.5".into()),
            },
            LifecycleEvent::Notification {
                message: Some("hi".into()),
                kind: Some("info".into()),
            },
            LifecycleEvent::ChildSessionStart {
                id: Some("agent-1".into()),
                role: Some("explore".into()),
            },
            LifecycleEvent::ChildSessionEnd {
                id: Some("agent-1".into()),
            },
        ];

        for ev in cases {
            let json = serde_json::to_string(&ev).expect("serialize");
            let back: LifecycleEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(ev, back, "roundtrip failed: {json}");
        }
    }

    #[test]
    fn terminal_and_starting_classification() {
        assert!(LifecycleEvent::WorkingEnd.is_terminal_for_turn());
        assert!(LifecycleEvent::Idle.is_terminal_for_turn());
        assert!(LifecycleEvent::SessionEnd { reason: None }.is_terminal_for_turn());
        assert!(!LifecycleEvent::WorkingStart.is_terminal_for_turn());

        assert!(LifecycleEvent::SessionStart.is_starting());
        assert!(LifecycleEvent::WorkingStart.is_starting());
        assert!(LifecycleEvent::PromptSubmit { prompt: None }.is_starting());
        assert!(!LifecycleEvent::WorkingEnd.is_starting());
    }
}
