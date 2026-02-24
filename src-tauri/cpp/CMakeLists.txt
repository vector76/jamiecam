cmake_minimum_required(VERSION 3.16)
project(cam_geometry CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ── OCCT paths ────────────────────────────────────────────────────────────────
# Pass via -DOCCT_INCLUDE_DIR=... and -DOCCT_LIB_DIR=... on the cmake command
# line, or set OCCT_INCLUDE_DIR / OCCT_LIB_DIR environment variables.

if(NOT DEFINED OCCT_INCLUDE_DIR AND DEFINED ENV{OCCT_INCLUDE_DIR})
    set(OCCT_INCLUDE_DIR "$ENV{OCCT_INCLUDE_DIR}")
endif()
if(NOT DEFINED OCCT_LIB_DIR AND DEFINED ENV{OCCT_LIB_DIR})
    set(OCCT_LIB_DIR "$ENV{OCCT_LIB_DIR}")
endif()

if(NOT DEFINED OCCT_INCLUDE_DIR OR NOT DEFINED OCCT_LIB_DIR)
    message(FATAL_ERROR
        "OCCT headers/libs not found.\n"
        "Set -DOCCT_INCLUDE_DIR=<path> and -DOCCT_LIB_DIR=<path>.\n"
        "Example: cmake -DOCCT_INCLUDE_DIR=/usr/include/opencascade "
        "-DOCCT_LIB_DIR=/usr/lib/x86_64-linux-gnu ..")
endif()

# ── Clipper2 sources ──────────────────────────────────────────────────────────
set(CLIPPER2_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party/Clipper2/Clipper2Lib")
set(CLIPPER2_SOURCES
    "${CLIPPER2_DIR}/src/clipper.engine.cpp"
    "${CLIPPER2_DIR}/src/clipper.offset.cpp"
    "${CLIPPER2_DIR}/src/clipper.rectclip.cpp"
)

# ── libcam_geometry static library ───────────────────────────────────────────
add_library(cam_geometry STATIC
    cam_geometry.cpp
    handle_registry.cpp
    ${CLIPPER2_SOURCES}
)

target_include_directories(cam_geometry PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${OCCT_INCLUDE_DIR}"
    "${CLIPPER2_DIR}/include"
)

target_link_directories(cam_geometry PUBLIC "${OCCT_LIB_DIR}")

target_link_libraries(cam_geometry PUBLIC
    TKBRep TKernel TKMath TKGeomBase TKGeom3d
    TKMesh TKSTL TKXSBase TKSTEP TKSTEPBase TKSTEPAttr TKShHealing
)

target_compile_options(cam_geometry PRIVATE -Wall -Wextra)

# ── Optional test binary ──────────────────────────────────────────────────────
option(BUILD_TESTS "Build C++ geometry tests" OFF)

if(BUILD_TESTS)
    # Resolve fixture directory relative to repo root (two levels above src-tauri/cpp/)
    set(FIXTURES_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../tests/fixtures")
    get_filename_component(FIXTURES_DIR "${FIXTURES_DIR}" ABSOLUTE)

    add_executable(test_geometry tests/test_geometry.cpp)
    target_include_directories(test_geometry PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}"
        "${CMAKE_CURRENT_SOURCE_DIR}/third_party/doctest"
    )
    target_compile_definitions(test_geometry PRIVATE
        FIXTURES_DIR="${FIXTURES_DIR}"
    )
    target_link_libraries(test_geometry PRIVATE cam_geometry)
    target_link_options(test_geometry PRIVATE
        "-Wl,-rpath,${OCCT_LIB_DIR}"
    )

    enable_testing()
    add_test(NAME geometry_tests COMMAND test_geometry)
endif()
