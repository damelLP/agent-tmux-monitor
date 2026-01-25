#!/bin/bash
# Daemon Integration Test Script
# Tests the atmd daemon for correctness and robustness

SOCK="${ATM_SOCKET:-/tmp/atm.sock}"
PASS=0
FAIL=0
DAEMON_PID=""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_pass() { echo -e "${GREEN}✓ PASS${NC}: $1"; ((PASS++)); }
log_fail() { echo -e "${RED}✗ FAIL${NC}: $1"; ((FAIL++)); }
log_info() { echo -e "${YELLOW}→${NC} $1"; }

cleanup() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null
        wait "$DAEMON_PID" 2>/dev/null
    fi
}
trap cleanup EXIT

send_msgs() {
    echo -e "$1" | timeout "${2:-2}" socat - UNIX-CONNECT:"$SOCK" 2>/dev/null
}

echo "═══════════════════════════════════════════════════════════════"
echo "  ATM Daemon Integration Tests"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Build
log_info "Building daemon..."
cargo build --bin atmd --release 2>/dev/null || cargo build --bin atmd

# Start daemon
log_info "Starting daemon..."
rm -f "$SOCK"
RUST_LOG=warn ./target/release/atmd 2>/dev/null &
DAEMON_PID=$!

# Wait for socket
for i in {1..50}; do [ -S "$SOCK" ] && break; sleep 0.1; done

if [ ! -S "$SOCK" ]; then
    log_fail "Daemon socket did not appear"
    exit 1
fi
log_pass "Daemon started (PID $DAEMON_PID)"
echo ""

# ─── Test 1: Handshake ───
echo "─── Test 1: Basic Handshake ───"
RESP=$(send_msgs '{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"test-1"}')
if echo "$RESP" | grep -q '"type":"connected"'; then
    log_pass "Handshake successful"
else
    log_fail "Handshake failed: $RESP"
fi

# ─── Test 2: Ping/Pong ───
echo ""
echo "─── Test 2: Ping/Pong ───"
RESP=$(send_msgs '{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"ping-test"}\n{"protocol_version":{"major":1,"minor":0},"type":"ping","seq":42}')
if echo "$RESP" | grep -q '"seq":42'; then
    log_pass "Ping/Pong works"
else
    log_fail "Ping/Pong failed"
fi

# ─── Test 3: Status Update ───
echo ""
echo "─── Test 3: Status Update (auto-registration) ───"
DATA='{"session_id":"test-abc","model":{"id":"claude-sonnet-4-20250514"},"cost":{"total_cost_usd":0.25,"total_duration_ms":15000},"context_window":{"total_input_tokens":5000,"total_output_tokens":2000,"context_window_size":200000}}'
RESP=$(send_msgs '{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"status-test"}\n{"protocol_version":{"major":1,"minor":0},"type":"status_update","data":'"$DATA"'}\n{"protocol_version":{"major":1,"minor":0},"type":"list_sessions"}')
if echo "$RESP" | grep -q 'test-abc'; then
    log_pass "Status update auto-registered session"
else
    log_fail "Status update failed"
fi

# ─── Test 4: Multiple Sessions ───
echo ""
echo "─── Test 4: Multiple Sessions ───"
CMDS='{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"multi"}'
for i in {1..10}; do
    D='{"session_id":"multi-'$i'","model":{"id":"claude-opus-4-5-20251101"},"cost":{"total_cost_usd":0.'$i',"total_duration_ms":'$((i*1000))'},"context_window":{"total_input_tokens":'$((i*100))',"total_output_tokens":'$((i*50))',"context_window_size":200000}}'
    CMDS="$CMDS"'\n{"protocol_version":{"major":1,"minor":0},"type":"status_update","data":'"$D"'}'
done
CMDS="$CMDS"'\n{"protocol_version":{"major":1,"minor":0},"type":"list_sessions"}'
RESP=$(send_msgs "$CMDS" 3)
CNT=$(echo "$RESP" | grep -o '"multi-' | wc -l)
if [ "$CNT" -ge 10 ]; then
    log_pass "Created 10 sessions"
