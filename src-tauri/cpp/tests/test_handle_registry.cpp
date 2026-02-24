// test_handle_registry.cpp
//
// Unit tests for HandleRegistryBase and cam_geometry.h constants/types.
//
// Compiles without OCCT: uses std::string and std::shared_ptr<std::string> as
// stand-ins for TopoDS_Shape and Handle(Poly_Triangulation) respectively.
//
// Build:
//   g++ -std=c++17 -I.. -pthread test_handle_registry.cpp -o test_handle_registry
// Run:
//   ./test_handle_registry

#include <atomic>
#include <cstdint>
#include <iostream>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

#include "handle_registry_base.h"
#include "cam_geometry.h"

// ---------------------------------------------------------------------------
// Minimal test framework
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

#define ASSERT_THROWS(label, expr) \
    do { \
        bool threw = false; \
        try { (void)(expr); } catch (...) { threw = true; } \
        ASSERT_TRUE(label, threw); \
    } while (0)

#define TEST(name) static void test_##name()

// ---------------------------------------------------------------------------
// Registry type alias for tests (mock types, no OCCT)
// ---------------------------------------------------------------------------

using MockShape = std::string;
using MockMesh  = std::shared_ptr<std::string>;
using MockReg   = HandleRegistryBase<MockShape, MockMesh>;

// ---------------------------------------------------------------------------
// Group 1: ID allocation
// ---------------------------------------------------------------------------

TEST(first_id_is_not_null) {
    MockReg reg;
    uint64_t id = reg.store_shape("s");
    ASSERT_NE("first shape ID is not CG_NULL_ID (0)", id, uint64_t{0});
}

TEST(successive_ids_are_unique) {
    MockReg reg;
    uint64_t a = reg.store_shape("a");
    uint64_t b = reg.store_shape("b");
    uint64_t c = reg.store_shape("c");
    ASSERT_NE("successive IDs are unique (a != b)", a, b);
    ASSERT_NE("successive IDs are unique (b != c)", b, c);
    ASSERT_NE("successive IDs are unique (a != c)", a, c);
}

TEST(shape_and_mesh_ids_are_unique_across_types) {
    MockReg reg;
    uint64_t sid = reg.store_shape("shape");
    uint64_t mid = reg.store_mesh(std::make_shared<std::string>("mesh"));
    ASSERT_NE("shape and mesh IDs never collide", sid, mid);
}

// ---------------------------------------------------------------------------
// Group 2: Shape store / retrieve / free
// ---------------------------------------------------------------------------

TEST(store_and_retrieve_shape) {
    MockReg reg;
    uint64_t id = reg.store_shape("hello");
    ASSERT_EQ("stored shape roundtrips correctly", reg.get_shape(id), "hello");
}

TEST(multiple_shapes_all_retrievable) {
    MockReg reg;
    uint64_t id_a = reg.store_shape("alpha");
    uint64_t id_b = reg.store_shape("beta");
    uint64_t id_c = reg.store_shape("gamma");
    ASSERT_EQ("shape alpha", reg.get_shape(id_a), "alpha");
    ASSERT_EQ("shape beta",  reg.get_shape(id_b), "beta");
    ASSERT_EQ("shape gamma", reg.get_shape(id_c), "gamma");
}

TEST(free_shape_removes_from_registry) {
    MockReg reg;
    uint64_t id = reg.store_shape("to-free");
    ASSERT_EQ("shape_count before free", reg.shape_count(), size_t{1});
    reg.free_shape(id);
    ASSERT_EQ("shape_count after free", reg.shape_count(), size_t{0});
}

TEST(get_shape_throws_after_free) {
    MockReg reg;
    uint64_t id = reg.store_shape("gone");
    reg.free_shape(id);
    ASSERT_THROWS("get_shape after free throws", reg.get_shape(id));
}

TEST(double_free_shape_returns_false) {
    MockReg reg;
    uint64_t id = reg.store_shape("x");
    reg.free_shape(id);
    bool second = reg.free_shape(id);
    ASSERT_TRUE("second free_shape returns false", !second);
}

