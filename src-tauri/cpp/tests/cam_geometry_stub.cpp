// cam_geometry_stub.cpp
//
// Minimal no-OCCT implementation of the cam_geometry.h C API used by the
// stub unit tests.  This implements only the observable contract:
//   - Thread-local error string (set_last_error / cg_last_error_message)
//   - All functions return CG_NULL_ID or appropriate error codes
//   - Null-handle operations are safe no-ops
//
// This file has NO dependency on OCCT, so the stub tests can be compiled
// and run in any environment.

#include "cam_geometry.h"

#include <cstring>
#include <string>

static thread_local std::string g_stub_last_error;

static void set_error(const char* msg) {
    g_stub_last_error = msg ? msg : "";
}

extern "C" {

const char* cg_last_error_message(void) {
    return g_stub_last_error.c_str();
}

CgShapeId cg_load_step(const char* path) {
    if (!path) { set_error("cg_load_step: null path"); return CG_NULL_ID; }
    set_error("OCCT not available in stub");
    return CG_NULL_ID;
}
CgShapeId cg_load_iges(const char* /*path*/) {
    set_error("not implemented"); return CG_NULL_ID;
}
CgMeshId  cg_load_stl(const char* path) {
    if (!path) { set_error("cg_load_stl: null path"); return CG_NULL_ID; }
    set_error("OCCT not available in stub");
    return CG_NULL_ID;
}
void cg_shape_free(CgShapeId /*id*/) {}

CgShapeId cg_shape_heal(CgShapeId /*id*/) {
    set_error("not implemented"); return CG_NULL_ID;
}

CgBbox cg_shape_bounding_box(CgShapeId id) {
    if (id == CG_NULL_ID) set_error("cg_shape_bounding_box: null handle");
    else set_error("not implemented");
    return CgBbox{0,0,0,0,0,0};
}
size_t cg_shape_faces(CgShapeId /*id*/, CgFaceId* /*out*/, size_t /*cap*/) {
    set_error("not implemented"); return 0;
}
size_t cg_shape_edges(CgShapeId /*id*/, CgEdgeId* /*out*/, size_t /*cap*/) {
    set_error("not implemented"); return 0;
}
void cg_face_free(CgFaceId /*id*/) {}
void cg_edge_free(CgEdgeId /*id*/) {}

CgMeshId cg_shape_tessellate(CgShapeId id, double /*c*/, double /*a*/) {
    if (id == CG_NULL_ID) { set_error("cg_shape_tessellate: null handle"); return CG_NULL_ID; }
    set_error("not implemented");
    return CG_NULL_ID;
}
size_t  cg_mesh_vertex_count(CgMeshId /*id*/)   { return 0; }
size_t  cg_mesh_triangle_count(CgMeshId /*id*/) { return 0; }
CgError cg_mesh_copy_vertices(CgMeshId id, double* /*out*/) {
    if (id == CG_NULL_ID) { set_error("null handle"); return CG_ERR_NULL_HANDLE; }
    set_error("not implemented"); return CG_ERR_NULL_HANDLE;
}
CgError cg_mesh_copy_normals(CgMeshId id, double* /*out*/) {
    if (id == CG_NULL_ID) { set_error("null handle"); return CG_ERR_NULL_HANDLE; }
    set_error("not implemented"); return CG_ERR_NULL_HANDLE;
}
CgError cg_mesh_copy_indices(CgMeshId id, uint32_t* /*out*/) {
    if (id == CG_NULL_ID) { set_error("null handle"); return CG_ERR_NULL_HANDLE; }
    set_error("not implemented"); return CG_ERR_NULL_HANDLE;
}
void cg_mesh_free(CgMeshId /*id*/) {}

CgSurfaceType cg_face_surface_type(CgFaceId /*id*/) { set_error("not implemented"); return CG_SURF_OTHER; }
CgUVBounds    cg_face_uv_bounds(CgFaceId /*id*/)    { set_error("not implemented"); return CgUVBounds{0,0,0,0}; }
CgPoint3 cg_face_eval_point(CgFaceId /*id*/, double /*u*/, double /*v*/)  { set_error("not implemented"); return CgPoint3{0,0,0}; }
CgVec3   cg_face_eval_normal(CgFaceId /*id*/, double /*u*/, double /*v*/) { set_error("not implemented"); return CgVec3{0,0,0}; }
CgVec3   cg_face_eval_du(CgFaceId /*id*/, double /*u*/, double /*v*/)     { set_error("not implemented"); return CgVec3{0,0,0}; }
CgVec3   cg_face_eval_dv(CgFaceId /*id*/, double /*u*/, double /*v*/)     { set_error("not implemented"); return CgVec3{0,0,0}; }
CgPoint2 cg_face_project_point(CgFaceId /*id*/, CgPoint3 /*p*/, double* /*d*/) { set_error("not implemented"); return CgPoint2{0,0}; }
CgError  cg_face_plane(CgFaceId /*id*/, CgVec3* /*n*/, CgPoint3* /*o*/)        { set_error("not implemented"); return CG_ERR_NO_RESULT; }
CgError  cg_face_cylinder(CgFaceId /*id*/, CgVec3* /*ax*/, CgPoint3* /*o*/, double* /*r*/) { set_error("not implemented"); return CG_ERR_NO_RESULT; }

void     cg_edge_param_range(CgEdgeId /*id*/, double* tmin, double* tmax) { set_error("not implemented"); if (tmin) *tmin=0; if (tmax) *tmax=0; }
CgPoint3 cg_edge_eval_point(CgEdgeId /*id*/, double /*t*/)                { set_error("not implemented"); return CgPoint3{0,0,0}; }
CgVec3   cg_edge_eval_tangent(CgEdgeId /*id*/, double /*t*/)              { set_error("not implemented"); return CgVec3{0,0,0}; }
double   cg_edge_length(CgEdgeId /*id*/)                                  { set_error("not implemented"); return 0.0; }
int      cg_edge_is_circle(CgEdgeId /*id*/, CgPoint3* /*c*/, CgVec3* /*ax*/, double* /*r*/) { set_error("not implemented"); return 0; }

double  cg_shape_distance(CgShapeId /*a*/, CgShapeId /*b*/)               { set_error("not implemented"); return -1.0; }
CgError cg_shape_section_at_z(CgShapeId /*id*/, double /*z*/, CgPoint3** pts, size_t* cnt) {
    set_error("not implemented");
    if (pts) *pts = nullptr;
    if (cnt) *cnt = 0;
    return CG_ERR_NO_RESULT;
}
void cg_section_free(CgPoint3* pts) { delete[] pts; }

size_t cg_shape_find_holes(CgShapeId /*id*/, double /*mn*/, double /*mx*/, CgHoleInfo** out) {
    set_error("not implemented"); if (out) *out = nullptr; return 0;
}
void cg_holes_free(CgHoleInfo* h) { delete[] h; }
size_t cg_shape_find_planar_faces(CgShapeId /*id*/, CgPlanarFaceInfo** out) {
    set_error("not implemented"); if (out) *out = nullptr; return 0;
}
void cg_planar_faces_free(CgPlanarFaceInfo* f) { delete[] f; }

CgError cg_poly_offset(const double* /*pts*/, size_t /*n*/, double /*d*/, double /*tol*/,
                        double** out, size_t* cnt) {
    set_error("not implemented"); if (out) *out=nullptr; if (cnt) *cnt=0; return CG_ERR_NO_RESULT;
}
void cg_poly_free(double* pts) { delete[] pts; }
CgError cg_poly_boolean(const double* /*a*/, size_t /*na*/, const double* /*b*/, size_t /*nb*/,
                         CgBoolOp /*op*/, double** out, size_t* cnt) {
    set_error("not implemented"); if (out) *out=nullptr; if (cnt) *cnt=0; return CG_ERR_NO_RESULT;
}

} // extern "C"
