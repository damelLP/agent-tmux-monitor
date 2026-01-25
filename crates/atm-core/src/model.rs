//! Claude model identification and metadata.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Claude model identifier.
///
/// Parsed from status line JSON: `model.id` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Model {
    /// Claude Opus 4.5 (claude-opus-4-5-20251101)
    #[serde(rename = "claude-opus-4-5-20251101")]
    Opus45,

    /// Claude Sonnet 4 (claude-sonnet-4-20250514)
    #[serde(rename = "claude-sonnet-4-20250514")]
    Sonnet4,

    /// Claude Haiku 3.5 (claude-3-5-haiku-20241022)
    #[serde(rename = "claude-3-5-haiku-20241022")]
    Haiku35,

    /// Claude Sonnet 3.5 v2 (claude-3-5-sonnet-20241022)
    #[serde(rename = "claude-3-5-sonnet-20241022")]
    Sonnet35V2,

    /// Unknown or future model
    #[serde(other)]
    Unknown,
}

impl Model {
    /// Returns a human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Opus45 => "Opus 4.5",
            Self::Sonnet4 => "Sonnet 4",
            Self::Haiku35 => "Haiku 3.5",
            Self::Sonnet35V2 => "Sonnet 3.5 v2",
            Self::Unknown => "Unknown",
        }
    }

    /// Returns the context window size for this model.
    pub fn context_window_size(&self) -> u32 {
        match self {
            Self::Opus45 => 200_000,
            Self::Sonnet4 => 200_000,
            Self::Haiku35 => 200_000,
            Self::Sonnet35V2 => 200_000,
            Self::Unknown => 200_000, // Default assumption
        }
    }

    /// Returns approximate cost per million input tokens (USD).
    pub fn input_cost_per_million(&self) -> f64 {
        match self {
            Self::Opus45 => 15.00,
            Self::Sonnet4 => 3.00,
            Self::Haiku35 => 0.80,
            Self::Sonnet35V2 => 3.00,
            Self::Unknown => 3.00, // Conservative default
        }
    }

    /// Returns approximate cost per million output tokens (USD).
    pub fn output_cost_per_million(&self) -> f64 {
        match self {
            Self::Opus45 => 75.00,
            Self::Sonnet4 => 15.00,
            Self::Haiku35 => 4.00,
            Self::Sonnet35V2 => 15.00,
            Self::Unknown => 15.00, // Conservative default
        }
    }

    /// Parses a model from its string ID.
    pub fn from_id(id: &str) -> Self {
        match id {
            "claude-opus-4-5-20251101" => Self::Opus45,
            "claude-sonnet-4-20250514" => Self::Sonnet4,
            "claude-3-5-haiku-20241022" => Self::Haiku35,
            "claude-3-5-sonnet-20241022" => Self::Sonnet35V2,
            _ => Self::Unknown,
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_parsing() {
        let model: Model = serde_json::from_str("\"claude-opus-4-5-20251101\"").unwrap();
        assert_eq!(model, Model::Opus45);
        assert_eq!(model.display_name(), "Opus 4.5");
    }

    #[test]
    fn test_model_unknown() {
        let model: Model = serde_json::from_str("\"claude-future-model\"").unwrap();
        assert_eq!(model, Model::Unknown);
    }

    #[test]
    fn test_model_from_id() {
        assert_eq!(Model::from_id("claude-opus-4-5-20251101"), Model::Opus45);
        assert_eq!(Model::from_id("unknown-model"), Model::Unknown);
    }
}
