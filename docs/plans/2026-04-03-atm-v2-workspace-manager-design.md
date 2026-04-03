# ATM v2: Agent Tmux Manager — Design Document

**Date:** 2026-04-03
**Status:** Draft
**Supersedes:** team-visibility-design, projects-feature, preview-pane-design (absorbs all three)

## Summary

ATM evolves from "Agent Tmux Monitor" (passive read-only dashboard) to "Agent Tmux Manager" — a tmux-native workspace manager for Claude Code agents. ATM owns the tmux layout, agents run in real tmux panes, and ATM adds its own TUI sidebar/popup alongside them.

## Terminology

| Term | Meaning |
|---|---|
| **Agent** | A single Claude Code process running in a tmux pane |
| **Subagent** | A short-lived agent spawned within a parent session (via the `Agent` tool). Discovered via `SubagentStart`/`SubagentStop` hook events. May or may not have its own tmux pane. |
| **Team** | Specifically refers to **Claude Code Agent Teams** — the experimental multi-session coordination feature (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`). Teams have a lead, teammates, shared task list, and mailbox. Discovered via `~/.claude/teams/` config files. ATM does not invent its own team concept. |
| **Project** | A git repository root. Groups all agents working in any worktree of that repo. |
| **Worktree** | A specific git worktree path (including the main checkout). Groups agents by branch. |

## Architecture

```
┌─────────────────────────────────────┐
│              atm (CLI/TUI)          │
│                                     │
│  TUI mode:  ratatui sidebar/popup   │
│  CLI mode:  spawn, kill, layout,    │
│             toggle-panel, send      │
│                                     │
│  Owns: tmux interaction, layouts    │
│  Talks to: atmd (unix socket)       │
│            tmux (direct CLI calls)  │
└──────────────┬──────────────────────┘
               │ unix socket + HTTP/WS
┌──────────────▼──────────────────────┐
│              atmd (daemon)          │
│                                     │
│  Registry: sessions, projects,      │
│            worktrees, teams         │
│  Discovery: /proc scan, hooks       │
│  Serves: TUI clients, web UI       │
│                                     │
│  Owns: data, events, state          │
│  No tmux dependency                 │
└─────────────────────────────────────┘
```

### Component Responsibilities

**`atmd` (daemon)** — Pure data. Owns session registry, auto-discovery via /proc scanning, hook event processing (SubagentStart/Stop for team tracking), project/worktree resolution. Gains HTTP/WS server (axum) for future web UI. No tmux dependency.

**`atm` (CLI/TUI)** — All tmux interaction. TUI runs as a tmux pane (sidebar) or tmux popup. CLI subcommands for spawning, killing, layout management. Talks to atmd for session data, talks to tmux directly for pane manipulation.

**`atm-web`** (deferred) — Sycamore WASM web UI. Kanban/dashboard views. Connects to atmd via WebSocket. Not designed in detail yet.

### Crate Structure

- **`atm-core`** — Domain types. Extended with project/worktree/team fields.
- **`atm-protocol`** — Parsing. Unchanged.
- **`atm-tmux`** — New crate. Thin tmux CLI wrapper behind a `TmuxClient` trait. Layout engine.
- **`atmd`** — Daemon. Extended with project/worktree resolution, team tracking, HTTP/WS.
- **`atm`** — TUI + CLI. Rewritten tree view, new keybindings, CLI subcommands.
- **`atm-web`** — Web UI. Deferred placeholder.

## Data Model

### Grouping Hierarchy

```
Project (git repo root)
├── Worktree: main (/home/user/myapp)
│   ├── Agent: claude-a1b2 (opus, working)
│   └── Agent: claude-c3d4 (sonnet, idle)
├── Worktree: feature/auth (/home/user/myapp-auth)
│   ├── CC Team: my-refactor (lead: e5f6)
│   │   ├── Agent: claude-e5f6 (opus, working)     ← lead
│   │   ├── Agent: claude-g7h8 (sonnet, working)   ← teammate
│   │   └── Agent: claude-i9j0 (sonnet, idle)      ← teammate
│   └── Agent: claude-k1l2 (opus, waiting)         ← solo, no team
└── Worktree: fix/typo (/home/user/myapp-typo)
    └── Agent: claude-m3n4 (haiku, idle)
```

Progressive nesting: Project > Worktree > CC Agent Team > Agent. The team level only appears when a CC Agent Team is detected (via `~/.claude/teams/` config) or when `SubagentStart` events create parent-child links. Solo agents sit directly under their worktree.

**Conditional worktree grouping:** The worktree level is only shown when a project has more than one worktree. A project with a single checkout (just `main`) skips the worktree level entirely — agents appear directly under the project:

```
# Single worktree — flat
▼ myapp
  > ● 42% a1b2 opus
    ○     c3d4 snnt

# Multiple worktrees — nested
▼ myapp
  ▼ main
    > ● 42% a1b2 opus
  ▼ feature/auth
      ! 89% k1l2 opus
  ▸ fix/typo (1)
```

This avoids a useless nesting level in the common single-worktree case.

### How Grouping is Derived

- **Project** — resolved from `working_directory` by walking up to find `.git`. Agents in the same repo (any worktree) share a project.
- **Worktree** — the specific worktree path. Matched against `git worktree list` output. Agents in the main checkout and agents in worktrees get separate groups.
- **CC Agent Team** — discovered from `~/.claude/teams/{team-name}/config.json`. The config provides member list, roles (lead vs teammate), and pane IDs. For subagents (not teams), `SubagentStart` hook events create parent-child links using the event's `session_id` (parent) and `agent_id`/`agent_type` (child).

### New Fields on SessionDomain

```rust
pub project_root: Option<String>,
pub worktree_path: Option<String>,
pub worktree_branch: Option<String>,
pub parent_session_id: Option<SessionId>,
pub child_session_ids: Vec<SessionId>,
```

### SubagentStart/Stop Data Flow

Currently, `connection.rs:handle_hook_event` drops `agent_id`, `agent_type`, and `agent_transcript_path` from `RawHookEvent` before forwarding to the registry. These fields must be forwarded through `RegistryCommand::ApplyHookEvent` so the registry actor can establish parent-child links.

The `AgentType::from_subagent_type()` parser already exists in `agent.rs` but is never called in production — it will be wired in.

### CC Agent Teams Integration

Claude Code's Agent Teams feature (experimental, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`) creates separate Claude instances that coordinate via file-based messaging. When `teammateMode` is `"tmux"` or `"auto"` (inside tmux), CC creates one tmux pane per teammate.

