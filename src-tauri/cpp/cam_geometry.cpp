// cam_geometry.cpp
//
// Phase 0 implementation of the plain-C geometry kernel API.
//
// Design:
//   - Every public function catches Standard_Failure (OCCT exceptions) and
//     std::exception and converts them to error codes / CG_NULL_ID returns.
//     No exception may escape across the C boundary.
//   - Errors are written to a thread-local string via set_last_error().
//   - Shape objects (TopoDS_Shape) are stored in the global OcctHandleRegistry
//     from handle_registry.h/.cpp.
//   - Mesh data (vertices, normals, indices flat buffers) is assembled into
//     CgMeshData structs stored in a separate file-local mesh store.
//   - Non-Phase-0 functions are stubbed: they call set_last_error("not
//     implemented") and return CG_NULL_ID / CG_ERR_NO_RESULT.

// ── OCCT includes ────────────────────────────────────────────────────────────
#include <BRepBndLib.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <IFSelect_ReturnStatus.hxx>
#include <Poly_Triangulation.hxx>
#include <RWStl.hxx>
#include <STEPControl_Reader.hxx>
#include <ShapeFix_Shape.hxx>
#include <Standard_Failure.hxx>
#include <TopAbs_Orientation.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Face.hxx>
#include <TopLoc_Location.hxx>
#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

// ── Standard library includes ────────────────────────────────────────────────
#include <atomic>
#include <cmath>
#include <cstring>
#include <memory>
#include <mutex>
#include <shared_mutex>
#include <string>
#include <unordered_map>
#include <vector>

// ── Project includes ─────────────────────────────────────────────────────────
#include "cam_geometry.h"
#include "handle_registry.h"

// ── Internal types ───────────────────────────────────────────────────────────

// Assembled flat mesh buffer stored in the mesh registry.
// All positions and normals are in world space (face location applied).
struct CgMeshData {
    std::vector<double>   vertices; // 3 doubles per vertex [x,y,z, ...]
    std::vector<double>   normals;  // 3 doubles per vertex [nx,ny,nz, ...] (unit)
    std::vector<uint32_t> indices;  // 3 uint32 per triangle [i0,i1,i2, ...]
};

// ── Thread-local error string ────────────────────────────────────────────────

static thread_local std::string g_last_error;

static void set_last_error(const char* msg) {
    g_last_error = msg ? msg : "";
}

static void set_last_error(const std::string& msg) {
    g_last_error = msg;
}

// ── Mesh data store ──────────────────────────────────────────────────────────
// Separate from the shape registry so that we store CgMeshData (flat buffers)
// rather than Poly_Triangulation (OCCT mesh) objects.  IDs are in a separate
// namespace from shape IDs; callers use CgMeshId vs CgShapeId to distinguish.

static std::shared_mutex                                              g_mesh_mutex;
static std::unordered_map<uint64_t, std::shared_ptr<CgMeshData>>     g_mesh_store;
static std::atomic<uint64_t>                                          g_mesh_next_id{1};

static uint64_t mesh_store_insert(std::shared_ptr<CgMeshData> data) {
    uint64_t id = g_mesh_next_id.fetch_add(1, std::memory_order_relaxed);
    std::unique_lock<std::shared_mutex> lock(g_mesh_mutex);
    g_mesh_store.emplace(id, std::move(data));
    return id;
}

// Returns nullptr when id is not found (caller sets error).
static std::shared_ptr<CgMeshData> mesh_store_get(uint64_t id) {
    std::shared_lock<std::shared_mutex> lock(g_mesh_mutex);
    auto it = g_mesh_store.find(id);
    if (it == g_mesh_store.end()) return nullptr;
    return it->second;
}

static bool mesh_store_erase(uint64_t id) {
    std::unique_lock<std::shared_mutex> lock(g_mesh_mutex);
    return g_mesh_store.erase(id) > 0;
}

