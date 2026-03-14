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

// OCCT headers for fixture generation
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <STEPControl_Writer.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>
#include <TopoDS_Shape.hxx>
#include <IFSelect_ReturnStatus.hxx>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <string>
#include <vector>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#ifndef FIXTURES_DIR
#  error "FIXTURES_DIR must be defined via -DFIXTURES_DIR=... at compile time"
#endif

static const char* STEP_PATH   = FIXTURES_DIR "/box.step";
static const char* STL_PATH    = FIXTURES_DIR "/box.stl";
static const char* SPHERE_PATH = FIXTURES_DIR "/sphere.step";

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

// ---------------------------------------------------------------------------
// Test suite: shape section at Z
// ---------------------------------------------------------------------------

TEST_SUITE("shape_section_at_z") {

TEST_CASE("section of 10x10x10 box at mid-height returns points") {
    CgShapeId id = cg_load_step(STEP_PATH);
    REQUIRE(id != CG_NULL_ID);

    CgPoint3* pts = nullptr;
    size_t    cnt = 0;
    CgError   err = cg_shape_section_at_z(id, 5.0, &pts, &cnt);
    INFO("last error: " << last_error());

    REQUIRE(err == CG_OK);
    REQUIRE(pts != nullptr);
    // A rectangular cross-section has 4 edges → 8 endpoint pairs
    REQUIRE(cnt > 0);
    CHECK(cnt % 2 == 0);   // always pairs of start/end points
    CHECK(cnt >= 8);        // at least 4 edges × 2 endpoints

    cg_section_free(pts);
    cg_shape_free(id);
}

TEST_CASE("section outside box bounds returns CG_ERR_NO_RESULT") {
    CgShapeId id = cg_load_step(STEP_PATH);
    REQUIRE(id != CG_NULL_ID);

    CgPoint3* pts = nullptr;
    size_t    cnt = 0;
    CgError   err = cg_shape_section_at_z(id, 999.0, &pts, &cnt);

    CHECK(err == CG_ERR_NO_RESULT);
    CHECK(pts == nullptr);
    CHECK(cnt == 0);

    cg_shape_free(id);
}

TEST_CASE("null out_points returns CG_ERR_NULL_HANDLE") {
    CgShapeId id = cg_load_step(STEP_PATH);
    REQUIRE(id != CG_NULL_ID);

    size_t  cnt = 0;
    CgError err = cg_shape_section_at_z(id, 5.0, nullptr, &cnt);
    CHECK(err == CG_ERR_NULL_HANDLE);

    cg_shape_free(id);
}

TEST_CASE("CG_NULL_ID returns CG_ERR_NULL_HANDLE") {
    CgPoint3* pts = nullptr;
    size_t    cnt = 0;
    CgError   err = cg_shape_section_at_z(CG_NULL_ID, 5.0, &pts, &cnt);
    CHECK(err == CG_ERR_NULL_HANDLE);
    CHECK(pts == nullptr);
    CHECK(cnt == 0);
}

TEST_CASE("cg_section_free of nullptr does not crash") {
    cg_section_free(nullptr);
}

} // TEST_SUITE shape_section_at_z

// ---------------------------------------------------------------------------
// Test suite: STEP fixture generation — plate_with_holes.step
// ---------------------------------------------------------------------------
//
// Expected hole data (downstream tests assert against these values):
//   Hole 1: center=(25,25), diameter=10, depth=20, through=true
//   Hole 2: center=(75,25), diameter=6,  depth=20, through=true
//   Hole 3: center=(50,75), diameter=8,  depth=12, through=false (blind)
//   Hole 4: center=(25,75), diameter=5,  tilted 30° from Z —
//           should NOT appear in Z-parallel filtered results
// ---------------------------------------------------------------------------

static const char* PLATE_PATH = FIXTURES_DIR "/plate_with_holes.step";

