//! Coding-agent *harness* identification.
//!
//! "Harness" matches the term used in the user-facing config (see bead
//! `agent-tmux-manager-bva`'s TOML `[harness.<name>]` sections) and in
//! pi's three-axis model (`harness × provider × model`). It's the
//! wrapper program a user runs — Claude Code, pi, future others —
//! distinct from the LLM provider/model it talks to.
//!
//! Distinct from `AgentType` (which currently enumerates Claude
//! subagent roles like `Explore`/`Plan`/`GeneralPurpose` —
//! misleadingly named, see bead `agent-tmux-manager-cag` for the
//! planned rename to `ClaudeSubagentRole`). `Harness` is the actual
//! harness identity.
//!
//! Set at session-creation time by whichever adapter is feeding the
//! daemon (Claude adapter → `ClaudeCode`; pi adapter → `Pi`; pure
//! `/proc` discovery → `Unknown` until we wire bead `hfv`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Which coding-agent harness is driving this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    /// Claude Code (Anthropic).
    ClaudeCode,
    /// pi (<https://pi.dev/>) — provider-agnostic harness.
    Pi,
    /// Unknown harness — discovered via process scanning before any
    /// adapter event arrived, or a future harness we haven't tagged.
    #[default]
    Unknown,
}

impl Harness {
    /// Short tag suitable for a TUI badge (`[claude]`, `[pi]`, `[?]`).
    #[must_use]
    pub fn short_tag(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Pi => "pi",
            Self::Unknown => "?",
        }
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ClaudeCode => "Claude Code",
            Self::Pi => "pi",
            Self::Unknown => "unknown",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Harness::ClaudeCode).unwrap(),
            "\"claude_code\""
        );
        assert_eq!(serde_json::to_string(&Harness::Pi).unwrap(), "\"pi\"");
        assert_eq!(
            serde_json::to_string(&Harness::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    #[test]
    fn harness_default_is_unknown() {
        assert_eq!(Harness::default(), Harness::Unknown);
    }

    #[test]
    fn short_tag_matches_display_intent() {
        assert_eq!(Harness::ClaudeCode.short_tag(), "claude");
        assert_eq!(Harness::Pi.short_tag(), "pi");
        assert_eq!(Harness::Unknown.short_tag(), "?");
    }
}
