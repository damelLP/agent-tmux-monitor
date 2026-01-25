#!/bin/bash
# CORRECTED: Test hooks based on official docs
# Works with: PreToolUse, PostToolUse, Notification, UserPromptSubmit, etc.

LOG_FILE="/tmp/atm-hooks-test.log"

echo "$(date): === Hook Triggered ===" >> "$LOG_FILE"

# Read hook event JSON from stdin (single read, not a loop)
input=$(cat)
echo "$input" >> "$LOG_FILE"

# Parse hook event
HOOK_EVENT=$(echo "$input" | jq -r '.hook_event_name // "N/A"')
SESSION_ID=$(echo "$input" | jq -r '.session_id // "N/A"')
TOOL_NAME=$(echo "$input" | jq -r '.tool_name // "N/A"')

echo "$(date): Parsed Data:" >> "$LOG_FILE"
echo "  Hook Event: $HOOK_EVENT" >> "$LOG_FILE"
echo "  Session ID: $SESSION_ID" >> "$LOG_FILE"
echo "  Tool Name: $TOOL_NAME" >> "$LOG_FILE"

# Event-specific parsing
if [ "$HOOK_EVENT" = "PreToolUse" ] || [ "$HOOK_EVENT" = "PostToolUse" ]; then
    TOOL_INPUT=$(echo "$input" | jq -c '.tool_input // {}')
    echo "  Tool Input: $TOOL_INPUT" >> "$LOG_FILE"

    # For file operations, log the file path
    FILE_PATH=$(echo "$input" | jq -r '.tool_input.file_path // "N/A"')
    if [ "$FILE_PATH" != "N/A" ]; then
        echo "  File Path: $FILE_PATH" >> "$LOG_FILE"
    fi
fi

if [ "$HOOK_EVENT" = "UserPromptSubmit" ]; then
    PROMPT=$(echo "$input" | jq -r '.prompt // "N/A"')
    echo "  Prompt: $PROMPT" >> "$LOG_FILE"
fi

if [ "$HOOK_EVENT" = "Notification" ]; then
    MESSAGE=$(echo "$input" | jq -r '.message // "N/A"')
    echo "  Message: $MESSAGE" >> "$LOG_FILE"
fi