**Key files ATM can read:**

- `~/.claude/teams/{team-name}/config.json` — team config with `members` array containing each teammate's name, agent ID, agent type, and **tmux pane ID**. Updated as teammates join/leave.
- `~/.claude/tasks/{team-name}/` — shared task list with status (pending/in-progress/completed) and dependencies.

**Discovery strategy for teams:**

1. Watch `~/.claude/teams/` for active team directories
2. Read `config.json` to get member list, pane IDs, and lead identity
3. Correlate with auto-discovered sessions via PID or pane ID matching
4. Read task list for status display in sidebar/web UI

**CC team hooks ATM can listen to:**

- `TeammateIdle` — teammate finished and is going idle
- `TaskCreated` — new task added to shared list
- `TaskCompleted` — task marked done

**Teams vs Subagents in the tree:**

| | Subagents | Agent Teams |
|---|---|---|
| Lifetime | Short-lived, within parent session | Long-lived, independent sessions |
| Discovery | `SubagentStart`/`SubagentStop` hook events | `~/.claude/teams/` config files |
| Communication | Report back to parent only | Mailbox system, peer-to-peer |
| Tree rendering | Nested under parent session | Grouped with lead marked, all members visible |
| Pane tracking | May not have own pane (in-process) | Each member has a tmux pane ID in config |

### Permission / Attention Alerts

CC fires a `Notification` hook event with `notification_type: "permission_prompt"` when waiting for user input. ATM already handles this (sets `SessionStatus::AttentionNeeded`, shows `!` icon with yellow background).

**Enhancements for v2:**

