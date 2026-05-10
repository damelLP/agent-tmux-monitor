# pi-spike — event-trace extension

Throwaway pi extension that logs every event pi fires during a real
session. Originally written to design ATM's vendor-neutral event
model against ground truth (bead `agent-tmux-manager-9dn`); kept in
the tree as the executable trace that informed `atm-pi-adapter`.

The shipping adapter is `crates/atm-pi-adapter/` (vocabulary in
`event::PiEventType`, lifecycle mapping in `translate.rs`). If pi
adds events later, re-run this spike to capture them before deciding
how to integrate.

## Run

```bash
pi --extension /abs/path/to/extensions/pi-spike/event-trace.ts
```

Then do real work for a while. The extension is read-only and
non-blocking; it should not change pi's behaviour.

## Output

- `/tmp/atm-pi-spike-<pid>-<startedAt>.jsonl` — one JSONL line per
  event, with full payload (truncated to 2000 chars per string) and a
  snapshot of `ExtensionContext` (cwd, model, isIdle,
  hasPendingMessages, contextUsage)
- `/tmp/atm-pi-spike-<pid>-<startedAt>-summary.txt` — written on
  `session_shutdown` (and on `process.exit` as a fallback) with event
  counts, first/last timestamps, and a list of events declared by pi
  but never observed in this session

Override defaults:

| Env var                | Default | Purpose                                  |
|------------------------|---------|------------------------------------------|
| `ATM_SPIKE_LOG_DIR`    | `/tmp`  | Where to write the JSONL + summary       |
| `ATM_SPIKE_MAX_STRING` | `2000`  | Per-string truncation cap inside payloads|

## Cleanup

Outputs go to `/tmp/`, so they vanish on reboot. To clean explicitly:

```bash
rm /tmp/atm-pi-spike-*.{jsonl,txt}
```
