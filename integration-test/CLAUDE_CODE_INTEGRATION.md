# Claude Code Integration Findings

**Date:** 2026-01-23
**Purpose:** Validate Claude Code Status Line API and Hooks system before implementing ATM daemon
**Status:** ✅ VALIDATED - Both Status Line and Hooks working correctly

---

## Executive Summary

Integration testing completed successfully. Both Status Line and Hooks systems work as expected with some important configuration corrections discovered.

### Key Findings

| Item | Previous Assumption | Validated Reality | Impact |
|------|---------------------|-------------------|--------|
| Context window data | Not available | ✅ **AVAILABLE** in `context_window` field | 🟢 Can build token tracking! |
| Hooks config location | Separate `hooks.json` file | ❌ Must be in `settings.json` | 🔴 Config fix required |
| Script paths | Relative `./script.sh` | ⚠️ Use `"$CLAUDE_PROJECT_DIR"/script.sh` | 🟡 Path resolution fix |
| Hook event name | `PermissionRequest` | ✅ `PreToolUse` with permission control | 🟢 Correct event exists |
| Status Line frequency | "~300ms" | ✅ "At most every 300ms" | 🟢 Confirmed |
| Session ID | Not sure where | ✅ Available in JSON `session_id` field | 🟢 Confirmed |

---

## Status Line API

### Configuration

**Location:** `.claude/settings.json` (NOT a separate file)

```json
{
  "statusLine": {
    "type": "command",
    "command": "\"$CLAUDE_PROJECT_DIR\"/statusline.sh"
  }
}
```

**Important:** Use `"$CLAUDE_PROJECT_DIR"` for reliable path resolution.

### How It Works

1. **Trigger:** Status line updates when conversation messages update
2. **Frequency:** At most every 300ms (rate-limited)
3. **Input:** JSON data passed via **stdin** (read ONCE, not in a loop!)
4. **Output:** **First line of stdout** becomes the status line text
5. **Styling:** ANSI color codes supported

### JSON Input Structure (VALIDATED)

From actual test log at `/tmp/atm-status-test.log`:

```json
{
  "session_id": "8e11bfb5-7dc2-432b-9206-928fa5c35731",
  "transcript_path": "/home/damel/.claude/projects/-home-damel-code-atm-integration-test/8e11bfb5-7dc2-432b-9206-928fa5c35731.jsonl",
  "cwd": "/home/damel/code/atm/integration-test",
  "model": {
    "id": "claude-opus-4-5-20251101",
    "display_name": "Opus 4.5"
  },
  "workspace": {
    "current_dir": "/home/damel/code/atm/integration-test",
    "project_dir": "/home/damel/code/atm/integration-test"
  },
  "version": "2.0.76",
  "output_style": {
    "name": "default"
  },
  "cost": {
    "total_cost_usd": 0.35205149999999996,
    "total_duration_ms": 35239,
    "total_api_duration_ms": 34204,
    "total_lines_added": 0,
    "total_lines_removed": 0
  },
  "context_window": {
    "total_input_tokens": 5141,
    "total_output_tokens": 1453,
    "context_window_size": 200000,
    "current_usage": {
      "input_tokens": 7,
      "output_tokens": 433,
      "cache_creation_input_tokens": 1396,
      "cache_read_input_tokens": 37532
    }
  },
  "exceeds_200k_tokens": false
}
```

### ✅ Context Window Data IS Available!

**This corrects the earlier assumption.** The `context_window` field provides:

| Field | Description | Example |
|-------|-------------|---------|
| `total_input_tokens` | Total input tokens used | `5141` |
| `total_output_tokens` | Total output tokens used | `1453` |
| `context_window_size` | Max context window | `200000` |
| `current_usage.input_tokens` | Current request input | `7` |
| `current_usage.output_tokens` | Current request output | `433` |
| `current_usage.cache_creation_input_tokens` | Tokens for cache creation | `1396` |
| `current_usage.cache_read_input_tokens` | Tokens read from cache | `37532` |
| `exceeds_200k_tokens` | Boolean flag | `false` |

**Context Usage Calculation:**
```
usage_percentage = (total_input_tokens + total_output_tokens) / context_window_size * 100
                 = (5141 + 1453) / 200000 * 100
                 = 3.3%
```

### Correct Script Pattern