TEST(shape_count_tracks_stores_and_frees) {
    MockReg reg;
    ASSERT_EQ("count starts at 0", reg.shape_count(), size_t{0});
    uint64_t a = reg.store_shape("a");
    ASSERT_EQ("count after 1 store", reg.shape_count(), size_t{1});
    uint64_t b = reg.store_shape("b");
    ASSERT_EQ("count after 2 stores", reg.shape_count(), size_t{2});
    reg.free_shape(a);
    ASSERT_EQ("count after 1 free", reg.shape_count(), size_t{1});
    reg.free_shape(b);
    ASSERT_EQ("count after 2 frees", reg.shape_count(), size_t{0});
}

// ---------------------------------------------------------------------------
// Group 3: Mesh store / retrieve / free
// ---------------------------------------------------------------------------

TEST(store_and_retrieve_mesh) {
    MockReg reg;
    auto mesh = std::make_shared<std::string>("mesh-data");
    uint64_t id = reg.store_mesh(mesh);
    ASSERT_EQ("stored mesh roundtrips correctly", *reg.get_mesh(id), "mesh-data");
}

TEST(free_mesh_removes_from_registry) {
    MockReg reg;
    uint64_t id = reg.store_mesh(std::make_shared<std::string>("m"));
    reg.free_mesh(id);
    ASSERT_EQ("mesh_count after free", reg.mesh_count(), size_t{0});
}

TEST(get_mesh_throws_after_free) {
    MockReg reg;
    uint64_t id = reg.store_mesh(std::make_shared<std::string>("gone"));
    reg.free_mesh(id);
    ASSERT_THROWS("get_mesh after free throws", reg.get_mesh(id));
}

TEST(double_free_mesh_returns_false) {
    MockReg reg;
    uint64_t id = reg.store_mesh(std::make_shared<std::string>("m"));
    reg.free_mesh(id);
    bool second = reg.free_mesh(id);
    ASSERT_TRUE("second free_mesh returns false", !second);
}

TEST(mesh_count_tracks_stores_and_frees) {
    MockReg reg;
    ASSERT_EQ("mesh count starts at 0", reg.mesh_count(), size_t{0});
    uint64_t id = reg.store_mesh(std::make_shared<std::string>("m"));
    ASSERT_EQ("mesh count after store", reg.mesh_count(), size_t{1});
    reg.free_mesh(id);
    ASSERT_EQ("mesh count after free", reg.mesh_count(), size_t{0});
}

// ---------------------------------------------------------------------------
// Group 4: Invalid ID access
// ---------------------------------------------------------------------------

TEST(get_shape_null_id_throws) {
    MockReg reg;
    ASSERT_THROWS("get_shape(0) throws", reg.get_shape(0));
}

TEST(get_mesh_null_id_throws) {
    MockReg reg;
    ASSERT_THROWS("get_mesh(0) throws", reg.get_mesh(0));
}

// ---------------------------------------------------------------------------
// Group 5: Map isolation (shape IDs cannot access mesh map and vice versa)
// ---------------------------------------------------------------------------

TEST(shape_id_not_accessible_as_mesh) {
    MockReg reg;
    uint64_t sid = reg.store_shape("shape");
    // The same numeric ID does not exist in the mesh map.
    ASSERT_THROWS("shape ID not accessible via get_mesh", reg.get_mesh(sid));
}

TEST(mesh_id_not_accessible_as_shape) {
    MockReg reg;
    uint64_t mid = reg.store_mesh(std::make_shared<std::string>("mesh"));
    ASSERT_THROWS("mesh ID not accessible via get_shape", reg.get_shape(mid));
}

// ---------------------------------------------------------------------------
// Group 6: Thread safety
// ---------------------------------------------------------------------------

TEST(concurrent_stores_produce_unique_ids) {
    MockReg reg;
    constexpr int N_THREADS     = 8;
    constexpr int N_PER_THREAD  = 200;

    std::vector<std::vector<uint64_t>> ids(N_THREADS);
    std::vector<std::thread> threads;
    threads.reserve(N_THREADS);

    for (int t = 0; t < N_THREADS; ++t) {
        threads.emplace_back([&reg, &ids, t]() {
            for (int i = 0; i < N_PER_THREAD; ++i) {
                std::string val = "t" + std::to_string(t) + "_" + std::to_string(i);
                ids[t].push_back(reg.store_shape(val));
            }
        });
    }
    for (auto& th : threads) th.join();

    // All IDs must be unique.
    std::set<uint64_t> all_ids;
    for (const auto& thread_ids : ids)
        for (uint64_t id : thread_ids)
            all_ids.insert(id);

    ASSERT_EQ("concurrent stores: all IDs unique",
              all_ids.size(), size_t{N_THREADS * N_PER_THREAD});
    ASSERT_EQ("concurrent stores: all shapes present",
              reg.shape_count(), size_t{N_THREADS * N_PER_THREAD});
}