// ── Helper: build CgMeshData from Poly_Triangulation ─────────────────────────
// Used by both cg_load_stl and cg_shape_tessellate.
// face_reversed: if true, winding order is flipped (TopAbs_REVERSED face).

static void append_triangulation(CgMeshData&                           out,
                                  const Handle(Poly_Triangulation)&      tri,
                                  const TopLoc_Location&                 loc,
                                  bool                                   face_reversed)
{
    const int nNodes     = tri->NbNodes();
    const int nTriangles = tri->NbTriangles();

    const uint32_t base = static_cast<uint32_t>(out.vertices.size() / 3);

    // Reserve space.
    out.vertices.resize(out.vertices.size() + nNodes * 3);
    out.normals.resize(out.normals.size()   + nNodes * 3, 0.0);
    out.indices.reserve(out.indices.size()  + nTriangles * 3);

    // Copy nodes (apply location transform to get world coordinates).
    for (int i = 1; i <= nNodes; ++i) {
        gp_Pnt p = tri->Node(i);
        if (!loc.IsIdentity()) {
            p.Transform(loc.Transformation());
        }
        const uint32_t vi = base + static_cast<uint32_t>(i - 1);
        out.vertices[vi * 3 + 0] = p.X();
        out.vertices[vi * 3 + 1] = p.Y();
        out.vertices[vi * 3 + 2] = p.Z();
    }

    // Copy triangles; accumulate area-weighted face normals to vertex normals.
    for (int t = 1; t <= nTriangles; ++t) {
        int n1, n2, n3;
        tri->Triangle(t).Get(n1, n2, n3);

        // Flip winding for reversed face orientation.
        if (face_reversed) std::swap(n1, n2);

        // Push indices (0-based, offset by base).
        out.indices.push_back(base + static_cast<uint32_t>(n1 - 1));
        out.indices.push_back(base + static_cast<uint32_t>(n2 - 1));
        out.indices.push_back(base + static_cast<uint32_t>(n3 - 1));

        // Compute face normal from cross product using already-transformed
        // world-space positions (avoids recomputing loc.Transformation()).
        const size_t i1 = (base + static_cast<uint32_t>(n1 - 1)) * 3;
        const size_t i2 = (base + static_cast<uint32_t>(n2 - 1)) * 3;
        const size_t i3 = (base + static_cast<uint32_t>(n3 - 1)) * 3;
        gp_Vec e1(out.vertices[i2] - out.vertices[i1],
                  out.vertices[i2+1] - out.vertices[i1+1],
                  out.vertices[i2+2] - out.vertices[i1+2]);
        gp_Vec e2(out.vertices[i3] - out.vertices[i1],
                  out.vertices[i3+1] - out.vertices[i1+1],
                  out.vertices[i3+2] - out.vertices[i1+2]);
        gp_Vec fn = e1.Crossed(e2); // area-weighted normal in world space

        // Accumulate to vertex normals (area weighting is implicit — longer
        // cross product = larger triangle = more weight).
        for (int vi : {n1, n2, n3}) {
            const uint32_t idx = (base + static_cast<uint32_t>(vi - 1)) * 3;
            out.normals[idx + 0] += fn.X();
            out.normals[idx + 1] += fn.Y();
            out.normals[idx + 2] += fn.Z();
        }
    }
}

// Normalize all vertex normals in out.  Called once after all faces are merged.
static void normalize_normals(CgMeshData& out) {
    const size_t nVerts = out.vertices.size() / 3;
    for (size_t i = 0; i < nVerts; ++i) {
        double nx = out.normals[i * 3 + 0];
        double ny = out.normals[i * 3 + 1];
        double nz = out.normals[i * 3 + 2];
        double len = std::sqrt(nx * nx + ny * ny + nz * nz);
        if (len > 1e-12) {
            out.normals[i * 3 + 0] = nx / len;
            out.normals[i * 3 + 1] = ny / len;
            out.normals[i * 3 + 2] = nz / len;
        }
        // If len == 0 (degenerate triangle only), leave normal as zero vector.
    }
}

