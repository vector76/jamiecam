// test_cam_geometry.cpp
//
// Unit tests for the cam_geometry.h C API.
//
// These tests are linked against cam_geometry_stub.cpp (no OCCT required) to
// verify the observable API contracts:
//   - cg_last_error_message() behaves correctly
//   - null / invalid handles are handled safely
//   - stub functions return the documented error codes
//   - null output pointers are handled safely
//
// Build (no OCCT needed):
//   g++ -std=c++17 -I.. test_cam_geometry.cpp cam_geometry_stub.cpp -o test_cam_geometry
// Run:
//   ./test_cam_geometry

#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>

#include "cam_geometry.h"

// ---------------------------------------------------------------------------
// Minimal test framework (same style as test_handle_registry.cpp)
// ---------------------------------------------------------------------------

static int g_pass = 0;
static int g_fail = 0;

static void pass(const char* label) {
    std::cout << "  PASS: " << label << "\n";
    ++g_pass;
}

static void fail(const char* label, const char* reason = "") {
    std::cout << "  FAIL: " << label;
    if (reason && reason[0]) std::cout << " (" << reason << ")";
    std::cout << "\n";
    ++g_fail;
}

#define ASSERT_TRUE(label, cond) \
    do { if (cond) pass(label); else fail(label, #cond " is false"); } while (0)

#define ASSERT_EQ(label, a, b) \
    do { if ((a) == (b)) pass(label); else fail(label, #a " != " #b); } while (0)

#define ASSERT_NE(label, a, b) \
    do { if ((a) != (b)) pass(label); else fail(label, #a " == " #b); } while (0)

#define TEST(name) static void test_##name()

// ---------------------------------------------------------------------------
// Group 1: Error message initialisation
// ---------------------------------------------------------------------------

TEST(error_message_initially_empty) {
    // On a fresh thread the error string should be empty.
    // (The stub initialises the thread-local to "" on first call.)
    const char* msg = cg_last_error_message();
    ASSERT_TRUE("cg_last_error_message() returns non-null pointer", msg != nullptr);
    ASSERT_EQ("initial error message is empty string", std::string(msg), std::string(""));
}

// ---------------------------------------------------------------------------
// Group 2: Null-path handling for import functions
// ---------------------------------------------------------------------------

TEST(load_step_null_path) {
    CgShapeId id = cg_load_step(nullptr);
    ASSERT_EQ("cg_load_step(null) returns CG_NULL_ID", id, CG_NULL_ID);
    ASSERT_TRUE("cg_load_step(null) sets error message",
                std::string(cg_last_error_message()).size() > 0);
}

TEST(load_stl_null_path) {
    CgMeshId id = cg_load_stl(nullptr);
    ASSERT_EQ("cg_load_stl(null) returns CG_NULL_ID", id, CG_NULL_ID);
    ASSERT_TRUE("cg_load_stl(null) sets error message",
                std::string(cg_last_error_message()).size() > 0);
}

TEST(load_step_missing_file) {
    CgShapeId id = cg_load_step("/nonexistent/path/file.step");
    ASSERT_EQ("cg_load_step(missing) returns CG_NULL_ID", id, CG_NULL_ID);
    ASSERT_TRUE("cg_load_step(missing) sets error message",
                std::string(cg_last_error_message()).size() > 0);
}

TEST(load_stl_missing_file) {
    CgMeshId id = cg_load_stl("/nonexistent/path/file.stl");
    ASSERT_EQ("cg_load_stl(missing) returns CG_NULL_ID", id, CG_NULL_ID);
    ASSERT_TRUE("cg_load_stl(missing) sets error message",
                std::string(cg_last_error_message()).size() > 0);
}

// ---------------------------------------------------------------------------
// Group 3: Null-handle free operations are no-ops
// ---------------------------------------------------------------------------

TEST(shape_free_null_is_noop) {
    cg_shape_free(CG_NULL_ID); // must not crash
    pass("cg_shape_free(CG_NULL_ID) does not crash");
}

TEST(mesh_free_null_is_noop) {
    cg_mesh_free(CG_NULL_ID); // must not crash
    pass("cg_mesh_free(CG_NULL_ID) does not crash");
}

TEST(face_free_null_is_noop) {
    cg_face_free(CG_NULL_ID);
    pass("cg_face_free(CG_NULL_ID) does not crash");
}

TEST(edge_free_null_is_noop) {
    cg_edge_free(CG_NULL_ID);
    pass("cg_edge_free(CG_NULL_ID) does not crash");
}

// ---------------------------------------------------------------------------
// Group 4: Null-handle queries return safe zero values
// ---------------------------------------------------------------------------

TEST(mesh_vertex_count_null) {
    ASSERT_EQ("cg_mesh_vertex_count(0) == 0", cg_mesh_vertex_count(CG_NULL_ID), size_t{0});
}

TEST(mesh_triangle_count_null) {
    ASSERT_EQ("cg_mesh_triangle_count(0) == 0", cg_mesh_triangle_count(CG_NULL_ID), size_t{0});
}

TEST(tessellate_null_handle) {
    CgMeshId id = cg_shape_tessellate(CG_NULL_ID, 0.1, 0.1);
    ASSERT_EQ("cg_shape_tessellate(null) == CG_NULL_ID", id, CG_NULL_ID);
    ASSERT_TRUE("cg_shape_tessellate(null) sets error",
                std::string(cg_last_error_message()).size() > 0);
}

TEST(shape_bounding_box_null) {
    CgBbox b = cg_shape_bounding_box(CG_NULL_ID);
    // Must not crash; all zeros is the documented sentinel.
    ASSERT_EQ("bbox.xmin == 0 for null handle", b.xmin, 0.0);
    ASSERT_EQ("bbox.ymin == 0 for null handle", b.ymin, 0.0);
    ASSERT_EQ("bbox.zmin == 0 for null handle", b.zmin, 0.0);
    ASSERT_EQ("bbox.xmax == 0 for null handle", b.xmax, 0.0);
    ASSERT_EQ("bbox.ymax == 0 for null handle", b.ymax, 0.0);
    ASSERT_EQ("bbox.zmax == 0 for null handle", b.zmax, 0.0);
    ASSERT_TRUE("cg_shape_bounding_box(null) sets error",
                std::string(cg_last_error_message()).size() > 0);
}

// ---------------------------------------------------------------------------
// Group 5: Null-argument handling for copy functions
// ---------------------------------------------------------------------------

TEST(mesh_copy_vertices_null_handle) {
    double buf[3] = {0};
    CgError e = cg_mesh_copy_vertices(CG_NULL_ID, buf);
    ASSERT_NE("cg_mesh_copy_vertices(null) != CG_OK", (int)e, (int)CG_OK);
}

TEST(mesh_copy_normals_null_handle) {
    double buf[3] = {0};
    CgError e = cg_mesh_copy_normals(CG_NULL_ID, buf);
    ASSERT_NE("cg_mesh_copy_normals(null) != CG_OK", (int)e, (int)CG_OK);
}

TEST(mesh_copy_indices_null_handle) {
    uint32_t buf[3] = {0};
    CgError e = cg_mesh_copy_indices(CG_NULL_ID, buf);
    ASSERT_NE("cg_mesh_copy_indices(null) != CG_OK", (int)e, (int)CG_OK);
}

// ---------------------------------------------------------------------------
// Group 6: Stub functions return documented error codes
// ---------------------------------------------------------------------------

TEST(load_iges_stub) {
    CgShapeId id = cg_load_iges("/some/file.iges");
    ASSERT_EQ("cg_load_iges returns CG_NULL_ID", id, CG_NULL_ID);
}

TEST(shape_heal_stub) {
    CgShapeId id = cg_shape_heal(1); // non-null but unregistered
    ASSERT_EQ("cg_shape_heal stub returns CG_NULL_ID", id, CG_NULL_ID);
}

TEST(face_surface_type_stub) {
    CgSurfaceType t = cg_face_surface_type(1);
    ASSERT_EQ("cg_face_surface_type stub returns CG_SURF_OTHER", (int)t, (int)CG_SURF_OTHER);
}

TEST(face_plane_stub) {
    CgVec3 n; CgPoint3 o;
    CgError e = cg_face_plane(1, &n, &o);
    ASSERT_EQ("cg_face_plane stub returns CG_ERR_NO_RESULT", (int)e, (int)CG_ERR_NO_RESULT);
}

TEST(face_cylinder_stub) {
    CgVec3 ax; CgPoint3 o; double r;
    CgError e = cg_face_cylinder(1, &ax, &o, &r);
    ASSERT_EQ("cg_face_cylinder stub returns CG_ERR_NO_RESULT", (int)e, (int)CG_ERR_NO_RESULT);
}

TEST(edge_is_circle_stub) {
    int result = cg_edge_is_circle(1, nullptr, nullptr, nullptr);
    ASSERT_EQ("cg_edge_is_circle stub returns 0", result, 0);
}

TEST(shape_distance_stub) {
    double d = cg_shape_distance(1, 2);
    ASSERT_EQ("cg_shape_distance stub returns -1.0", d, -1.0);
}

TEST(shape_section_at_z_stub) {
    CgPoint3* pts = nullptr;
    size_t cnt = 99;
    CgError e = cg_shape_section_at_z(1, 0.0, &pts, &cnt);
    ASSERT_EQ("cg_shape_section_at_z stub returns CG_ERR_NO_RESULT",
              (int)e, (int)CG_ERR_NO_RESULT);
    ASSERT_EQ("cg_shape_section_at_z stub sets out_points to null",
              pts, (CgPoint3*)nullptr);
    ASSERT_EQ("cg_shape_section_at_z stub sets out_count to 0", cnt, size_t{0});
}

TEST(find_holes_stub) {
    CgHoleInfo* holes = nullptr;
    size_t n = cg_shape_find_holes(1, 1.0, 10.0, &holes);
    ASSERT_EQ("cg_shape_find_holes stub returns 0", n, size_t{0});
    ASSERT_EQ("cg_shape_find_holes stub sets *out to null", holes, (CgHoleInfo*)nullptr);
}

TEST(find_planar_faces_stub) {
    CgPlanarFaceInfo* faces = nullptr;
    size_t n = cg_shape_find_planar_faces(1, &faces);
    ASSERT_EQ("cg_shape_find_planar_faces stub returns 0", n, size_t{0});
    ASSERT_EQ("cg_shape_find_planar_faces stub sets *out to null",
              faces, (CgPlanarFaceInfo*)nullptr);
}

TEST(poly_offset_stub) {
    double pts[] = {0,0, 1,0, 1,1, 0,1};
    double* out = nullptr;
    size_t cnt = 99;
    CgError e = cg_poly_offset(pts, 4, 1.0, 0.01, &out, &cnt);
    ASSERT_EQ("cg_poly_offset stub returns CG_ERR_NO_RESULT",
              (int)e, (int)CG_ERR_NO_RESULT);
    ASSERT_EQ("cg_poly_offset stub sets *out to null", out, (double*)nullptr);
    ASSERT_EQ("cg_poly_offset stub sets *cnt to 0", cnt, size_t{0});
}

TEST(poly_boolean_stub) {
    double a[] = {0,0, 1,0, 1,1}; double b[] = {0,0, 2,0, 2,2};
    double* out = nullptr; size_t cnt = 99;
    CgError e = cg_poly_boolean(a, 3, b, 3, CG_BOOL_UNION, &out, &cnt);
    ASSERT_EQ("cg_poly_boolean stub returns CG_ERR_NO_RESULT",
              (int)e, (int)CG_ERR_NO_RESULT);
}

// ---------------------------------------------------------------------------
// Group 7: Free functions accept nullptr without crashing
// ---------------------------------------------------------------------------

TEST(section_free_null_is_noop) {
    cg_section_free(nullptr);
    pass("cg_section_free(nullptr) does not crash");
}

TEST(holes_free_null_is_noop) {
    cg_holes_free(nullptr);
    pass("cg_holes_free(nullptr) does not crash");
}

TEST(planar_faces_free_null_is_noop) {
    cg_planar_faces_free(nullptr);
    pass("cg_planar_faces_free(nullptr) does not crash");
}

TEST(poly_free_null_is_noop) {
    cg_poly_free(nullptr);
    pass("cg_poly_free(nullptr) does not crash");
}

// ---------------------------------------------------------------------------
// Group 8: Edge param range with null output pointers is safe
// ---------------------------------------------------------------------------

TEST(edge_param_range_null_outputs) {
    cg_edge_param_range(1, nullptr, nullptr);
    pass("cg_edge_param_range(id, null, null) does not crash");
}

TEST(edge_param_range_valid_outputs) {
    double tmin = -9.0, tmax = -9.0;
    cg_edge_param_range(1, &tmin, &tmax);
    ASSERT_EQ("cg_edge_param_range stub sets tmin=0", tmin, 0.0);
    ASSERT_EQ("cg_edge_param_range stub sets tmax=0", tmax, 0.0);
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

int main() {
    std::cout << "=== cam_geometry C API contract tests (stub) ===\n";

    // Group 1: Error message
    test_error_message_initially_empty();

    // Group 2: Null-path import
    test_load_step_null_path();
    test_load_stl_null_path();
    test_load_step_missing_file();
    test_load_stl_missing_file();

    // Group 3: Free no-ops
    test_shape_free_null_is_noop();
    test_mesh_free_null_is_noop();
    test_face_free_null_is_noop();
    test_edge_free_null_is_noop();

    // Group 4: Null-handle safe queries
    test_mesh_vertex_count_null();
    test_mesh_triangle_count_null();
    test_tessellate_null_handle();
    test_shape_bounding_box_null();

    // Group 5: Copy functions with null handle
    test_mesh_copy_vertices_null_handle();
    test_mesh_copy_normals_null_handle();
    test_mesh_copy_indices_null_handle();

    // Group 6: Stub error returns
    test_load_iges_stub();
    test_shape_heal_stub();
    test_face_surface_type_stub();
    test_face_plane_stub();
    test_face_cylinder_stub();
    test_edge_is_circle_stub();
    test_shape_distance_stub();
    test_shape_section_at_z_stub();
    test_find_holes_stub();
    test_find_planar_faces_stub();
    test_poly_offset_stub();
    test_poly_boolean_stub();

    // Group 7: Free-null no-ops
    test_section_free_null_is_noop();
    test_holes_free_null_is_noop();
    test_planar_faces_free_null_is_noop();
    test_poly_free_null_is_noop();

    // Group 8: Edge param range
    test_edge_param_range_null_outputs();
    test_edge_param_range_valid_outputs();

    std::cout << "\n=== Results: " << g_pass << " passed, " << g_fail << " failed ===\n";
    return g_fail > 0 ? 1 : 0;
}
