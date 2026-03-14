//! Safe Rust wrappers for OCCT surface evaluation functions.
//!
//! Provides a RAII [`OcctFace`] handle and five wrapper functions around the
//! C surface evaluation API.  All `unsafe` code is isolated in this module.
//!
//! # Limitations
//!
//! The C functions (`cg_face_surface_type`, `cg_face_uv_bounds`,
//! `cg_face_eval_point`, `cg_face_eval_normal`, `cg_face_project_point`) return
//! their results by value with no status code.  Error detection relies on the
//! caller supplying a valid, non-zero `OcctFace` obtained from [`shape_faces`].
//! A degenerate (all-zero) normal from [`face_eval_normal`] is treated as an
//! error; all other functions trust the handle is valid.

use super::safe::{GeometryError, OcctShape};

// ── OcctFace ──────────────────────────────────────────────────────────────────

/// Safe owner of a face handle returned by [`shape_faces`].
///
/// The handle is released via `cg_face_free` when this value is dropped.
#[derive(Debug)]
pub struct OcctFace(u64);

impl OcctFace {
    /// Return the raw handle id for use within the `geometry` module.
    pub(super) fn raw_id(&self) -> u64 {
        self.0
    }
}

#[cfg(cam_geometry_bindings)]
impl Drop for OcctFace {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: handle was obtained from cg_shape_faces and has not been
            // freed before (Rust ownership ensures a single owner).
            unsafe { super::ffi::cg_face_free(self.0) }
        }
    }
}

#[cfg(not(cam_geometry_bindings))]
impl Drop for OcctFace {
    fn drop(&mut self) {
        // No-op: OCCT was not compiled in, so no handle was ever allocated.
    }
}

// ── shape_faces ───────────────────────────────────────────────────────────────

/// Return all face handles for `shape`.
///
/// Calls `cg_shape_faces` twice: first with a null/zero-capacity request to
/// obtain the count, then again to fill an allocated buffer.  Each returned
/// [`OcctFace`] must be freed individually; dropping it does so automatically.
#[cfg(cam_geometry_bindings)]
pub fn shape_faces(shape: &OcctShape) -> Result<Vec<OcctFace>, GeometryError> {
    // Pass null / 0 to get the count.
    let count = unsafe { super::ffi::cg_shape_faces(shape.raw_id(), std::ptr::null_mut(), 0) };
    if count == 0 {
        return Ok(Vec::new());
    }
    // Allocate a buffer of u64 (CgFaceId is typedef uint64_t).
    let mut ids: Vec<u64> = vec![0u64; count];
    let filled = unsafe { super::ffi::cg_shape_faces(shape.raw_id(), ids.as_mut_ptr(), count) };
    ids.truncate(filled);
    Ok(ids.into_iter().map(OcctFace).collect())
}

#[cfg(not(cam_geometry_bindings))]
pub fn shape_faces(_shape: &OcctShape) -> Result<Vec<OcctFace>, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_surface_type ─────────────────────────────────────────────────────────

