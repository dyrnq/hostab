#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="${1:-$PROJECT_DIR/target/release/hostab}"
TEST_HOSTS="$SCRIPT_DIR/hosts.test"
WORK_DIR="$(mktemp -d)"
HOSTS_FILE="$WORK_DIR/hosts"

RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
pass_count=0; fail_count=0

pass() { echo -e "  ${GREEN}PASS${NC} $1"; pass_count=$((pass_count + 1)); }
fail() { echo -e "  ${RED}FAIL${NC} $1 — $2"; fail_count=$((fail_count + 1)); }
assert_contains() {
    local d="$1" n="$2" h="$3"
    if echo "$h" | grep -qF "$n"; then pass "$d"; else fail "$d" "expected to contain '$n'"; fi
}
assert_not_contains() {
    local d="$1" n="$2" h="$3"
    if ! echo "$h" | grep -qF "$n"; then pass "$d"; else fail "$d" "expected NOT to contain '$n'"; fi
}

cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

echo "=== hostab integration tests ==="
echo "Binary: $BINARY"
echo ""

[ -x "$BINARY" ] || { echo "Error: $BINARY not found."; exit 1; }

cp "$TEST_HOSTS" "$HOSTS_FILE"
HOSTAB="$BINARY --hosts-file $HOSTS_FILE"

# ── 1. entry list ──────────────────────────────────────────────
echo "--- 1. entry list ---"

OUT=$($HOSTAB e list)
assert_contains "list: localhost" "127.0.0.1" "$OUT"
assert_contains "list: app.local" "app.local" "$OUT"

OUT=$($HOSTAB e list --ipv4)
assert_contains "ipv4: has entry" "127.0.0.1" "$OUT"
assert_not_contains "ipv4: no ipv6" "::1" "$OUT"

OUT=$($HOSTAB e list --ipv6)
assert_contains "ipv6: has ::1" "::1" "$OUT"

OUT=$($HOSTAB -o json e list)
assert_contains "json: valid" "[" "$OUT"

# ── 2. entry add / rm ──────────────────────────────────────────
echo "--- 2. entry add / rm ---"

$HOSTAB e add 10.99.0.10 test-add.local -q
OUT=$($HOSTAB e list -o raw)
assert_contains "add: test-add.local" "test-add.local" "$OUT"

$HOSTAB e rm test-add.local -q
OUT=$($HOSTAB e list -o raw)
assert_not_contains "rm: test-add.local gone" "test-add.local" "$OUT"

$HOSTAB e add 10.99.0.20 ip-rm-test.local -q
$HOSTAB e rm --ip 10.99.0.20 -q
OUT=$($HOSTAB e list -o raw)
assert_not_contains "rm-ip: gone" "ip-rm-test.local" "$OUT"

# ── 3. search (via list with pattern) ───────────────────────────
echo "--- 3. search ---"

OUT=$($HOSTAB e list -f prod)
assert_contains "search: prod.local" "prod.local" "$OUT"

OUT=$($HOSTAB e list -f "database")
assert_contains "search: comment" "database" "$OUT"

OUT=$($HOSTAB e list -f "192.168")
assert_contains "search: IP" "192.168" "$OUT"

# ── 4. disable / enable / toggle ────────────────────────────────
echo "--- 4. disable/enable/toggle ---"

$HOSTAB e disable api.local -q
OUT=$($HOSTAB cat)
assert_contains "disable: api.local commented" "# 10.0.0.1 api.local" "$OUT"
assert_contains "disable: app.local still present" "app.local" "$OUT"

$HOSTAB e enable api.local -q
OUT=$($HOSTAB cat)
assert_contains "enable: merged back" "app.local api.local" "$OUT"
assert_not_contains "enable: no disabled line" "# 10.0.0.1 api.local" "$OUT"

$HOSTAB e toggle api.local -q
OUT=$($HOSTAB cat)
assert_contains "toggle: disabled" "# 10.0.0.1 api.local" "$OUT"

$HOSTAB e toggle api.local -q
OUT=$($HOSTAB cat)
assert_contains "toggle: back" "app.local api.local" "$OUT"

# ── 4b. edit (move hostname to new IP) ─────────────────────────
echo "--- 4b. edit ---"

$HOSTAB e edit api.local --ip 10.99.0.99 -q
OUT=$($HOSTAB cat)
assert_contains "edit: api.local on new IP" "10.99.0.99 api.local" "$OUT"
assert_contains "edit: app.local still on original" "10.0.0.1 app.local" "$OUT"
assert_not_contains "edit: api.local gone from old IP" "app.local api.local" "$OUT"

# Move back
$HOSTAB e edit api.local --ip 10.0.0.1 -q

# ── 5. verify ──────────────────────────────────────────────────
echo "--- 5. verify ---"

OUT=$($HOSTAB verify)
assert_contains "verify: duplicate localhost" "localhost" "$OUT"
assert_contains "verify: table header" "LINE" "$OUT"

# ── 6. merge ───────────────────────────────────────────────────
echo "--- 6. merge ---"

echo "10.99.0.1 merge-a.local" > "$WORK_DIR/merge_a"
echo "10.99.0.2 merge-b.local # comment" > "$WORK_DIR/merge_b"
$HOSTAB merge -s "$WORK_DIR/merge_a" -s "$WORK_DIR/merge_b" -t "$WORK_DIR/merged" -q
OUT=$(cat "$WORK_DIR/merged")
assert_contains "merge: file a" "merge-a.local" "$OUT"
assert_contains "merge: annotation" "### source:" "$OUT"

echo "10.99.0.3 merge-c.local" > "$WORK_DIR/merge_c"
$HOSTAB merge -s "$WORK_DIR/merge_c" -t "$WORK_DIR/merged" -q
OUT=$(cat "$WORK_DIR/merged")
assert_not_contains "merge: old gone" "merge-a.local" "$OUT"
assert_contains "merge: new present" "merge-c.local" "$OUT"

# ── 7. cat ─────────────────────────────────────────────────────
echo "--- 7. cat ---"

OUT=$($HOSTAB cat)
assert_contains "cat: raw" "localhost" "$OUT"

# ── 8. completion ──────────────────────────────────────────────
echo "--- 8. completion ---"

OUT=$($HOSTAB completion bash)
assert_contains "completion bash" "complete" "$OUT"

# ── Summary ────────────────────────────────────────────────────
echo ""
echo "========================================"
echo -e "Results: ${GREEN}$pass_count passed${NC}, ${RED}$fail_count failed${NC}"
echo "========================================"
[ "$fail_count" -gt 0 ] && exit 1
