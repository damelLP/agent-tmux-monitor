# Claude Code Hook Events Reference

A comprehensive guide to all hook event types in Claude Code.

## Summary Table

| Hook | When It Fires | Can Block? | Primary Purpose |
|------|---------------|------------|-----------------|
| **PreToolUse** | Before tool executes | Yes | Gate/modify tool calls |
| **PostToolUse** | After tool succeeds | Partial | React to tool results |
| **Stop** | Claude finishes responding | Yes | Ensure task completion |
| **SubagentStop** | Subagent finishes | Yes | Verify subagent work |
| **SubagentStart** | Subagent spawned | No | Logging/monitoring |
| **SessionStart** | Session begins/resumes | No | Load context/env vars |
| **SessionEnd** | Session terminates | No | Cleanup/logging |
| **UserPromptSubmit** | User sends prompt | Yes | Add context/validate |
| **PreCompact** | Before context compaction | No | Backup/logging |
| **Setup** | `--init` or `--maintenance` | No | One-time setup tasks |
| **Notification** | Claude sends notification | No | External alerts |

---

## PreToolUse — The Gatekeeper

**Fires:** After Claude decides to use a tool, before execution

**Use for:** Validating commands, blocking dangerous operations, auto-approving safe ones, modifying tool inputs.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": { "command": "rm -rf /tmp/test", "description": "Delete temp files" },
  "tool_use_id": "tool_123"
}
```

### Return Options

| Decision | Effect |
|----------|--------|
| `"allow"` | Auto-approve without user prompt |
| `"deny"` | Block with reason shown to Claude |
| `"ask"` | Show permission dialog (default) |
| `updatedInput` | Modify the tool parameters |

### Example Return

```json
{
  "decision": "allow",
  "additionalContext": "Auto-approved: read-only operation"
}
```

---

## PostToolUse — The Inspector

**Fires:** Immediately after a tool completes successfully

**Use for:** Running linters after writes, validating outputs, logging, triggering follow-up actions.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "PostToolUse",
  "tool_name": "Write",
  "tool_input": { "file_path": "/path/to/file.ts", "content": "..." },
  "tool_response": { "success": true },
  "tool_use_id": "tool_123"
}
```

### Notes

- Tool already ran — you can inform Claude of issues but can't undo
- Return `decision: "block"` to prompt Claude with a reason

---

## Stop — The Completion Guard

**Fires:** When Claude finishes responding (NOT on user interrupts)

**Use for:** Ensuring all tasks completed, requiring tests before stopping, verifying requirements met.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "Stop",
  "stop_hook_active": false
}
```

### Key Field

- `stop_hook_active` — Check this to prevent infinite loops when blocking

### Example Return (Block)

```json
{
  "decision": "block",
  "reason": "Tests have not been run yet. Please run the test suite."
}
```

---

## SubagentStop — The Subagent Guard

**Fires:** When a Task-spawned subagent finishes

**Use for:** Verifying subagent completed its assigned work, checking for errors.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "SubagentStop",
  "agent_id": "agent_456",
  "agent_transcript_path": "/path/to/subagents/transcript.jsonl",
  "stop_hook_active": false
}
```

---

## SubagentStart — The Subagent Logger

**Fires:** When Claude spawns a subagent via Task tool

**Use for:** Tracking which agents are used, logging, initializing monitoring.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "SubagentStart",
  "agent_id": "agent_456",
  "agent_type": "Explore"
}
```

---

## SessionStart — The Initializer

**Fires:** On new session, resume, /clear, or compaction

**Use for:** Loading context (git info, recent changes), setting environment variables, initializing tools.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "SessionStart",
  "source": "startup",
  "model": "claude-sonnet-4-20250514",
  "agent_type": null
}
```

### Source Values

| Value | Meaning |
|-------|---------|
| `"startup"` | New session started |
| `"resume"` | Existing session resumed |
| `"clear"` | After /clear command |
| `"compact"` | After context compaction |

### Environment Variables

- `CLAUDE_ENV_FILE` — Write to this file to persist env vars across the session

**Important:** Runs frequently — keep hooks fast!

---

## SessionEnd — The Cleaner

**Fires:** When session terminates

