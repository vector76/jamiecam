// cam_geometry.h
//
// Public plain-C API for the JamieCam geometry kernel.
//
// This header is the sole contract between the Rust layer and the C++/OCCT
// implementation.  It uses only plain-C types so that:
//   - bindgen can generate complete Rust FFI bindings automatically.
//   - The file can be included from both C and C++ translation units.
//   - No OCCT headers are required on the Rust side.
//
// Conventions:
//   - Functions that return a handle return CG_NULL_ID (0) on failure.
//   - Functions returning int return CG_OK (0) on success.
//   - All functions that can fail set a thread-local error string readable
//     via cg_last_error_message().
//   - Callers own all returned handles and must free them with the
//     corresponding cg_*_free() function.

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Error handling ──────────────────────────────────────────────────────── */

// Error codes returned by all functions that can fail.
// Functions that return a handle return 0 on failure.
// Functions returning int return CG_OK (0) on success.
typedef enum {
    CG_OK                 = 0,
    CG_ERR_FILE_NOT_FOUND = 1,
    CG_ERR_PARSE_FAILED   = 2,
    CG_ERR_NULL_HANDLE    = 3,
    CG_ERR_INVALID_ARG    = 4,
    CG_ERR_OCCT_EXCEPTION = 5,
    CG_ERR_NO_RESULT      = 6,
} CgError;

// Retrieve a human-readable description of the last error on this thread.
// The returned pointer is valid until the next call on this thread.
const char* cg_last_error_message(void);

/* ── Primitive types ─────────────────────────────────────────────────────── */

typedef struct { double x, y, z; }                             CgPoint3;
typedef struct { double x, y, z; }                             CgVec3;
typedef struct { double u, v; }                                CgPoint2;
typedef struct { double xmin, ymin, zmin, xmax, ymax, zmax; } CgBbox;
typedef struct { double umin, umax, vmin, vmax; }              CgUVBounds;

// Opaque handles — uint64 IDs into the handle registry.
typedef uint64_t CgShapeId;
typedef uint64_t CgFaceId;
typedef uint64_t CgEdgeId;
typedef uint64_t CgMeshId;
typedef uint64_t CgCurveId;

// The null / invalid handle value.
#define CG_NULL_ID UINT64_C(0)

// Surface type classification returned by cg_face_surface_type().
typedef enum {
    CG_SURF_PLANE    = 0,
    CG_SURF_CYLINDER = 1,
    CG_SURF_CONE     = 2,
    CG_SURF_SPHERE   = 3,
    CG_SURF_TORUS    = 4,
    CG_SURF_BSPLINE  = 5,
    CG_SURF_BEZIER   = 6,
    CG_SURF_OFFSET   = 7,
    CG_SURF_OTHER    = 8,
} CgSurfaceType;

/* ── Shape import ────────────────────────────────────────────────────────── */

// Load a STEP file; healing is applied automatically.
// Returns CG_NULL_ID on failure.
CgShapeId cg_load_step(const char* path);

// Load an IGES file; healing is applied automatically.
// Returns CG_NULL_ID on failure.
CgShapeId cg_load_iges(const char* path);

// Load an STL file (mesh-only — no topology).
// Returns CG_NULL_ID on failure.
CgMeshId cg_load_stl(const char* path);

// Free a shape and remove it from the registry.
void cg_shape_free(CgShapeId id);

/* ── Shape healing ───────────────────────────────────────────────────────── */

// Attempt to repair a shape (fix tolerances, sew shells, remove duplicates).
// Returns a new handle to the healed shape.  Caller must free the original.
// Returns CG_NULL_ID on failure.
CgShapeId cg_shape_heal(CgShapeId id);

/* ── Shape topology traversal ────────────────────────────────────────────── */

// Return the axis-aligned bounding box of shape id.
CgBbox cg_shape_bounding_box(CgShapeId id);

// Write face handles into out_faces (caller allocates).
// Pass NULL for out_faces to query the count first.
// Returns the total face count.
size_t cg_shape_faces(CgShapeId id, CgFaceId* out_faces, size_t capacity);

