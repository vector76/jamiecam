#!/usr/bin/env bash
# run_cpp_tests.sh — compile and run the C++ unit tests for handle_registry.
#
# The tests use mock types so no OCCT installation is required.
#
# Usage:
#   bash src-tauri/cpp/tests/run_cpp_tests.sh
#   (or from the tests/ directory)  bash run_cpp_tests.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CPP_DIR="$(dirname "$SCRIPT_DIR")"
BIN="$SCRIPT_DIR/test_handle_registry"

CXX="${CXX:-g++}"

echo "Compiling $BIN ..."
"$CXX" -std=c++17 -I"$CPP_DIR" -pthread -Wall -Wextra \
    "$SCRIPT_DIR/test_handle_registry.cpp" \
    -o "$BIN"

echo "Running tests ..."
"$BIN"
