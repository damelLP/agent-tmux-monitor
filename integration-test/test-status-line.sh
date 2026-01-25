#!/bin/bash
# CORRECTED: Test status line component based on official docs

LOG_FILE="/tmp/atm-status-test.log"

# Read JSON input ONCE from stdin (not a loop!)
input=$(cat)

echo "$(date): === Status Line Update ===" >> "$LOG_FILE"
echo "$input" >> "$LOG_FILE"

# Parse JSON fields using jq
HOOK_EVENT=$(echo "$input" | jq -r '.hook_event_name // "N/A"')
SESSION_ID=$(echo "$input" | jq -r '.session_id // "N/A"')
MODEL_ID=$(echo "$input" | jq -r '.model.id // "N/A"')
MODEL_DISPLAY=$(echo "$input" | jq -r '.model.display_name // "N/A"')
CURRENT_DIR=$(echo "$input" | jq -r '.workspace.current_dir // "N/A"')
PROJECT_DIR=$(echo "$input" | jq -r '.workspace.project_dir // "N/A"')
COST=$(echo "$input" | jq -r '.cost.total_cost_usd // "N/A"')
DURATION=$(echo "$input" | jq -r '.cost.total_duration_ms // "N/A"')
LINES_ADDED=$(echo "$input" | jq -r '.cost.total_lines_added // "N/A"')
LINES_REMOVED=$(echo "$input" | jq -r '.cost.total_lines_removed // "N/A"')

# Context window data
INPUT_TOKENS=$(echo "$input" | jq -r '.context_window.total_input_tokens // "N/A"')
OUTPUT_TOKENS=$(echo "$input" | jq -r '.context_window.total_output_tokens // "N/A"')
CONTEXT_SIZE=$(echo "$input" | jq -r '.context_window.context_window_size // "N/A"')
EXCEEDS_200K=$(echo "$input" | jq -r '.exceeds_200k_tokens // "N/A"')

echo "$(date): Parsed Data:" >> "$LOG_FILE"
echo "  Hook Event: $HOOK_EVENT" >> "$LOG_FILE"
echo "  Session ID: $SESSION_ID" >> "$LOG_FILE"
echo "  Model: $MODEL_DISPLAY ($MODEL_ID)" >> "$LOG_FILE"
echo "  Current Dir: $CURRENT_DIR" >> "$LOG_FILE"
echo "  Project Dir: $PROJECT_DIR" >> "$LOG_FILE"
echo "  Cost: \$$COST" >> "$LOG_FILE"
echo "  Duration: ${DURATION}ms" >> "$LOG_FILE"
echo "  Lines: +$LINES_ADDED -$LINES_REMOVED" >> "$LOG_FILE"
echo "  Context: $INPUT_TOKENS in / $OUTPUT_TOKENS out / $CONTEXT_SIZE max" >> "$LOG_FILE"
echo "  Exceeds 200k: $EXCEEDS_200K" >> "$LOG_FILE"

# Output status line (first line of stdout becomes the status line)
echo "[$MODEL_DISPLAY] \$$COST | +$LINES_ADDED -$LINES_REMOVED"