TEST_SUITE("step_fixture_generation") {

TEST_CASE("generate plate_with_holes.step fixture and verify round-trip") {
    // --- Build the plate: 100×100×20 mm box ---
    TopoDS_Shape plate = BRepPrimAPI_MakeBox(100.0, 100.0, 20.0).Shape();

    // Helper: subtract a vertical cylinder at (cx, cy) from top face downward.
    auto subtract_vertical_hole = [&](double cx, double cy,
                                       double diameter, double depth) {
        double radius = diameter / 2.0;
        // Place cylinder axis at (cx, cy, 20-depth) pointing up, height = depth+1
        // For through-holes depth >= 20, start below the box.
        double z_start = 20.0 - depth;
        double cyl_height = depth + 1.0; // extra to ensure clean cut
        if (z_start <= 0.0) {
            cyl_height += (-z_start) + 0.5;
            z_start = -0.5; // start slightly below bottom face
        }
        gp_Ax2 ax(gp_Pnt(cx, cy, z_start), gp_Dir(0, 0, 1));
        TopoDS_Shape cyl = BRepPrimAPI_MakeCylinder(ax, radius, cyl_height).Shape();
        plate = BRepAlgoAPI_Cut(plate, cyl).Shape();
    };

    // Hole 1: through-hole at (25,25), diameter 10 mm
    subtract_vertical_hole(25.0, 25.0, 10.0, 20.0);

    // Hole 2: through-hole at (75,25), diameter 6 mm
    subtract_vertical_hole(75.0, 25.0, 6.0, 20.0);

    // Hole 3: blind hole at (50,75), diameter 8 mm, depth 12 mm
    subtract_vertical_hole(50.0, 75.0, 8.0, 12.0);

    // Hole 4: tilted hole at (25,75), diameter 5 mm, axis 30° from Z
    {
        double radius = 2.5;
        double tilt_rad = 30.0 * M_PI / 180.0;
        // Axis tilted 30° from Z toward X
        gp_Dir tilted_dir(std::sin(tilt_rad), 0.0, std::cos(tilt_rad));
        gp_Ax2 ax(gp_Pnt(25.0, 75.0, 20.0), tilted_dir);
        // Cylinder long enough to penetrate the plate and extend past both faces
        TopoDS_Shape cyl = BRepPrimAPI_MakeCylinder(ax, radius, 40.0).Shape();
        // Shift so it starts below the bottom face and extends past the top
        gp_Trsf shift;
        shift.SetTranslation(gp_Vec(
            -30.0 * std::sin(tilt_rad), 0.0, -30.0 * std::cos(tilt_rad)));
        TopoDS_Shape shifted = BRepBuilderAPI_Transform(cyl, shift, true).Shape();
        plate = BRepAlgoAPI_Cut(plate, shifted).Shape();
    }

    // --- Export to STEP ---
    STEPControl_Writer writer;
    IFSelect_ReturnStatus ws = writer.Transfer(plate, STEPControl_AsIs);
    REQUIRE(ws == IFSelect_RetDone);

    IFSelect_ReturnStatus stat = writer.Write(PLATE_PATH);
    REQUIRE(stat == IFSelect_RetDone);

    // --- Round-trip: load back via C API ---
    CgShapeId id = cg_load_step(PLATE_PATH);
    INFO("last error: " << last_error());
    REQUIRE(id != CG_NULL_ID);

    // Verify bounding box is approximately 100×100×20
    CgBbox bb = cg_shape_bounding_box(id);
    CHECK(bb.xmax - bb.xmin == doctest::Approx(100.0).epsilon(1e-3));
    CHECK(bb.ymax - bb.ymin == doctest::Approx(100.0).epsilon(1e-3));
    CHECK(bb.zmax - bb.zmin == doctest::Approx(20.0).epsilon(1e-3));

    cg_shape_free(id);
}

} // TEST_SUITE step_fixture_generation

// ---------------------------------------------------------------------------
// Test suite: Hole detection — cg_shape_find_holes
// ---------------------------------------------------------------------------

