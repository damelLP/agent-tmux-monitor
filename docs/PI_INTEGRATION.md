# pi Integration Findings

> **Status:** ✅ SPIKE COMPLETE — populated from real traces captured under
> `/tmp/atm-pi-spike-*.jsonl` on 2026-04-30 and 2026-05-02.
> Tracks beads issue **agent-tmux-manager-9dn**.

Mirrors the existing `CLAUDE_CODE_INTEGRATION.md` (in
`.worktrees/feature-agent-teams/integration-test/`) but for pi
(<https://pi.dev/>, npm package `@mariozechner/pi-coding-agent` v0.70.6).

## Executive Summary

pi is the first non-Claude coding agent ATM will integrate with. Unlike
Claude Code's `~/.claude/settings.json` hooks (12 named events, JSON-on-stdin,
shell-script invocation), pi exposes:

- A **TypeScript extension API** (`@mariozechner/pi-coding-agent`) with a
  `pi.on(eventName, handler)` event subscription model. **26 declared event
  names; 19 of those plus 2 undeclared (`tool_call`, `tool_result`) were
  observed in the real-session trace.**
- A **`--mode rpc` JSON-RPC interface** over stdin/stdout. Bidirectional —
  pi accepts inbound commands too, not only emits events.
- **Tailable JSONL session files** at
  `~/.pi/agent/sessions/<encoded-cwd>/<ISO>_<uuid>.jsonl` — a *second*
  observation channel that does not require running an extension.
- **Rich `ExtensionContext`** including `ctx.isIdle()`,
  `ctx.hasPendingMessages()`, `ctx.getContextUsage()`, `ctx.abort()` —
  state inference does not require parsing the event stream.

### Key Findings

1. **`tool_call` and `tool_result` events are real** — they fire alongside
   `tool_execution_start`/`tool_execution_end`, even though they're not in
   the `dist/core/extensions/types.d.ts` event-name union. The
   `permission-gate.ts` example uses `tool_call`. Treat as first-class.
2. **Permission prompts have no dedicated event.** Pi gates synchronously
   inside an extension's `tool_call` handler via
   `ctx.ui.select(...)`. A *passive observer* cannot detect "waiting on
   user permission". An *active extension* can — and ATM's pi adapter
   must register a `tool_call` handler if it wants to surface
   `NeedsInput` accurately.
3. **Session files are tailable JSONL.** ATM gets a low-cost read-only
   observation channel (no extension required) — useful for
   crash recovery, post-hoc analysis, and observing sessions where ATM's
   extension wasn't installed at start-time. Format:
   first line `{type:"session",version:3,id,timestamp,cwd}`, then events
   prefixed with `type` (e.g. `model_change`, `message`, …).
4. **Cost data is structured.** `context.messages[].usage.cost.{input,
   output, cacheRead, cacheWrite, total}` is emitted on every `context`
   event — ATM does not need to scrape a status line. Bonus: pi reports
   `provider`/`model`/`api`, so we know which model+provider is in use
   (the trace caught pi running `gpt-5.5` via `openai-codex`). Important:
   **pi is provider-agnostic** — it is not an OpenAI-only or Anthropic-only
   harness. The vendor model needs to capture `(harness, provider, model)`
   as three separate axes, not collapse them.
5. **Working/Stopped boundaries are crisp; Idle is too.**
   `agent_start`/`agent_end` bracket the working state; `session_shutdown`
   carries `reason: "quit" | …` distinguishing graceful exit from crash;
   `ctx.isIdle()` plus `!ctx.hasPendingMessages()` is a direct signal.

## Detection / Discovery

### Process detection (`/proc/<pid>/comm` and `cmdline`)

`pi` is installed as a node-shebanged JS at
`~/.nvm/versions/node/v25.2.1/bin/pi`. Empirically `comm` is unreliable
across node versions/wrappers (often reports `node` rather than `pi`),
so `cmdline` is the authoritative match:

```bash
# Most reliable — matches both the bin path and globally installed package
pgrep -fn 'pi-coding-agent|/bin/pi$'
```

For ATM's process scanner, prefer `cmdline` substring match over `comm`.

### Session storage layout

```
~/.pi/
├── agent/
│   ├── auth.json
│   ├── claude-bridge.json
│   ├── settings.json
│   ├── bin/
│   ├── extensions/                 # installed extension packages
│   ├── extensions.disabled/
│   └── sessions/
│       └── <encoded-cwd>/          # e.g. --home-damel-git-agent-tmux-manager--
│           └── <ISO>_<uuid>.jsonl  # one file per session, JSONL
├── tasks/
└── teams/
```

`<encoded-cwd>` is the absolute working dir with `/` replaced by `-` and
wrapped in `--…--`.

Session JSONL format (line-delimited, append-only):

```jsonl
{"type":"session","version":3,"id":"019db4b6-7164-70b9-a7bd-9bb250d1a768","timestamp":"2026-04-22T10:22:28.708Z","cwd":"/home/damel/git/agent-tmux-manager"}
{"type":"model_change","id":"…", …}
{"type":"message","id":"…","role":"user", …}
…
```

The header is a `session` record (note `version:3` — pi versions its
on-disk schema). Tailable read-only by ATM without an extension.

## Extension API

### Invocation

```bash
# Local TS file (no install needed — pi loads via @mariozechner/jiti)
pi --extension /abs/path/to/event-trace.ts

# Or installed package
pi install npm:@atm/pi-hook
```

### Subscription shape

```ts
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";

export default function (pi: ExtensionAPI) {
    pi.on("session_start", async (event, ctx) => { /* ... */ });
}
```

### Observed event vocabulary

Source: `dist/core/extensions/types.d.ts` (declared) + spike traces
(observed). "Observed?" column reflects the
`pid1226148-2026-05-02` and `pid799362-2026-04-30` runs combined.

| Event                          | Observed? | Payload fields                                              | Notes |
|--------------------------------|-----------|-------------------------------------------------------------|-------|
| `resources_discover`           | ✅        | `cwd, reason, type`                                         | Fires once at startup |
| `session_start`                | ✅        | `reason, type` (`reason: "startup"`)                        | |
| `session_before_switch`        | ❌        | `[unobserved]`                                              | No session switch in trace |
| `session_before_fork`          | ❌        | `[unobserved]`                                              | No fork in trace |
| `session_before_compact`       | ❌        | `[unobserved]`                                              | No compaction triggered (short sessions); Claude analog: `PreCompact` |
| `session_compact`              | ❌        | `[unobserved]`                                              | Pair with `session_before_compact` |
| `session_shutdown`             | ✅        | `reason, type` (`reason: "quit"`)                           | Claude analog: `SessionEnd` |
| `session_before_tree`          | ❌        | `[unobserved]`                                              | No tree navigation in trace |
| `session_tree`                 | ❌        | `[unobserved]`                                              | |
| `context`                      | ✅        | `messages[], type`                                          | Full conversation snapshot; messages carry `usage.cost`, `provider`, `model`, `api` |
| `before_provider_request`      | ✅        | `payload, type`                                             | `payload` includes `model`, `store`, `instructions`, full request body |
| `after_provider_response`      | ✅        | `headers, status, type`                                     | HTTP-level — useful for rate-limit diagnostics |
| `before_agent_start`           | ✅        | `prompt, systemPrompt, systemPromptOptions, type`           | Fires before first turn only |
| `agent_start`                  | ✅        | `type`                                                      | **ATM `Working` begins** |
| `agent_end`                    | ✅        | `messages, type`                                            | Final messages; **ATM `Working` ends** |
| `turn_start`                   | ✅        | `timestamp, turnIndex, type`                                | |
| `turn_end`                     | ✅        | `message, toolResults, turnIndex, type`                     | |
| `message_start`                | ✅        | `message, type`                                             | |
| `message_update`               | ✅        | `assistantMessageEvent, message, type`                      | High-frequency; rate-limit before forwarding to ATM core |
| `message_end`                  | ✅        | `message, type`                                             | |
| `tool_execution_start`         | ✅        | `args, toolCallId, toolName, type`                          | Claude analog: `PreToolUse` |
| `tool_execution_update`        | ❌        | `[unobserved]`                                              | Claude has no equivalent |
| `tool_execution_end`           | ✅        | `isError, result, toolCallId, toolName, type`               | Claude analogs: `PostToolUse` / `PostToolUseFailure` |
| `tool_call`                    | ✅        | `input, toolCallId, toolName, type`                         | **Undeclared in type union but real.** Permission-gate hook point — handler can return `{block: true, reason}` |
| `tool_result`                  | ✅        | `content, details?, input, isError, toolCallId, toolName, type` | Undeclared in type union but real |
| `model_select`                 | ❌        | `[unobserved]`                                              | No model switch in trace |
| `user_bash`                    | ❌        | `[unobserved]`                                              | No `!bash` invocations in trace; Claude has no equivalent |
| `input`                        | ✅        | `source, text, type` (`source: "interactive"`)              | Claude analog: `UserPromptSubmit` |

