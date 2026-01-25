#!/bin/bash
# Debug script to trace hook execution chain
# This will capture the process tree when a hook runs

LOG=/tmp/atm-ppid-debug.log
echo "========== Hook called at $(date -Iseconds) ==========" >> "$LOG"
echo "My PID ($$): $$" >> "$LOG"
echo "My PPID (\$PPID): $PPID" >> "$LOG"
echo "" >> "$LOG"

# Try to read parent process info from /proc
if [ -d "/proc/$PPID" ]; then
    echo "Parent process ($PPID) cmdline:" >> "$LOG"
    cat /proc/$PPID/cmdline 2>/dev/null | tr '\0' ' ' >> "$LOG"
    echo "" >> "$LOG"
    echo "Parent process ($PPID) comm:" >> "$LOG"
    cat /proc/$PPID/comm 2>/dev/null >> "$LOG"
else
    echo "Parent process ($PPID) does NOT exist in /proc!" >> "$LOG"
fi

echo "" >> "$LOG"

# Also get grandparent
GPPID=$(awk '/^PPid:/ {print $2}' /proc/$PPID/status 2>/dev/null)
if [ -n "$GPPID" ] && [ -d "/proc/$GPPID" ]; then
    echo "Grandparent PID: $GPPID" >> "$LOG"
    echo "Grandparent cmdline:" >> "$LOG"
    cat /proc/$GPPID/cmdline 2>/dev/null | tr '\0' ' ' >> "$LOG"
    echo "" >> "$LOG"
    echo "Grandparent comm:" >> "$LOG"
    cat /proc/$GPPID/comm 2>/dev/null >> "$LOG"
fi

echo "" >> "$LOG"
echo "Full process tree from current process:" >> "$LOG"
pstree -p $$ 2>/dev/null >> "$LOG" || echo "pstree not available" >> "$LOG"

echo "" >> "$LOG"
echo "===== END =====" >> "$LOG"

# Forward stdin to the real hook
cat | /home/damel/code/atm/scripts/atm-hooks.sh

exit 0