TEST(concurrent_reads_succeed) {
    MockReg reg;
    uint64_t id = reg.store_shape("shared-value");

    constexpr int N_READERS = 16;
    std::atomic<int> successes{0};
    std::vector<std::thread> threads;
    threads.reserve(N_READERS);

    for (int i = 0; i < N_READERS; ++i) {
        threads.emplace_back([&]() {
            if (reg.get_shape(id) == "shared-value")
                successes.fetch_add(1, std::memory_order_relaxed);
        });
    }
    for (auto& th : threads) th.join();

    ASSERT_EQ("concurrent reads: all readers got correct value",
              successes.load(), N_READERS);
}

// ---------------------------------------------------------------------------
// Group 7: cam_geometry.h constants and type layout
// ---------------------------------------------------------------------------

TEST(cg_null_id_is_zero) {
    ASSERT_EQ("CG_NULL_ID == 0", CG_NULL_ID, UINT64_C(0));
}

TEST(cg_error_codes) {
    ASSERT_EQ("CG_OK == 0",                 (int)CG_OK,                 0);
    ASSERT_EQ("CG_ERR_FILE_NOT_FOUND == 1", (int)CG_ERR_FILE_NOT_FOUND, 1);
    ASSERT_EQ("CG_ERR_PARSE_FAILED == 2",   (int)CG_ERR_PARSE_FAILED,   2);
    ASSERT_EQ("CG_ERR_NULL_HANDLE == 3",    (int)CG_ERR_NULL_HANDLE,    3);
    ASSERT_EQ("CG_ERR_INVALID_ARG == 4",    (int)CG_ERR_INVALID_ARG,    4);
    ASSERT_EQ("CG_ERR_OCCT_EXCEPTION == 5", (int)CG_ERR_OCCT_EXCEPTION, 5);
    ASSERT_EQ("CG_ERR_NO_RESULT == 6",      (int)CG_ERR_NO_RESULT,      6);
}

TEST(cg_surface_type_enum) {
    ASSERT_EQ("CG_SURF_PLANE == 0",    (int)CG_SURF_PLANE,    0);
    ASSERT_EQ("CG_SURF_CYLINDER == 1", (int)CG_SURF_CYLINDER, 1);
    ASSERT_EQ("CG_SURF_CONE == 2",     (int)CG_SURF_CONE,     2);
    ASSERT_EQ("CG_SURF_SPHERE == 3",   (int)CG_SURF_SPHERE,   3);
    ASSERT_EQ("CG_SURF_TORUS == 4",    (int)CG_SURF_TORUS,    4);
    ASSERT_EQ("CG_SURF_BSPLINE == 5",  (int)CG_SURF_BSPLINE,  5);
    ASSERT_EQ("CG_SURF_BEZIER == 6",   (int)CG_SURF_BEZIER,   6);
    ASSERT_EQ("CG_SURF_OFFSET == 7",   (int)CG_SURF_OFFSET,   7);
    ASSERT_EQ("CG_SURF_OTHER == 8",    (int)CG_SURF_OTHER,    8);
}

TEST(cg_bool_op_enum) {
    ASSERT_EQ("CG_BOOL_UNION == 0",        (int)CG_BOOL_UNION,        0);
    ASSERT_EQ("CG_BOOL_DIFFERENCE == 1",   (int)CG_BOOL_DIFFERENCE,   1);
    ASSERT_EQ("CG_BOOL_INTERSECTION == 2", (int)CG_BOOL_INTERSECTION, 2);
}