**Firing order (one-turn-with-tools session):**

```
session_start
  resources_discover
  input
  before_agent_start
  agent_start
    turn_start
      context
      before_provider_request
      message_start … message_end
      after_provider_response
      message_update*           (streaming)
      tool_execution_start, tool_call
      tool_result, tool_execution_end
    turn_end
    [next turn iterates: turn_start … turn_end]
  agent_end
session_shutdown
```

### ExtensionContext fields available to handlers

Confirmed from `dist/core/extensions/types.d.ts`:

| Field / method                  | Type                                     | Use for ATM                        |
|---------------------------------|------------------------------------------|------------------------------------|
| `ctx.cwd`                       | `string`                                 | working directory                  |
| `ctx.hasUI`                     | `boolean`                                | false in print/RPC mode            |
| `ctx.ui`                        | `ExtensionUIContext`                     | `ctx.ui.select(prompt, choices)` etc — used for permission prompts |
| `ctx.sessionManager`            | `ReadonlySessionManager`                 | session id, parent, branch info    |
| `ctx.getModel()`                | `Model \| undefined`                     | current model id + provider        |
| `ctx.isIdle()`                  | `boolean`                                | direct `Idle` state signal         |
| `ctx.hasPendingMessages()`      | `boolean`                                | distinguishes idle vs queued       |
| `ctx.getSignal()`               | `AbortSignal \| undefined`               | observe cancellation               |
| `ctx.abort()`                   | `void`                                   | cooperative cancel — relevant for future `atm interrupt` |
| `ctx.getContextUsage()`         | `ContextUsage \| undefined`              | context tokens (analog to Claude statusLine) |

## Lifecycle State Inference

ATM tracks four states per session: `Working`, `Idle`, `NeedsInput`,
`Stopped`. Mapping rules from pi events, validated by the spike:

| ATM state    | Inference rule                                                                                 | Confidence |
|--------------|------------------------------------------------------------------------------------------------|------------|
| `Working`    | `agent_start` fired and `agent_end` not yet fired                                              | 🟢 HIGH    |
| `Idle`       | `agent_end` fired, `ctx.isIdle() === true`, `!ctx.hasPendingMessages()`                        | 🟢 HIGH    |
| `NeedsInput` | Adapter registers `tool_call` handler; if it calls `ctx.ui.select(...)` it emits synthetic `NeedsInput`; clears on resolution | 🟡 MEDIUM (requires active participation; passive observers cannot detect this state) |
| `Stopped`    | `session_shutdown` fired (any `reason`), OR pi process gone from `/proc`                       | 🟢 HIGH    |

