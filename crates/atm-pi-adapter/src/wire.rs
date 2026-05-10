//! pi wire payload deserialization.
//!
//! Matches the JSONL format the pi extension API produces (verified
//! against real traces in `/tmp/atm-pi-spike-*.jsonl` from spike
//! `agent-tmux-manager-9dn`).
//!
//! The TS extension subscribes via `pi.on(eventName, handler)` and
//! forwards `{ event: <name>, payload: <object> }` to atmd. We
//! deserialize that envelope here.

use serde::Deserialize;

use crate::event::PiEventType;

/// Wire envelope: pi extension forwards `{event, payload}` per event.
///
/// This is what the TS extension (`pi-atm`, bead
/// `agent-tmux-manager-6dx`) sends to atmd over the existing Unix
/// socket protocol.
#[derive(Debug, Clone, Deserialize)]
pub struct RawPiEvent {
    /// pi event name (e.g. `"agent_start"`, `"tool_execution_start"`).
    pub event: PiEventType,

    /// Event-specific payload — flat object with the fields pi
    /// includes for that event. Captured as a tagged union.
    #[serde(default)]
    pub payload: PiPayload,

    // === Fields injected by the TS extension ===
    /// Session id (resolved from pi's session manager at emission time).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Pi process pid (so atmd can correlate to /proc).
    #[serde(default)]
    pub pid: Option<u32>,
    /// Tmux pane id when running inside tmux.
    #[serde(default)]
    pub tmux_pane: Option<String>,
}

/// Pi event payload — fields vary per event type.
///
/// Modeled as one struct with all `Option` fields because pi's runtime
/// JSON shape is essentially that: each event fills the relevant
/// fields and leaves the rest absent. This mirrors how
/// [`atm_claude_adapter::RawHookEvent`] handles Claude.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PiPayload {
    // === Tool events (tool_execution_start/end, tool_call, tool_result) ===
    #[serde(rename = "toolName", default)]
    pub tool_name: Option<String>,
    #[serde(rename = "toolCallId", default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(rename = "isError", default)]
    pub is_error: Option<bool>,

    // === Session lifecycle (session_start, session_shutdown) ===
    #[serde(default)]
    pub reason: Option<String>,

    // === Input event (interactive prompt) ===
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub text: Option<String>,

    // === Model select ===
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,

    // === Context event (carries cost/tokens) ===
    /// `messages[]` — full conversation snapshot.
    #[serde(default)]
    pub messages: Option<serde_json::Value>,

    // === Permission gate (synthesized by extension's tool_call handler) ===
    /// True when the upstream pi extension is signalling that pi has
    /// reached a `ctx.ui.select(...)` permission prompt and is awaiting
    /// user input. Pi itself does not emit a `block: true` payload, so
    /// this flag is set by an adapter-side TS extension and consumed
    /// here in `translate.rs` to drive `NeedsInput`.
    #[serde(default)]
    pub needs_user_input: Option<bool>,

    // === Synthetic atm events (atm_needs_input_open) ===
    /// Title/prompt of the dialog that just opened. Some extensions
    /// (pi-amplike) include the bash command being gated; the title
    /// surfaces in the TUI's activity-detail field.
    #[serde(default)]
    pub title: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample line from `/tmp/atm-pi-spike-pid1226148-2026-05-02T08-55-56-043Z.jsonl`.
    #[test]
    fn parse_session_start_real_payload() {
        let json = r#"{
            "event": "session_start",
            "payload": {"type": "session_start", "reason": "startup"}
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::SessionStart);
        assert_eq!(raw.payload.reason.as_deref(), Some("startup"));
    }

    #[test]
    fn parse_input_real_payload() {
        let json = r#"{
            "event": "input",
            "payload": {
                "type": "input",
                "text": "Use ls to list /tmp",
                "source": "interactive"
            }
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::Input);
        assert_eq!(raw.payload.source.as_deref(), Some("interactive"));
        assert_eq!(raw.payload.text.as_deref(), Some("Use ls to list /tmp"));
    }

    #[test]
    fn parse_tool_execution_start_real_payload() {
        let json = r#"{
            "event": "tool_execution_start",
            "payload": {
                "type": "tool_execution_start",
                "toolName": "ls",
                "toolCallId": "call_QPdBI",
                "args": {"path": "/tmp", "limit": 3}
            }
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::ToolExecutionStart);
        assert_eq!(raw.payload.tool_name.as_deref(), Some("ls"));
        assert_eq!(raw.payload.tool_call_id.as_deref(), Some("call_QPdBI"));
    }

    #[test]
    fn parse_tool_call_undeclared_event() {
        // tool_call is undeclared in pi's TypeScript types but real.
        let json = r#"{
            "event": "tool_call",
            "payload": {
                "type": "tool_call",
                "toolName": "ls",
                "toolCallId": "call_QPdBI",
                "input": {"path": "/tmp"}
            }
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::ToolCall);
        assert_eq!(raw.payload.tool_name.as_deref(), Some("ls"));
        assert!(raw.payload.input.is_some());
    }

    #[test]
    fn parse_tool_result_with_error_flag() {
        let json = r#"{
            "event": "tool_result",
            "payload": {
                "type": "tool_result",
                "toolName": "ls",
                "toolCallId": "call_QPdBI",
                "isError": false,
                "content": [{"type":"text","text":"a\nb\n"}]
            }
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::ToolResult);
        assert_eq!(raw.payload.is_error, Some(false));
    }

    #[test]
    fn parse_session_shutdown_with_reason() {
        let json = r#"{
            "event": "session_shutdown",
            "payload": {"type":"session_shutdown","reason":"quit"}
        }"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::SessionShutdown);
        assert_eq!(raw.payload.reason.as_deref(), Some("quit"));
    }

    #[test]
    fn parse_unknown_event_falls_back_to_other() {
        let json = r#"{"event": "future_pi_event", "payload": {}}"#;
        let raw: RawPiEvent = serde_json::from_str(json).unwrap();
        assert_eq!(raw.event, PiEventType::Other("future_pi_event".to_string()));
    }
}