TEST_SUITE("hole_detection") {

TEST_CASE("plate_with_holes returns 3 Z-parallel holes") {
    CgShapeId id = cg_load_step(PLATE_PATH);
    INFO("last error: " << last_error());
    REQUIRE(id != CG_NULL_ID);

    CgHoleInfo* holes = nullptr;
    size_t count = cg_shape_find_holes(id, 0.0, 1000.0, &holes);
    INFO("last error: " << last_error());
    REQUIRE(count == 3);
    REQUIRE(holes != nullptr);

    // Sort holes by diameter for deterministic assertions.
    std::vector<CgHoleInfo> sorted(holes, holes + count);
    std::sort(sorted.begin(), sorted.end(),
              [](const CgHoleInfo& a, const CgHoleInfo& b) {
                  return a.diameter < b.diameter;
              });

    // Hole with diameter 6mm: center=(75,25), depth=20, through
    CHECK(sorted[0].diameter == doctest::Approx(6.0).epsilon(1e-3));
    CHECK(sorted[0].center.x == doctest::Approx(75.0).epsilon(1e-3));
    CHECK(sorted[0].center.y == doctest::Approx(25.0).epsilon(1e-3));
    CHECK(sorted[0].depth == doctest::Approx(20.0).epsilon(1e-3));
    CHECK(sorted[0].is_through == 1);

    // Hole with diameter 8mm: center=(50,75), depth=12, blind
    CHECK(sorted[1].diameter == doctest::Approx(8.0).epsilon(1e-3));
    CHECK(sorted[1].center.x == doctest::Approx(50.0).epsilon(1e-3));
    CHECK(sorted[1].center.y == doctest::Approx(75.0).epsilon(1e-3));
    CHECK(sorted[1].depth == doctest::Approx(12.0).epsilon(1e-3));
    CHECK(sorted[1].is_through == 0);

    // Hole with diameter 10mm: center=(25,25), depth=20, through
    CHECK(sorted[2].diameter == doctest::Approx(10.0).epsilon(1e-3));
    CHECK(sorted[2].center.x == doctest::Approx(25.0).epsilon(1e-3));
    CHECK(sorted[2].center.y == doctest::Approx(25.0).epsilon(1e-3));
    CHECK(sorted[2].depth == doctest::Approx(20.0).epsilon(1e-3));
    CHECK(sorted[2].is_through == 1);

    cg_holes_free(holes);
    cg_shape_free(id);
}

TEST_CASE("diameter filter restricts results") {
    CgShapeId id = cg_load_step(PLATE_PATH);
    REQUIRE(id != CG_NULL_ID);

    CgHoleInfo* holes = nullptr;
    size_t count = cg_shape_find_holes(id, 7.0, 11.0, &holes);
    INFO("last error: " << last_error());
    REQUIRE(count == 2);
    REQUIRE(holes != nullptr);

    // Should return only 8mm and 10mm holes.
    std::vector<CgHoleInfo> sorted(holes, holes + count);
    std::sort(sorted.begin(), sorted.end(),
              [](const CgHoleInfo& a, const CgHoleInfo& b) {
                  return a.diameter < b.diameter;
              });
    CHECK(sorted[0].diameter == doctest::Approx(8.0).epsilon(1e-3));
    CHECK(sorted[1].diameter == doctest::Approx(10.0).epsilon(1e-3));

    cg_holes_free(holes);
    cg_shape_free(id);
}

TEST_CASE("model with no cylindrical faces returns 0 holes") {
    // Load the plain box fixture (no holes).
    static const char* BOX_PATH = FIXTURES_DIR "/box.step";
    CgShapeId id = cg_load_step(BOX_PATH);
    REQUIRE(id != CG_NULL_ID);

    CgHoleInfo* holes = nullptr;
    size_t count = cg_shape_find_holes(id, 0.0, 1000.0, &holes);
    CHECK(count == 0);
    CHECK(holes == nullptr);
    // No error should be set for zero results on a valid shape.

    cg_shape_free(id);
}

} // TEST_SUITE hole_detection

// ---------------------------------------------------------------------------
// Helper: generate sphere.step if it doesn't already exist
// ---------------------------------------------------------------------------