- **Bubble-up**: if any agent in a group needs attention, the group header (project/worktree/team) shows the attention indicator too. Collapsed groups must still surface alerts.
- **Tmux pane highlighting**: optionally set pane style on the agent's tmux pane (e.g., `tmux select-pane -t <pane> -P 'bg=#332200'`) to make the alert visible even when the ATM panel isn't focused.
- **Bell/activity integration**: trigger tmux bell or activity flag on panes needing attention, so tmux's own status bar can show alerts.

### SubagentStart-to-Child Correlation

When `SubagentStart` fires on the parent, it carries `agent_id`, `agent_type`, and `agent_transcript_path`. The `session_id` on the event is the **parent's** session ID. The challenge is correlating this with the child's session entry in the registry.

**Correlation strategy:**

1. On `SubagentStart`: store a pending correlation entry keyed by `agent_id` with the parent's `session_id` and `agent_type`.
2. When a new session appears (via discovery or `SessionStart` hook): check if its working directory, PID ancestry, or transcript path matches a pending correlation entry.
3. On match: set `parent_session_id` on the child, add child to parent's `child_session_ids`.
4. On `SubagentStop`: mark the correlation as ended. The child session may linger briefly (configurable) before removal.

**Buffering**: pending correlations have a TTL (e.g., 30 seconds). If no child session matches within the TTL, the entry expires. This handles cases where subagents run in-process and never appear as separate sessions.

## Tmux Integration

### Own Crate: `atm-tmux`

Thin wrapper over `Command::new("tmux")`. No external dependency on tmux crates.

```rust
#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("tmux command failed: {command} — {stderr}")]
    CommandFailed { command: String, stderr: String },
    #[error("tmux not found in PATH")]
    NotFound,
    #[error("failed to parse tmux output: {0}")]
    ParseError(String),
    #[error("pane not found: {0}")]
    PaneNotFound(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait TmuxClient: Send + Sync {
    /// Split a pane, returning the new pane ID (e.g., "%7").
    async fn split_window(&self, target: &str, size: &str, command: Option<&str>) -> Result<String, TmuxError>;
    /// Create a new window, returning the new pane ID.
    async fn new_window(&self, session: &str, command: Option<&str>) -> Result<String, TmuxError>;
    async fn kill_pane(&self, pane: &str) -> Result<(), TmuxError>;
    async fn resize_pane(&self, pane: &str, width: Option<u16>, height: Option<u16>) -> Result<(), TmuxError>;
    async fn send_keys(&self, pane: &str, keys: &str) -> Result<(), TmuxError>;
    async fn list_panes(&self) -> Result<Vec<PaneInfo>, TmuxError>;
    async fn display_popup(&self, width: &str, height: &str, command: &str) -> Result<(), TmuxError>;
    async fn select_pane(&self, pane: &str) -> Result<(), TmuxError>;
}
```

Real implementation uses `tokio::process::Command` (async). Mock implementation records calls for testing. Integration tests run against a real tmux server (available in CI via `apt install tmux`).

### Layout Templates

Defined at three levels (most specific wins):

1. **Global defaults** — `~/.config/atm/config.toml`
2. **Named built-ins** — `solo`, `pair`, `squad`, `grid`
3. **Per-project overrides** — `.atm/layout.toml` in project root

A layout defines a tree of slots. Each slot has a role, size, split direction, and optional children.

