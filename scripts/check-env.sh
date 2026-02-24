#!/usr/bin/env bash
# JamieCam environment check script — Linux / macOS
#
# Verifies that all required tools and environment variables are present
# before attempting a build.  Exits non-zero if any check fails.
#
# Usage:
#   ./scripts/check-env.sh
#
# See docs/build-and-dev-setup.md for installation instructions.

set -euo pipefail

# ── Minimum required versions ─────────────────────────────────────────────────
MIN_RUSTC_VER="1.77.0"
MIN_NODE_MAJOR=20
MIN_PNPM_MAJOR=9
MIN_CMAKE_VER="3.20.0"

# ── ANSI colour helpers ───────────────────────────────────────────────────────
_red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
_green() { printf '\033[0;32m%s\033[0m\n' "$*"; }

# ── Result counters (globals so they survive sub-function calls) ──────────────
_PASS=0
_FAIL=0

_pass() { _green "  ✓ $*"; (( _PASS++ )) || true; }
_fail() { _red   "  ✗ $*"; (( _FAIL++ )) || true; }

# ── version_ge VER_A VER_B ────────────────────────────────────────────────────
# Returns 0 (true) if VER_A >= VER_B.
# Pure-bash implementation: portable across Linux and macOS (BSD sort does not
# support the GNU -V / --version-sort flag).
version_ge() {
    local a="$1" b="$2"
    local -a ap bp
    IFS='.' read -ra ap <<< "$a"
    IFS='.' read -ra bp <<< "$b"
    local i
    for i in 0 1 2; do
        local av="${ap[$i]:-0}" bv="${bp[$i]:-0}"
        if   [ "$av" -gt "$bv" ]; then return 0
        elif [ "$av" -lt "$bv" ]; then return 1
        fi
    done
    return 0  # equal → a >= b
}

# ── Individual checks ─────────────────────────────────────────────────────────

check_rustc() {
    if ! command -v rustc &>/dev/null; then
        _fail "rustc not found — install Rust: https://rustup.rs"
        return
    fi
    local ver
    ver=$(rustc --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    if version_ge "$ver" "$MIN_RUSTC_VER"; then
        _pass "rustc $ver (>= $MIN_RUSTC_VER)"
    else
        _fail "rustc $ver is below minimum $MIN_RUSTC_VER — run: rustup update stable"
    fi
}

check_cargo() {
    if command -v cargo &>/dev/null; then
        local ver
        ver=$(cargo --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
        _pass "cargo $ver"
    else
        _fail "cargo not found — install Rust: https://rustup.rs"
    fi
}

check_node() {
    if ! command -v node &>/dev/null; then
        _fail "node not found — install Node.js $MIN_NODE_MAJOR LTS: https://nodejs.org"
        return
    fi
    local ver major
    ver=$(node --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
    major=$(printf '%s' "$ver" | cut -d. -f1)
    if [ "$major" -ge "$MIN_NODE_MAJOR" ]; then
        _pass "node v$ver (>= $MIN_NODE_MAJOR)"
    else
        _fail "node v$ver is below minimum v$MIN_NODE_MAJOR — update Node.js"
    fi
}

check_pnpm() {
    if ! command -v pnpm &>/dev/null; then
        _fail "pnpm not found — run: npm install -g pnpm"
        return
    fi
    local ver major
    ver=$(pnpm --version | grep -oE '[0-9]+\.[0-9]+' | head -1)
    major=$(printf '%s' "$ver" | cut -d. -f1)
    if [ "$major" -ge "$MIN_PNPM_MAJOR" ]; then
        _pass "pnpm $ver (>= $MIN_PNPM_MAJOR)"
    else
        _fail "pnpm $ver is below minimum $MIN_PNPM_MAJOR — run: npm install -g pnpm"
    fi
}

check_cmake() {
    if ! command -v cmake &>/dev/null; then
        _fail "cmake not found — install cmake >= $MIN_CMAKE_VER"
        return
    fi
    local ver
    ver=$(cmake --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    if version_ge "$ver" "$MIN_CMAKE_VER"; then
        _pass "cmake $ver (>= $MIN_CMAKE_VER)"
    else
        _fail "cmake $ver is below minimum $MIN_CMAKE_VER — upgrade cmake"
    fi
}

check_occt_include() {
    if [ -z "${OCCT_INCLUDE_DIR:-}" ]; then
        _fail "OCCT_INCLUDE_DIR is not set — see docs/build-and-dev-setup.md"
        return
    fi
    if [ -f "${OCCT_INCLUDE_DIR}/Standard.hxx" ]; then
        _pass "OCCT_INCLUDE_DIR=${OCCT_INCLUDE_DIR} (Standard.hxx found)"
    else
        _fail "OCCT_INCLUDE_DIR=${OCCT_INCLUDE_DIR} set but Standard.hxx not found"
    fi
}

check_occt_lib() {
    if [ -z "${OCCT_LIB_DIR:-}" ]; then
        _fail "OCCT_LIB_DIR is not set — see docs/build-and-dev-setup.md"
        return
    fi
    if ls "${OCCT_LIB_DIR}/"*TKBRep* >/dev/null 2>&1; then
        _pass "OCCT_LIB_DIR=${OCCT_LIB_DIR} (libTKBRep found)"
    else
        _fail "OCCT_LIB_DIR=${OCCT_LIB_DIR} set but libTKBRep not found"
    fi
}

check_libclang() {
    if [ -z "${LIBCLANG_PATH:-}" ]; then
        _fail "LIBCLANG_PATH is not set — see docs/build-and-dev-setup.md"
        return
    fi
    if ls "${LIBCLANG_PATH}/"libclang* >/dev/null 2>&1; then
        _pass "LIBCLANG_PATH=${LIBCLANG_PATH} (libclang found)"
    else
        _fail "LIBCLANG_PATH=${LIBCLANG_PATH} set but libclang not found"
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    echo "JamieCam environment check"
    echo "=========================="
    echo ""
    echo "Toolchain:"
    check_rustc
    check_cargo
    check_node
    check_pnpm
    check_cmake
    echo ""
    echo "Environment variables:"
    check_occt_include
    check_occt_lib
    check_libclang
    echo ""
    printf 'Results: %d passed, %d failed\n' "$_PASS" "$_FAIL"
    if [ "$_FAIL" -gt 0 ]; then
        _red "Environment check FAILED — fix the issues above before building."
        echo "See docs/build-and-dev-setup.md for setup instructions."
        exit 1
    else
        _green "Environment check PASSED — ready to build!"
    fi
}

# Allow sourcing for unit tests without executing main
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
