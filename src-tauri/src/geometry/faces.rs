//! Face-level geometry API — wraps the C face index API, provides
//! face fingerprinting, and exposes the shared `enumerate_faces` helper.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::safe::{GeometryError, OcctShape};

// ── Public types ──────────────────────────────────────────────────────────────

/// Geometric properties of a single B-rep face.
#[derive(Debug, Clone)]
pub struct FaceInfo {
    pub centroid: [f64; 3],
    pub normal: [f64; 3],
    pub area: f64,
}

/// Face with its computed fingerprint — used by both IPC and the planner.
#[derive(Debug, Clone)]
pub struct FaceDescriptor {
    pub fingerprint: String,
    pub face_idx: usize,
    pub centroid: [f64; 3],
    pub normal: [f64; 3],
    pub area: f64,
}

// ── face_count ────────────────────────────────────────────────────────────────

/// Return total number of faces (planar and non-planar) in `shape`.
#[cfg(cam_geometry_bindings)]
pub fn face_count(shape: &OcctShape) -> Result<usize, GeometryError> {
    Ok(unsafe { super::ffi::cg_shape_face_count(shape.raw_id()) as usize })
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_count(_shape: &OcctShape) -> Result<usize, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_info ─────────────────────────────────────────────────────────────────

/// Return geometric info for face `face_idx` of `shape`.
///
/// Returns [`GeometryError::ImportFailed`] for non-planar or out-of-range
/// indices (mirrors the `CG_ERR_NO_RESULT` result from C).
#[cfg(cam_geometry_bindings)]
pub fn face_info(shape: &OcctShape, face_idx: usize) -> Result<FaceInfo, GeometryError> {
    let mut out = unsafe { std::mem::zeroed::<super::ffi::CgFaceInfo>() };
    let err = unsafe { super::ffi::cg_face_info(shape.raw_id(), face_idx, &mut out) };
    if err as u32 != super::ffi::CgError::CG_OK as u32 {
        return Err(GeometryError::ImportFailed {
            message: format!("cg_face_info returned error for face {face_idx}"),
        });
    }
    Ok(FaceInfo {
        centroid: out.centroid,
        normal: out.normal,
        area: out.area,
    })
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_info(_shape: &OcctShape, _face_idx: usize) -> Result<FaceInfo, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_boundary ─────────────────────────────────────────────────────────────

/// Return the outer-wire boundary of a planar face as XY pairs.
///
/// Returns [`GeometryError::ImportFailed`] for non-planar faces.
/// The caller does not need to free anything — the raw allocation is freed
/// inside this function before it returns.
#[cfg(cam_geometry_bindings)]
pub fn face_boundary(shape: &OcctShape, face_idx: usize) -> Result<Vec<(f64, f64)>, GeometryError> {
    let mut out_points: *mut f64 = std::ptr::null_mut();
    let mut out_count: usize = 0;
    let err = unsafe {
        super::ffi::cg_face_boundary_poly(shape.raw_id(), face_idx, &mut out_points, &mut out_count)
    };
    if err as u32 != super::ffi::CgError::CG_OK as u32 {
        return Err(GeometryError::ImportFailed {
            message: format!("cg_face_boundary_poly returned error for face {face_idx}"),
        });
    }
    // SAFETY: on CG_OK the C layer guarantees out_points points to an array of
    // out_count * 2 f64 values (flat x,y pairs). We copy into an owned Vec,
    // then free the C allocation.
    let result = unsafe {
        let slice = std::slice::from_raw_parts(out_points, out_count * 2);
        let pairs: Vec<(f64, f64)> = slice.chunks_exact(2).map(|c| (c[0], c[1])).collect();
        super::ffi::cg_poly_free(out_points);
        pairs
    };
    Ok(result)
}

#[cfg(not(cam_geometry_bindings))]
pub fn face_boundary(
    _shape: &OcctShape,
    _face_idx: usize,
) -> Result<Vec<(f64, f64)>, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── face_fingerprint ──────────────────────────────────────────────────────────

/// Compute a stable 64-character hex SHA-256 fingerprint for a planar face.
///
/// The fingerprint is derived from a canonical string that rounds every field
/// to 4 decimal places, ensuring identical geometry produces identical keys.
/// This function has no cfg gate — it compiles everywhere.
pub fn face_fingerprint(info: &FaceInfo) -> String {
    let canonical = format!(
        "cx:{:.4},cy:{:.4},cz:{:.4},nx:{:.4},ny:{:.4},nz:{:.4},a:{:.4}",
        info.centroid[0],
        info.centroid[1],
        info.centroid[2],
        info.normal[0],
        info.normal[1],
        info.normal[2],
        info.area,
    );
    let hash = Sha256::digest(canonical.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in hash.iter() {
        write!(hex, "{:02x}", byte).unwrap();
    }
    hex
}

// ── enumerate_faces ───────────────────────────────────────────────────────────

/// Iterate every face in `shape` and return a descriptor for each planar face.
///
/// Non-planar faces (where [`face_info`] returns an error) are silently skipped.
#[cfg(cam_geometry_bindings)]
pub fn enumerate_faces(shape: &OcctShape) -> Result<Vec<FaceDescriptor>, GeometryError> {
    let count = face_count(shape)?;
    let mut descriptors = Vec::new();
    for idx in 0..count {
        match face_info(shape, idx) {
            Ok(info) => {
                let fingerprint = face_fingerprint(&info);
                descriptors.push(FaceDescriptor {
                    fingerprint,
                    face_idx: idx,
                    centroid: info.centroid,
                    normal: info.normal,
                    area: info.area,
                });
            }
            Err(_) => {
                // Non-planar face — skip silently.
            }
        }
    }
    Ok(descriptors)
}

#[cfg(not(cam_geometry_bindings))]
pub fn enumerate_faces(_shape: &OcctShape) -> Result<Vec<FaceDescriptor>, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "OCCT not available".into(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_face_info(centroid: [f64; 3], normal: [f64; 3], area: f64) -> FaceInfo {
        FaceInfo {
            centroid,
            normal,
            area,
        }
    }

    #[test]
    fn face_fingerprint_is_deterministic() {
        let info = make_face_info([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], 25.0);
        let a = face_fingerprint(&info);
        let b = face_fingerprint(&info);
        assert_eq!(a, b);
    }

    #[test]
    fn face_fingerprint_differs_for_different_inputs() {
        let a = face_fingerprint(&make_face_info([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], 25.0));
        let b = face_fingerprint(&make_face_info([9.0, 8.0, 7.0], [1.0, 0.0, 0.0], 10.0));
        assert_ne!(a, b);
    }

    #[test]
    fn face_fingerprint_stable_known_value() {
        let info = make_face_info([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], 25.0);
        let result = face_fingerprint(&info);
        assert_eq!(result.len(), 64);
        assert_eq!(
            result,
            "64f91781c51c7e91ee64aa4188ebbf4c41fe5f2958217150d13c73e750bdec9b"
        );
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_info_stub_returns_error() {
        let shape = OcctShape::new_for_test(0);
        assert!(face_info(&shape, 0).is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn face_boundary_stub_returns_error() {
        let shape = OcctShape::new_for_test(0);
        assert!(face_boundary(&shape, 0).is_err());
    }
}