**Rust struct (the intended tree):**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    pub name: String,
    pub root: Slot,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Slot {
    pub role: SlotRole,
    pub size: String,                // "75%", "30%"
    pub direction: SplitDirection,   // "horizontal" or "vertical"
    #[serde(default)]
    pub children: Vec<Slot>,
    #[serde(default = "default_count")]
    pub count: u8,                   // for agent slots: initial count
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotRole { Agent, Editor, AtmPanel, Popup }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection { Horizontal, Vertical }
```

**TOML config** — uses inline `children` arrays to make the tree structure explicit:

```toml
[layout]
name = "my-standard"

# Root: horizontal split — 75% workspace | 25% ATM sidebar
[layout.root]
role = "workspace"
direction = "horizontal"
size = "100%"

[[layout.root.children]]
role = "workspace"
direction = "vertical"
size = "75%"

  # Workspace children: 70% editor on top, 30% agent below
  [[layout.root.children.children]]
  role = "editor"
  size = "70%"
  direction = "horizontal"

  [[layout.root.children.children]]
  role = "agent"
  size = "30%"
  direction = "horizontal"
  count = 1

[[layout.root.children]]
role = "atm_panel"
direction = "vertical"
size = "25%"
```

**Parsed tree for the above:**

```
root (workspace, horizontal, 100%)
├── workspace (vertical, 75%)
│   ├── editor (70%)
│   └── agent (30%, count=1)
└── atm_panel (vertical, 25%)
```

This matches the user's preferred layout: left side has editor (top 70%) + agent (bottom 30%), right side is the ATM sidebar (25% width).

**Slot roles:**
- `agent` — ATM spawns Claude Code here, monitors it
- `editor` — regular terminal pane, ATM doesn't touch it
- `atm-panel` — the monitoring TUI sidebar
- `popup` — no pane created; ATM panel uses `tmux display-popup`

When spawning new agents, ATM fills empty agent slots or splits existing agent panes. ATM tracks pane IDs, not positions — manual resizing is fine.

## Agent Spawning & Lifecycle

### Two Ways to Spawn

**Tmux hotkey (quick):** Bound in tmux config (e.g., `prefix + C-n`), calls `atm spawn`. Creates a new agent pane in the current project workspace with default model and working directory.

**ATM panel (deliberate):** Press `o` in the sidebar. Prompts for worktree, model, initial task. Creates pane and launches Claude Code with those settings.

### Spawn Mechanics

```
atm spawn
  → determines project context from cwd
  → calls tmux split-window / new-window with "claude --model <model>"
  → auto-discovery in atmd picks up the new process
  → daemon resolves project/worktree and links to group
```

### Lifecycle Actions (TUI Keybindings)

| Key     | Action                              | Confirmation? |
|---------|-------------------------------------|---------------|
| `dd`    | Kill agent + close pane (SIGTERM)   | Yes           |
| `I`     | Interrupt current turn (SIGINT)     | No            |
| `o`     | Spawn new agent                     | Prompt        |
| `s`     | Send text to agent                  | Input line    |
| `Enter` | Jump to agent's tmux pane           | No            |

### Auto-Discovery

Unchanged. If you manually run `claude` in any tmux pane, the daemon finds it via /proc scanning and adds it to the sidebar under the appropriate project group based on its working directory.

## TUI Sidebar

Runs as a thin tmux pane or popup. Shows the grouped agent tree.

### Sidebar Rendering

```
▼ myapp
  ▼ main
    > ● 42% a1b2 opus
      ○     c3d4 snnt
  ▼ feature/auth
    ▼ ● my-refactor (cc-team)
      > ★ 67% e5f6 opus  ← lead
        ●     g7h8 snnt
        ○     i9j0 snnt
      ! 89% k1l2 opus
  ▸ fix/typo (1)
```

Collapsed groups show agent count. CC Agent Team level only renders when a team is detected. The `★` icon marks the team lead. Navigation uses vim keys: `j`/`k` moves between visible rows, `h`/`l` collapses/expands group nodes, `gg`/`G`/counts work on the flattened visible list.

### Panel Modes

- **Sidebar pane** — persistent, part of the layout template (`role = "atm-panel"`)
- **Popup** — summoned/dismissed with hotkey, for layouts without sidebar space

Which mode is used is determined by the layout template.

## CLI Subcommands

The `atm` binary does double duty: no arguments launches the TUI, subcommands act as CLI client.

| Command                              | What it does                    |
|--------------------------------------|---------------------------------|
| `atm` (no args)                     | Launch TUI                      |
| `atm spawn [--worktree PATH] [--model MODEL]` | Spawn agent in current project |
| `atm toggle-panel`                   | Show/hide ATM sidebar pane      |
| `atm kill <session-id>`              | Kill an agent                   |
| `atm interrupt <session-id>`         | Send SIGINT                     |
| `atm send <session-id> <text>`       | Send text to agent pane         |
| `atm layout <name>`                  | Apply a layout template         |
| `atm list [--format FORMAT]`         | List agents for scripting       |
| `atm status`                         | One-line summary for tmux status bar |

### `atm list` — Scriptable Agent Listing

Machine-friendly output for piping into scripts, fzf, etc.

```bash
# Default: one agent per line, tab-separated
$ atm list
a1b2c3d4  working  42%  opus   myapp/main        %5
k1l2m3n4  waiting  89%  opus   myapp/feature/auth %8
c3d4e5f6  idle      0%  snnt   myapp/main        %6

# JSON for programmatic use
$ atm list --format json
[{"id":"a1b2c3d4","status":"working","context_pct":42,...}]

# Filter by project
$ atm list --project myapp

# Filter by status
$ atm list --status waiting

# Just IDs (for scripting)
$ atm list --format ids
a1b2c3d4
k1l2m3n4
c3d4e5f6
```

### `atm status` — Tmux Status Bar Integration

One-line summary designed for embedding in tmux's `status-right`:

```bash
$ atm status
3↑ 1! $4.32   # 3 agents running, 1 needs attention, $4.32 total cost
```

Format: `{active_count}↑ [{attention_count}!] ${total_cost}`

- `↑` count = agents in Working or Idle status
- `!` count = agents in AttentionNeeded status (omitted if 0)
- `$` = aggregate cost across all agents

Usage in tmux config:
```tmux
set -g status-right '#(atm status)'
set -g status-interval 5
```

### Tmux Bindings

ATM ships a config snippet users source in `.tmux.conf`:

```tmux
# ~/.tmux.conf
source-file ~/.config/atm/tmux-bindings.conf
```

**Contents of `tmux-bindings.conf`:**

```tmux
# ATM — Agent Tmux Manager bindings
# Source this in your .tmux.conf: source-file ~/.config/atm/tmux-bindings.conf

# Spawn a new Claude agent in current project
bind C-n run-shell "atm spawn"

# Toggle ATM sidebar panel
bind C-a run-shell "atm toggle-panel"

# ATM popup overlay (alternative to sidebar)
bind C-s display-popup -E -w 35% -h 100% -x 0 "atm"

# Status bar integration (add to status-right)
# set -g status-right '#(atm status) | %H:%M'
```

Three bindings: spawn, toggle sidebar, popup. All agent management (kill, jump, send) happens through the ATM TUI where you have selection context. Users can add their own bindings using `atm list` + fzf if they want shortcuts beyond these.

Users can override any binding in their own `.tmux.conf` after the `source-file` line.

## Web UI (Deferred)

Future work. Architecture supports it: `atmd` will have an axum HTTP/WS server alongside the Unix socket. Sycamore WASM frontend connects via WebSocket, receives the same `SessionEvent` broadcast stream. Kanban/dashboard views over agent activity. No detailed design needed yet.

## Testing Strategy

### Unit Tests (fast, no tmux)
- Data model: project/worktree resolution, tree building, grouping logic
- Layout config parsing: TOML in, layout struct out
- CLI argument parsing
- TUI tree navigation: pure logic, no terminal

### Integration Tests (mock tmux)
- `TmuxClient` trait with mock implementation that records calls
- Verify spawn/kill/layout commands produce correct tmux invocations
- TUI rendering via ratatui `TestBackend`

### Integration Tests (real tmux, CI)
- Spin up tmux server in GitHub Actions
- Create sessions, split panes, verify layout
- Test auto-discovery picks up spawned processes
- Test kill/interrupt signals reach target panes

### Daemon Tests
- Existing patterns: registry actor testable via handle, hook events with mock JSON
- New: project/worktree resolution, SubagentStart/Stop parent-child linking

## What Stays vs What Changes

### Stays (reuse as-is)
- `atm-core` domain types: `SessionId`, `Money`, `ContextUsage`, `SessionStatus`, `HookEventType`
- `atm-protocol` parsing: `RawHookEvent`, `RawStatusLine`
- `atmd` discovery: /proc scanning, `get_parent_pid`, `find_pane_for_pid`
- `atmd` registry actor pattern: mpsc command loop, broadcast channel
- Hook event pipeline: Unix socket, connection handler, event flow

### Extends
- `SessionDomain` — gains project/worktree/CC Agent Team fields
- `RegistryActor` — gains project/worktree resolution, SubagentStart/Stop handling, CC Agent Teams discovery
- `RegistryCommand` — gains new command variants for spawn feedback
- `atmd` — gains axum HTTP/WS server
- `atm` binary — gains CLI subcommands alongside TUI mode

### Rewrites
- `atm` TUI rendering — flat list becomes tree view with collapsible project/worktree/team groups
- `atm` navigation — tree cursor with vim-style expand/collapse (`h`/`l`) replaces flat `selected_index`
- `atm` keybindings — DFA gains operator states (`dd` kill, `o` spawn, `s` send)
- `SessionView` — gains grouping fields, parent/child info, CC Agent Team membership

### New
- `atm-tmux` crate — `TmuxClient` trait, real impl, mock, layout engine
- Layout template system — config parsing, tmux pane creation
- Tmux bindings config — ships with ATM
- `atm-web` crate — placeholder for future Sycamore web UI

## Error Handling Strategy

Failure modes at each component boundary:

### atm ↔ tmux (CLI calls)

| Failure | Handling |
|---|---|
| tmux not installed / not in PATH | `TmuxError::NotFound` — TUI shows "tmux not available", CLI subcommands exit with clear error |
| tmux command fails (pane doesn't exist, session dead) | `TmuxError::CommandFailed` — log stderr, surface to user as inline status message. Don't crash. |
| Pane ID stale (pane was closed externally) | `TmuxError::PaneNotFound` — remove stale pane reference, trigger re-discovery |
| tmux server not running | Same as NotFound — ATM can still run as a monitor (daemon doesn't need tmux) |

### atm ↔ atmd (unix socket)

| Failure | Handling |
|---|---|
| Daemon not running | TUI shows "connecting to daemon..." with retry. CLI subcommands: start daemon automatically or exit with instructions. |
| Daemon crashes mid-session | TUI reconnects with backoff. Session data rebuilds from discovery on daemon restart. |
| Socket permission denied | Clear error message with path to socket file |

### atmd ↔ Claude Code (hook events)

| Failure | Handling |
|---|---|
| Hook event with unknown event type | Log warning, skip. Don't crash. (Already handled.) |
| Hook event for non-existent session | Create session from hook if PID available. (Already handled.) |
| SubagentStart with no matching child session | Buffered correlation with TTL. Expires silently after 30s. |
| `~/.claude/teams/` config malformed | Log warning, skip team. Individual agents still visible via discovery. |

### atmd ↔ filesystem (discovery, team config)

| Failure | Handling |
|---|---|
| `/proc` not available (non-Linux) | Discovery disabled, rely solely on hook events for session registration |
| `git` not in PATH | Project/worktree grouping disabled, agents shown ungrouped |
| `~/.claude/teams/` not readable | Team discovery disabled, subagent tracking still works via hooks |

**General principle:** Every failure degrades gracefully. ATM never crashes on external failures. Features that depend on unavailable resources are disabled, and the rest continues working.

## Deferred / Out of Scope

Requirements from prior designs that are explicitly dropped or deferred:

| Requirement | Source | Disposition | Rationale |
|---|---|---|---|
| Preview pane (three-zone detail view) | preview-pane-design | **Deferred to post-v2** | The sidebar is thin; detail views can use `Enter` to jump to the actual pane. Web UI can provide richer detail later. |
| Rule-based activity summary | preview-pane-design | **Dropped** | Over-engineered for the sidebar format. Status icon + activity label is sufficient. |
| Auto-derived session names | agent-tmux-monitor-ub2 | **Deferred** | Nice-to-have but not blocking. Sessions identified by short ID + model for now. |
| Pluggable task context rendering | preview-pane-design | **Dropped** | Premature abstraction. Revisit when web UI is built. |
| Web UI (Sycamore) implementation | This design | **Deferred** | Architecture supports it (HTTP/WS endpoint in daemon), but TUI is the priority. Web UI designed and built separately later. |
| Watchdog / auto-compact / auto-restart | amux inspiration | **Out of scope** | ATM is a workspace manager, not an unattended orchestrator. May revisit later. |
| Task board / kanban in TUI | amux inspiration | **Out of scope for TUI** | Deferred to web UI. TUI can show CC Agent Teams task status read-only. |
| Inter-agent REST API | amux inspiration | **Out of scope** | CC Agent Teams has its own mailbox system. ATM doesn't need to add another. |

## Open Questions

### Carried Forward

1. **Subagent pane visibility** — Do subagents (spawned via the `Agent` tool, not CC Agent Teams) get their own tmux pane? Or do they run in-process within the parent? If in-process, they'll never appear as separate sessions via discovery. We need to test this with real Claude Code to determine what ATM can actually observe.

2. **SubagentStop lifecycle** — When a subagent stops, should its row linger in the tree? For how long? Should it fade out, or disappear immediately? CC Agent Teams teammates have explicit shutdown, but subagents just stop.

3. **CC Agent Teams stability** — The feature is experimental with known limitations (no session resumption, task status lag, orphaned tmux sessions). How defensively should ATM code against these?

### New

4. **Team config watching** — Should ATM poll `~/.claude/teams/` on an interval, use `inotify`/`fanotify`, or rely on hook events (`TeammateIdle`, `TaskCreated`, `TaskCompleted`) for real-time updates? Polling is simpler but slower. `inotify` adds a Linux dependency. Hook events may not cover all changes.

5. **Layout interaction with CC Agent Teams** — When CC's teammate mode creates panes, it has its own layout ideas. If ATM also has a layout template, they'll conflict. Options: (a) ATM detects CC-created team panes and incorporates them into its tree without trying to control layout, (b) ATM provides the layout and CC's `teammateMode` is set to `in-process` (ATM manages pane creation), (c) user chooses per-project.

6. **Multiple projects in one tmux session** — If a user has agents from different git repos in the same tmux window, the tree shows multiple project roots. Is this a supported use case or should ATM encourage one project per tmux session/window?

7. **Tmux session vs window vs pane granularity** — Does ATM create a tmux *session* per project, a *window* per project within a shared session, or just manage panes within whatever window the user is in? This affects layout templates and hotkey behavior.

8. **`atm spawn` default model** — Should `atm spawn` default to the model from the most recent agent in the same project, a user-configured default, or always prompt?

## Implementation Phases

1. **Data model + SubagentStart/Stop wiring** — add project/worktree/CC Agent Team fields to SessionDomain/SessionView, forward subagent hook data through the pipeline, wire AgentType parsing
2. **`atm-tmux` crate** — TmuxClient trait, real impl, mock, basic tests
3. **CLI subcommands** — `atm spawn`, `atm kill`, `atm interrupt`, `atm send`, `atm list`, `atm layout`, `atm status`
4. **TUI rewrite** — split into three sub-phases:
   - **4a. Tree model** — build the grouping data structure (Project > Worktree > CC Agent Team > Agent) from flat session list. Conditional worktree nesting. Pure logic, fully unit-testable.
   - **4b. Tree rendering** — replace flat `List` widget with tree-aware rendering. Indentation, collapse indicators (`▼`/`▸`), agent count on collapsed groups, attention bubble-up on group headers. `★` for CC Agent Team leads.
   - **4c. Keybindings** — extend DFA with tree navigation (`h`/`l` expand/collapse) and operator states (`dd` kill, `o` spawn, `s` send, `I` interrupt, `x` close). Confirmation dialogs for destructive actions.
5. **Layout templates** — config parsing, preset layouts (solo/pair/squad/grid), per-project overrides, tmux-bindings.conf
6. **CC Agent Teams integration** — watch `~/.claude/teams/`, correlate with discovered sessions, show team structure in tree, read shared task list
7. **HTTP/WS endpoint** — axum server in daemon (web UI itself deferred)
