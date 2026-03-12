//! Hole detection — wraps the C++ `cg_shape_find_holes` API.

use super::safe::{GeometryError, OcctShape};

/// Description of a single detected hole in a B-rep shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoleDescriptor {
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub depth: f64,
    pub is_through: bool,
}

/// Find cylindrical holes in `shape` whose diameter falls within
/// `[min_diameter, max_diameter]`.
///
/// # Errors
/// - [`GeometryError::NotImplemented`] — OCCT bindings were not compiled in.
#[cfg(cam_geometry_bindings)]
pub fn find_holes(
    shape: &OcctShape,
    min_diameter: f64,
    max_diameter: f64,
) -> Result<Vec<HoleDescriptor>, GeometryError> {
    let mut out_holes: *mut super::ffi::CgHoleInfo = std::ptr::null_mut();
    let count = unsafe {
        super::ffi::cg_shape_find_holes(shape.raw_id(), min_diameter, max_diameter, &mut out_holes)
    };

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let hole = unsafe { *out_holes.add(i) };
        result.push(HoleDescriptor {
            center_x: hole.center.x,
            center_y: hole.center.y,
            radius: hole.diameter / 2.0,
            depth: hole.depth,
            is_through: hole.is_through != 0,
        });
    }

    if !out_holes.is_null() {
        unsafe { super::ffi::cg_holes_free(out_holes) };
    }

    Ok(result)
}

#[cfg(not(cam_geometry_bindings))]
pub fn find_holes(
    _shape: &OcctShape,
    _min_diameter: f64,
    _max_diameter: f64,
) -> Result<Vec<HoleDescriptor>, GeometryError> {
    Err(GeometryError::NotImplemented)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn find_holes_stub_returns_not_implemented() {
        let shape = OcctShape::new_for_test(0);
        assert!(matches!(
            find_holes(&shape, 0.0, 100.0),
            Err(GeometryError::NotImplemented)
        ));
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn find_holes_in_plate_fixture() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/plate_with_holes.step"
        ));
        let (_mesh, shape) =
            super::super::importer::import_with_shape(path).expect("fixture should load");
        let shape = shape.expect("STEP file should produce a shape");

        let mut holes = find_holes(&shape, 0.0, 100.0).expect("find_holes should succeed");
        assert_eq!(holes.len(), 3, "plate_with_holes.step should have 3 holes");

        // Sort by center_x then center_y for deterministic comparison.
        holes.sort_by(|a, b| {
            a.center_x
                .partial_cmp(&b.center_x)
                .unwrap()
                .then(a.center_y.partial_cmp(&b.center_y).unwrap())
        });

        let eps = 0.5;

        // Hole 1: center=(25,25), radius=5, depth=20, through
        assert!((holes[0].center_x - 25.0).abs() < eps, "hole0 cx");
        assert!((holes[0].center_y - 25.0).abs() < eps, "hole0 cy");
        assert!((holes[0].radius - 5.0).abs() < eps, "hole0 radius");
        assert!((holes[0].depth - 20.0).abs() < eps, "hole0 depth");
        assert!(holes[0].is_through, "hole0 should be through");

        // Hole 2: center=(50,75), radius=4, depth=12, blind
        assert!((holes[1].center_x - 50.0).abs() < eps, "hole1 cx");
        assert!((holes[1].center_y - 75.0).abs() < eps, "hole1 cy");
        assert!((holes[1].radius - 4.0).abs() < eps, "hole1 radius");
        assert!((holes[1].depth - 12.0).abs() < eps, "hole1 depth");
        assert!(!holes[1].is_through, "hole1 should be blind");

        // Hole 3: center=(75,25), radius=3, depth=20, through
        assert!((holes[2].center_x - 75.0).abs() < eps, "hole2 cx");
        assert!((holes[2].center_y - 25.0).abs() < eps, "hole2 cy");
        assert!((holes[2].radius - 3.0).abs() < eps, "hole2 radius");
        assert!((holes[2].depth - 20.0).abs() < eps, "hole2 depth");
        assert!(holes[2].is_through, "hole2 should be through");
    }
}