// Write edge handles into out_edges (caller allocates).
// Pass NULL for out_edges to query the count first.
// Returns the total edge count.
size_t cg_shape_edges(CgShapeId id, CgEdgeId* out_edges, size_t capacity);

// Free a face handle returned by cg_shape_faces().
void cg_face_free(CgFaceId id);

// Free an edge handle returned by cg_shape_edges().
void cg_edge_free(CgEdgeId id);

/* ── Tessellation ────────────────────────────────────────────────────────── */

// Tessellate the entire shape into a single merged triangle mesh.
//   chord_tol:  maximum chord deviation from the true surface (mm).
//   angle_tol:  maximum angular deviation (radians).
// Returns CG_NULL_ID on failure.
CgMeshId cg_shape_tessellate(CgShapeId id, double chord_tol, double angle_tol);

// Return the number of vertices in the mesh (each vertex is 3 doubles).
size_t cg_mesh_vertex_count(CgMeshId id);

// Return the number of triangles in the mesh (each triangle is 3 uint32 indices).
size_t cg_mesh_triangle_count(CgMeshId id);

// Copy vertex positions into a caller-allocated buffer.
// out_vertices must hold at least cg_mesh_vertex_count(id) * 3 doubles.
// Layout: [x0,y0,z0, x1,y1,z1, ...]
CgError cg_mesh_copy_vertices(CgMeshId id, double* out_vertices);

// Copy per-vertex normals into a caller-allocated buffer.
// out_normals must hold at least cg_mesh_vertex_count(id) * 3 doubles.
// Layout: [nx0,ny0,nz0, nx1,ny1,nz1, ...]
CgError cg_mesh_copy_normals(CgMeshId id, double* out_normals);

// Copy triangle indices into a caller-allocated buffer.
// out_indices must hold at least cg_mesh_triangle_count(id) * 3 uint32s.
// Layout: [i0,i1,i2, i3,i4,i5, ...]
CgError cg_mesh_copy_indices(CgMeshId id, uint32_t* out_indices);

// Free a mesh and remove it from the registry.
void cg_mesh_free(CgMeshId id);

/* ── Surface evaluation ──────────────────────────────────────────────────── */

// Return the surface type of a face.
CgSurfaceType cg_face_surface_type(CgFaceId id);

// Return the UV parameter bounds of a face.
CgUVBounds cg_face_uv_bounds(CgFaceId id);

// Evaluate the 3D point on the surface at parameter (u, v).
CgPoint3 cg_face_eval_point(CgFaceId id, double u, double v);

// Evaluate the outward surface normal at (u, v).
CgVec3 cg_face_eval_normal(CgFaceId id, double u, double v);

// Evaluate the first partial derivative with respect to u at (u, v).
CgVec3 cg_face_eval_du(CgFaceId id, double u, double v);

// Evaluate the first partial derivative with respect to v at (u, v).
CgVec3 cg_face_eval_dv(CgFaceId id, double u, double v);

// Project point onto the face; returns the nearest UV parameters.
// out_dist: distance from point to surface (may be NULL).
CgPoint2 cg_face_project_point(CgFaceId id, CgPoint3 point, double* out_dist);

// For planar faces: return the plane normal and an on-plane origin point.
// Returns CG_ERR_INVALID_ARG if the face is not planar.
CgError cg_face_plane(CgFaceId id, CgVec3* out_normal, CgPoint3* out_origin);

// For cylindrical faces: return axis direction, origin, and radius.
// Returns CG_ERR_INVALID_ARG if the face is not cylindrical.
CgError cg_face_cylinder(CgFaceId id, CgVec3* out_axis,
                          CgPoint3* out_origin, double* out_radius);

/* ── Edge / curve evaluation ─────────────────────────────────────────────── */

// Return the parametric range [tmin, tmax] of an edge.
void cg_edge_param_range(CgEdgeId id, double* out_tmin, double* out_tmax);

// Evaluate the 3D point on the edge curve at parameter t.
CgPoint3 cg_edge_eval_point(CgEdgeId id, double t);

// Evaluate the unit tangent vector on the edge curve at parameter t.
CgVec3 cg_edge_eval_tangent(CgEdgeId id, double t);