**`NeedsInput` is the load-bearing finding.** Pi's permission gating is
implemented inside extensions, not as a discrete event — see
`examples/extensions/permission-gate.ts`:

```ts
pi.on("tool_call", async (event, ctx) => {
    if (event.toolName !== "bash") return undefined;
    const command = event.input.command as string;
    if (isDangerous(command)) {
        if (!ctx.hasUI) return { block: true, reason: "…" };
        const choice = await ctx.ui.select(`⚠️ Allow?`, ["Yes", "No"]);
        if (choice !== "Yes") return { block: true, reason: "Blocked by user" };
    }
    return undefined;
});
```

Implications for `@atm/pi-hook` (T5):

- The adapter **must** register a `tool_call` handler to be in the gate
  loop. If it doesn't, ATM has no way to know pi is awaiting permission.
- The adapter forwards a synthetic `NeedsInput` lifecycle event to ATM
  core, then awaits the user's choice via `ctx.ui` (or via ATM's TUI if
  we route through the daemon).
- File-tail observation alone is **insufficient** for `NeedsInput`. The
  on-disk JSONL captures completed turns, not paused-mid-turn UI state.

## Inbound RPC

Pi accepts `--mode rpc` over stdin/stdout. Documented in pi's
`docs/rpc.md` (1407 lines — out of scope to enumerate here; review when
T8/T9 begin).

Future capability for ATM (Phase-N, **out of scope** for this epic):

- `atm send <id> "fix tests"` could send a structured user message via
  RPC instead of injecting keystrokes into the tmux pane.
- `atm interrupt <id>` could call `ctx.abort()` via RPC instead of Ctrl+C.

Calling out so the abstraction (`LifecycleEvent`, `VendorAdapter`)
doesn't lock into "events flow one way".

## Updated data model implications for ATM

Confirmed expected impact on `atm-core`:

- **`HookEventType` (Claude-specific) → `LifecycleEvent` (vendor-neutral)**
  with both Claude and pi adapters translating into it. See beads
  `agent-tmux-manager-cy1` (T6). The minimal vendor-neutral set the
  spike justifies:
  `SessionStart`, `WorkingStart`, `WorkingEnd`, `Idle`, `NeedsInput`,
  `NeedsInputResolved`, `ToolCallStart`, `ToolCallEnd`, `MessageDelta`
  (rate-limited), `ContextUpdate {tokens, cost}`, `SessionEnd {reason}`.
- **`AgentVendor` enum** added alongside today's `AgentType` (which is
  renamed to `ClaudeSubagentRole`). See `agent-tmux-manager-cag` (T2).
  But: vendor is two-axis — `harness` (claude-code | pi | …) AND
  `provider/model` (anthropic/claude-sonnet-4-6, openai-codex/gpt-5.5,
  …). Pi is provider-agnostic; one pi session can use Anthropic in turn
  N and OpenAI in turn N+1 if user invokes `model_select`. The `Session`
  record needs `harness`, `provider`, `model` as three distinct fields.
- **`VendorMetadata` tagged enum** for fields that don't generalize:
  - pi: `session_tree_path`, `branch_id`, `session_jsonl_path`
  - claude: `transcript_path`, `cost_usd`, `subagent_role`
  See `agent-tmux-manager-kdu` (T7).
- **Two observation channels for pi.** The adapter should support both:
  - **Live (extension)** — required for `NeedsInput` detection and for
    inbound RPC.
  - **Tail (JSONL)** — fallback when the extension isn't installed in a
    running session, or for replay/audit. Same `LifecycleEvent` output.

## Surprises

Items that contradicted (or extended beyond) pre-spike assumptions:

- **`tool_call` / `tool_result` are real events but absent from the
  declared type union** in `types.d.ts`. The pre-spike doc flagged this
  as a `[TBD — confirm]`; confirmed real.
