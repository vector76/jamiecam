#!/usr/bin/env bash
# Unit tests for scripts/install-hooks.sh
#
# Run from repo root:
#   bash scripts/tests/install-hooks-test.sh
#
# Tests that install-hooks.sh produces a valid, executable pre-commit hook
# containing the expected commands, using a temporary git repository.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INSTALL_HOOKS="$REPO_ROOT/scripts/install-hooks.sh"

# ── Test infrastructure ───────────────────────────────────────────────────────
_T_PASS=0
_T_FAIL=0
_T_ERRORS=()

t_pass() { printf 'PASS: %s\n' "$1"; (( _T_PASS++ )) || true; }
t_fail() { printf 'FAIL: %s\n' "$1"; (( _T_FAIL++ )) || true; _T_ERRORS+=("$1"); }

assert_file_exists() {
    if [ -f "$2" ]; then t_pass "$1"; else t_fail "$1 — file not found: $2"; fi
}

assert_executable() {
    if [ -x "$2" ]; then t_pass "$1"; else t_fail "$1 — not executable: $2"; fi
}

assert_file_contains() {
    # Use -e to avoid grep treating leading '--' in patterns as option flags
    if grep -qF -e "$2" "$3" 2>/dev/null; then
        t_pass "$1"
    else
        t_fail "$1 — pattern '$2' not found in $3"
    fi
}

# ── Temporary git repo to run the installer against ──────────────────────────
TMPDIR_TEST=$(mktemp -d)
trap 'rm -rf "$TMPDIR_TEST"' EXIT

GIT_REPO="$TMPDIR_TEST/test-repo"
mkdir -p "$GIT_REPO"
git init -q "$GIT_REPO"

HOOK_FILE="$GIT_REPO/.git/hooks/pre-commit"

echo "=== install-hooks.sh unit tests ==="
echo ""

# Run the installer inside the temp repo
(cd "$GIT_REPO" && bash "$INSTALL_HOOKS") &>/dev/null

# ── Assertions ───────────────────────────────────────────────────────────────
assert_file_exists "hook file is created at .git/hooks/pre-commit" "$HOOK_FILE"
assert_executable  "hook file is executable"                        "$HOOK_FILE"

assert_file_contains "hook contains cargo fmt --check" \
    "cargo fmt" "$HOOK_FILE"
assert_file_contains "hook checks formatting (--check flag)" \
    "-- --check" "$HOOK_FILE"
assert_file_contains "hook contains cargo clippy" \
    "cargo clippy" "$HOOK_FILE"
assert_file_contains "hook enforces clippy warnings as errors" \
    "-D warnings" "$HOOK_FILE"
assert_file_contains "hook contains pnpm typecheck" \
    "pnpm typecheck" "$HOOK_FILE"
assert_file_contains "hook has set -euo pipefail (stops on first error)" \
    "set -euo pipefail" "$HOOK_FILE"

# Idempotency: running a second time should overwrite cleanly
(cd "$GIT_REPO" && bash "$INSTALL_HOOKS") &>/dev/null
assert_file_exists  "hook file still exists after second install" "$HOOK_FILE"
assert_executable   "hook still executable after second install"  "$HOOK_FILE"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "=================================="
printf 'Test results: %d passed, %d failed\n' "$_T_PASS" "$_T_FAIL"

if [ "${#_T_ERRORS[@]}" -gt 0 ]; then
    echo "Failed tests:"
    for err in "${_T_ERRORS[@]}"; do
        printf '  - %s\n' "$err"
    done
    exit 1
else
    echo "All tests passed!"
fi
