#!/usr/bin/env bash
# run_cam_geometry_tests.sh
#
# Compile and run cam_geometry C API contract tests.
#
# Two test targets:
#   1. STUB tests (always run): link against cam_geometry_stub.cpp — no OCCT needed.
#   2. OCCT tests (skipped when OCCT is absent): link against the real
#      cam_geometry.cpp + handle_registry.cpp — requires OCCT headers and libs.
#
# Usage:
#   bash src-tauri/cpp/tests/run_cam_geometry_tests.sh
#   OCCT_INCLUDE_DIR=/usr/include/opencascade OCCT_LIB_DIR=/usr/lib/x86_64-linux-gnu \
#       bash src-tauri/cpp/tests/run_cam_geometry_tests.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CPP_DIR="$(dirname "$SCRIPT_DIR")"
CXX="${CXX:-g++}"

# ── 1. Stub tests (no OCCT) ──────────────────────────────────────────────────
echo "=== Building stub tests ==="
STUB_BIN="$SCRIPT_DIR/test_cam_geometry_stub"
"$CXX" -std=c++17 -I"$CPP_DIR" -Wall -Wextra \
    "$SCRIPT_DIR/test_cam_geometry.cpp" \
    "$SCRIPT_DIR/cam_geometry_stub.cpp" \
    -o "$STUB_BIN"

echo "=== Running stub tests ==="
"$STUB_BIN"

# ── 2. OCCT integration tests (conditional) ──────────────────────────────────
OCCT_INCLUDE="${OCCT_INCLUDE_DIR:-}"
OCCT_LIB="${OCCT_LIB_DIR:-}"

# Try to auto-detect if not set.
if [[ -z "$OCCT_INCLUDE" ]]; then
    for candidate in /usr/include/opencascade /usr/local/include/opencascade \
                     /opt/opencascade/include/opencascade; do
        if [[ -f "$candidate/Standard.hxx" ]]; then
            OCCT_INCLUDE="$candidate"
            break
        fi
    done
fi
if [[ -z "$OCCT_LIB" ]]; then
    for candidate in /usr/lib/x86_64-linux-gnu /usr/lib /usr/local/lib \
                     /opt/opencascade/lib; do
        if ls "$candidate"/libTKBRep.* 2>/dev/null | grep -q .; then
            OCCT_LIB="$candidate"
            break
        fi
    done
fi

if [[ -z "$OCCT_INCLUDE" || -z "$OCCT_LIB" ]]; then
    echo ""
    echo "=== OCCT integration tests: SKIPPED (OCCT not found) ==="
    echo "    Set OCCT_INCLUDE_DIR and OCCT_LIB_DIR to enable."
    exit 0
fi

echo ""
echo "=== Building OCCT integration tests (OCCT found at $OCCT_INCLUDE) ==="

OCCT_BIN="$SCRIPT_DIR/test_cam_geometry_occt"
"$CXX" -std=c++17 \
    -I"$CPP_DIR" \
    -I"$OCCT_INCLUDE" \
    "$SCRIPT_DIR/test_cam_geometry.cpp" \
    "$CPP_DIR/cam_geometry.cpp" \
    "$CPP_DIR/handle_registry.cpp" \
    -L"$OCCT_LIB" \
    -lTKBRep -lTKernel -lTKMath -lTKGeomBase -lTKGeom3d \
    -lTKMesh -lTKSTL -lTKXSBase -lTKSTEP -lTKSTEPBase -lTKSTEPAttr -lTKShHealing \
    -Wl,-rpath,"$OCCT_LIB" \
    -pthread \
    -o "$OCCT_BIN"

echo "=== Running OCCT integration tests ==="
"$OCCT_BIN"
