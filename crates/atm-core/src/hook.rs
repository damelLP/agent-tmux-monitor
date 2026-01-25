//! Hook event types from Claude Code.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Types of hook events from Claude Code.
///
/// Based on validated Claude Code hook documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventType {
    /// Before a tool is executed (can be used for permission checks)
    PreToolUse,

    /// After a tool completes execution
    PostToolUse,

    /// When a new session starts (not currently used, future)
    SessionStart,

    /// When a session ends (not currently used, future)
    SessionEnd,

    /// Notification event (informational)
    Notification,
}

impl HookEventType {
    /// Returns true if this is a pre-execution event.
    pub fn is_pre_event(&self) -> bool {
        matches!(self, Self::PreToolUse | Self::SessionStart)
    }

    /// Returns true if this is a post-execution event.
    pub fn is_post_event(&self) -> bool {
        matches!(self, Self::PostToolUse | Self::SessionEnd)
    }

    /// Parses from a hook event name string.
    pub fn from_event_name(name: &str) -> Option<Self> {
        match name {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "SessionStart" => Some(Self::SessionStart),
            "SessionEnd" => Some(Self::SessionEnd),
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
            Self::SessionStart => write!(f, "SessionStart"),
            Self::SessionEnd => write!(f, "SessionEnd"),
            Self::Notification => write!(f, "Notification"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_parsing() {
        assert_eq!(
            HookEventType::from_event_name("PreToolUse"),
            Some(HookEventType::PreToolUse)
        );
        assert_eq!(HookEventType::from_event_name("Unknown"), None);
    }

    #[test]
    fn test_hook_event_classification() {
        assert!(HookEventType::PreToolUse.is_pre_event());
        assert!(HookEventType::PostToolUse.is_post_event());
        assert!(!HookEventType::PreToolUse.is_post_event());
    }
}