- **No dedicated permission event.** Pre-spike speculation in the doc
  asked "does pi expose a permission-prompt signal, or is it
  client-side?" Answer: client-side, inside `tool_call` handlers. Has
  meaningful adapter-architecture consequences (see Lifecycle table
  above).
- **Sessions are plain JSONL on disk** with a versioned header — a much
  simpler observation surface than expected, and stronger than Claude's
  (Claude only feeds hooks via stdin; ATM cannot tail a Claude transcript
  without running a hook). Adapter design should expose the
  channel-of-observation as a parameter.
- **`context` event carries provider/model/cost on every emit.** ATM
  status-line scraping (the Claude path) is unnecessary on the pi side.
- **Pi is provider-agnostic.** Trace captured `provider: "openai-codex",
  model: "gpt-5.5"` — the assumption that "pi == Anthropic" was wrong.
  Vendor model needs `harness` and `provider/model` as separate axes.
- **`session_shutdown.reason` is structured** (observed value: `"quit"`),
  so ATM can distinguish graceful exit from crash without process-level
  inference.

## Validation Evidence

Sample JSONL lines from `/tmp/atm-pi-spike-pid1226148-2026-05-02T08-55-56-043Z.jsonl`:

```jsonl
{"event":"session_start","payload":{"type":"session_start","reason":"startup"}}
{"event":"input","payload":{"type":"input","text":"Use ls to list /tmp …","source":"interactive"}}
{"event":"agent_start","payload":{"type":"agent_start"}}
{"event":"context","payload":{"type":"context","messages":[{"role":"user", …},{"role":"assistant","content":[{"type":"toolCall", …}], "api":"openai-codex-responses","provider":"openai-codex","model":"gpt-5.5","usage":{"input":1088,"output":55,"totalTokens":1143,"cost":{"input":0.00544,"output":0.00165,"total":0.00709}}, "stopReason":"toolUse"}]}}
{"event":"tool_execution_start","payload":{"type":"tool_execution_start","toolName":"ls","toolCallId":"call_QPdBI…","args":{…}}}
{"event":"tool_call","payload":{"type":"tool_call","toolName":"ls","toolCallId":"call_QPdBI…","input":{"path":"/tmp","limit":3}}}
{"event":"tool_result","payload":{"type":"tool_result","toolName":"ls","toolCallId":"call_QPdBI…","content":[{"type":"text","text":".font-unix/\n.ICE-unix/\n.X0-lock\n…"}],"isError":false}}
{"event":"tool_execution_end","payload":{"type":"tool_execution_end","toolName":"ls","toolCallId":"call_QPdBI…","isError":false,"result":{…}}}
{"event":"agent_end","payload":{"type":"agent_end","messages":[…]}}
{"event":"session_shutdown","payload":{"type":"session_shutdown","reason":"quit"}}
```

Trace summaries (firing counts) under
`/tmp/atm-pi-spike-pid*-summary.txt`.

## Confidence Level

🟢 **HIGH** — every event in the firing-order chain that ATM relies on
for `Working`/`Idle`/`Stopped` was observed at least once with payloads
matching expectations. `NeedsInput` is medium-confidence by design (pi's
permission model is structurally extension-mediated, not event-driven);
this is now an architectural input to T5/T6, not an unknown.

Unobserved events (`session_before_*`, `session_compact`,
`session_tree`, `model_select`, `user_bash`, `tool_execution_update`)
are exercised by user actions the spike sessions didn't perform. They
are documented in the table for completeness and can be validated
incrementally as ATM's pi adapter grows.

## Next Steps

1. ✅ Run the spike — done (2 substantive traces captured)
2. ✅ Document findings — this doc
3. → Close `agent-tmux-manager-9dn`
4. → Unblocks: T2 (`cag`), T5 (`6dx`), T6 (`cy1`), T7 (`kdu`)
5. → Open follow-up beads issue if/when an `actualize` pass uncovers
   that T5/T6 hit a vocabulary gap (e.g. compaction, model_select)
   not covered by these two traces.