**Use for:** Cleanup (temp files, processes), logging session stats, saving state.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "SessionEnd",
  "reason": "logout"
}
```

### Reason Values

| Value | Meaning |
|-------|---------|
| `"clear"` | Session cleared with /clear |
| `"logout"` | User logged out |
| `"prompt_input_exit"` | User exited during prompt |
| `"other"` | Other reasons |

---

## UserPromptSubmit — The Prompt Interceptor

**Fires:** When user submits a prompt, before Claude processes it

**Use for:** Adding contextual info, validating prompts, blocking sensitive requests.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "UserPromptSubmit",
  "prompt": "Help me refactor the authentication module"
}
```

### Return Options

1. **Plain text stdout** — Added as context automatically
2. **JSON with `additionalContext`** — String added to context
3. **JSON with `decision: "block"`** — Prevents processing (must include reason)

---

## PreCompact — The Backup Signal

**Fires:** Before context window compaction

**Use for:** Logging what's being discarded, backing up conversation, metrics.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "PreCompact",
  "trigger": "auto",
  "custom_instructions": ""
}
```

### Trigger Values

| Value | Meaning |
|-------|---------|
| `"manual"` | User ran /compact command |
| `"auto"` | Context window full |

---

## Setup — The One-Time Installer

**Fires:** Only with `--init`, `--init-only`, or `--maintenance` flags

**Use for:** Installing dependencies, running migrations, expensive one-time operations.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "Setup",
  "trigger": "init"
}
```

### Trigger Values

| Value | Meaning |
|-------|---------|
| `"init"` | From --init or --init-only |
| `"maintenance"` | From --maintenance |

---

## Notification — The Alert Router

**Fires:** When Claude Code sends any notification

**Use for:** Desktop alerts, Slack messages, external notification systems.

### Payload

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "permission_mode": "default",
  "hook_event_name": "Notification",
  "message": "Claude needs your permission to use Bash",
  "notification_type": "permission_prompt"
}
```

### Notification Types

| Type | Meaning |
|------|---------|
| `permission_prompt` | Permission dialog shown |
| `idle_prompt` | Claude waiting 60+ seconds |
| `auth_success` | Login succeeded |
| `elicitation_dialog` | MCP input needed |

### Example Configuration with Matchers

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "permission_prompt",
        "hooks": [{ "type": "command", "command": "/path/to/permission-alert.sh" }]
      },
      {
        "matcher": "idle_prompt",
        "hooks": [{ "type": "command", "command": "/path/to/idle-alert.sh" }]
      }
    ]
  }
}
```

---

## Exit Codes Reference

| Exit Code | Meaning | Visible To |
|-----------|---------|------------|
| **0** | Success | Depends on return JSON |
| **2** | Blocking error | Claude (as reason for blocking) |
| **Other** | Non-blocking error | User only (stderr) |

---

## Hook Configuration

Hooks are configured in settings files:

| File | Scope |
|------|-------|
| `~/.claude/settings.json` | User-level (all projects) |
| `.claude/settings.json` | Project-level (committed) |
| `.claude/settings.local.json` | Local project (not committed) |

### Basic Hook Structure

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/validate-bash.sh"
          }
        ]
      }
    ]
  }
}
```

---

## Key Concepts

### Matchers

Filter which events trigger your hook:

- **PreToolUse/PostToolUse**: Match by tool name (`"Bash"`, `"Write"`, `"Edit"`, etc.)
- **Notification**: Match by notification type (`"permission_prompt"`, `"idle_prompt"`, etc.)
- **PreCompact**: Match by trigger (`"manual"`, `"auto"`)
- **Setup**: Match by trigger (`"init"`, `"maintenance"`)

### Prompt-Based Hooks

Stop and SubagentStop support prompt-based hooks that use AI to make context-aware decisions:

```json
{
  "hooks": {
    "Stop": [
      {
        "type": "prompt",
        "prompt": "Check if all user requirements have been met. Return {\"decision\": \"block\", \"reason\": \"...\"} if not."
      }
    ]
  }
}
```

### Additional Context

Most hooks can return `additionalContext` — a string that gets added to Claude's context for the current turn.

---

## Sources

- [Claude Code Hooks Reference](https://docs.anthropic.com/en/docs/claude-code/hooks)
- [Claude Code Hooks Guide](https://docs.anthropic.com/en/docs/claude-code/hooks-guide)
