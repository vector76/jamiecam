#!/usr/bin/env bash
# Unit tests for scripts/check-env.sh
#
# Run from repo root:
#   bash scripts/tests/check-env-test.sh
#
# Each test runs in a subshell that sources check-env.sh, then overrides
# _pass/_fail with silent versions so output doesn't interfere with counter
# capture.  Mock binaries are written to a temp dir prepended to PATH.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CHECK_ENV="$REPO_ROOT/scripts/check-env.sh"

# ── Test infrastructure ───────────────────────────────────────────────────────
_T_PASS=0
_T_FAIL=0
_T_ERRORS=()

t_pass() { printf 'PASS: %s\n' "$1"; (( _T_PASS++ )) || true; }
t_fail() { printf 'FAIL: %s\n' "$1"; (( _T_FAIL++ )) || true; _T_ERRORS+=("$1"); }

# assert_counter DESCRIPTION EXPECTED ACTUAL
assert_counter() {
    local desc="$1" expected="$2" actual="$3"
    if [ "$actual" -eq "$expected" ] 2>/dev/null; then
        t_pass "$desc"
    else
        t_fail "$desc — expected $expected, got '$actual'"
    fi
}

# ── Temporary directory for mock files and binaries ──────────────────────────
TMPDIR_TEST=$(mktemp -d)
trap 'rm -rf "$TMPDIR_TEST"' EXIT

MOCK_BIN="$TMPDIR_TEST/bin"
mkdir -p "$MOCK_BIN"

# make_mock_cmd NAME OUTPUT — creates a stub executable that prints OUTPUT
make_mock_cmd() {
    local name="$1" output="$2"
    printf '#!/usr/bin/env bash\nprintf "%%s\\n" "%s"\n' "$output" \
        > "$MOCK_BIN/$name"
    chmod +x "$MOCK_BIN/$name"
}

# Mock OCCT / libclang directory trees
MOCK_OCCT_INCLUDE="$TMPDIR_TEST/occt-include"
MOCK_OCCT_INCLUDE_EMPTY="$TMPDIR_TEST/occt-include-empty"
MOCK_OCCT_LIB="$TMPDIR_TEST/occt-lib"
MOCK_OCCT_LIB_EMPTY="$TMPDIR_TEST/occt-lib-empty"
MOCK_LIBCLANG="$TMPDIR_TEST/libclang"
MOCK_LIBCLANG_EMPTY="$TMPDIR_TEST/libclang-empty"

mkdir -p "$MOCK_OCCT_INCLUDE" "$MOCK_OCCT_INCLUDE_EMPTY" \
         "$MOCK_OCCT_LIB"     "$MOCK_OCCT_LIB_EMPTY" \
         "$MOCK_LIBCLANG"     "$MOCK_LIBCLANG_EMPTY"

touch "$MOCK_OCCT_INCLUDE/Standard.hxx"
touch "$MOCK_OCCT_LIB/libTKBRep.so"
touch "$MOCK_LIBCLANG/libclang.so.16"

# Source check-env.sh to get version_ge (the source guard prevents main)
# shellcheck source=../check-env.sh
source "$CHECK_ENV"

echo "=== check-env.sh unit tests ==="
echo ""

# ── version_ge ────────────────────────────────────────────────────────────────
echo "--- version_ge ---"

version_ge 1.77.0 1.77.0 \
    && t_pass "version_ge: 1.77.0 >= 1.77.0 (equal)"    \
    || t_fail "version_ge: 1.77.0 >= 1.77.0 (equal)"

version_ge 1.80.0 1.77.0 \
    && t_pass "version_ge: 1.80.0 >= 1.77.0 (greater)"  \
    || t_fail "version_ge: 1.80.0 >= 1.77.0 (greater)"

version_ge 2.0.0 1.77.0 \
    && t_pass "version_ge: 2.0.0 >= 1.77.0 (major up)"  \
    || t_fail "version_ge: 2.0.0 >= 1.77.0 (major up)"

version_ge 1.76.9 1.77.0 \
    && t_fail "version_ge: 1.76.9 < 1.77.0 should fail" \
    || t_pass "version_ge: 1.76.9 < 1.77.0 (correctly rejected)"

version_ge 0.9.0 1.0.0 \
    && t_fail "version_ge: 0.9.0 < 1.0.0 should fail"   \
    || t_pass "version_ge: 0.9.0 < 1.0.0 (correctly rejected)"

