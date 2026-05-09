# @atm/pi-hook

pi extension that forwards lifecycle events to the ATM daemon (`atmd`)
so pi sessions show up in the ATM TUI alongside Claude Code sessions.

This is the **runtime** counterpart to the Rust-side `atm-pi-adapter`
crate. The adapter knows how to translate pi events into vendor-neutral
`LifecycleEvent`; this extension is what subscribes to pi's event API
at runtime and feeds those events through to the daemon.

Tracks beads `agent-tmux-manager-6dx`.

## How it fits

```
                  ┌─────────────────────────────────────┐
                  │  pi (real, running)                  │
                  │  ┌────────────────────────────────┐ │
                  │  │  @atm/pi-hook (this extension) │ │
                  │  └─────────────┬──────────────────┘ │
                  └────────────────┼─────────────────────┘
                                   │ pi event JSON
                              Unix socket /tmp/atm.sock
                                   │ MessageType::PiEvent
                                   ▼
                       ┌──────────────────────┐
                       │  atmd                │
                       │   ↓                  │
                       │  RawPiEvent          │
                       │   ↓ (atm-pi-adapter) │
                       │  LifecycleEvent      │
                       │   ↓                  │
                       │  Session registry    │
                       │   ↓                  │
                       │  TUI broadcast       │
                       └──────────────────────┘
                                   │
                                   ▼
                              atm TUI
```

## Install (development)

For local development, run pi with the extension as a file path:

```sh
pi --extension /abs/path/to/agent-tmux-manager/extensions/pi/atm-pi-hook.ts
```

pi loads `.ts` files directly via `@mariozechner/jiti`, so no compile
step is needed.

## Install (published)

Once published to npm:

```sh
pi install npm:@atm/pi-hook
```

(`atm setup` will eventually do this automatically once bead
`agent-tmux-manager-yjj` lands. For now installation is manual.)

## Configuration

| Env var       | Default          | Purpose                                   |
|---------------|------------------|-------------------------------------------|
| `ATM_SOCKET`  | `/tmp/atm.sock`  | Daemon socket path                        |
| `ATM_DEBUG`   | unset            | When `1`, append debug to `/tmp/atm-pi-hook.log` |

If `atmd` is not running (socket missing), the extension is a no-op —
pi keeps working unaffected.

## Forwarded events

Only events that drive `LifecycleEvent` translation are forwarded
(keeping wire traffic low):

| pi event                | Reason forwarded                                      |
|-------------------------|-------------------------------------------------------|
| `session_start`         | → `LifecycleEvent::SessionStart`                      |
| `session_shutdown`      | → `LifecycleEvent::SessionEnd`                        |
| `agent_start`           | → `LifecycleEvent::WorkingStart`                      |
| `agent_end`             | → `LifecycleEvent::WorkingEnd`                        |
| `tool_call`             | → `LifecycleEvent::ToolCallStart` (or `NeedsInput` if extension synthesizes a permission gate — see below) |
| `tool_execution_end`    | → `LifecycleEvent::ToolCallEnd`                       |
| `model_select`          | → `LifecycleEvent::ProviderModelChange`               |
| `input` (interactive)   | → `LifecycleEvent::PromptSubmit`                      |
| `session_before_compact`| → `LifecycleEvent::ContextCompactStart`               |

High-frequency events (`message_update`, `before_provider_request`,
`turn_start`/`turn_end`, `context`) and duplicates of forwarded events
(`tool_execution_start`, `tool_result`) are **not** subscribed — the
daemon-side adapter would suppress them anyway.

## Permission-gate (NeedsInput) — follow-up

Pi has no dedicated permission-prompt event. Permission gating happens
*inside* an extension's `tool_call` handler via `ctx.ui.select(...)`.
This v1 of the extension does **not** synthesize the
`needs_user_input: true` flag — meaning the daemon will see
`tool_call` as a regular `ToolCallStart`, not `NeedsInput`.

Adding permission-gate participation requires this extension to *be*
the gate (call `ctx.ui.select` itself before forwarding), which only
makes sense once `atm send/reply` (issue
[#yet-to-be-filed]) can drive responses back through the daemon.

## Live test

1. Make sure atmd is running: `pgrep -af atmd` or start one with
   `atmd start -d`.
2. In one terminal launch atm: `atm`.
3. In another terminal start pi with this extension:
   ```sh
   pi --extension /home/damel/git/agent-tmux-manager/extensions/pi/atm-pi-hook.ts
   ```
4. Ask pi to do something. Watch the new session appear in the atm TUI
   with status flipping between `working` and `idle` as pi works.
5. Quit pi. The session disappears from the TUI.