/// Return the surface type of `face` as a `u32` (maps to `CgSurfaceType`).
#[cfg(cam_geometry_bindings)]
pub fn face_surface_type(face: &OcctFace) -> Result<u32, GeometryError> {
    let st = unsafe { super::ffi::cg_face_surface_type(face.raw_id()) };
    Ok(st as u32)
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_surface_type(_face: &OcctFace) -> Result<u32, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_uv_bounds ────────────────────────────────────────────────────────────

/// Return the parametric (UV) domain of `face` as `(umin, umax, vmin, vmax)`.
#[cfg(cam_geometry_bindings)]
pub fn face_uv_bounds(face: &OcctFace) -> Result<(f64, f64, f64, f64), GeometryError> {
    let b = unsafe { super::ffi::cg_face_uv_bounds(face.raw_id()) };
    Ok((b.umin, b.umax, b.vmin, b.vmax))
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_uv_bounds(_face: &OcctFace) -> Result<(f64, f64, f64, f64), GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_eval_point ───────────────────────────────────────────────────────────

/// Evaluate the 3-D point on `face` at parametric coordinates `(u, v)`.
///
/// Returns `[x, y, z]`.
#[cfg(cam_geometry_bindings)]
pub fn face_eval_point(face: &OcctFace, u: f64, v: f64) -> Result<[f64; 3], GeometryError> {
    let p = unsafe { super::ffi::cg_face_eval_point(face.raw_id(), u, v) };
    Ok([p.x, p.y, p.z])
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_eval_point(_face: &OcctFace, _u: f64, _v: f64) -> Result<[f64; 3], GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_eval_normal ──────────────────────────────────────────────────────────

/// Evaluate the surface normal of `face` at parametric coordinates `(u, v)`.
///
/// Returns `[nx, ny, nz]`.  Returns [`GeometryError::ImportFailed`] if the
/// result is degenerate (all-zero), which indicates an invalid handle or a
/// singular parametric point.
#[cfg(cam_geometry_bindings)]
pub fn face_eval_normal(face: &OcctFace, u: f64, v: f64) -> Result<[f64; 3], GeometryError> {
    let n = unsafe { super::ffi::cg_face_eval_normal(face.raw_id(), u, v) };
    if n.x == 0.0 && n.y == 0.0 && n.z == 0.0 {
        return Err(GeometryError::ImportFailed {
            message: "cg_face_eval_normal returned degenerate (all-zero) normal".into(),
        });
    }
    Ok([n.x, n.y, n.z])
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_eval_normal(_face: &OcctFace, _u: f64, _v: f64) -> Result<[f64; 3], GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_project_point ────────────────────────────────────────────────────────

/// Project a 3-D point `(x, y, z)` onto `face`.
///
/// Returns `([u, v], distance)` where `[u, v]` is the nearest parametric
/// coordinate and `distance` is the Euclidean distance from the input point
/// to the closest point on the surface.
#[cfg(cam_geometry_bindings)]
pub fn face_project_point(
    face: &OcctFace,
    x: f64,
    y: f64,
    z: f64,
) -> Result<([f64; 2], f64), GeometryError> {
    let mut dist: f64 = 0.0;
    let pt = super::ffi::CgPoint3 { x, y, z };
    let uv = unsafe { super::ffi::cg_face_project_point(face.raw_id(), pt, &mut dist) };
    Ok(([uv.u, uv.v], dist))
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_project_point(
    _face: &OcctFace,
    _x: f64,
    _y: f64,
    _z: f64,
) -> Result<([f64; 2], f64), GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── CurvatureResult ──────────────────────────────────────────────────────────

/// Principal curvature values and direction at a surface point.
#[derive(Debug, Clone)]
pub struct CurvatureResult {
    pub k1: f64,
    pub k2: f64,
    pub dir1: [f64; 3],
}

// ── face_eval_curvature ──────────────────────────────────────────────────────

/// Evaluate the principal curvatures of `face` at parametric coordinates `(u, v)`.
///
/// Returns a [`CurvatureResult`] with the maximum (`k1`) and minimum (`k2`)
/// principal curvatures and the direction of maximum curvature (`dir1`).
#[cfg(cam_geometry_bindings)]
pub fn face_eval_curvature(
    face: &OcctFace,
    u: f64,
    v: f64,
) -> Result<CurvatureResult, GeometryError> {
    let r = unsafe { super::ffi::cg_face_eval_curvature(face.raw_id(), u, v) };
    if r.success as u32 != super::ffi::CgError::CG_OK as u32 {
        return Err(GeometryError::ImportFailed {
            message: super::safe::last_error_message(),
        });
    }
    Ok(CurvatureResult {
        k1: r.k1,
        k2: r.k2,
        dir1: [r.dir1.x, r.dir1.y, r.dir1.z],
    })
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_eval_curvature(
    _face: &OcctFace,
    _u: f64,
    _v: f64,
) -> Result<CurvatureResult, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Stub tests — run without OCCT bindings.

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn shape_faces_stub_returns_error() {
        let shape = OcctShape::new_for_test(0);
        assert!(shape_faces(&shape).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_surface_type_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_surface_type(&face).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_uv_bounds_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_uv_bounds(&face).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_eval_point_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_eval_point(&face, 0.0, 0.0).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_eval_normal_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_eval_normal(&face, 0.0, 0.0).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_project_point_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_project_point(&face, 0.0, 0.0, 0.0).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_eval_curvature_stub_returns_error() {
        let face = OcctFace(0);
        assert!(face_eval_curvature(&face, 0.0, 0.0).is_err());
    }

    // OCCT integration tests — only run with real bindings.

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn shape_faces_box_returns_six_faces() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        let shape = OcctShape::load_step(&path).expect("load box.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        assert_eq!(faces.len(), 6, "a box has 6 faces");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn face_uv_bounds_ordered() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        let shape = OcctShape::load_step(&path).expect("load box.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        let (umin, umax, vmin, vmax) = face_uv_bounds(&faces[0]).expect("face_uv_bounds");
        assert!(umin < umax, "umin < umax");
        assert!(vmin < vmax, "vmin < vmax");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn face_eval_normal_is_unit_length() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        let shape = OcctShape::load_step(&path).expect("load box.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        let (umin, umax, vmin, vmax) = face_uv_bounds(&faces[0]).expect("face_uv_bounds");
        let u_mid = (umin + umax) / 2.0;
        let v_mid = (vmin + vmax) / 2.0;
        let [nx, ny, nz] = face_eval_normal(&faces[0], u_mid, v_mid).expect("face_eval_normal");
        let length = (nx * nx + ny * ny + nz * nz).sqrt();
        assert!(
            (length - 1.0).abs() < 1e-6,
            "normal must be unit length, got {length}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn face_project_point_round_trip() {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        let shape = OcctShape::load_step(&path).expect("load box.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        let (umin, umax, vmin, vmax) = face_uv_bounds(&faces[0]).expect("face_uv_bounds");
        let u_mid = (umin + umax) / 2.0;
        let v_mid = (vmin + vmax) / 2.0;

        // Evaluate the 3-D point at the UV centre.
        let [x0, y0, z0] = face_eval_point(&faces[0], u_mid, v_mid).expect("face_eval_point");

        // Project that point back to UV space.
        let ([u1, v1], dist) =
            face_project_point(&faces[0], x0, y0, z0).expect("face_project_point");

        // The distance from a surface point to itself should be near zero.
        assert!(
            dist < 1e-6,
            "round-trip distance should be near zero, got {dist}"
        );

        // Re-evaluate at the projected UV to confirm we get the same XYZ.
        let [x1, y1, z1] = face_eval_point(&faces[0], u1, v1).expect("face_eval_point second");
        assert!(
            (x1 - x0).abs() < 1e-6,
            "x round-trip mismatch: {x0} vs {x1}"
        );
        assert!(
            (y1 - y0).abs() < 1e-6,
            "y round-trip mismatch: {y0} vs {y1}"
        );
        assert!(
            (z1 - z0).abs() < 1e-6,
            "z round-trip mismatch: {z0} vs {z1}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn face_eval_curvature_sphere_constant() {
        // sphere.step has known constant curvature = 1/radius on every face.
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/sphere.step");
        let shape = OcctShape::load_step(&path).expect("load sphere.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        assert!(!faces.is_empty(), "sphere must have at least one face");

        let (umin, umax, vmin, vmax) = face_uv_bounds(&faces[0]).expect("face_uv_bounds");
        let u_mid = (umin + umax) / 2.0;
        let v_mid = (vmin + vmax) / 2.0;

        let curv = face_eval_curvature(&faces[0], u_mid, v_mid).expect("face_eval_curvature");

        // For a sphere of radius R, both principal curvatures = 1/R.
        // The fixture sphere has radius 10, so expected curvature = 0.1.
        let expected = 0.1;
        let tol = 1e-4;
        assert!(
            (curv.k1.abs() - expected).abs() < tol,
            "k1 should be ~{expected}, got {}",
            curv.k1
        );
        assert!(
            (curv.k2.abs() - expected).abs() < tol,
            "k2 should be ~{expected}, got {}",
            curv.k2
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn face_eval_curvature_box_flat() {
        // box.step faces are planar — curvature should be zero.
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        let shape = OcctShape::load_step(&path).expect("load box.step");
        let faces = shape_faces(&shape).expect("shape_faces");
        assert!(!faces.is_empty(), "box must have at least one face");

        let (umin, umax, vmin, vmax) = face_uv_bounds(&faces[0]).expect("face_uv_bounds");
        let u_mid = (umin + umax) / 2.0;
        let v_mid = (vmin + vmax) / 2.0;

        let curv = face_eval_curvature(&faces[0], u_mid, v_mid).expect("face_eval_curvature");

        let tol = 1e-6;
        assert!(
            curv.k1.abs() < tol,
            "k1 should be ~0 on a flat face, got {}",
            curv.k1
        );
        assert!(
            curv.k2.abs() < tol,
            "k2 should be ~0 on a flat face, got {}",
            curv.k2
        );
    }
}