# ── check_occt_include ────────────────────────────────────────────────────────
echo ""
echo "--- check_occt_include ---"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    unset OCCT_INCLUDE_DIR
    check_occt_include
    echo "$_FAIL"
)
assert_counter "check_occt_include: unset → _FAIL=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    OCCT_INCLUDE_DIR="$MOCK_OCCT_INCLUDE" check_occt_include
    echo "$_PASS"
)
assert_counter "check_occt_include: valid dir with Standard.hxx → _PASS=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    OCCT_INCLUDE_DIR="$MOCK_OCCT_INCLUDE_EMPTY" check_occt_include
    echo "$_FAIL"
)
assert_counter "check_occt_include: dir without Standard.hxx → _FAIL=1" 1 "$result"

# ── check_occt_lib ────────────────────────────────────────────────────────────
echo ""
echo "--- check_occt_lib ---"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    unset OCCT_LIB_DIR
    check_occt_lib
    echo "$_FAIL"
)
assert_counter "check_occt_lib: unset → _FAIL=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    OCCT_LIB_DIR="$MOCK_OCCT_LIB" check_occt_lib
    echo "$_PASS"
)
assert_counter "check_occt_lib: valid dir with libTKBRep → _PASS=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    OCCT_LIB_DIR="$MOCK_OCCT_LIB_EMPTY" check_occt_lib
    echo "$_FAIL"
)
assert_counter "check_occt_lib: dir without libTKBRep → _FAIL=1" 1 "$result"

# ── check_libclang ────────────────────────────────────────────────────────────
echo ""
echo "--- check_libclang ---"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    unset LIBCLANG_PATH
    check_libclang
    echo "$_FAIL"
)
assert_counter "check_libclang: unset → _FAIL=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    LIBCLANG_PATH="$MOCK_LIBCLANG" check_libclang
    echo "$_PASS"
)
assert_counter "check_libclang: valid dir with libclang → _PASS=1" 1 "$result"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    LIBCLANG_PATH="$MOCK_LIBCLANG_EMPTY" check_libclang
    echo "$_FAIL"
)
assert_counter "check_libclang: dir without libclang → _FAIL=1" 1 "$result"

# ── check_rustc (via mock binaries) ──────────────────────────────────────────
echo ""
echo "--- check_rustc ---"

make_mock_cmd "rustc" "rustc 1.80.0 (abc123 2024-07-01)"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_rustc
    echo "$_PASS"
)
assert_counter "check_rustc: 1.80.0 >= 1.77.0 → _PASS=1" 1 "$result"

make_mock_cmd "rustc" "rustc 1.70.0 (abc123 2023-01-01)"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_rustc
    echo "$_FAIL"
)
assert_counter "check_rustc: 1.70.0 < 1.77.0 → _FAIL=1" 1 "$result"

# ── check_node (via mock binary) ─────────────────────────────────────────────
echo ""
echo "--- check_node ---"

make_mock_cmd "node" "v20.11.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_node
    echo "$_PASS"
)
assert_counter "check_node: v20 >= 20 → _PASS=1" 1 "$result"

make_mock_cmd "node" "v18.20.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_node
    echo "$_FAIL"
)
assert_counter "check_node: v18 < 20 → _FAIL=1" 1 "$result"

# ── check_pnpm (via mock binary) ─────────────────────────────────────────────
echo ""
echo "--- check_pnpm ---"

make_mock_cmd "pnpm" "9.12.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_pnpm
    echo "$_PASS"
)
assert_counter "check_pnpm: 9.12 >= 9 → _PASS=1" 1 "$result"

make_mock_cmd "pnpm" "8.15.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_pnpm
    echo "$_FAIL"
)
assert_counter "check_pnpm: 8.x < 9 → _FAIL=1" 1 "$result"

# ── check_cmake (via mock binary) ────────────────────────────────────────────
echo ""
echo "--- check_cmake ---"

make_mock_cmd "cmake" "cmake version 3.25.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_cmake
    echo "$_PASS"
)
assert_counter "check_cmake: 3.25 >= 3.20 → _PASS=1" 1 "$result"

make_mock_cmd "cmake" "cmake version 3.10.0"

result=$(
    source "$CHECK_ENV"
    _pass() { (( _PASS++ )) || true; }
    _fail() { (( _FAIL++ )) || true; }
    _PASS=0; _FAIL=0
    PATH="$MOCK_BIN:$PATH" check_cmake
    echo "$_FAIL"
)
assert_counter "check_cmake: 3.10 < 3.20 → _FAIL=1" 1 "$result"

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
