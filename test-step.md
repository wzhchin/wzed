#!/usr/bin/env bash
set -euo pipefail

WZED="target/debug/wzed"
SESSION="$HOME/.config/wzed/session.json"
SOCK="$HOME/.local/share/wzed.sock"
LOG="/tmp/wzed-test/editor.log"
PASS=0
FAIL=0

assert_eq() {
    local label="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "  PASS: $label (expected=$expected, actual=$actual)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label (expected=$expected, actual=$actual)"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local label="$1" haystack="$2" needle="$3"
    if echo "$haystack" | grep -q "$needle"; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label (\"$needle\" not found)"
        FAIL=$((FAIL + 1))
    fi
}

get_session_field() {
    python3 -c "import json,sys; d=json.load(open('$SESSION')); print($1)"
}

# Clean up
echo "=== Setup ==="
for pid in $(pgrep -f "target/debug/wzed" 2>/dev/null); do kill "$pid" 2>/dev/null; done || true
sleep 1
python3 -c "from pathlib import Path
for p in [Path('$SOCK'), Path('$SESSION')]:
    p.unlink() if p.exists() else None"
echo "original content" > /tmp/wzed-test/test.txt
echo "fn main() {}" > /tmp/wzed-test/main.rs

# Step 1: --list-commands
echo ""
echo "=== Step 1: --list-commands ==="
LIST=$($WZED --list-commands 2>&1)
for cmd in save-file open-file new-file close-tab toggle-find toggle-replace; do
    assert_contains "command '$cmd' listed" "$LIST" "^$cmd "
done

# Step 2: Start editor with file
echo ""
echo "=== Step 2: Open file on startup ==="
$WZED /tmp/wzed-test/test.txt 2>"$LOG" &
EDITOR_PID=$!
sleep 3
TAB_COUNT=$(get_session_field "len(d['tabs'])")
HAS_TEST=$(get_session_field "any('test.txt' in (t.get('path') or '') for t in d['tabs'])")
assert_eq "tab count" "2" "$TAB_COUNT"
assert_eq "has test.txt" "True" "$HAS_TEST"

# Step 3: Open another file via IPC
echo ""
echo "=== Step 3: Open file via IPC ==="
$WZED /tmp/wzed-test/main.rs 2>&1
sleep 1
TAB_COUNT=$(get_session_field "len(d['tabs'])")
HAS_MAIN=$(get_session_field "any('main.rs' in (t.get('path') or '') for t in d['tabs'])")
assert_eq "tab count" "3" "$TAB_COUNT"
assert_eq "has main.rs" "True" "$HAS_MAIN"

# Step 4: New file via command
echo ""
echo "=== Step 4: New file via command ==="
$WZED -c "new-file" 2>&1
sleep 1
TAB_COUNT=$(get_session_field "len(d['tabs'])")
LAST_PATH=$(get_session_field "d['tabs'][-1]['path']")
assert_eq "tab count" "4" "$TAB_COUNT"
assert_eq "new tab is untitled" "None" "$LAST_PATH"

# Step 5: Set text via IPC
echo ""
echo "=== Step 5: Set text via IPC ==="
$WZED -c "set-text:Hello WZed Editor" 2>&1
sleep 1
ACTIVE_CONTENT=$(get_session_field "d['tabs'][d['active']].get('unsaved_content', '')")
assert_eq "active tab content" "Hello WZed Editor" "$ACTIVE_CONTENT"

# Step 6: Save via save-as
echo ""
echo "=== Step 6: Save-as via IPC ==="
$WZED -c "save-as:/tmp/wzed-test/newfile.txt" 2>&1
sleep 1
FILE_CONTENT=$(cat /tmp/wzed-test/newfile.txt 2>/dev/null || echo "FILE_NOT_FOUND")
assert_eq "saved file content" "Hello WZed Editor" "$FILE_CONTENT"

# Step 7: Switch tab
echo ""
echo "=== Step 7: Switch tab ==="
ORIG_ACTIVE=$(get_session_field "d['active']")
$WZED -c "switch-tab:0" 2>&1
sleep 1
NEW_ACTIVE=$(get_session_field "d['active']")
assert_eq "active tab index" "0" "$NEW_ACTIVE"

# Switch back
$WZED -c "switch-tab:3" 2>&1
sleep 1
BACK_ACTIVE=$(get_session_field "d['active']")
assert_eq "switch back to tab 3" "3" "$BACK_ACTIVE"

# Step 8: Close tab
echo ""
echo "=== Step 8: Close tab ==="
BEFORE_COUNT=$(get_session_field "len(d['tabs'])")
$WZED -c "close-tab" 2>&1
sleep 1
AFTER_COUNT=$(get_session_field "len(d['tabs'])")
assert_eq "tab count decreased" "$((BEFORE_COUNT - 1))" "$AFTER_COUNT"

# Step 9: Toggle command
echo ""
echo "=== Step 9: Toggle find ==="
$WZED -c "toggle-find" 2>&1
sleep 1
NO_CRASH=$(get_session_field "len(d['tabs'])")
assert_eq "editor still running (no crash)" "$AFTER_COUNT" "$NO_CRASH"

# Step 10: Nonexistent command
echo ""
echo "=== Step 10: Nonexistent command ==="
$WZED -c "nonexistent-xyz" 2>&1
sleep 1
NO_CRASH=$(get_session_field "len(d['tabs'])")
assert_eq "editor still running (no crash)" "$AFTER_COUNT" "$NO_CRASH"

# Step 11: Save file (existing tab with path)
echo ""
echo "=== Step 11: Save file ==="
$WZED -c "switch-tab:1" 2>&1
sleep 1
$WZED -c "set-text:updated content" 2>&1
sleep 1
$WZED -c "save-file" 2>&1
sleep 1
FILE_CONTENT=$(cat /tmp/wzed-test/test.txt)
assert_eq "saved file content" "updated content" "$FILE_CONTENT"

# Step 12: Session persistence
echo ""
echo "=== Step 12: Session persistence ==="
SESSION_BEFORE=$(cat "$SESSION")
kill "$EDITOR_PID" 2>/dev/null
sleep 2
python3 -c "from pathlib import Path; p=Path('$SOCK'); p.unlink() if p.exists() else None"
$WZED 2>"$LOG" &
NEW_PID=$!
sleep 3
SESSION_AFTER=$(cat "$SESSION")
TAB_COUNT_BEFORE=$(echo "$SESSION_BEFORE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['tabs']))")
TAB_COUNT_AFTER=$(echo "$SESSION_AFTER" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['tabs']))")
assert_eq "tab count preserved" "$TAB_COUNT_BEFORE" "$TAB_COUNT_AFTER"

# Cleanup
kill "$NEW_PID" 2>/dev/null || true

# Summary
echo ""
echo "========================================="
echo "  Results: $PASS passed, $FAIL failed"
echo "========================================="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