```bash
#!/bin/bash
# Read JSON input ONCE from stdin
input=$(cat)

# Parse fields
SESSION_ID=$(echo "$input" | jq -r '.session_id')
MODEL=$(echo "$input" | jq -r '.model.display_name')
COST=$(echo "$input" | jq -r '.cost.total_cost_usd')
TOTAL_IN=$(echo "$input" | jq -r '.context_window.total_input_tokens')
TOTAL_OUT=$(echo "$input" | jq -r '.context_window.total_output_tokens')
CTX_SIZE=$(echo "$input" | jq -r '.context_window.context_window_size')

# Calculate context percentage
CTX_PCT=$(echo "scale=1; ($TOTAL_IN + $TOTAL_OUT) / $CTX_SIZE * 100" | bc)

# Output status line (first line only!)
echo "[$MODEL] Context: ${CTX_PCT}% | Cost: \$$COST"
```

---

## Hooks System

### Configuration

**Location:** `.claude/settings.json` (NOT a separate `hooks.json` file!)

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/my-hook.sh"
          }
        ]
      }
    ]
  }
}
```

### Critical Configuration Discovery

**Hooks MUST be in `settings.json`, NOT a separate `hooks.json` file.**

This was the root cause of hooks not working initially:
- ❌ `.claude/hooks.json` - Claude Code ignores this file
- ✅ `.claude/settings.json` with `"hooks": {...}` - This works

### Available Hook Events (VALIDATED)

| Event | Description | Matcher Support | Tested |
|-------|-------------|----------------|--------|
| `PreToolUse` | Before tool execution | ✅ Yes (tool name patterns) | ✅ |
| `PostToolUse` | After tool execution | ✅ Yes (tool name patterns) | ✅ |
| `Notification` | When notifications sent | ❌ No | - |
| `UserPromptSubmit` | When user submits prompt | ❌ No | ✅ |
| `Stop` | When main agent finishes | ❌ No | - |
| `SubagentStop` | When subagent finishes | ❌ No | - |
| `PreCompact` | Before compact operation | ✅ Yes (`manual`/`auto`) | - |
| `SessionStart` | When session starts | ✅ Yes (`startup`/`resume`/`clear`/`compact`) | ✅ |
| `SessionEnd` | When session ends | ❌ No | - |

### JSON Input Structure - PreToolUse (VALIDATED)

From actual test log at `/tmp/atm-hooks-test.log`:

```json
{
  "session_id": "8e11bfb5-7dc2-432b-9206-928fa5c35731",
  "transcript_path": "/home/damel/.claude/projects/-home-damel-code-atm-integration-test/8e11bfb5-7dc2-432b-9206-928fa5c35731.jsonl",
  "cwd": "/home/damel/code/atm/integration-test",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "ls -la /home/damel/code/atm/integration-test/.claude/",
    "description": "List files in .claude directory"
  },
  "tool_use_id": "toolu_01Q3aFytofjt3vUUhG4HNd6k"
}
```

### JSON Input Structure - PostToolUse (VALIDATED)

```json
{
  "session_id": "8e11bfb5-7dc2-432b-9206-928fa5c35731",
  "transcript_path": "/home/damel/.claude/projects/-home-damel-code-atm-integration-test/8e11bfb5-7dc2-432b-9206-928fa5c35731.jsonl",
  "cwd": "/home/damel/code/atm/integration-test",
  "permission_mode": "default",
  "hook_event_name": "PostToolUse",
  "tool_name": "Bash",
  "tool_input": {
    "command": "ls -la ...",
    "description": "List files..."
  },
  "tool_response": {
    "stdout": "total 16\ndrwxr-xr-x 2 damel users 4096 ...",
    "stderr": "",
    "interrupted": false,
    "isImage": false
  },
  "tool_use_id": "toolu_01Q3aFytofjt3vUUhG4HNd6k"
}
```

### Hook Output (Exit Codes)

- **Exit 0**: Success, `stdout` shown in transcript mode (Ctrl-R)
- **Exit 2**: Blocking error, `stderr` fed to Claude for processing
- **Other**: Non-blocking error, `stderr` shown to user

### Permission Control via PreToolUse

To control tool permissions, return JSON from hook:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Auto-approved"
  }
}
```

Options: `"allow"` | `"deny"` | `"ask"`

---

## Session Identification

### ✅ Session ID Available

The `session_id` field is **always provided** in JSON for:
- Status Line updates
- All hook events

**Format:** UUID string (e.g., `"8e11bfb5-7dc2-432b-9206-928fa5c35731"`)

### Available Environment Variables

- `$CLAUDE_PROJECT_DIR` - Absolute path to project root (where Claude Code was started)
- Standard shell variables: `$$` (PID), `$PPID`, `$PWD`, etc.

---

## Updated Data Model for ATM

Based on validated JSON structures:

### SessionDomain