else
    log_fail "Expected 10+ sessions, found $CNT"
fi

# ─── Test 5: Rapid Updates ───
echo ""
echo "─── Test 5: Rapid Updates (50x) ───"
START=$(date +%s%N)
CMDS='{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"rapid"}'
for i in {1..50}; do
    D='{"session_id":"rapid","model":{"id":"claude-sonnet-4-20250514"},"cost":{"total_cost_usd":0.0'$i',"total_duration_ms":'$((i*100))'},"context_window":{"total_input_tokens":'$((i*10))',"total_output_tokens":'$((i*5))',"context_window_size":200000}}'
    CMDS="$CMDS"'\n{"protocol_version":{"major":1,"minor":0},"type":"status_update","data":'"$D"'}'
done
send_msgs "$CMDS" 5 >/dev/null
END=$(date +%s%N)
MS=$(( (END - START) / 1000000 ))
if [ "$MS" -lt 5000 ]; then
    log_pass "50 updates in ${MS}ms"
else
    log_fail "Too slow: ${MS}ms"
fi

# ─── Test 6: Malformed JSON ───
echo ""
echo "─── Test 6: Malformed JSON Handling ───"
send_msgs 'garbage\n{"protocol_version":{"major":1,"minor":0},"type":"connect"}' >/dev/null 2>&1
if kill -0 "$DAEMON_PID" 2>/dev/null; then
    log_pass "Daemon survived malformed JSON"
else
    log_fail "Daemon crashed"
fi

# ─── Test 7: Subscription ───
echo ""
echo "─── Test 7: Subscription ───"
RESP=$(send_msgs '{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"sub"}\n{"protocol_version":{"major":1,"minor":0},"type":"subscribe"}')
if echo "$RESP" | grep -q '"type":"session_list"'; then
    log_pass "Subscription works"
else
    log_fail "Subscription failed"
fi

# ─── Test 8: Context Calculation ───
echo ""
echo "─── Test 8: Context Usage (5141+1453=3.3%) ───"
D='{"session_id":"ctx-test","model":{"id":"claude-sonnet-4-20250514"},"cost":{"total_cost_usd":0.35,"total_duration_ms":35000},"context_window":{"total_input_tokens":5141,"total_output_tokens":1453,"context_window_size":200000}}'
RESP=$(send_msgs '{"protocol_version":{"major":1,"minor":0},"type":"connect","client_id":"ctx"}\n{"protocol_version":{"major":1,"minor":0},"type":"status_update","data":'"$D"'}\n{"protocol_version":{"major":1,"minor":0},"type":"list_sessions"}')
if echo "$RESP" | grep -q '"context_percentage":3\.'; then
    log_pass "Context: ~3.3%"
else
    PCT=$(echo "$RESP" | grep -o '"context_percentage":[0-9.]*' | head -1)
    log_fail "Context wrong: $PCT"
fi

# ─── Test 9: Memory ───
echo ""
echo "─── Test 9: Memory Usage ───"
MEM_KB=$(ps -o rss= -p "$DAEMON_PID" 2>/dev/null || echo "0")
MEM_MB=$((MEM_KB / 1024))
if [ "$MEM_MB" -lt 50 ]; then
    log_pass "Memory: ${MEM_MB}MB"
else
    log_fail "Memory too high: ${MEM_MB}MB"
fi

# ─── Test 10: Shutdown ───
echo ""
echo "─── Test 10: Graceful Shutdown ───"
kill -TERM "$DAEMON_PID" 2>/dev/null
for i in {1..30}; do
    kill -0 "$DAEMON_PID" 2>/dev/null || break
    sleep 0.1
done
if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    log_pass "Graceful shutdown"
    DAEMON_PID=""
else
    log_fail "Shutdown timeout"
fi

# Results
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"
echo "═══════════════════════════════════════════════════════════════"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