// Return the arc length of the edge.
double cg_edge_length(CgEdgeId id);

// Test whether the edge lies on a circle.
// If true, writes circle center, axis, and radius.  Output pointers may be NULL.
// Returns 1 if the edge is a circle/arc, 0 otherwise.
int cg_edge_is_circle(CgEdgeId id, CgPoint3* out_center,
                       CgVec3* out_axis, double* out_radius);

/* ── Geometric queries ───────────────────────────────────────────────────── */

// Return the minimum distance between two shapes.
double cg_shape_distance(CgShapeId a, CgShapeId b);

// Intersect a shape with the horizontal plane Z = z_value.
// On success, writes a flat array of CgPoint3 values forming polyline segments
// (pairs: start, end, start, end, ...) into *out_points and the element count
// into *out_count.  Caller frees via cg_section_free().
// Returns CG_ERR_NO_RESULT if there is no intersection.
CgError cg_shape_section_at_z(CgShapeId id, double z_value,
                               CgPoint3** out_points, size_t* out_count);

// Free memory allocated by cg_shape_section_at_z().
void cg_section_free(CgPoint3* points);

/* ── Feature detection ───────────────────────────────────────────────────── */

typedef struct {
    CgPoint3 center;   // hole centre at the top-face level
    CgVec3   axis;     // hole axis direction (unit vector)
    double   diameter; // mm
    double   depth;    // mm, positive downward
    int      is_through; // 1 if through-hole, 0 if blind
} CgHoleInfo;

typedef struct {
    CgFaceId face_id;
    CgVec3   normal;
    double   area;     // mm²
    double   z_height; // Z coordinate of the plane (Z-up WCS)
} CgPlanarFaceInfo;

// Detect cylindrical holes whose diameter falls in [min_diameter, max_diameter].
// Writes results into *out_holes (caller frees via cg_holes_free()).
// Returns the number of holes found.
size_t cg_shape_find_holes(CgShapeId id,
                            double min_diameter, double max_diameter,
                            CgHoleInfo** out_holes);

// Free the array allocated by cg_shape_find_holes().
void cg_holes_free(CgHoleInfo* holes);

// Detect planar (flat) faces in the shape.
// Writes results into *out_faces (caller frees via cg_planar_faces_free()).
// Returns the number of planar faces found.
size_t cg_shape_find_planar_faces(CgShapeId id, CgPlanarFaceInfo** out_faces);

// Free the array allocated by cg_shape_find_planar_faces().
void cg_planar_faces_free(CgPlanarFaceInfo* faces);

/* ── 2D polygon operations (Clipper2) ───────────────────────────────────── */

// Offset a closed 2D polygon by delta mm (positive = outward, negative = inward).
//   points:        flat [x,y, x,y, ...] array of input polygon vertices
//   point_count:   number of (x,y) pairs
//   delta:         offset distance in mm
//   arc_tolerance: maximum deviation from true arc when approximating curves
//   out_points:    flat [x,y, x,y, ...] result array (caller frees via cg_poly_free)
//   out_count:     number of (x,y) pairs in the result
// Returns CG_ERR_NO_RESULT if the offset collapses the polygon entirely.
CgError cg_poly_offset(const double* points, size_t point_count,
                        double delta, double arc_tolerance,
                        double** out_points, size_t* out_count);

// Free an array allocated by cg_poly_offset() or cg_poly_boolean().
void cg_poly_free(double* points);

// Boolean operations on 2D polygon sets.
typedef enum {
    CG_BOOL_UNION        = 0,
    CG_BOOL_DIFFERENCE   = 1,
    CG_BOOL_INTERSECTION = 2,
} CgBoolOp;

// Perform a boolean operation between two closed 2D polygons.
//   a_points / a_count: first polygon (flat [x,y, ...] pairs)
//   b_points / b_count: second polygon
//   op:                 union, difference, or intersection
//   out_points / out_count: result polygon (caller frees via cg_poly_free)
CgError cg_poly_boolean(const double* a_points, size_t a_count,
                         const double* b_points, size_t b_count,
                         CgBoolOp op,
                         double** out_points, size_t* out_count);

#ifdef __cplusplus
}
#endif
