# pi-spike — event-trace extension

Throwaway pi extension that logs every event pi fires during a real session,
so we can design ATM's vendor-neutral event model against ground truth.

Tracks beads issue **agent-tmux-manager-9dn**. Once findings are written into
`docs/PI_INTEGRATION.md`, this whole directory can stay as the basis for the
real `extensions/pi/` adapter (T5) — the event-subscription scaffolding is
identical.

## Run

```bash
pi --extension /home/damel/git/agent-tmux-manager/.worktrees/multi-vendor-agents/extensions/pi-spike/event-trace.ts
```

Then do real work for ~30–60 minutes. The extension is read-only and
non-blocking; it should not change pi's behaviour.

## Output

- `/tmp/atm-pi-spike-<pid>-<startedAt>.jsonl` — one JSONL line per event,
  with full payload (truncated to 2000 chars per string) and a snapshot of
  `ExtensionContext` (cwd, model, isIdle, hasPendingMessages, contextUsage)
- `/tmp/atm-pi-spike-<pid>-<startedAt>-summary.txt` — written on
  `session_shutdown` (and on `process.exit` as a fallback) with event
  counts, first/last timestamps, and a list of events declared by pi but
  never observed in this session

Override defaults:

| Env var                | Default | Purpose                                  |
|------------------------|---------|------------------------------------------|
| `ATM_SPIKE_LOG_DIR`    | `/tmp`  | Where to write the JSONL + summary       |
| `ATM_SPIKE_MAX_STRING` | `2000`  | Per-string truncation cap inside payloads|

## What we're trying to learn

Per `docs/PI_INTEGRATION.md` (open questions section):

1. **Event vocabulary.** Which of the 26 declared events actually fire during
   normal use? Which are exotic (only fire under fork/compact/handoff)?
2. **Lifecycle coverage.** Can we infer ATM's four states from events alone?
   - `Working` ⇐ `agent_start` … `agent_end`?
   - `Idle` ⇐ `agent_end` + `ctx.isIdle() === true`?
   - `NeedsInput` ⇐ ??? (does pi expose any "waiting for permission" signal,
     or is interactive-tool detection the only path?)
   - `Stopped` ⇐ `session_shutdown`?
3. **Payload shapes.** What fields appear on each event? Particularly
   `tool_execution_*` (tool name, args, result), `before_provider_request`
   (model, token estimate, cost), `context` (token usage).
4. **Session storage path.** Where under `~/.pi/agent/` does pi persist
   sessions? What format? Can ATM's discovery tail those files for sessions
   that don't have the extension installed?
5. **`/proc` detection.** What is the `comm` value for a running pi process?
   (Likely `pi` or `node`, since pi runs under node.) Validate before
   building `PiDiscoverer` (T3).

## Findings checklist

After running, fill in these sections of `docs/PI_INTEGRATION.md`:

- [ ] Event vocabulary table (event → frequency → observed payload schema)
- [ ] Lifecycle state inference rules (events → ATM state)
- [ ] Session storage location + format
- [ ] /proc comm detection notes
- [ ] Inbound RPC capability summary (relevant for future structured
      `atm send` / `atm interrupt` over RPC instead of tmux keystrokes)
- [ ] Surprises — anything that contradicted assumptions in the design doc

## Cleanup

Outputs go to `/tmp/`, so they vanish on reboot. To clean explicitly:

```bash
rm /tmp/atm-pi-spike-*.{jsonl,txt}
```
