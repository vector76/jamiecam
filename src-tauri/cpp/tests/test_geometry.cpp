// test_geometry.cpp
//
// Integration tests for the cam_geometry C API.
//
// These tests require OCCT and the fixture files in tests/fixtures/.
// They are compiled as part of the CMake BUILD_TESTS=ON target and
// exercised by ctest, or run directly as ./test_geometry.
//
// Fixtures used:
//   FIXTURES_DIR/box.step  — 10×10×10 mm STEP AP214 box
//   FIXTURES_DIR/box.stl   — same box as binary STL (12 triangles)
//
// Build:
//   cmake -B build -DOCCT_INCLUDE_DIR=... -DOCCT_LIB_DIR=... -DBUILD_TESTS=ON
//   cmake --build build
//   ctest --test-dir build

#define DOCTEST_CONFIG_IMPLEMENT_WITH_MAIN
#include "doctest.h"

#include "cam_geometry.h"

#include <cstring>
#include <string>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#ifndef FIXTURES_DIR
#  error "FIXTURES_DIR must be defined via -DFIXTURES_DIR=... at compile time"
#endif

static const char* STEP_PATH = FIXTURES_DIR "/box.step";
static const char* STL_PATH  = FIXTURES_DIR "/box.stl";

static std::string last_error() {
    return std::string(cg_last_error_message());
}

// ---------------------------------------------------------------------------
// Test suite: STEP loading
// ---------------------------------------------------------------------------

TEST_SUITE("step_loading") {

TEST_CASE("load known STEP file returns non-null handle") {
    CgShapeId id = cg_load_step(STEP_PATH);
    INFO("last error: " << last_error());
    CHECK(id != CG_NULL_ID);
    if (id != CG_NULL_ID) cg_shape_free(id);
}

TEST_CASE("load STEP with null path returns CG_NULL_ID and sets error") {
    CgShapeId id = cg_load_step(nullptr);
    CHECK(id == CG_NULL_ID);
    CHECK(last_error().size() > 0);
}

TEST_CASE("load STEP with non-existent path returns CG_NULL_ID and sets error") {
    CgShapeId id = cg_load_step("/nonexistent/path/missing.step");
    CHECK(id == CG_NULL_ID);
    CHECK(last_error().size() > 0);
}

} // TEST_SUITE step_loading

// ---------------------------------------------------------------------------
// Test suite: STL loading
// ---------------------------------------------------------------------------

TEST_SUITE("stl_loading") {

TEST_CASE("load known STL file returns non-null mesh handle") {
    CgMeshId id = cg_load_stl(STL_PATH);
    INFO("last error: " << last_error());
    CHECK(id != CG_NULL_ID);
    if (id != CG_NULL_ID) cg_mesh_free(id);
}

TEST_CASE("load STL with null path returns CG_NULL_ID and sets error") {
    CgMeshId id = cg_load_stl(nullptr);
    CHECK(id == CG_NULL_ID);
    CHECK(last_error().size() > 0);
}

TEST_CASE("load STL with non-existent path returns CG_NULL_ID and sets error") {
    CgMeshId id = cg_load_stl("/nonexistent/path/missing.stl");
    CHECK(id == CG_NULL_ID);
    CHECK(last_error().size() > 0);
}

} // TEST_SUITE stl_loading

// ---------------------------------------------------------------------------
// Test suite: tessellation
// ---------------------------------------------------------------------------

TEST_SUITE("tessellation") {

TEST_CASE("tessellate STEP shape produces non-empty mesh") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    CgMeshId mesh = cg_shape_tessellate(shape, 0.1, 0.5);
    INFO("last error: " << last_error());
    CHECK(mesh != CG_NULL_ID);

    if (mesh != CG_NULL_ID) {
        CHECK(cg_mesh_vertex_count(mesh) > 0);
        CHECK(cg_mesh_triangle_count(mesh) > 0);
        cg_mesh_free(mesh);
    }
    cg_shape_free(shape);
}

