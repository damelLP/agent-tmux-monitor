//! pi adapter for ATM.
//!
//! All pi-specific knowledge — the event vocabulary, the wire payload
//! shape, and the translation into vendor-neutral
//! `atm_core::LifecycleEvent` — lives in this crate. Symmetric with
//! `atm_claude_adapter`.
//!
//! Pi is a coding agent (<https://pi.dev/>, npm package
//! `@mariozechner/pi-coding-agent`) that exposes a TypeScript extension
//! API. ATM's pi adapter is a TypeScript extension (`pi-atm`,
//! tracked by bead `agent-tmux-manager-6dx`) that subscribes to pi's
//! `pi.on(eventName, handler)` events and forwards them to atmd over
//! the existing Unix socket. The Rust types in this crate describe the
//! wire payloads that extension produces and the translation into
//! `LifecycleEvent`.
//!
//! ## Event vocabulary
//!
//! 28 pi events: 26 declared in pi's
//! `dist/core/extensions/types.d.ts` plus 2 undeclared but real
//! (`tool_call`, `tool_result`). See `event::PiEventType` for the
//! enumeration and `translate.rs` for per-variant lifecycle mapping.
//! Originally captured by the `agent-tmux-manager-9dn` spike under
//! `extensions/pi-spike/`.
//!
//! ## Three-axis vendor model
//!
//! Pi is *provider-agnostic* — a single pi session can switch between
//! Anthropic, OpenAI, and other providers via `model_select`. The
//! three axes are:
//!
//! - **harness** = `pi`
//! - **provider** = `anthropic` / `openai-codex` / etc.
//! - **model** = `claude-sonnet-4-6` / `gpt-5.5` / etc.
//!
//! Provider+model changes ride [`atm_core::LifecycleEvent::ProviderModelChange`].
//!
//! ## NeedsInput is extension-mediated
//!
//! Pi has *no* dedicated permission-prompt event. Permission gating
//! happens *inside* an extension's `tool_call` handler via
//! `ctx.ui.select(...)`. ATM's pi adapter therefore registers a
//! `tool_call` handler and synthesizes
//! [`atm_core::NeedsInputReason::PermissionGate`] when the gate is
//! reached. A passive observer cannot detect this state; only an active
//! extension can.

pub mod event;
pub mod translate;
pub mod wire;

pub use event::PiEventType;
pub use wire::RawPiEvent;