static void ensure_sphere_fixture() {
    // Try to load it first; if it succeeds, we're done.
    CgShapeId probe = cg_load_step(SPHERE_PATH);
    if (probe != CG_NULL_ID) {
        cg_shape_free(probe);
        return;
    }

    TopoDS_Shape sphere = BRepPrimAPI_MakeSphere(10.0).Shape();
    STEPControl_Writer writer;
    IFSelect_ReturnStatus ws = writer.Transfer(sphere, STEPControl_AsIs);
    REQUIRE(ws == IFSelect_RetDone);
    IFSelect_ReturnStatus stat = writer.Write(SPHERE_PATH);
    REQUIRE(stat == IFSelect_RetDone);
}

// ---------------------------------------------------------------------------
// Test suite: surface evaluation
// ---------------------------------------------------------------------------

TEST_SUITE("surface_evaluation") {

TEST_CASE("setup sphere fixture") {
    ensure_sphere_fixture();
}

TEST_CASE("cg_face_eval_point on box face returns finite coordinates") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgUVBounds uv = cg_face_uv_bounds(faces[0]);
    double u_mid = 0.5 * (uv.umin + uv.umax);
    double v_mid = 0.5 * (uv.vmin + uv.vmax);

    CgPoint3 pt = cg_face_eval_point(faces[0], u_mid, v_mid);
    INFO("last error: " << last_error());
    CHECK(std::isfinite(pt.x));
    CHECK(std::isfinite(pt.y));
    CHECK(std::isfinite(pt.z));

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

TEST_CASE("cg_face_eval_normal returns unit vector on box face") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgUVBounds uv = cg_face_uv_bounds(faces[0]);
    double u_mid = 0.5 * (uv.umin + uv.umax);
    double v_mid = 0.5 * (uv.vmin + uv.vmax);

    CgVec3 n = cg_face_eval_normal(faces[0], u_mid, v_mid);
    INFO("last error: " << last_error());
    double len = std::sqrt(n.x*n.x + n.y*n.y + n.z*n.z);
    CHECK(len == doctest::Approx(1.0).epsilon(1e-6));

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

TEST_CASE("cg_face_project_point round-trips on sphere face") {
    CgShapeId shape = cg_load_step(SPHERE_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgUVBounds uv = cg_face_uv_bounds(faces[0]);
    double u = uv.umin + 0.1 * (uv.umax - uv.umin);
    double v = uv.vmin + 0.1 * (uv.vmax - uv.vmin);

    CgPoint3 orig = cg_face_eval_point(faces[0], u, v);

    double dist = 0.0;
    CgPoint2 uv_proj = cg_face_project_point(faces[0], orig, &dist);
    INFO("last error: " << last_error());

    CgPoint3 re_eval = cg_face_eval_point(faces[0], uv_proj.u, uv_proj.v);
    CHECK(re_eval.x == doctest::Approx(orig.x).epsilon(1e-6));
    CHECK(re_eval.y == doctest::Approx(orig.y).epsilon(1e-6));
    CHECK(re_eval.z == doctest::Approx(orig.z).epsilon(1e-6));

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

TEST_CASE("cg_face_surface_type on box face is CG_SURF_PLANE") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgSurfaceType t = cg_face_surface_type(faces[0]);
    INFO("last error: " << last_error());
    CHECK(t == CG_SURF_PLANE);

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

TEST_CASE("cg_face_surface_type on sphere face is CG_SURF_SPHERE") {
    CgShapeId shape = cg_load_step(SPHERE_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgSurfaceType t = cg_face_surface_type(faces[0]);
    INFO("last error: " << last_error());
    CHECK(t == CG_SURF_SPHERE);

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

TEST_CASE("cg_face_uv_bounds gives sensible range on box face") {
    CgShapeId shape = cg_load_step(STEP_PATH);
    REQUIRE(shape != CG_NULL_ID);

    const size_t kCap = 64;
    CgFaceId faces[kCap];
    size_t nfaces = cg_shape_faces(shape, faces, kCap);
    REQUIRE(nfaces > 0);

    CgUVBounds uv = cg_face_uv_bounds(faces[0]);
    INFO("last error: " << last_error());
    CHECK(uv.umin < uv.umax);
    CHECK(uv.vmin < uv.vmax);

    for (size_t i = 0; i < nfaces; ++i) cg_face_free(faces[i]);
    cg_shape_free(shape);
}

} // TEST_SUITE surface_evaluation