TEST(cg_point3_member_layout) {
    CgPoint3 p{1.0, 2.0, 3.0};
    ASSERT_EQ("CgPoint3.x", p.x, 1.0);
    ASSERT_EQ("CgPoint3.y", p.y, 2.0);
    ASSERT_EQ("CgPoint3.z", p.z, 3.0);
}

TEST(cg_bbox_member_layout) {
    CgBbox b{1.0, 2.0, 3.0, 4.0, 5.0, 6.0};
    ASSERT_EQ("CgBbox.xmin", b.xmin, 1.0);
    ASSERT_EQ("CgBbox.ymin", b.ymin, 2.0);
    ASSERT_EQ("CgBbox.zmin", b.zmin, 3.0);
    ASSERT_EQ("CgBbox.xmax", b.xmax, 4.0);
    ASSERT_EQ("CgBbox.ymax", b.ymax, 5.0);
    ASSERT_EQ("CgBbox.zmax", b.zmax, 6.0);
}

TEST(cg_uv_bounds_member_layout) {
    CgUVBounds uv{0.0, 1.0, -1.0, 2.0};
    ASSERT_EQ("CgUVBounds.umin", uv.umin,  0.0);
    ASSERT_EQ("CgUVBounds.umax", uv.umax,  1.0);
    ASSERT_EQ("CgUVBounds.vmin", uv.vmin, -1.0);
    ASSERT_EQ("CgUVBounds.vmax", uv.vmax,  2.0);
}

TEST(cg_hole_info_member_layout) {
    CgHoleInfo h{};
    h.center     = {1.0, 2.0, 3.0};
    h.axis       = {0.0, 0.0, 1.0};
    h.diameter   = 6.0;
    h.depth      = 10.0;
    h.is_through = 0;
    ASSERT_EQ("CgHoleInfo.diameter",   h.diameter,   6.0);
    ASSERT_EQ("CgHoleInfo.depth",      h.depth,      10.0);
    ASSERT_EQ("CgHoleInfo.is_through", h.is_through, 0);
}

TEST(cg_planar_face_info_member_layout) {
    CgPlanarFaceInfo f{};
    f.face_id  = 42;
    f.area     = 100.0;
    f.z_height = -5.0;
    ASSERT_EQ("CgPlanarFaceInfo.face_id",  f.face_id,  uint64_t{42});
    ASSERT_EQ("CgPlanarFaceInfo.area",     f.area,     100.0);
    ASSERT_EQ("CgPlanarFaceInfo.z_height", f.z_height, -5.0);
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

int main() {
    std::cout << "=== HandleRegistryBase tests ===\n";

    // Group 1: ID allocation
    test_first_id_is_not_null();
    test_successive_ids_are_unique();
    test_shape_and_mesh_ids_are_unique_across_types();

    // Group 2: Shape operations
    test_store_and_retrieve_shape();
    test_multiple_shapes_all_retrievable();
    test_free_shape_removes_from_registry();
    test_get_shape_throws_after_free();
    test_double_free_shape_returns_false();
    test_shape_count_tracks_stores_and_frees();

    // Group 3: Mesh operations
    test_store_and_retrieve_mesh();
    test_free_mesh_removes_from_registry();
    test_get_mesh_throws_after_free();
    test_double_free_mesh_returns_false();
    test_mesh_count_tracks_stores_and_frees();

    // Group 4: Invalid ID access
    test_get_shape_null_id_throws();
    test_get_mesh_null_id_throws();

    // Group 5: Map isolation
    test_shape_id_not_accessible_as_mesh();
    test_mesh_id_not_accessible_as_shape();

    // Group 6: Thread safety
    test_concurrent_stores_produce_unique_ids();
    test_concurrent_reads_succeed();

    std::cout << "\n=== cam_geometry.h constants and layout ===\n";

    // Group 7: cam_geometry.h
    test_cg_null_id_is_zero();
    test_cg_error_codes();
    test_cg_surface_type_enum();
    test_cg_bool_op_enum();
    test_cg_point3_member_layout();
    test_cg_bbox_member_layout();
    test_cg_uv_bounds_member_layout();
    test_cg_hole_info_member_layout();
    test_cg_planar_face_info_member_layout();

    // Summary
    std::cout << "\n=== Results: " << g_pass << " passed, " << g_fail << " failed ===\n";
    return g_fail > 0 ? 1 : 0;
}
