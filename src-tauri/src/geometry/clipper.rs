//! Safe Rust wrappers for Clipper2 2D polygon offset and boolean operations.
//!
//! All `unsafe` code is isolated here. Every public function returns a `Result`
//! so callers never deal with raw pointers or C error codes.

use super::safe::GeometryError;

// ── BoolOp ────────────────────────────────────────────────────────────────────

/// Boolean operation kind for [`poly_boolean`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

// ── Helper (bindings only) ────────────────────────────────────────────────────

/// Copy the last C-layer error message into an owned [`String`].
#[cfg(cam_geometry_bindings)]
fn last_error_message() -> String {
    // SAFETY: `cg_last_error_message` returns a thread-local pointer valid
    // until the next FFI call on this thread. We copy it into an owned
    // String immediately.
    unsafe {
        let ptr = super::ffi::cg_last_error_message();
        if ptr.is_null() {
            return "unknown error".into();
        }
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

// ── poly_offset ───────────────────────────────────────────────────────────────

/// Offset a closed 2-D polygon by `delta` mm (positive = outward, negative = inward).
///
/// `arc_tolerance` controls the maximum chord deviation when approximating
/// rounded corners.
///
/// Returns [`GeometryError::ImportFailed`] if the offset collapses the polygon
/// entirely or the C layer reports an error.
#[cfg(cam_geometry_bindings)]
pub fn poly_offset(
    points: &[(f64, f64)],
    delta: f64,
    arc_tolerance: f64,
) -> Result<Vec<(f64, f64)>, GeometryError> {
    let flat: Vec<f64> = points.iter().flat_map(|&(x, y)| [x, y]).collect();
    let mut out_ptr: *mut f64 = std::ptr::null_mut();
    let mut out_count: usize = 0;

    let err = unsafe {
        super::ffi::cg_poly_offset(
            flat.as_ptr(),
            flat.len() / 2,
            delta,
            arc_tolerance,
            &mut out_ptr,
            &mut out_count,
        )
    };

    if err as u32 == super::ffi::CgError::CG_ERR_NO_RESULT as u32 {
        return Err(GeometryError::ImportFailed {
            message: "offset collapsed polygon".into(),
        });
    }

    if err as u32 != super::ffi::CgError::CG_OK as u32 {
        return Err(GeometryError::ImportFailed {
            message: last_error_message(),
        });
    }

    // SAFETY: on CG_OK the C layer guarantees out_ptr points to an array of
    // out_count * 2 f64 values allocated by cg_poly_offset. We copy into an
    // owned Vec before freeing.
    let result = unsafe {
        let slice = std::slice::from_raw_parts(out_ptr, out_count * 2);
        let vec: Vec<(f64, f64)> = slice.chunks_exact(2).map(|c| (c[0], c[1])).collect();
        super::ffi::cg_poly_free(out_ptr);
        vec
    };

    Ok(result)
}

#[cfg(not(cam_geometry_bindings))]
pub fn poly_offset(
    _points: &[(f64, f64)],
    _delta: f64,
    _arc_tolerance: f64,
) -> Result<Vec<(f64, f64)>, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "Clipper2 not available".into(),
    })
}

// ── poly_boolean ──────────────────────────────────────────────────────────────

/// Perform a boolean operation between two closed 2-D polygons.
///
/// Returns [`GeometryError::ImportFailed`] if the operation produces no result
/// or the C layer reports an error.
#[cfg(cam_geometry_bindings)]
pub fn poly_boolean(
    a: &[(f64, f64)],
    b: &[(f64, f64)],
    op: BoolOp,
) -> Result<Vec<(f64, f64)>, GeometryError> {
    let flat_a: Vec<f64> = a.iter().flat_map(|&(x, y)| [x, y]).collect();
    let flat_b: Vec<f64> = b.iter().flat_map(|&(x, y)| [x, y]).collect();

    let c_op = match op {
        BoolOp::Union => super::ffi::CgBoolOp::CG_BOOL_UNION,
        BoolOp::Difference => super::ffi::CgBoolOp::CG_BOOL_DIFFERENCE,
        BoolOp::Intersection => super::ffi::CgBoolOp::CG_BOOL_INTERSECTION,
    };

    let mut out_ptr: *mut f64 = std::ptr::null_mut();
    let mut out_count: usize = 0;

    let err = unsafe {
        super::ffi::cg_poly_boolean(
            flat_a.as_ptr(),
            flat_a.len() / 2,
            flat_b.as_ptr(),
            flat_b.len() / 2,
            c_op,
            &mut out_ptr,
            &mut out_count,
        )
    };

    if err as u32 == super::ffi::CgError::CG_ERR_NO_RESULT as u32 {
        return Err(GeometryError::ImportFailed {
            message: "boolean operation produced no result".into(),
        });
    }

    if err as u32 != super::ffi::CgError::CG_OK as u32 {
        return Err(GeometryError::ImportFailed {
            message: last_error_message(),
        });
    }

    // SAFETY: same guarantee as poly_offset — out_ptr is valid for out_count*2
    // f64 values on CG_OK.
    let result = unsafe {
        let slice = std::slice::from_raw_parts(out_ptr, out_count * 2);
        let vec: Vec<(f64, f64)> = slice.chunks_exact(2).map(|c| (c[0], c[1])).collect();
        super::ffi::cg_poly_free(out_ptr);
        vec
    };

    Ok(result)
}

#[cfg(not(cam_geometry_bindings))]
pub fn poly_boolean(
    _a: &[(f64, f64)],
    _b: &[(f64, f64)],
    _op: BoolOp,
) -> Result<Vec<(f64, f64)>, GeometryError> {
    Err(GeometryError::ImportFailed {
        message: "Clipper2 not available".into(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Stub tests (always run without Clipper2) ──────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn poly_offset_stub_returns_error() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(matches!(
            poly_offset(&square, -1.0, 0.1),
            Err(GeometryError::ImportFailed { .. })
        ));
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn poly_boolean_stub_returns_error() {
        let a = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let b = vec![(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)];
        assert!(matches!(
            poly_boolean(&a, &b, BoolOp::Union),
            Err(GeometryError::ImportFailed { .. })
        ));
    }

    // ── Integration tests (Clipper2 bindings only) ────────────────────────

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn poly_offset_shrinks_square() {
        // 10×10 square; inward offset by 1 mm → 8×8 square within [1,9]×[1,9]
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let result = poly_offset(&square, -1.0, 0.1).expect("offset should succeed");
        assert_eq!(result.len(), 4, "shrunken square should have 4 vertices");
        for (x, y) in &result {
            assert!(*x >= 1.0 && *x <= 9.0, "x={x} out of [1,9]");
            assert!(*y >= 1.0 && *y <= 9.0, "y={y} out of [1,9]");
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn poly_offset_returns_error_on_collapse() {
        // 1×1 square offset inward by 2 mm collapses entirely
        let square = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let result = poly_offset(&square, -2.0, 0.1);
        assert!(result.is_err(), "collapsed polygon should return Err");
    }
}
