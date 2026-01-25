# Agent Tmux Monitor - Project Roadmap

**Version:** 1.0
**Last Updated:** 2026-01-23
**Timeline:** 6-8 weeks to production-ready MVP
**Status:** Planning Complete - Ready for Implementation

---

## Executive Summary

Agent Tmux Monitor is an htop-style monitoring system for Claude Code agents running across tmux sessions. This roadmap outlines a disciplined, phased approach to deliver a production-ready MVP in 6-8 weeks, incorporating comprehensive risk mitigation strategies based on critical architecture review.

**Quick Stats:**
- **Architecture:** Daemon-based with Unix socket IPC, Rust + ratatui TUI, bash integration
- **Core Components:** 3 (daemon, TUI, shell integration)
- **Implementation Phases:** 5 (MVP achieved by Phase 3)
- **Risk Level:** LOW (after Week 1 validation)
- **Team Size:** 1-2 developers (Rust experience recommended)

---

## Table of Contents

1. [Project Vision](#project-vision)
2. [Timeline Overview](#timeline-overview)
3. [Weekly Breakdown](#weekly-breakdown)
4. [Critical Path & Blockers](#critical-path--blockers)
5. [Key Decision Points](#key-decision-points)
6. [Risk Mitigation](#risk-mitigation)
7. [What Changed from Original Plan](#what-changed-from-original-plan)
8. [Success Criteria](#success-criteria)
9. [Team Recommendations](#team-recommendations)
10. [Deployment Checklist](#deployment-checklist)
11. [Future Enhancements](#future-enhancements)

---

## Project Vision

### The Problem

Developers running multiple Claude Code agents across tmux sessions have no centralized visibility into:
- Which sessions have active agents
- Context usage approaching limits
- Agents waiting for permission
- Cost accumulation across sessions
- Real-time agent activity

### The Solution

Agent Tmux Monitor provides a tmux-integrated, real-time monitoring dashboard that:
- Shows all Claude Code sessions at a glance (htop-style interface)
- Displays context usage with visual progress bars
- Highlights agents waiting for permission
- Enables instant navigation to any session
- Tracks cost and resource consumption
- Updates in real-time (300ms polling)

### Core Value Proposition

> **"Know what your AI agents are doing, before they hit limits"**

Agent Tmux Monitor gives developers situational awareness across their entire Claude Code fleet, preventing context exhaustion surprises and enabling efficient multi-session workflows.

---

## Timeline Overview

### 6-8 Week Plan

```
┌─────────────────────────────────────────────────────────────────┐
│                      ATM TIMELINE                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│ Week 1:  Planning & Validation            🔴 CRITICAL          │
│          └─ Validate assumptions, complete specs               │
│                                                                 │
│ Week 2:  Phase 1 - Core Daemon            🟢 Implementation    │
│          └─ Unix socket server, registry, protocol            │
│                                                                 │
│ Week 3:  Phase 1 - Daemon Polish          🟢 Implementation    │
│          └─ Error handling, resource limits, tests            │
│                                                                 │
│ Week 4:  Phase 2 - Basic TUI              🟢 Implementation    │
│          └─ Connection, rendering, navigation                 │
│                                                                 │
│ Week 5:  Phase 3 - Integration            🟡 Integration       │
│          └─ Shell scripts, installation, E2E tests            │
│                                           ✅ MVP MILESTONE      │
│                                                                 │
│ Week 6:  Phase 4 - Rich UI                🔵 Polish           │
│          └─ Split-pane, tmux jumping, themes                  │
│                                                                 │
│ Week 7:  Phase 5 - Advanced Features      🔵 Polish           │
│          └─ Filtering, actions, config system                 │
│                                                                 │
│ Week 8:  Final Polish & Documentation     🎯 Delivery          │
│          └─ Bug fixes, docs, release prep                     │
│                                           🚀 PRODUCTION READY  │
└─────────────────────────────────────────────────────────────────┘
```

### Milestone Definitions

- **Week 1 Complete:** All architectural uncertainties resolved, specs complete
- **Week 3 Complete:** Daemon can track sessions, handle errors gracefully
- **Week 5 Complete:** ✅ **MVP** - Full integration with Claude Code, basic monitoring works
- **Week 8 Complete:** 🚀 **Production Ready** - Polished UI, comprehensive features, documentation

---

## Weekly Breakdown

### Week 1: Planning & Validation (3-4 days)
**Status:** 🔴 BLOCKING - Must complete before implementation
**Detailed Plan:** [`WEEK_1_PLANNING_AND_VALIDATION.md`](atm-plan/WEEK_1_PLANNING_AND_VALIDATION.md)

#### Objectives
Resolve all CRITICAL and HIGH priority issues identified in architecture review before writing code.

#### Key Tasks

**Day 1-2: Claude Code Integration Validation**
- Set up test environment with `.claude/settings.json` and `.claude/hooks.json`
- Create test scripts to verify:
  - Status line component receives JSON every 300ms
  - Hooks execute on permission requests and tool usage
  - Session IDs are available or can be generated
- Document actual JSON structures vs. assumptions
- **Output:** `CLAUDE_CODE_INTEGRATION.md` with confirmed integration details

**Day 2: Architecture Specifications**
- Define concurrency model (Actor pattern with message passing)
- Specify error handling strategy (error types, retry policies, graceful degradation)
- Document resource limits (max sessions: 100, max clients: 10, retention policies)
- Add protocol versioning to all message schemas
- **Outputs:**
  - `docs/CONCURRENCY_MODEL.md`
  - `docs/ERROR_HANDLING.md`
  - `docs/RESOURCE_LIMITS.md`
  - `docs/PROTOCOL_VERSIONING.md`

**Day 3-4: Domain Model Design**
- Refactor data structures to separate domain logic from infrastructure
- Replace stringly-typed fields with type-safe enums (`AgentType`, `Model`)
- Create value objects (`Money`, `MessageCount`, `ContextUsage`)
- Design domain services (`SessionAggregator`, `CostCalculator`)
- **Output:** `docs/DOMAIN_MODEL.md` with clean architecture specs

#### Success Criteria
- ✅ Claude Code integration validated with actual test data
- ✅ All CRITICAL architectural gaps documented and resolved
- ✅ Domain model follows DDD/Clean Architecture principles
- ✅ Confidence level: HIGH to proceed to Week 2

#### Risks & Mitigation
**Risk:** Claude Code integration doesn't work as assumed
**Impact:** Could block Phase 3 entirely
**Mitigation:** Validate in Week 1, explore alternatives if needed (polling logs, etc.)

---

### Week 2-3: Phase 1 - Core Daemon
**Status:** 🟢 Foundation Implementation
**Duration:** 2 weeks

#### Week 2: Core Implementation

**Objectives:**
- Implement Unix socket server with tokio
- Build session registry using Actor pattern
- Implement protocol message handling
- Add process monitoring (CPU, memory)

**Deliverables:**
- `atmd` binary that starts/stops as daemon
- Session registry with concurrent-safe access
- Protocol serialization/deserialization
- Unit tests for core components

**Key Implementation Details:**

```rust
// Actor-based registry (no locks, no race conditions)
pub struct RegistryActor {
    receiver: mpsc::Receiver<RegistryCommand>,
    registry: HashMap<SessionId, SessionDomain>,
    event_publisher: broadcast::Sender<SessionEvent>,
}

// Message protocol with versioning
pub struct ClientMessage {
    protocol_version: String,  // "1.0"
    message_type: MessageType,
    // ...
}
```

#### Week 3: Robustness & Polish

**Objectives:**
- Implement error handling throughout daemon
- Add resource limits and cleanup logic
- Implement session lifecycle state machine
- Add comprehensive testing

**Deliverables:**
- Graceful error recovery (daemon never crashes)
- Automatic stale session cleanup (90s threshold)
- Resource limits enforced (100 sessions, 10 clients max)
- Integration tests with mock clients

**Key Features:**
- Exponential backoff for reconnections
- Change detection (only broadcast when values change)
- Throttling (max 10 broadcasts/sec)
- Memory monitoring and alerts

#### Success Criteria
- ✅ Daemon runs stably for 24+ hours
- ✅ Handles 10+ concurrent sessions without issues
- ✅ Gracefully handles malformed messages
- ✅ Memory usage < 10MB under normal load
- ✅ All unit and integration tests pass

---

### Week 4: Phase 2 - Basic TUI
**Status:** 🟢 Implementation

#### Objectives
Create minimal TUI that connects to daemon and displays sessions in a navigable list.

#### Key Tasks

**TUI Infrastructure:**
- Set up ratatui + crossterm
- Implement daemon client connection with retry logic
- Create event loop (keyboard input + daemon updates)
- Implement graceful shutdown

**UI Components:**
- Simple list view of sessions
- Keyboard navigation (j/k, arrow keys)
- Session selection highlighting
- Connection status indicator

**Error Handling:**
- Show "Daemon Disconnected" banner when daemon down
- Auto-reconnect with exponential backoff (1s → 30s)
- Graceful exit on Ctrl+C

#### Deliverables
- `atm` binary that launches TUI
- List view showing session ID, agent type, context %
- Keyboard navigation working smoothly
- Connection resilience (survives daemon restarts)

#### Success Criteria
- ✅ TUI renders in < 100ms
- ✅ Navigation feels responsive (< 50ms input latency)
- ✅ Reconnects to daemon automatically
- ✅ No crashes during normal operation

---

### Week 5: Phase 3 - Shell Integration
**Status:** 🟡 Critical Integration Phase
**Milestone:** ✅ **MVP ACHIEVED**

#### Objectives
Connect Claude Code sessions to daemon via bash scripts, complete end-to-end testing.

#### Key Tasks

**Status Line Component (`atm-status.sh`):**
```bash
#!/bin/bash
# Reads status JSON from stdin, sends to daemon socket
# Non-blocking with 100ms timeout
# Always exits 0 (never breaks Claude Code)

SOCKET="/tmp/atm.sock"

# Quick check - daemon available?
[ ! -S "$SOCKET" ] && exit 0

# Read status line JSON
while IFS= read -r line; do
    # Extract fields, format for daemon
    # Send with timeout, silent failure
    echo "$message" | timeout 0.1 nc -U "$SOCKET" 2>/dev/null || exit 0
done

exit 0
```

**Hooks Component (`atm-hooks.sh`):**
```bash
#!/bin/bash
# Handles PermissionRequest, PreToolUse, PostToolUse hooks
# Sends events to daemon for status tracking

SOCKET="/tmp/atm.sock"
[ ! -S "$SOCKET" ] && exit 0

# Read hook event JSON
IFS= read -r line

# Parse and forward to daemon
echo "$line" | timeout 0.1 nc -U "$SOCKET" 2>/dev/null || exit 0
exit 0
```

**Installation Script (`install.sh`):**
- Copy binaries to `/usr/local/bin/`
- Install bash scripts to PATH
- Create config directories
- Generate example configs

**Documentation:**
- Installation guide
- Configuration examples
- Troubleshooting common issues

#### Deliverables
- Working shell scripts tested with real Claude Code sessions
- Installation script for easy setup
- End-to-end integration tests
- User documentation

#### Success Criteria
- ✅ **MVP COMPLETE:** Full integration with Claude Code
- ✅ Real-time session updates appear in TUI
- ✅ Status updates don't slow down Claude Code
- ✅ Daemon down doesn't break Claude Code
- ✅ Installation takes < 5 minutes

#### Go/No-Go Decision Point
**Question:** Does the integration work smoothly enough for real use?

**Go Criteria:**
- Status updates appear reliably within 1 second
- No noticeable impact on Claude Code performance
- Bash scripts handle errors gracefully

**If No-Go:**
- Debug integration issues (Week 5 extended)
- Consider alternative integration methods
- Re-evaluate architecture if fundamental issues

---

### Week 6: Phase 4 - Rich UI
**Status:** 🔵 Polish & Enhancement

#### Objectives
Implement full split-pane interface with detailed session view and tmux navigation.

#### Key Features

**Split-Pane Layout (40/60):**
```
┌─ Overview ───────────┬─ Selected: main:0 → editor ───────┐
│ Total: 4 sessions    │ Agent: general-purpose            │
│ Active: 3 | Wait: 1  │ Model: claude-sonnet-4-5          │
│                      │                                   │
│ Global Context: 36%  │ Status: ● THINKING                │
│ ██████░░░░░░░░░      │ Context: ████████████░░░░░ 45%   │
│                      │                                   │
│ Total Cost: $0.23    │ Cost: $0.08                       │
│                      │ CPU: 15.2% | Mem: 234 MB          │
│ ┌──────────────────┐ │                                   │
│ │● main:0  | 45%   │ │                                   │
│ │▶ work:1  | 23%   │ │                                   │
│ │⏸ proj:2  | 12%  │ │                                   │
│ │● dev:3   | 67%   │ │                                   │
│ └──────────────────┘ │                                   │
└──────────────────────┴───────────────────────────────────┘
 q: quit | j/k: nav | Enter: jump | Tab: toggle
```

**Overview Panel:**
- Summary statistics (total sessions, active/waiting counts)
- Global context usage bar (aggregated across all sessions)
- Total cost display

**Session List Panel:**
- Status icons (● active, ▶ thinking, ⏸ waiting)
- Session location (tmux session:window)
- Context percentage
- Time since started
- Highlight selected session

**Detail Panel:**
- Agent type and model
- Current status and action
- Context usage progress bar with token counts
- Session timing (started, last active)
- Process stats (CPU, memory)
- Cost accumulation

**Tmux Integration:**
- Detect current tmux session/window
- "Jump to session" on Enter key → executes `tmux switch-client`
- Show current session indicator

**Visual Polish:**
- Color themes (status-based: green/yellow/red)
- Smooth animations (no flicker)
- Responsive layout (terminal resize handling)

#### Deliverables
- Full split-pane UI matching design mockup
- Real-time updates with < 100ms latency
- Tmux session jumping working
- Polished visual appearance

#### Success Criteria
- ✅ UI matches design specification
- ✅ Real-time updates feel smooth
- ✅ Tmux jumping works reliably
- ✅ No rendering artifacts or flicker

---

### Week 7-8: Phase 5 - Advanced Features & Final Polish
**Status:** 🔵 Enhancement
**Milestone:** 🚀 **PRODUCTION READY**

#### Week 7: Advanced Features

**Filtering & Search:**
- Filter by agent type (general-purpose, explore, etc.)
- Filter by status (active, waiting, idle)
- Search by session name
- Keyboard shortcuts for quick filters

**Configuration System:**
- `~/.config/atm/config.json` for settings
- Custom keybindings
- Color theme selection
- Refresh rate control

**Help System:**
- Help screen (press `?`)
- Keybinding reference
- Feature overview
- Quick start guide

**Optional Session Actions (if time permits):**
- Kill session (send SIGTERM) - with confirmation
- View session details in full screen

#### Week 8: Final Polish & Release Prep

**Documentation:**
- User guide (installation, usage, troubleshooting)
- Architecture documentation
- Contributing guide
- README with screenshots

**Testing:**
- Manual testing with 10+ concurrent sessions
- Stress testing (daemon stability over days)
- Error scenario testing (daemon crashes, network issues)
- Installation testing on clean system

**Bug Fixes:**
- Address any issues found in testing
- Performance optimization if needed
- UI refinements based on feedback

**Release Preparation:**
- Version tagging
- Release notes
- Binary packaging
- Installation verification

#### Deliverables
- Complete documentation
- Stable release binaries
- Installation packages
- Production-ready system

#### Success Criteria
- ✅ All tests pass
- ✅ Documentation complete
- ✅ Installation tested on clean system
- ✅ Performance meets targets (see [Success Criteria](#success-criteria))
- ✅ **Ready for production use**

---

## Critical Path & Blockers

### Critical Path Items

These items MUST complete successfully for the project to succeed:

1. **Week 1: Claude Code Integration Validation** (BLOCKING)
   - Cannot proceed without confirming integration points work
   - Alternative approaches needed if assumptions fail

2. **Week 2-3: Daemon Stability** (BLOCKING)
   - TUI and shell components depend on working daemon
   - Must handle concurrency and errors correctly

3. **Week 5: Bash Script Integration** (BLOCKING)
   - MVP depends on successful Claude Code → Daemon connection
   - Bash scripts must not impact Claude Code performance

### Known Blockers

| Blocker | Severity | Week | Mitigation |
|---------|----------|------|------------|
| Claude Code integration assumptions invalid | CRITICAL | 1 | Validate early, explore alternatives (log polling, etc.) |
| Rust learning curve slows development | HIGH | 2-5 | Use pair programming, allocate extra time for Week 2 |
| Bash scripts cause Claude Code hangs | HIGH | 5 | Non-blocking socket writes, always exit 0, timeouts |
| tmux integration doesn't work as expected | MEDIUM | 6 | Fallback to manual navigation, skip auto-jump feature |

### Dependency Chain

```
Week 1 (Validation)
    ↓
Week 2-3 (Daemon)
    ↓
Week 4 (TUI) ──┐
               ├──→ Week 6 (Rich UI)
Week 5 (Shell) ┘      ↓
               Week 7-8 (Polish)
```

**Sequential Dependencies:**
- TUI requires working daemon
- Shell integration requires working daemon
- Rich UI requires basic TUI
- Advanced features require shell integration

**Parallel Opportunities:**
- Week 4 (TUI) and Week 5 (Shell) can partially overlap
- Documentation can start in Week 6
- Testing can begin as soon as each component is complete

---

## Key Decision Points

### Decision Point 1: End of Week 1
**Question:** Are Claude Code integration assumptions valid?

**Options:**
- **Go:** Integration confirmed → Proceed to Week 2 with HIGH confidence
- **Adjust:** Minor deviations found → Update protocol spec, proceed with MEDIUM confidence
- **No-Go:** Major issues found → Explore alternative integration (1-2 week delay)

**Decision Makers:** Technical lead, architect

---

### Decision Point 2: End of Week 3
**Question:** Is daemon stable enough for integration?

**Criteria:**
- Runs for 24+ hours without crash
- Handles 10+ sessions
- Memory usage < 10MB
- All tests pass

**Options:**
- **Go:** All criteria met → Proceed to Phase 2/3
- **Extend:** Close but needs work → Add 2-3 days to Week 3
- **Redesign:** Fundamental issues → Revisit architecture (major delay)

**Decision Makers:** Technical lead

---

### Decision Point 3: End of Week 5 (MVP Gate)
**Question:** Is MVP functional enough for real-world use?

**Criteria:**
- Integration works with real Claude Code sessions
- Status updates appear within 1 second
- No noticeable Claude Code performance impact
- Daemon resilient to errors

**Options:**
- **Go:** MVP complete → Proceed to polish phases
- **Extend:** Integration issues → Add 1 week to debug/fix
- **Pivot:** Can't achieve acceptable integration → Consider read-only log parsing approach

**Decision Makers:** Product owner, technical lead

---

### Decision Point 4: End of Week 7
**Question:** Ship now or add Week 8?

**Criteria:**
- All core features working
- Documentation sufficient
- No critical bugs

**Options:**
- **Ship:** Good enough for v1.0 → Skip Week 8, release now
- **Polish:** Need refinement → Execute Week 8 for production readiness
- **Extend:** Major issues → Add 1-2 weeks before release

**Decision Makers:** Product owner

---

## Risk Mitigation

### Lessons from Critique Report

The comprehensive architecture review identified several critical issues that could derail the project. This roadmap incorporates mitigations for all high-severity risks.

#### Risk 1: Unvalidated Integration Assumptions
**Original Issue:** Plan assumes Claude Code status line and hooks work as specified, but never validated
**Severity:** CRITICAL (could block Phase 3)
**Critique Finding:** Lines 324-332 of implementation plan

**Mitigation:**
- ✅ **Week 1 dedicated to validation** - Test actual Claude Code integration before writing code
- ✅ **Document actual behavior** - Record real JSON structures, timing, limitations
- ✅ **Prepare fallbacks** - If assumptions fail, explore log polling or other approaches
- ✅ **Early validation prevents weeks of wasted implementation**

**Investment:** 1-2 days (Week 1)
**Savings:** Potentially 2-3 weeks of rework

---

#### Risk 2: Missing Concurrency Model
**Original Issue:** No synchronization strategy specified, risk of race conditions and data corruption
**Severity:** CRITICAL (data corruption risk)
**Critique Finding:** Lines 115-171 of critique report

**Mitigation:**
- ✅ **Actor pattern chosen** - Single-threaded registry access via message passing
- ✅ **Zero locks needed** - Eliminates deadlock and contention issues
- ✅ **Documented in Week 1** - Clear implementation guidance before coding
- ✅ **Message passing prevents race conditions structurally**

**Implementation:**
```rust
// All registry operations go through message passing
pub enum RegistryCommand {
    Register { session, respond_to },
    Update { id, context, respond_to },
    Get { id, respond_to },
}

// Single task owns registry - impossible to have races
pub struct RegistryActor {
    receiver: mpsc::Receiver<RegistryCommand>,
    registry: HashMap<SessionId, Session>,
}
```

**Investment:** 4 hours (Week 1)
**Benefit:** Eliminates entire class of concurrency bugs

---

#### Risk 3: Inadequate Error Handling
**Original Issue:** Vague "handle gracefully" without specifics, bash scripts could hang Claude Code
**Severity:** CRITICAL (reliability risk)
**Critique Finding:** Lines 173-227 of critique report

**Mitigation:**
- ✅ **Explicit error types defined** - DaemonError, TuiError, shell exit codes
- ✅ **Retry policies specified** - Exponential backoff, timeout values
- ✅ **Graceful degradation documented** - Bash scripts always exit 0, never break Claude
- ✅ **Non-blocking socket writes** - 100ms timeout prevents hangs

**Key Principles:**
1. **Never break Claude Code** - Shell scripts exit 0 even on failure
2. **Observable failures** - All errors logged with context
3. **Automatic recovery** - TUI reconnects, daemon continues on bad messages

**Investment:** 4 hours (Week 1)
**Benefit:** Prevents production incidents and customer frustration

---

#### Risk 4: Unbounded Memory Growth
**Original Issue:** Session registry could accumulate dead sessions indefinitely
**Severity:** HIGH (OOM risk)
**Critique Finding:** Lines 328-370 of critique report

**Mitigation:**
- ✅ **Max sessions: 100** - Hard limit prevents unbounded growth
- ✅ **Automatic cleanup** - Remove stale sessions (90s no heartbeat)
- ✅ **Age-based eviction** - Remove sessions > 24 hours old
- ✅ **LRU eviction** - If over limit, remove oldest first

**Implementation:**
```rust
const MAX_SESSIONS: usize = 100;
const STALE_THRESHOLD: Duration = Duration::from_secs(90);

// Cleanup runs every 30 seconds
async fn cleanup_task() {
    let mut interval = interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        registry.remove_stale_sessions();
    }
}
```

**Investment:** 1 day (Week 3)
**Benefit:** Daemon can run indefinitely without OOM

---

#### Risk 5: Status Update Flooding
**Original Issue:** 10 sessions × 33 updates/sec = excessive CPU usage and rendering
**Severity:** HIGH (performance issue)
**Critique Finding:** Lines 372-425 of critique report

**Mitigation:**
- ✅ **Change detection** - Only broadcast when values actually change
- ✅ **Render throttling** - TUI updates max 10 Hz (not 33+ Hz)
- ✅ **Batch updates** - Combine multiple changes into single broadcast
- ✅ **Message coalescing** - Pending updates accumulated before sending

**Implementation:**
```rust
// Daemon: only broadcast changes
if session.context.used_percentage != new_context.used_percentage {
    broadcast(SessionEvent::Updated(id));
}

// TUI: throttle rendering to 10 Hz
let mut render_interval = interval(Duration::from_millis(100));
loop {
    select! {
        Some(msg) = rx.recv() => app.handle_message(msg),
        _ = render_interval.tick() => terminal.draw(&app)?,
    }
}
```

**Investment:** 1 day (Week 3)
**Benefit:** Reduces CPU from 100% to < 10% under load

---

#### Risk 6: Missing Protocol Versioning
**Original Issue:** No version field, breaking changes would be catastrophic
**Severity:** HIGH (upgrade nightmare)
**Critique Finding:** Lines 427-475 of critique report

**Mitigation:**
- ✅ **Version field in all messages** - "protocol_version": "1.0"
- ✅ **Version negotiation on handshake** - Reject incompatible clients
- ✅ **Backward compatibility plan** - Support N-1 versions during transitions
- ✅ **Clear upgrade paths** - Major/minor version semantics documented

**Example:**
```json
{
  "protocol_version": "1.0",
  "type": "register",
  "session_id": "abc123",
  ...
}
```

**Investment:** 2 hours (Week 1)
**Benefit:** Enables protocol evolution without breaking deployments

---

#### Risk 7: Scope Creep
**Original Issue:** Plan includes 40-50% more features than requirements
**Severity:** HIGH (timeline risk)
**Critique Finding:** Lines 477-517 of critique report

**Mitigation:**
- ✅ **MVP clearly defined** - Phases 1-3 deliver core requirements only
- ✅ **Enhanced features deferred** - Phases 4-5 are polish, can be cut if needed
- ✅ **Cost tracking kept** - Already integrated, minimal overhead, high value
- ✅ **Process stats optional** - Can be removed in Week 3 if timeline slips
- ✅ **History deferred** - SQLite session history moved to post-MVP

**Feature Priority:**

| Feature | Priority | Phase | Can Cut? |
|---------|----------|-------|----------|
| Session monitoring | P0 (core) | 1-3 | No |
| Real-time updates | P0 (core) | 1-3 | No |
| Context display | P0 (core) | 1-3 | No |
| Split-pane UI | P1 (MVP+) | 4 | Yes |
| Cost tracking | P1 (MVP+) | 3 | Yes |
| Process stats | P2 (nice) | 4 | Yes |
| Session history | P3 (future) | Post-MVP | Yes |
| Filtering | P3 (future) | 5 | Yes |

**Investment:** Continuous scope management
**Benefit:** Ensures MVP delivery on time

---

### Additional Risk Mitigations

#### Rust Development Velocity
**Risk:** Team unfamiliar with Rust, slower development
**Mitigation:**
- Allocate 20% extra time for Weeks 2-4
- Use pair programming for Rust learning
- Leverage established libraries (tokio, ratatui)
- Consider prototyping in Go if Rust too slow

#### tmux Dependency
**Risk:** Non-tmux users can't use Agent Tmux Monitor
**Mitigation:**
- Document tmux requirement clearly
- TUI works without tmux (just can't auto-jump)
- Future: Add support for other terminals

#### Single Point of Failure
**Risk:** Daemon crash stops all monitoring
**Mitigation:**
- Focus heavily on daemon stability (Week 3)
- Automatic restart on crash (systemd unit)
- TUI auto-reconnects
- Future: Add daemon health checks

---

## What Changed from Original Plan

This roadmap incorporates critical improvements based on comprehensive architecture review. Here's what changed and why:

### Major Changes

#### 1. Added Week 1: Planning & Validation (NEW)
**Original:** Jump straight to implementation
**Revised:** Validate assumptions and complete specs first

**Why Changed:**
- Critique revealed unvalidated Claude Code integration assumptions
- Missing concurrency model, error handling, resource limits
- Domain model issues that would cause technical debt
- **Investment:** 3-4 days upfront
- **Savings:** Prevents 2-3 weeks of rework

**Confidence Impact:** LOW → HIGH confidence for implementation

---

#### 2. Chose Actor Pattern for Concurrency (SPECIFIED)
**Original:** "Implement session registry" (no details)
**Revised:** Actor model with message passing, zero locks

**Why Changed:**
- Critique identified missing synchronization strategy
- Actor pattern eliminates race conditions structurally
- Simpler to reason about than locks
- Natural fit for Rust/tokio

**Impact:** Eliminates entire class of concurrency bugs

---

#### 3. Comprehensive Error Handling Strategy (SPECIFIED)
**Original:** Vague "handle gracefully" throughout
**Revised:** Explicit error types, retry policies, graceful degradation

**Why Changed:**
- Critique showed bash scripts could hang Claude Code
- No specification for error recovery
- Risk of daemon crashes from unhandled errors

**Key Additions:**
- Non-blocking socket writes with timeouts
- Exponential backoff for reconnections
- Bash scripts always exit 0
- Typed errors with thiserror

**Impact:** Prevents production incidents

---

#### 4. Resource Limits Defined (SPECIFIED)
**Original:** No mention of limits
**Revised:** Max 100 sessions, 10 clients, 90s stale threshold, 24hr max age

**Why Changed:**
- Critique identified unbounded memory growth risk
- No cleanup strategy specified
- Could lead to OOM and crashes

**Key Additions:**
- Automatic stale session cleanup
- LRU eviction when over limit
- Age-based removal
- Memory monitoring

**Impact:** Daemon can run indefinitely

---

#### 5. Protocol Versioning Added (ADDED)
**Original:** No version field in messages
**Revised:** All messages include "protocol_version": "1.0"

**Why Changed:**
- Critique showed upgrade path would be nightmare
- Bash scripts and binaries deployed separately
- No way to evolve protocol without breaking changes

**Key Additions:**
- Version negotiation on handshake
- Backward compatibility plan
- Major/minor version semantics

**Impact:** Enables safe protocol evolution

---

#### 6. Domain Model Refactored (IMPROVED)
**Original:** Single `Session` struct mixing domain and infrastructure
**Revised:** Separate `SessionDomain` + `SessionInfrastructure`, value objects, domain services

**Why Changed:**
- Critique identified domain logic mixed with infrastructure
- Stringly-typed entities (agent_type: String)
- Primitive obsession (cost_usd: f64)
- No domain services for aggregation

**Key Improvements:**
- Type-safe enums (AgentType, Model)
- Value objects (Money, MessageCount)
- Domain services (SessionAggregator, CostCalculator)
- Testable without tmux/OS dependencies

**Impact:** Cleaner architecture, less technical debt

---

#### 7. Scope Management Strategy (CLARIFIED)
**Original:** All features in implementation plan treated equally
**Revised:** Clear MVP (Phases 1-3), optional enhancements (Phases 4-5)

**Why Changed:**
- Critique identified 40-50% scope creep beyond requirements
- Features like SQLite history add significant complexity
- Risk of delayed core feature delivery

**Priorities Clarified:**
- **P0 (Must Have):** Session monitoring, real-time updates, context display
- **P1 (MVP+):** Split-pane UI, cost tracking (kept - already integrated)
- **P2 (Nice):** Process stats, tmux jumping
- **P3 (Future):** Session history, filtering, configuration

**Impact:** Ensures MVP delivery on time

---

#### 8. Change Detection & Throttling (ADDED)
**Original:** Broadcast every status update (33+/sec)
**Revised:** Only broadcast changes, throttle to 10 Hz

**Why Changed:**
- Critique showed 10 sessions × 33 updates/sec = excessive CPU
- TUI re-rendering 33+ FPS wastes resources
- No mention of optimization strategy

**Key Additions:**
- Daemon: only broadcast when values change
- TUI: render at most 10 Hz
- Batch multiple updates together

**Impact:** Reduces CPU from 100% to < 10%

---

### Minor Changes

#### 9. Bash Script Error Handling Hardened
**Added:** Non-blocking socket writes, always exit 0, timeouts

#### 10. Session Lifecycle State Machine Defined
**Added:** Registration → Active → Stale(90s) → Cleanup flow

#### 11. Memory and Resource Monitoring
**Added:** Periodic memory usage logging, alerts > 100MB

#### 12. Testing Strategy Enhanced
**Added:** Stress testing (24+ hours), error scenario testing

---

### What Stayed the Same

These core decisions remain unchanged (they were already sound):

- **Architecture:** Daemon-based with Unix sockets ✅
- **Technology Stack:** Rust + ratatui + tokio ✅
- **Protocol:** Newline-delimited JSON ✅
- **Phased Approach:** 5 phases from foundation to polish ✅
- **UI Design:** Split-pane htop-style interface ✅
- **Integration:** Bash scripts for Claude Code ✅

---

### Summary of Impact

| Change | Time Added | Risk Reduced | Quality Improved |
|--------|------------|--------------|------------------|
| Week 1: Validation & Specs | +3-4 days | HIGH → LOW | +++++ |
| Actor Model Concurrency | 0 (spec only) | CRITICAL | ++++ |
| Error Handling Strategy | 0 (spec only) | CRITICAL | +++++ |
| Resource Limits | +1 day (impl) | HIGH | ++++ |
| Protocol Versioning | +2 hours | HIGH | +++ |
| Domain Model Refactor | +2-3 days | MEDIUM | +++++ |
| Change Detection | +1 day | MEDIUM | +++ |

**Total Time Added:** ~1 week
**Total Risk Reduced:** Massive (3 CRITICAL, 3 HIGH severity issues)
**Total Quality Improved:** Production-ready vs. prototype

**Verdict:** Adding 1 week of planning/polish is excellent tradeoff for eliminating critical risks and technical debt.

---

## Success Criteria

### Overall Project Success

The project is successful when all these criteria are met:

#### Functionality
- ✅ Daemon reliably tracks all Claude Code sessions
- ✅ Real-time updates appear within 500ms
- ✅ TUI displays all required information (context, status, cost)
- ✅ Tmux session jumping works
- ✅ Can monitor 10+ concurrent sessions

#### Performance
- ✅ TUI renders initial screen in < 100ms
- ✅ TUI input latency < 50ms
- ✅ Daemon memory usage < 10MB under normal load
- ✅ Daemon CPU usage < 5% with 10 sessions
- ✅ No noticeable impact on Claude Code performance

#### Reliability
- ✅ Zero crashes during normal operation
- ✅ Daemon runs stably for 24+ hours
- ✅ Graceful handling of errors (network, bad data, etc.)
- ✅ Automatic recovery from failures

#### Usability
- ✅ Installation takes < 5 minutes
- ✅ Clear documentation for setup and usage
- ✅ Intuitive keyboard navigation
- ✅ Helpful error messages

#### Quality
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Code follows Rust best practices
- ✅ No clippy warnings
- ✅ Comprehensive documentation

---

### Phase-Specific Success Criteria

#### Phase 1: Core Daemon (Week 2-3)
- ✅ `atmd start` launches daemon successfully
- ✅ `atmd status` shows daemon running
- ✅ Unix socket created at `/tmp/atm.sock`
- ✅ Accepts connections from test clients
- ✅ Handles 10+ concurrent sessions
- ✅ Session registration/update/unregister work
- ✅ Broadcasts to multiple clients
- ✅ Handles malformed messages gracefully
- ✅ Memory usage < 10MB with 10 sessions
- ✅ Runs for 24+ hours without crash
- ✅ All tests pass (unit + integration)

#### Phase 2: Basic TUI (Week 4)
- ✅ `atm` launches TUI
- ✅ Connects to daemon successfully
- ✅ Lists all active sessions
- ✅ Shows session ID, agent type, context %
- ✅ Keyboard navigation (j/k/arrows) works smoothly
- ✅ 'q' quits gracefully
- ✅ Reconnects automatically if daemon restarts
- ✅ Shows "Daemon Disconnected" when appropriate
- ✅ Renders in < 100ms
- ✅ No flicker or rendering artifacts

#### Phase 3: Shell Integration (Week 5) - MVP
- ✅ `atm-status.sh` receives status line JSON
- ✅ Status updates sent to daemon every ~300ms
- ✅ `atm-hooks.sh` receives hook events
- ✅ Real Claude Code sessions appear in TUI
- ✅ Context % updates in real-time
- ✅ "Waiting for permission" status shows correctly
- ✅ No noticeable Claude Code slowdown
- ✅ Bash scripts don't hang if daemon down
- ✅ Installation script works on clean system
- ✅ **MVP: Full integration working end-to-end**

#### Phase 4: Rich UI (Week 6)
- ✅ Split-pane layout renders correctly
- ✅ Overview panel shows global stats
- ✅ Session list panel shows all sessions with icons
- ✅ Detail panel shows selected session info
- ✅ Progress bars render correctly
- ✅ Colors indicate status (green/yellow/red)
- ✅ `Enter` key jumps to tmux session
- ✅ Tab switches between panels
- ✅ Real-time updates smooth and responsive
- ✅ Terminal resize handled gracefully

#### Phase 5: Advanced Features (Week 7-8)
- ✅ Filtering works (by type, status)
- ✅ Search finds sessions by name
- ✅ Configuration file loaded
- ✅ Help screen shows all keybindings
- ✅ Documentation complete and clear
- ✅ All bugs from testing resolved
- ✅ Installation tested on clean system
- ✅ Performance targets met
- ✅ **Production ready**

---

### Go/No-Go Gates

Each phase has specific criteria that must be met before proceeding:

#### Gate 1: After Week 1
**Question:** Can we proceed with implementation?

**Must Have:**
- Claude Code integration validated
- Concurrency model documented
- Error handling strategy documented
- Resource limits specified
- Protocol versioning added

**If Not Met:** Extend Week 1, explore alternatives

---

#### Gate 2: After Week 3
**Question:** Is daemon stable enough?

**Must Have:**
- Runs 24+ hours without crash
- Handles 10+ sessions
- Memory < 10MB
- All tests pass

**If Not Met:** Extend Week 3, may need redesign

---

#### Gate 3: After Week 5 (MVP)
**Question:** Does integration work well enough?

**Must Have:**
- Real sessions appear in TUI
- Updates within 1 second
- No Claude Code impact
- Bash scripts reliable

**If Not Met:** Extend Week 5, may need alternative approach

---

#### Gate 4: Before Release (Week 8)
**Question:** Ready to ship?

**Must Have:**
- All success criteria met
- Documentation complete
- No critical bugs
- Installation tested

**If Not Met:** Extend until criteria met

---

## Team Recommendations

### Required Skills

#### Core Team (1-2 Developers)

**Primary Developer (Required):**
- **Rust:** Intermediate+ (understand ownership, async/await, tokio)
- **Systems Programming:** Unix sockets, process management, signals
- **TUI Development:** Experience with terminal applications preferred
- **Bash Scripting:** Comfortable with shell scripting, JSON parsing
- **Testing:** Unit testing, integration testing mindset

**Secondary Developer (Optional but Recommended):**
- **Claude Code:** Familiar with Claude Code CLI and configuration
- **tmux:** Power user, understand session/window management
- **Testing/QA:** Can perform comprehensive testing
- **Documentation:** Can write clear user-facing docs

### Experience Levels

**Minimum Viable Team:**
- 1 Rust developer (intermediate+) = 6-8 weeks full-time

**Recommended Team:**
- 1 Senior Rust developer = 5-6 weeks (80% time)
- 1 Junior developer / QA = 3-4 weeks (50% time, Weeks 5-8)

**Optimal Team:**
- 1 Senior Rust developer = 5 weeks (80% time)
- 1 Mid-level developer = 4 weeks (60% time, Weeks 3-6)
- 1 Technical writer = 1 week (Week 8 documentation)

### Learning Curve Considerations

**If Team is New to Rust:**
- Add 20-30% time to Weeks 2-4
- Use pair programming for learning
- Consider Go prototype first (2-3 weeks) to validate design
- Lean heavily on established libraries (tokio, ratatui, serde)

**If Team is New to TUI Development:**
- Budget extra time for ratatui learning (Week 4)
- Study existing ratatui examples
- Start with simple layout, iterate

**If Team is New to Claude Code:**
- Budget 1-2 days in Week 1 for familiarization
- Set up test Claude Code instances
- Understand configuration and hooks system

### Development Environment

**Required Tools:**
- Rust 1.70+ with cargo
- tmux (for testing)
- Claude Code CLI (latest version)
- Unix-like OS (Linux or macOS)
- Git for version control

**Recommended Tools:**
- rust-analyzer for IDE support
- clippy for linting
- cargo-watch for auto-recompile
- tokio-console for async debugging
- jq for JSON testing

---

## Deployment Checklist

### Pre-Deployment (Week 8)

#### Code Quality
- [ ] All tests pass (unit, integration, e2e)
- [ ] No clippy warnings
- [ ] Code reviewed and approved
- [ ] Error handling comprehensive
- [ ] Logging appropriate (not too verbose)

#### Documentation
- [ ] README.md complete with examples
- [ ] INSTALLATION.md with step-by-step guide
- [ ] USAGE.md with screenshots
- [ ] TROUBLESHOOTING.md with common issues
- [ ] Architecture docs for contributors
- [ ] CHANGELOG.md with release notes

#### Testing
- [ ] Manual testing on clean system
- [ ] Test with 10+ concurrent sessions
- [ ] Daemon stability test (24+ hours)
- [ ] Error scenario testing
- [ ] Installation tested on:
  - [ ] Ubuntu 22.04+
  - [ ] macOS Ventura+
  - [ ] Arch Linux
- [ ] Performance benchmarks meet targets

#### Build & Release
- [ ] Release build optimized (`--release`)
- [ ] Version number set (v1.0.0)
- [ ] Git tag created
- [ ] Binaries built for:
  - [ ] Linux x86_64
  - [ ] macOS x86_64
  - [ ] macOS ARM64 (Apple Silicon)
- [ ] Checksums generated
- [ ] Release notes written

---

### Deployment Steps

#### 1. Build Release Binaries
```bash
# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# macOS Intel
cargo build --release --target x86_64-apple-darwin

# macOS Apple Silicon
cargo build --release --target aarch64-apple-darwin
```

#### 2. Package Binaries
```bash
# Create tarball
tar czf atm-v1.0.0-linux-x86_64.tar.gz \
    target/release/atm \
    target/release/atmd \
    components/atm-status.sh \
    components/atm-hooks.sh \
    install.sh \
    README.md

# Generate checksum
sha256sum atm-v1.0.0-linux-x86_64.tar.gz > checksums.txt
```

#### 3. Create GitHub Release
- [ ] Tag version: `v1.0.0`
- [ ] Release title: "Agent Tmux Monitor v1.0.0 - Initial Release"
- [ ] Attach binaries and checksums
- [ ] Copy release notes from CHANGELOG.md

#### 4. Update Documentation
- [ ] Update README with installation links
- [ ] Verify all documentation links work
- [ ] Add screenshots/GIFs to README

---

### Installation (User-Facing)

**Quick Install:**
```bash
# Download latest release
curl -LO https://github.com/[user]/atm/releases/download/v1.0.0/atm-v1.0.0-linux-x86_64.tar.gz

# Extract
tar xzf atm-v1.0.0-linux-x86_64.tar.gz
cd atm-v1.0.0

# Run installer
sudo ./install.sh

# Configure Claude Code
# Edit ~/.claude/settings.json to add status line
# Edit ~/.claude/hooks.json to add hooks

# Start daemon
atmd start

# Launch TUI
atm
```

**Verification:**
```bash
# Check daemon is running
atmd status

# Check binaries installed
which atm atmd

# Check shell scripts installed
which atm-status.sh atm-hooks.sh
```

---

### Post-Deployment

#### Monitoring
- [ ] Check daemon logs: `~/.local/state/atm/atm.log`
- [ ] Monitor memory usage
- [ ] Verify real Claude Code sessions appear

#### User Support
- [ ] Monitor GitHub issues
- [ ] Respond to questions
- [ ] Document common problems

#### Maintenance
- [ ] Plan for bug fixes (v1.0.1, v1.0.2)
- [ ] Gather feedback for v1.1 features
- [ ] Monitor performance in production

---

## Future Enhancements

These features are explicitly **out of scope** for v1.0 MVP but planned for future releases:

### v1.1 (1-2 months post-MVP)

**Performance & Monitoring:**
- [ ] Session history with SQLite backend
- [ ] Historical context usage charts
- [ ] Cost tracking over time (daily/weekly reports)
- [ ] Performance profiling dashboard

**User Experience:**
- [ ] Configuration UI (in-TUI settings)
- [ ] Multiple color themes
- [ ] Custom keybindings
- [ ] Session annotations/notes

**Robustness:**
- [ ] Automatic crash recovery (systemd integration)
- [ ] Health checks and self-healing
- [ ] Better error messages with suggestions
- [ ] Log rotation and management

### v1.2 (3-4 months post-MVP)

**Power User Features:**
- [ ] Advanced filtering (regex, combinators)
- [ ] Session groups and workspaces
- [ ] Custom alerts (context thresholds, cost limits)
- [ ] Export session data (JSON, CSV)

**Integration:**
- [ ] Read-only web dashboard
- [ ] Slack/Discord notifications
- [ ] Prometheus metrics export
- [ ] API for programmatic access

**Control Plane:**
- [ ] Kill/restart sessions from TUI
- [ ] Batch operations (kill all waiting)
- [ ] Session priority management
- [ ] Resource quotas per session

### v2.0 (6-12 months, major release)

**Multi-User Support:**
- [ ] Team dashboard (see colleagues' agents)
- [ ] Role-based access control
- [ ] Centralized logging
- [ ] Shared session annotations

**Advanced Architecture:**
- [ ] Network-based daemon (remote monitoring)
- [ ] Multi-host support (monitor agents on different machines)
- [ ] High availability (daemon failover)
- [ ] Plugin system for extensions

**Cross-Platform:**
- [ ] Support for other terminals (kitty, alacritty, etc.)
- [ ] Windows support (WSL2)
- [ ] Non-tmux mode (direct terminal integration)

**AI Features:**
- [ ] Intelligent context management (suggest when to split conversation)
- [ ] Anomaly detection (unusual cost spikes, hung agents)
- [ ] Predictive alerts (context will hit limit in X minutes)
- [ ] Session recommendations

---

### Feature Requests to Explore

Based on user feedback, consider:

- **Session recording/replay:** Record entire agent conversation for debugging
- **Diff view:** Compare context usage across sessions
- **Time travel:** See session state at any point in history
- **LLM cost optimizer:** Suggest cheaper models for simple tasks
- **Collaborative features:** Share sessions with team members
- **Integration with other AI tools:** Cursor, GitHub Copilot, etc.

---

### Non-Goals (Explicitly Not Building)

These are explicitly **out of scope** and not planned:

- ❌ GUI application (TUI only)
- ❌ Starting/stopping Claude Code from Agent Tmux Monitor (monitoring only)
- ❌ Modifying agent behavior
- ❌ Session content analysis (privacy concerns)
- ❌ Cloud-hosted service
- ❌ Mobile app

---

## Appendix

### Reference Documents

- **Implementation Plan:** [`ATM_IMPLEMENTATION_PLAN.md`](./ATM_IMPLEMENTATION_PLAN.md)
- **Critique Report:** [`ATM_CRITIQUE_REPORT.md`](./ATM_CRITIQUE_REPORT.md)
- **Week 1 Plan:** [`atm-plan/WEEK_1_PLANNING_AND_VALIDATION.md`](./atm-plan/WEEK_1_PLANNING_AND_VALIDATION.md)

### Key Metrics

**Development Effort:**
- Planning: 3-4 days
- Implementation: 4-5 weeks
- Polish: 1-2 weeks
- **Total: 6-8 weeks**

**Code Estimates:**
- Daemon: ~2000 LOC
- TUI: ~1500 LOC
- Shared types: ~500 LOC
- Shell scripts: ~200 LOC
- Tests: ~1000 LOC
- **Total: ~5000 LOC**

**Complexity:**
- Async concurrency: HIGH
- Protocol design: MEDIUM
- TUI rendering: MEDIUM
- Bash integration: LOW-MEDIUM
- **Overall: MEDIUM-HIGH**

### Technology Versions

**Core Dependencies:**
```toml
tokio = "1.35"           # Async runtime
ratatui = "0.26"         # TUI framework
crossterm = "0.27"       # Terminal manipulation
serde = "1.0"            # Serialization
serde_json = "1.0"       # JSON protocol
chrono = "0.4"           # Time handling
sysinfo = "0.30"         # Process monitoring
anyhow = "1.0"           # Error handling
thiserror = "1.0"        # Error types
tracing = "0.1"          # Logging
tracing-subscriber = "0.3"  # Log configuration
```

### Contact & Support

**Questions during planning/implementation:**
- Review original plan: `ATM_IMPLEMENTATION_PLAN.md`
- Check critique report: `ATM_CRITIQUE_REPORT.md`
- Consult weekly plans: `atm-plan/WEEK_*.md`

**After release:**
- GitHub Issues: [Repository issues]
- Discussions: [GitHub discussions]
- Documentation: [Project docs]

---

## Conclusion

This roadmap provides a comprehensive, risk-mitigated path to delivering Agent Tmux Monitor v1.0 in 6-8 weeks. Key success factors:

1. **Week 1 validation prevents costly rework** - Test assumptions before building
2. **Phased approach enables early feedback** - MVP at Week 5, polish after
3. **Risk mitigation baked in** - All critical issues from critique addressed
4. **Clear decision points** - Go/no-go gates prevent proceeding with issues
5. **Scope management** - MVP clearly defined, enhancements optional

**Confidence Level:** With Week 1 validation complete, confidence is **HIGH** that this plan will deliver a production-ready MVP on time.

**Next Steps:**
1. Review this roadmap with team
2. Begin Week 1 validation immediately
3. Adjust timeline based on findings
4. Proceed to implementation with confidence

---

**Document Version:** 1.0
**Status:** Ready for Review
**Approval Required:** Technical Lead, Product Owner
**Target Start Date:** [To be determined]
**Expected Completion:** [Start date] + 6-8 weeks