```rust
pub struct SessionDomain {
    // Identity
    pub session_id: String,              // ✅ Available
    pub transcript_path: PathBuf,        // ✅ Available

    // Model info
    pub model_id: String,                // ✅ Available (.model.id)
    pub model_display_name: String,      // ✅ Available (.model.display_name)

    // Workspace
    pub current_dir: PathBuf,            // ✅ Available (.workspace.current_dir)
    pub project_dir: PathBuf,            // ✅ Available (.workspace.project_dir)

    // Cost tracking
    pub total_cost_usd: f64,             // ✅ Available (.cost.total_cost_usd)
    pub total_duration_ms: u64,          // ✅ Available (.cost.total_duration_ms)
    pub total_api_duration_ms: u64,      // ✅ Available (.cost.total_api_duration_ms)
    pub total_lines_added: u32,          // ✅ Available (.cost.total_lines_added)
    pub total_lines_removed: u32,        // ✅ Available (.cost.total_lines_removed)

    // Context tracking - NOW AVAILABLE!
    pub total_input_tokens: u64,         // ✅ Available (.context_window.total_input_tokens)
    pub total_output_tokens: u64,        // ✅ Available (.context_window.total_output_tokens)
    pub context_window_size: u64,        // ✅ Available (.context_window.context_window_size)
    pub cache_read_tokens: u64,          // ✅ Available (.context_window.current_usage.cache_read_input_tokens)
    pub exceeds_200k_tokens: bool,       // ✅ Available (.exceeds_200k_tokens)

    // Status (derived from hooks)
    pub agent_status: AgentStatus,
    pub last_tool_used: Option<String>,
    pub last_tool_use_id: Option<String>,
    pub last_update: DateTime<Utc>,
}

impl SessionDomain {
    /// Calculate context usage percentage
    pub fn context_usage_percentage(&self) -> f64 {
        if self.context_window_size == 0 {
            return 0.0;
        }
        let used = self.total_input_tokens + self.total_output_tokens;
        (used as f64 / self.context_window_size as f64) * 100.0
    }
}
```

### AgentStatus

```rust
pub enum AgentStatus {
    Thinking,                    // No recent hook activity
    RunningTool(String),         // PreToolUse hook fired
    ToolComplete(String),        // PostToolUse hook fired
    WaitingForPermission,        // Notification hook fired
    Idle,                        // Stop hook fired
}
```

---

## Test Configuration (Working)

### .claude/settings.json

```json
{
  "statusLine": {
    "type": "command",
    "command": "\"$CLAUDE_PROJECT_DIR\"/test-status-line.sh"
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/test-hooks.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/test-hooks.sh"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/test-hooks.sh"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/test-hooks.sh"
          }
        ]
      }
    ]
  }
}
```

---

## Validation Evidence

### Status Line Log Sample

```
Fri 23 Jan 22:42:36 GMT 2026: === Status Line Update ===
{"session_id":"8e11bfb5-7dc2-432b-9206-928fa5c35731",...,"context_window":{"total_input_tokens":5141,"total_output_tokens":1453,"context_window_size":200000,...}}
```

### Hooks Log Sample

```
Fri 23 Jan 22:42:23 GMT 2026: === Hook Triggered ===
{"session_id":"8e11bfb5-7dc2-432b-9206-928fa5c35731",...,"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{...}}
```

---

## Confidence Level

| Area | Status | Confidence |
|------|--------|------------|
| Status Line works | ✅ Validated | 🟢 HIGH |
| Hooks work | ✅ Validated | 🟢 HIGH |
| Context window data available | ✅ Validated | 🟢 HIGH |
| Session ID available | ✅ Validated | 🟢 HIGH |
| Cost tracking available | ✅ Validated | 🟢 HIGH |
| Tool tracking via hooks | ✅ Validated | 🟢 HIGH |

**Overall:** 🟢 **HIGH** - All integration points validated and working. Ready to proceed to Week 2 implementation.

---

## Lessons Learned

1. **Always test assumptions** - The original plan assumed context window data wasn't available, but it IS
2. **Configuration location matters** - Hooks must be in `settings.json`, not a separate file
3. **Use absolute paths** - `"$CLAUDE_PROJECT_DIR"` ensures scripts are found regardless of CWD
4. **Read official docs** - The Claude Code documentation is accurate and comprehensive

---

## Next Steps

1. ✅ Status Line integration validated
2. ✅ Hooks integration validated
3. ✅ Context window data confirmed available
4. ✅ Document actual JSON structures
5. ⏭️ Complete Day 2: Architecture specifications
6. ⏭️ Complete Days 3-4: Domain model design
7. ⏭️ Gate 1 approval for Week 2 implementation

---

## Appendix: Test Files

- **Status Line Test Log:** `/tmp/atm-status-test.log`
- **Hooks Test Log:** `/tmp/atm-hooks-test.log`
- **Test Scripts:** `integration-test/test-status-line.sh`, `integration-test/test-hooks.sh`
- **Working Config:** `integration-test/.claude/settings.json`
