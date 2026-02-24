//! Geometry kernel interface — thin Rust layer over the C++ OCCT wrapper.
//!
//! # Module structure
//!
//! ```text
//! geometry/
//! ├── ffi.rs      — raw bindgen-generated extern "C" declarations (private)
//! ├── safe.rs     — safe Rust wrappers with RAII and Result<T, E> (public API)
//! └── importer.rs — high-level import dispatcher (STEP/IGES/STL → MeshData)
//! ```
//!
//! All `unsafe` code lives in `safe.rs`. Code outside the `geometry` module
//! should only use the types re-exported from here.

// Raw bindings are private — callers use the safe wrappers below.
mod ffi;

pub mod importer;
pub mod safe;

pub use importer::import;
pub use safe::{GeometryError, MeshData, OcctMesh, OcctShape};

#[cfg(test)]
#[cfg(cam_geometry_bindings)]
mod tests {
    use super::ffi;

    // ── Error enum ─────────────────────────────────────────────────────────

    /// CG_OK must be 0 (success value used as the default/null error).
    #[test]
    fn cg_error_ok_is_zero() {
        assert_eq!(ffi::CgError::CG_OK as u32, 0);
    }

    #[test]
    fn cg_error_file_not_found_is_one() {
        assert_eq!(ffi::CgError::CG_ERR_FILE_NOT_FOUND as u32, 1);
    }

    #[test]
    fn cg_error_parse_failed_is_two() {
        assert_eq!(ffi::CgError::CG_ERR_PARSE_FAILED as u32, 2);
    }

    #[test]
    fn cg_error_no_result_is_six() {
        assert_eq!(ffi::CgError::CG_ERR_NO_RESULT as u32, 6);
    }

    // ── Surface type enum ─────────────────────────────────────────────────

    #[test]
    fn cg_surf_plane_is_zero() {
        assert_eq!(ffi::CgSurfaceType::CG_SURF_PLANE as u32, 0);
    }

    #[test]
    fn cg_surf_other_is_eight() {
        assert_eq!(ffi::CgSurfaceType::CG_SURF_OTHER as u32, 8);
    }

    // ── Bool op enum ──────────────────────────────────────────────────────

    #[test]
    fn cg_bool_op_union_is_zero() {
        assert_eq!(ffi::CgBoolOp::CG_BOOL_UNION as u32, 0);
    }

    #[test]
    fn cg_bool_op_difference_is_one() {
        assert_eq!(ffi::CgBoolOp::CG_BOOL_DIFFERENCE as u32, 1);
    }

    #[test]
    fn cg_bool_op_intersection_is_two() {
        assert_eq!(ffi::CgBoolOp::CG_BOOL_INTERSECTION as u32, 2);
    }

    // ── Struct layouts ────────────────────────────────────────────────────

    /// CgPoint3 = three f64 fields → 24 bytes (no padding at natural alignment).
    #[test]
    fn cg_point3_size_is_24() {
        assert_eq!(std::mem::size_of::<ffi::CgPoint3>(), 24);
    }

    /// CgVec3 has the same layout as CgPoint3.
    #[test]
    fn cg_vec3_size_is_24() {
        assert_eq!(std::mem::size_of::<ffi::CgVec3>(), 24);
    }

    /// CgPoint2 = two f64 fields → 16 bytes.
    #[test]
    fn cg_point2_size_is_16() {
        assert_eq!(std::mem::size_of::<ffi::CgPoint2>(), 16);
    }

    /// CgBbox = six f64 fields → 48 bytes.
    #[test]
    fn cg_bbox_size_is_48() {
        assert_eq!(std::mem::size_of::<ffi::CgBbox>(), 48);
    }

    /// CgUVBounds = four f64 fields → 32 bytes.
    #[test]
    fn cg_uv_bounds_size_is_32() {
        assert_eq!(std::mem::size_of::<ffi::CgUVBounds>(), 32);
    }

    // ── Opaque handle types ───────────────────────────────────────────────

    /// CgShapeId is typedef uint64_t — must be 8 bytes on all targets.
    #[test]
    fn cg_shape_id_is_8_bytes() {
        assert_eq!(std::mem::size_of::<ffi::CgShapeId>(), 8);
    }

    #[test]
    fn cg_mesh_id_is_8_bytes() {
        assert_eq!(std::mem::size_of::<ffi::CgMeshId>(), 8);
    }

    #[test]
    fn cg_face_id_is_8_bytes() {
        assert_eq!(std::mem::size_of::<ffi::CgFaceId>(), 8);
    }

    // ── Feature-detection structs ─────────────────────────────────────────

    /// CgHoleInfo layout: CgPoint3 + CgVec3 + f64 + f64 + i32 + (4 bytes padding).
    #[test]
    fn cg_hole_info_size() {
        // center(24) + axis(24) + diameter(8) + depth(8) + is_through(4) + pad(4) = 72
        assert_eq!(std::mem::size_of::<ffi::CgHoleInfo>(), 72);
    }
}
