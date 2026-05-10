//! Translation from Claude raw `RawHookEvent` to vendor-neutral
//! `LifecycleEvent`.
//!
//! This is the *only* place Claude semantics get mapped to atm-core
//! types. The daemon calls this at the connection boundary and
//! everything downstream sees only `LifecycleEvent`.

use atm_core::{LifecycleEvent, NeedsInputReason, NotificationKind, Tool};

use crate::event::ClaudeEventType;
use crate::wire::RawHookEvent;

impl RawHookEvent {
    /// Translates this Claude raw event into a vendor-neutral
    /// `LifecycleEvent`.
    ///
    /// Returns `None` if `hook_event_name` does not match a known
    /// Claude event. The translation collapses Claude-specific
    /// distinctions where the underlying concept is vendor-neutral
    /// (e.g. `PostToolUse`/`PostToolUseFailure` both become
    /// `ToolCallEnd`, distinguished by `is_error`).
    ///
    /// Carry-through fidelity: `tool_use_id` (Claude `tool_use_id` /
    /// pi `toolCallId`) and tool input rides `ToolCallStart`. `source`
    /// (SessionStart), `reason` (SessionEnd), `trigger` (PreCompact),
    /// and `prompt` (UserPromptSubmit) are preserved on their target
    /// variants.
    pub fn to_lifecycle_event(&self) -> Option<LifecycleEvent> {
        let ev = self.event_type()?;
        let tool = Tool::from(self.tool_name.as_deref().unwrap_or(""));
        Some(match ev {
            ClaudeEventType::PreToolUse => {
                if tool.is_interactive() {
                    LifecycleEvent::NeedsInput {
                        reason: NeedsInputReason::InteractiveTool { tool },
                    }
                } else {
                    LifecycleEvent::ToolCallStart {
                        name: tool,
                        tool_use_id: self.tool_use_id.clone(),
                        input: self.tool_input.clone(),
                    }
                }
            }
            ClaudeEventType::PostToolUse => LifecycleEvent::ToolCallEnd {
                name: tool,
                tool_use_id: self.tool_use_id.clone(),
                is_error: false,
            },
            ClaudeEventType::PostToolUseFailure => LifecycleEvent::ToolCallEnd {
                name: tool,
                tool_use_id: self.tool_use_id.clone(),
                is_error: true,
            },
            ClaudeEventType::UserPromptSubmit => LifecycleEvent::PromptSubmit {
                prompt: self.prompt.clone(),
            },
            ClaudeEventType::Stop => LifecycleEvent::WorkingEnd,
            ClaudeEventType::SubagentStart => LifecycleEvent::ChildSessionStart {
                id: self.agent_id.clone(),
                role: self.agent_type.clone(),
            },
            ClaudeEventType::SubagentStop => LifecycleEvent::ChildSessionEnd {
                id: self.agent_id.clone(),
            },
            ClaudeEventType::SessionStart => LifecycleEvent::SessionStart {
                source: self.source.clone(),
            },
            ClaudeEventType::SessionEnd => LifecycleEvent::SessionEnd {
                reason: self.reason.clone(),
            },
            ClaudeEventType::PreCompact => LifecycleEvent::ContextCompactStart {
                trigger: self.trigger.clone(),
            },
            ClaudeEventType::Setup => LifecycleEvent::Notification {
                message: None,
                kind: Some(NotificationKind::Setup),
            },
            ClaudeEventType::Notification => {
                let kind = self
                    .notification_type
                    .as_deref()
                    .map(NotificationKind::from);
                match kind {
                    Some(
                        k @ (NotificationKind::PermissionPrompt
                        | NotificationKind::ElicitationDialog),
                    ) => LifecycleEvent::NeedsInput {
                        reason: NeedsInputReason::Notification {
                            kind: k,
                            // Claude `Notification` events don't carry
                            // a per-prompt label — only a kind tag.
                            label: None,
                        },
                    },
                    Some(NotificationKind::IdlePrompt) => LifecycleEvent::Idle,
                    _ => LifecycleEvent::Notification {
                        message: self.message.clone(),
                        kind,
                    },
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str) -> RawHookEvent {
        RawHookEvent {
            session_id: "s".into(),
            hook_event_name: name.into(),
            cwd: None,
            permission_mode: None,
            pid: None,
            tmux_pane: None,
            tool_name: None,
            tool_input: None,
            tool_response: None,
            tool_use_id: None,
            prompt: None,
            stop_hook_active: None,
            agent_id: None,
            agent_type: None,
            agent_transcript_path: None,
            source: None,
            reason: None,
            model: None,
            trigger: None,
            custom_instructions: None,
            notification_type: None,
            message: None,
        }
    }

    #[test]
    fn pre_tool_use_non_interactive_carries_tool_use_id_and_input() {
        let mut e = raw("PreToolUse");
        e.tool_name = Some("Bash".into());
        e.tool_use_id = Some("toolu_01abc".into());
        e.tool_input = Some(serde_json::json!({"command": "ls"}));
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::ToolCallStart {
                name: Tool::Bash,
                tool_use_id: Some("toolu_01abc".into()),
                input: Some(serde_json::json!({"command": "ls"})),
            })
        );
    }

    #[test]
    fn pre_tool_use_unknown_tool_lands_in_other() {
        let mut e = raw("PreToolUse");
        e.tool_name = Some("mcp__github__list_issues".into());
        match e.to_lifecycle_event() {
            Some(LifecycleEvent::ToolCallStart { name, .. }) => {
                assert_eq!(name, Tool::Other("mcp__github__list_issues".into()));
            }
            other => panic!("expected ToolCallStart, got {other:?}"),
        }
    }

    #[test]
    fn pre_tool_use_interactive_becomes_needs_input() {
        for (name, expected) in [
            ("AskUserQuestion", Tool::AskUserQuestion),
            ("EnterPlanMode", Tool::EnterPlanMode),
            ("ExitPlanMode", Tool::ExitPlanMode),
        ] {
            let mut e = raw("PreToolUse");
            e.tool_name = Some(name.into());
            assert_eq!(
                e.to_lifecycle_event(),
                Some(LifecycleEvent::NeedsInput {
                    reason: NeedsInputReason::InteractiveTool { tool: expected }
                }),
                "tool {name} should map to NeedsInput"
            );
        }
    }

    #[test]
    fn post_tool_use_distinguishes_failure() {
        let mut ok = raw("PostToolUse");
        ok.tool_name = Some("Bash".into());
        ok.tool_use_id = Some("toolu_xyz".into());
        let mut fail = raw("PostToolUseFailure");
        fail.tool_name = Some("Bash".into());
        fail.tool_use_id = Some("toolu_xyz".into());

        assert_eq!(
            ok.to_lifecycle_event(),
            Some(LifecycleEvent::ToolCallEnd {
                name: Tool::Bash,
                tool_use_id: Some("toolu_xyz".into()),
                is_error: false,
            })
        );
        assert_eq!(
            fail.to_lifecycle_event(),
            Some(LifecycleEvent::ToolCallEnd {
                name: Tool::Bash,
                tool_use_id: Some("toolu_xyz".into()),
                is_error: true,
            })
        );
    }

    #[test]
    fn user_prompt_carries_prompt() {
        let mut e = raw("UserPromptSubmit");
        e.prompt = Some("hello".into());
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::PromptSubmit {
                prompt: Some("hello".into())
            })
        );
    }