TEST_CASE("tessellated box mesh vertex and triangle counts are plausible") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    CgMeshId mesh = cg_shape_tessellate(shape, 0.1, 0.5);
    REQUIRE(mesh != CG_NULL_ID);

    size_t nv = cg_mesh_vertex_count(mesh);
    size_t nt = cg_mesh_triangle_count(mesh);
    // A box has 6 rectangular faces; even a coarse tessellation produces >= 12 triangles.
    CHECK(nt >= 12);
    // Each triangle has 3 vertices; shared vertices reduce total but nv >= nt is typical.
    CHECK(nv >= 8);

    cg_mesh_free(mesh);
    cg_shape_free(shape);
}

TEST_CASE("tessellate with null handle returns CG_NULL_ID and sets error") {
    CgMeshId mesh = cg_shape_tessellate(CG_NULL_ID, 0.1, 0.5);
    CHECK(mesh == CG_NULL_ID);
    CHECK(last_error().size() > 0);
}

} // TEST_SUITE tessellation

// ---------------------------------------------------------------------------
// Test suite: bounding box
// ---------------------------------------------------------------------------

TEST_SUITE("bounding_box") {

TEST_CASE("bounding box of loaded STEP box is approximately 10x10x10") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    CgBbox bb = cg_shape_bounding_box(shape);
    // Allow 1e-3 mm tolerance for OCCT's internal precision.
    CHECK(bb.xmax - bb.xmin == doctest::Approx(10.0).epsilon(1e-3));
    CHECK(bb.ymax - bb.ymin == doctest::Approx(10.0).epsilon(1e-3));
    CHECK(bb.zmax - bb.zmin == doctest::Approx(10.0).epsilon(1e-3));

    cg_shape_free(shape);
}

} // TEST_SUITE bounding_box

// ---------------------------------------------------------------------------
// Test suite: mesh data copy
// ---------------------------------------------------------------------------

TEST_SUITE("mesh_data_copy") {

TEST_CASE("copy_vertices/normals/indices from STL mesh succeed") {
    CgMeshId mesh = cg_load_stl(STL_PATH);
    REQUIRE(mesh != CG_NULL_ID);

    size_t nv = cg_mesh_vertex_count(mesh);
    size_t nt = cg_mesh_triangle_count(mesh);
    REQUIRE(nv > 0);
    REQUIRE(nt > 0);

    std::vector<double>   verts(nv * 3);
    std::vector<double>   norms(nv * 3);
    std::vector<uint32_t> idxs(nt * 3);

    CHECK(cg_mesh_copy_vertices(mesh, verts.data()) == CG_OK);
    CHECK(cg_mesh_copy_normals(mesh, norms.data())  == CG_OK);
    CHECK(cg_mesh_copy_indices(mesh, idxs.data())   == CG_OK);

    // All indices must be valid vertex references.
    for (uint32_t idx : idxs) {
        CHECK(idx < static_cast<uint32_t>(nv));
    }

    cg_mesh_free(mesh);
}

TEST_CASE("STL box mesh has expected triangle count") {
    CgMeshId mesh = cg_load_stl(STL_PATH);
    REQUIRE(mesh != CG_NULL_ID);
    // Our fixture is 12 triangles (2 per face × 6 faces).
    CHECK(cg_mesh_triangle_count(mesh) == 12);
    cg_mesh_free(mesh);
}

} // TEST_SUITE mesh_data_copy

// ---------------------------------------------------------------------------
// Test suite: free / double-free safety
// ---------------------------------------------------------------------------

TEST_SUITE("free_safety") {

TEST_CASE("cg_shape_free does not crash; double-free is safe") {
    CgShapeId id = cg_load_step(STEP_PATH);
    REQUIRE(id != CG_NULL_ID);
    cg_shape_free(id);   // first free
    cg_shape_free(id);   // second free — must not crash
}

TEST_CASE("cg_mesh_free does not crash; double-free is safe") {
    CgMeshId id = cg_load_stl(STL_PATH);
    REQUIRE(id != CG_NULL_ID);
    cg_mesh_free(id);   // first free
    cg_mesh_free(id);   // second free — must not crash
}

TEST_CASE("cg_shape_free(CG_NULL_ID) is safe") {
    cg_shape_free(CG_NULL_ID);  // must not crash
}

TEST_CASE("cg_mesh_free(CG_NULL_ID) is safe") {
    cg_mesh_free(CG_NULL_ID);  // must not crash
}

} // TEST_SUITE free_safety
