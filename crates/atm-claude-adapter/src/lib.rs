//! Claude Code adapter for ATM.
//!
//! All Claude-specific knowledge — the raw event vocabulary, the wire
//! payload shape, and the translation into vendor-neutral
//! `atm_core::LifecycleEvent` — lives in this crate. The daemon
//! (`atmd`) calls into the adapter at the connection boundary; nothing
//! in `atm-core` or `atm-protocol` references Claude.
//!
//! ## Layers
//!
//! - [`event`] — `ClaudeEventType` enum (the 12 Claude hook event names)
//! - [`wire`] — `RawHookEvent` struct (deserialized JSON Claude sends
//!   on stdin to the hook script)
//! - [`translate`] — translation from raw event to `LifecycleEvent`

pub mod event;
pub mod translate;
pub mod wire;

pub use event::ClaudeEventType;
pub use wire::RawHookEvent;