// ── Public C API ─────────────────────────────────────────────────────────────

extern "C" {

/* ── Error handling ──────────────────────────────────────────────────────── */

const char* cg_last_error_message(void) {
    return g_last_error.c_str();
}

/* ── Shape import ────────────────────────────────────────────────────────── */

// OCCT's STEP protocol initialisation is not thread-safe: the global schema
// registry is lazily populated on the first TransferRoots() call, and
// concurrent initialisations corrupt the registry, yielding StepSelect_StepType
// exceptions.  Serialise all STEP reads with this mutex.
static std::mutex g_step_mutex;

CgShapeId cg_load_step(const char* path) {
    if (!path) {
        set_last_error("cg_load_step: null path");
        return CG_NULL_ID;
    }
    std::lock_guard<std::mutex> lock(g_step_mutex);
    try {
        STEPControl_Reader reader;
        IFSelect_ReturnStatus status = reader.ReadFile(path);
        if (status != IFSelect_RetDone) {
            set_last_error(std::string("STEP: failed to read '") + path + "'");
            return CG_NULL_ID;
        }
        Standard_Integer nRoots = reader.TransferRoots();
        if (nRoots == 0) {
            set_last_error("STEP: no transferable roots found");
            return CG_NULL_ID;
        }
        TopoDS_Shape raw = reader.OneShape();

        // Always attempt healing — real-world STEP files often have tolerance
        // violations or gap issues that cause downstream algorithms to fail.
        ShapeFix_Shape fixer(raw);
        fixer.Perform();
        TopoDS_Shape healed = fixer.Shape();

        return registry_store_shape(healed);
    } catch (const Standard_Failure& ex) {
        set_last_error(std::string("STEP exception: ") + ex.GetMessageString());
        return CG_NULL_ID;
    } catch (const std::exception& ex) {
        set_last_error(std::string("STEP std::exception: ") + ex.what());
        return CG_NULL_ID;
    } catch (...) {
        set_last_error("STEP: unknown exception");
        return CG_NULL_ID;
    }
}

// NOTE: when implementing cg_load_iges, hold g_step_mutex for the duration of
// the reader call.  OCCT's IGES schema registry has the same global-init
// thread-safety issue as the STEP registry — see cg_load_step above.
CgShapeId cg_load_iges(const char* /*path*/) {
    set_last_error("not implemented");
    return CG_NULL_ID;
}

CgMeshId cg_load_stl(const char* path) {
    if (!path) {
        set_last_error("cg_load_stl: null path");
        return CG_NULL_ID;
    }
    try {
        Handle(Poly_Triangulation) tri = RWStl::ReadFile(path);
        if (tri.IsNull()) {
            set_last_error(std::string("STL: failed to read '") + path + "'");
            return CG_NULL_ID;
        }

        auto data = std::make_shared<CgMeshData>();
        TopLoc_Location identity; // identity transform for STL (no face location)
        append_triangulation(*data, tri, identity, /*face_reversed=*/false);
        normalize_normals(*data);

        return mesh_store_insert(std::move(data));
    } catch (const Standard_Failure& ex) {
        set_last_error(std::string("STL exception: ") + ex.GetMessageString());
        return CG_NULL_ID;
    } catch (const std::exception& ex) {
        set_last_error(std::string("STL std::exception: ") + ex.what());
        return CG_NULL_ID;
    } catch (...) {
        set_last_error("STL: unknown exception");
        return CG_NULL_ID;
    }
}

void cg_shape_free(CgShapeId id) {
    if (id == CG_NULL_ID) return;
    registry_free_shape(id);
}

/* ── Shape healing ───────────────────────────────────────────────────────── */

CgShapeId cg_shape_heal(CgShapeId /*id*/) {
    set_last_error("not implemented");
    return CG_NULL_ID;
}

/* ── Shape topology traversal ────────────────────────────────────────────── */

CgBbox cg_shape_bounding_box(CgShapeId id) {
    CgBbox result{0, 0, 0, 0, 0, 0};
    if (id == CG_NULL_ID) {
        set_last_error("cg_shape_bounding_box: null handle");
        return result;
    }
    try {
        const TopoDS_Shape& shape = registry_get_shape(id);
        Bnd_Box box;
        BRepBndLib::AddOptimal(shape, box);
        if (box.IsVoid()) {
            set_last_error("cg_shape_bounding_box: empty/void shape");
            return result;
        }
        box.Get(result.xmin, result.ymin, result.zmin,
                result.xmax, result.ymax, result.zmax);
        return result;
    } catch (const std::out_of_range&) {
        set_last_error("cg_shape_bounding_box: invalid shape ID");
        return result;
    } catch (const Standard_Failure& ex) {
        set_last_error(std::string("BBox exception: ") + ex.GetMessageString());
        return result;
    } catch (...) {
        set_last_error("BBox: unknown exception");
        return result;
    }
}

size_t cg_shape_faces(CgShapeId /*id*/, CgFaceId* /*out_faces*/, size_t /*capacity*/) {
    set_last_error("not implemented");
    return 0;
}

size_t cg_shape_edges(CgShapeId /*id*/, CgEdgeId* /*out_edges*/, size_t /*capacity*/) {
    set_last_error("not implemented");
    return 0;
}

void cg_face_free(CgFaceId id) {
    if (id == CG_NULL_ID) return;
    registry_free_shape(id);
}

void cg_edge_free(CgEdgeId id) {
    if (id == CG_NULL_ID) return;
    registry_free_shape(id);
}

/* ── Tessellation ────────────────────────────────────────────────────────── */

CgMeshId cg_shape_tessellate(CgShapeId id, double chord_tol, double angle_tol) {
    if (id == CG_NULL_ID) {
        set_last_error("cg_shape_tessellate: null handle");
        return CG_NULL_ID;
    }
    try {
        const TopoDS_Shape& shape = registry_get_shape(id);

        // Mesh the shape (stores triangulations inside the shape's topology).
        BRepMesh_IncrementalMesh mesher(shape, chord_tol,
                                        /*isRelative=*/Standard_False,
                                        angle_tol,
                                        /*isParallel=*/Standard_True);
        if (!mesher.IsDone()) {
            set_last_error("cg_shape_tessellate: mesher did not complete");
            return CG_NULL_ID;
        }

        auto data = std::make_shared<CgMeshData>();

        // Iterate over all faces and merge their triangulations.
        for (TopExp_Explorer ex(shape, TopAbs_FACE); ex.More(); ex.Next()) {
            const TopoDS_Face& face = TopoDS::Face(ex.Current());
            TopLoc_Location loc;
            Handle(Poly_Triangulation) tri = BRep_Tool::Triangulation(face, loc);
            if (tri.IsNull()) continue; // face not meshed (degenerate)

            const bool reversed = (face.Orientation() == TopAbs_REVERSED);
            append_triangulation(*data, tri, loc, reversed);
        }

        if (data->indices.empty()) {
            set_last_error("cg_shape_tessellate: no triangles produced");
            return CG_NULL_ID;
        }

        normalize_normals(*data);
        return mesh_store_insert(std::move(data));

    } catch (const std::out_of_range&) {
        set_last_error("cg_shape_tessellate: invalid shape ID");
        return CG_NULL_ID;
    } catch (const Standard_Failure& ex) {
        set_last_error(std::string("Tessellate exception: ") + ex.GetMessageString());
        return CG_NULL_ID;
    } catch (...) {
        set_last_error("Tessellate: unknown exception");
        return CG_NULL_ID;
    }
}

size_t cg_mesh_vertex_count(CgMeshId id) {
    if (id == CG_NULL_ID) return 0;
    auto mesh = mesh_store_get(id);
    if (!mesh) return 0;
    return mesh->vertices.size() / 3;
}

size_t cg_mesh_triangle_count(CgMeshId id) {
    if (id == CG_NULL_ID) return 0;
    auto mesh = mesh_store_get(id);
    if (!mesh) return 0;
    return mesh->indices.size() / 3;
}

CgError cg_mesh_copy_vertices(CgMeshId id, double* out_vertices) {
    if (id == CG_NULL_ID || !out_vertices) {
        set_last_error("cg_mesh_copy_vertices: null argument");
        return CG_ERR_NULL_HANDLE;
    }
    auto mesh = mesh_store_get(id);
    if (!mesh) {
        set_last_error("cg_mesh_copy_vertices: invalid mesh ID");
        return CG_ERR_NULL_HANDLE;
    }
    std::memcpy(out_vertices, mesh->vertices.data(),
                mesh->vertices.size() * sizeof(double));
    return CG_OK;
}

CgError cg_mesh_copy_normals(CgMeshId id, double* out_normals) {
    if (id == CG_NULL_ID || !out_normals) {
        set_last_error("cg_mesh_copy_normals: null argument");
        return CG_ERR_NULL_HANDLE;
    }
    auto mesh = mesh_store_get(id);
    if (!mesh) {
        set_last_error("cg_mesh_copy_normals: invalid mesh ID");
        return CG_ERR_NULL_HANDLE;
    }
    std::memcpy(out_normals, mesh->normals.data(),
                mesh->normals.size() * sizeof(double));
    return CG_OK;
}

CgError cg_mesh_copy_indices(CgMeshId id, uint32_t* out_indices) {
    if (id == CG_NULL_ID || !out_indices) {
        set_last_error("cg_mesh_copy_indices: null argument");
        return CG_ERR_NULL_HANDLE;
    }
    auto mesh = mesh_store_get(id);
    if (!mesh) {
        set_last_error("cg_mesh_copy_indices: invalid mesh ID");
        return CG_ERR_NULL_HANDLE;
    }
    std::memcpy(out_indices, mesh->indices.data(),
                mesh->indices.size() * sizeof(uint32_t));
    return CG_OK;
}

void cg_mesh_free(CgMeshId id) {
    if (id == CG_NULL_ID) return;
    mesh_store_erase(id);
}

/* ── Surface evaluation (stubs) ──────────────────────────────────────────── */

CgSurfaceType cg_face_surface_type(CgFaceId /*id*/) {
    set_last_error("not implemented");
    return CG_SURF_OTHER;
}

CgUVBounds cg_face_uv_bounds(CgFaceId /*id*/) {
    set_last_error("not implemented");
    return CgUVBounds{0, 0, 0, 0};
}

CgPoint3 cg_face_eval_point(CgFaceId /*id*/, double /*u*/, double /*v*/) {
    set_last_error("not implemented");
    return CgPoint3{0, 0, 0};
}

CgVec3 cg_face_eval_normal(CgFaceId /*id*/, double /*u*/, double /*v*/) {
    set_last_error("not implemented");
    return CgVec3{0, 0, 0};
}

CgVec3 cg_face_eval_du(CgFaceId /*id*/, double /*u*/, double /*v*/) {
    set_last_error("not implemented");
    return CgVec3{0, 0, 0};
}

CgVec3 cg_face_eval_dv(CgFaceId /*id*/, double /*u*/, double /*v*/) {
    set_last_error("not implemented");
    return CgVec3{0, 0, 0};
}

CgPoint2 cg_face_project_point(CgFaceId /*id*/, CgPoint3 /*point*/, double* /*out_dist*/) {
    set_last_error("not implemented");
    return CgPoint2{0, 0};
}

CgError cg_face_plane(CgFaceId /*id*/, CgVec3* /*out_normal*/, CgPoint3* /*out_origin*/) {
    set_last_error("not implemented");
    return CG_ERR_NO_RESULT;
}

CgError cg_face_cylinder(CgFaceId /*id*/, CgVec3* /*out_axis*/,
                          CgPoint3* /*out_origin*/, double* /*out_radius*/) {
    set_last_error("not implemented");
    return CG_ERR_NO_RESULT;
}

/* ── Edge / curve evaluation (stubs) ─────────────────────────────────────── */

void cg_edge_param_range(CgEdgeId /*id*/, double* out_tmin, double* out_tmax) {
    set_last_error("not implemented");
    if (out_tmin) *out_tmin = 0.0;
    if (out_tmax) *out_tmax = 0.0;
}

CgPoint3 cg_edge_eval_point(CgEdgeId /*id*/, double /*t*/) {
    set_last_error("not implemented");
    return CgPoint3{0, 0, 0};
}

CgVec3 cg_edge_eval_tangent(CgEdgeId /*id*/, double /*t*/) {
    set_last_error("not implemented");
    return CgVec3{0, 0, 0};
}

double cg_edge_length(CgEdgeId /*id*/) {
    set_last_error("not implemented");
    return 0.0;
}

int cg_edge_is_circle(CgEdgeId /*id*/, CgPoint3* /*out_center*/,
                       CgVec3* /*out_axis*/, double* /*out_radius*/) {
    set_last_error("not implemented");
    return 0;
}

/* ── Geometric queries (stubs) ───────────────────────────────────────────── */

double cg_shape_distance(CgShapeId /*a*/, CgShapeId /*b*/) {
    set_last_error("not implemented");
    return -1.0;
}

CgError cg_shape_section_at_z(CgShapeId /*id*/, double /*z_value*/,
                               CgPoint3** out_points, size_t* out_count) {
    set_last_error("not implemented");
    if (out_points) *out_points = nullptr;
    if (out_count)  *out_count  = 0;
    return CG_ERR_NO_RESULT;
}

void cg_section_free(CgPoint3* points) {
    delete[] points;
}

/* ── Feature detection (stubs) ───────────────────────────────────────────── */

size_t cg_shape_find_holes(CgShapeId /*id*/,
                            double /*min_diameter*/, double /*max_diameter*/,
                            CgHoleInfo** out_holes) {
    set_last_error("not implemented");
    if (out_holes) *out_holes = nullptr;
    return 0;
}

void cg_holes_free(CgHoleInfo* holes) {
    delete[] holes;
}

size_t cg_shape_find_planar_faces(CgShapeId /*id*/, CgPlanarFaceInfo** out_faces) {
    set_last_error("not implemented");
    if (out_faces) *out_faces = nullptr;
    return 0;
}

void cg_planar_faces_free(CgPlanarFaceInfo* faces) {
    delete[] faces;
}

/* ── 2D polygon operations (stubs — Clipper2 impl in later phase) ────────── */

CgError cg_poly_offset(const double* /*points*/, size_t /*point_count*/,
                        double /*delta*/, double /*arc_tolerance*/,
                        double** out_points, size_t* out_count) {
    set_last_error("not implemented");
    if (out_points) *out_points = nullptr;
    if (out_count)  *out_count  = 0;
    return CG_ERR_NO_RESULT;
}

void cg_poly_free(double* points) {
    delete[] points;
}

CgError cg_poly_boolean(const double* /*a_points*/, size_t /*a_count*/,
                         const double* /*b_points*/, size_t /*b_count*/,
                         CgBoolOp /*op*/,
                         double** out_points, size_t* out_count) {
    set_last_error("not implemented");
    if (out_points) *out_points = nullptr;
    if (out_count)  *out_count  = 0;
    return CG_ERR_NO_RESULT;
}

} // extern "C"