    #[test]
    fn stop_to_working_end() {
        assert_eq!(
            raw("Stop").to_lifecycle_event(),
            Some(LifecycleEvent::WorkingEnd)
        );
    }

    #[test]
    fn subagent_to_child_session() {
        let mut start = raw("SubagentStart");
        start.agent_id = Some("a-1".into());
        start.agent_type = Some("explore".into());
        assert_eq!(
            start.to_lifecycle_event(),
            Some(LifecycleEvent::ChildSessionStart {
                id: Some("a-1".into()),
                role: Some("explore".into()),
            })
        );

        let mut stop = raw("SubagentStop");
        stop.agent_id = Some("a-1".into());
        assert_eq!(
            stop.to_lifecycle_event(),
            Some(LifecycleEvent::ChildSessionEnd {
                id: Some("a-1".into()),
            })
        );
    }

    #[test]
    fn session_start_carries_source() {
        let mut e = raw("SessionStart");
        e.source = Some("resume".into());
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::SessionStart {
                source: Some("resume".into())
            })
        );
    }

    #[test]
    fn session_end_carries_reason() {
        let mut end = raw("SessionEnd");
        end.reason = Some("clear".into());
        assert_eq!(
            end.to_lifecycle_event(),
            Some(LifecycleEvent::SessionEnd {
                reason: Some("clear".into())
            })
        );
    }

    #[test]
    fn pre_compact_carries_trigger() {
        let mut e = raw("PreCompact");
        e.trigger = Some("auto".into());
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::ContextCompactStart {
                trigger: Some("auto".into())
            })
        );
    }

    #[test]
    fn setup_to_setup_notification() {
        assert_eq!(
            raw("Setup").to_lifecycle_event(),
            Some(LifecycleEvent::Notification {
                message: None,
                kind: Some(NotificationKind::Setup),
            })
        );
    }

    #[test]
    fn notification_permission_to_needs_input() {
        let mut e = raw("Notification");
        e.notification_type = Some("permission_prompt".into());
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::NeedsInput {
                reason: NeedsInputReason::Notification {
                    kind: NotificationKind::PermissionPrompt,
                    label: None,
                }
            })
        );
    }

    #[test]
    fn notification_idle_to_idle() {
        let mut e = raw("Notification");
        e.notification_type = Some("idle_prompt".into());
        assert_eq!(e.to_lifecycle_event(), Some(LifecycleEvent::Idle));
    }

    #[test]
    fn notification_generic_passthrough() {
        let mut e = raw("Notification");
        e.notification_type = Some("info".into());
        e.message = Some("hi".into());
        assert_eq!(
            e.to_lifecycle_event(),
            Some(LifecycleEvent::Notification {
                message: Some("hi".into()),
                kind: Some(NotificationKind::Info),
            })
        );
    }

    #[test]
    fn unknown_event_returns_none() {
        let e = raw("NotARealEvent");
        assert_eq!(e.to_lifecycle_event(), None);
    }
}
