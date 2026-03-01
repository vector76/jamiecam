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

// ---------------------------------------------------------------------------
// Test suite: 2D polygon operations (Clipper2)
// ---------------------------------------------------------------------------

// Helper: unit square [0,1]x[0,1] as flat xy pairs (4 points, CCW).
static const double kUnitSquare[] = {
    0.0, 0.0,
    1.0, 0.0,
    1.0, 1.0,
    0.0, 1.0,
};
static const size_t kUnitSquareCount = 4;

// Helper: compute signed area of a flat xy polygon.
static double poly_area(const double* pts, size_t n) {
    double area = 0.0;
    for (size_t i = 0; i < n; ++i) {
        size_t j = (i + 1) % n;
        area += pts[i*2] * pts[j*2+1] - pts[j*2] * pts[i*2+1];
    }
    return area * 0.5;
}

TEST_SUITE("poly_offset") {

TEST_CASE("inward offset of unit square returns smaller polygon") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_offset(kUnitSquare, kUnitSquareCount,
                                 -0.1, 0.01, &out, &cnt);
    INFO("last error: " << last_error());
    REQUIRE(err == CG_OK);
    REQUIRE(cnt > 0);
    double area = std::abs(poly_area(out, cnt));
    CHECK(area < 0.85);   // inset by 0.1 on each side → 0.8×0.8 = 0.64 mm²
    CHECK(area > 0.55);
    cg_poly_free(out);
}

TEST_CASE("outward offset of unit square returns larger polygon") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_offset(kUnitSquare, kUnitSquareCount,
                                 0.1, 0.01, &out, &cnt);
    INFO("last error: " << last_error());
    REQUIRE(err == CG_OK);
    REQUIRE(cnt > 0);
    double area = std::abs(poly_area(out, cnt));
    CHECK(area > 1.2);   // expanded by 0.1 on each side, plus rounded corners
    cg_poly_free(out);
}

TEST_CASE("large inward offset collapses polygon and returns CG_ERR_NO_RESULT") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_offset(kUnitSquare, kUnitSquareCount,
                                 -10.0, 0.01, &out, &cnt);
    CHECK(err == CG_ERR_NO_RESULT);
    CHECK(out == nullptr);
    CHECK(cnt == 0);
}

TEST_CASE("cg_poly_free of nullptr does not crash") {
    cg_poly_free(nullptr);
}

} // TEST_SUITE poly_offset

// Two overlapping unit squares offset by 0.5 in X — overlap is 0.5×1.
static const double kSquareB[] = {
    0.5, 0.0,
    1.5, 0.0,
    1.5, 1.0,
    0.5, 1.0,
};
static const size_t kSquareBCount = 4;

TEST_SUITE("poly_boolean") {

TEST_CASE("intersection of two overlapping squares returns overlap region") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_boolean(kUnitSquare, kUnitSquareCount,
                                   kSquareB,   kSquareBCount,
                                   CG_BOOL_INTERSECTION,
                                   &out, &cnt);
    INFO("last error: " << last_error());
    REQUIRE(err == CG_OK);
    REQUIRE(cnt > 0);
    double area = std::abs(poly_area(out, cnt));
    CHECK(area > 0.45);   // overlap is 0.5×1 = 0.5 mm²
    CHECK(area < 0.55);
    cg_poly_free(out);
}

TEST_CASE("union of two overlapping squares returns merged region") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_boolean(kUnitSquare, kUnitSquareCount,
                                   kSquareB,   kSquareBCount,
                                   CG_BOOL_UNION,
                                   &out, &cnt);
    INFO("last error: " << last_error());
    REQUIRE(err == CG_OK);
    REQUIRE(cnt > 0);
    double area = std::abs(poly_area(out, cnt));
    CHECK(area > 1.4);   // union of two 1 mm² squares with 0.5 overlap = 1.5 mm²
    CHECK(area < 1.6);
    cg_poly_free(out);
}

TEST_CASE("difference removes clip region from subject") {
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_boolean(kUnitSquare, kUnitSquareCount,
                                   kSquareB,   kSquareBCount,
                                   CG_BOOL_DIFFERENCE,
                                   &out, &cnt);
    INFO("last error: " << last_error());
    REQUIRE(err == CG_OK);
    REQUIRE(cnt > 0);
    double area = std::abs(poly_area(out, cnt));
    CHECK(area > 0.45);   // remainder is 0.5×1 = 0.5 mm²
    CHECK(area < 0.55);
    cg_poly_free(out);
}

TEST_CASE("intersection of non-overlapping squares returns CG_ERR_NO_RESULT") {
    static const double far_square[] = {
        5.0, 5.0,
        6.0, 5.0,
        6.0, 6.0,
        5.0, 6.0,
    };
    double* out = nullptr;
    size_t  cnt = 0;
    CgError err = cg_poly_boolean(kUnitSquare, kUnitSquareCount,
                                   far_square, 4,
                                   CG_BOOL_INTERSECTION,
                                   &out, &cnt);
    CHECK(err == CG_ERR_NO_RESULT);
    CHECK(out == nullptr);
    CHECK(cnt == 0);
}

} // TEST_SUITE poly_boolean
