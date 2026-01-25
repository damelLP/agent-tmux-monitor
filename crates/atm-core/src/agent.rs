//! Agent type identification.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of Claude Code agent.
///
/// Claude Code spawns different agent types for different purposes:
/// - Main agent for general tasks
/// - Specialized subagents for exploration, planning, code review
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// General-purpose main agent
    GeneralPurpose,

    /// Task/explore subagent for file exploration
    Explore,

    /// Planning subagent for task breakdown
    Plan,

    /// Code review subagent
    CodeReviewer,

    /// File search/analysis subagent
    FileSearch,

    /// Custom or unknown agent type
    Custom(String),
}

impl AgentType {
    /// Returns a short identifier for display.
    pub fn short_name(&self) -> &str {
        match self {
            Self::GeneralPurpose => "main",
            Self::Explore => "explore",
            Self::Plan => "plan",
            Self::CodeReviewer => "review",
            Self::FileSearch => "search",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// Returns a descriptive label for the agent type.
    pub fn label(&self) -> &str {
        match self {
            Self::GeneralPurpose => "General Purpose",
            Self::Explore => "Explorer",
            Self::Plan => "Planner",
            Self::CodeReviewer => "Code Reviewer",
            Self::FileSearch => "File Search",
            Self::Custom(_) => "Custom",
        }
    }

    /// Parses an agent type from a subagent_type string.
    pub fn from_subagent_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "general-purpose" | "general_purpose" => Self::GeneralPurpose,
            "explore" | "explorer" => Self::Explore,
            "plan" | "planner" => Self::Plan,
            "code-reviewer" | "code_reviewer" | "codereview" => Self::CodeReviewer,
            "file-search" | "file_search" | "filesearch" => Self::FileSearch,
            _ => Self::Custom(s.to_string()),
        }
    }
}

impl Default for AgentType {
    fn default() -> Self {
        Self::GeneralPurpose
    }
}

impl fmt::Display for AgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_parsing() {
        assert_eq!(
            AgentType::from_subagent_type("general-purpose"),
            AgentType::GeneralPurpose
        );
        assert_eq!(AgentType::from_subagent_type("explore"), AgentType::Explore);
        assert_eq!(
            AgentType::from_subagent_type("custom-agent"),
            AgentType::Custom("custom-agent".to_string())
        );
    }

    #[test]
    fn test_agent_type_short_name() {
        assert_eq!(AgentType::GeneralPurpose.short_name(), "main");
        assert_eq!(AgentType::Explore.short_name(), "explore");
    }
}
