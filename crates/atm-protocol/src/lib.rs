//! ATM Protocol - Wire protocol for daemon communication
//!
//! This crate provides message types and parsing for communication
//! between Claude Code status scripts and the daemon, and between
//! the daemon and TUI clients.

pub mod message;
pub mod parse;
pub mod version;

pub use message::{ClientMessage, DaemonMessage, MessageType};
pub use parse::{RawContextWindow, RawCost, RawHookEvent, RawModel, RawStatusLine, RawWorkspace};
pub use version::ProtocolVersion;
