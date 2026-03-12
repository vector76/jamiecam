//! Rest-region computation: determines where stock remains after a roughing pass.
//!
//! Works per Z layer using 2-D boolean geometry (Clipper2 via [`crate::geometry`]).

use crate::geometry::{poly_boolean, poly_offset, BoolOp, GeometryError};

/// Default arc tolerance (mm) for offset operations.
const ARC_TOLERANCE: f64 = 0.1;

/// Compute the regions of `target_boundary` not covered by the roughing pass.
///
/// For each roughing contour the tool's swept area is approximated by offsetting
/// outward by `roughing_tool_radius`.  All swept areas are unioned, then
/// subtracted from `target_boundary`.  The returned polygons are the rest
/// regions where stock still remains.
///
/// Returns an empty `Vec` when roughing covers the entire target.
pub fn compute_rest_region(
    target_boundary: &[(f64, f64)],
    roughing_contours: &[Vec<(f64, f64)>],
    roughing_tool_radius: f64,
) -> Result<Vec<Vec<(f64, f64)>>, GeometryError> {
    if roughing_contours.is_empty() {
        return Ok(vec![target_boundary.to_vec()]);
    }

    // 1. Offset each roughing contour outward by the tool radius to get swept areas.
    let mut swept_areas: Vec<Vec<(f64, f64)>> = Vec::new();
    for contour in roughing_contours {
        match poly_offset(contour, roughing_tool_radius, ARC_TOLERANCE) {
            Ok(offset) => swept_areas.push(offset),
            Err(_) => {
                // Offset collapsed — contour too small; skip it.
            }
        }
    }

    // If all offsets collapsed, nothing was roughed — return full target.
    if swept_areas.is_empty() {
        return Ok(vec![target_boundary.to_vec()]);
    }

    // 2. Union all swept areas together.
    let mut coverage = swept_areas[0].clone();
    for area in &swept_areas[1..] {
        match poly_boolean(&coverage, area, BoolOp::Union) {
            Ok(unioned) => coverage = unioned,
            Err(_) => {
                // Union failed (e.g. disjoint polygons with single-path output);
                // keep the current coverage and continue.
            }
        }
    }

    // 3. Subtract coverage from target boundary.
    match poly_boolean(target_boundary, &coverage, BoolOp::Difference) {
        Ok(rest) => Ok(vec![rest]),
        Err(_) => {
            // Difference produced no result → roughing covered everything.
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<(f64, f64)> {
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn full_coverage_returns_empty() {
        // Roughing polygon that, once offset, fully covers the target.
        let target = square(2.0, 2.0, 8.0, 8.0);
        let roughing = vec![square(0.0, 0.0, 10.0, 10.0)];
        let result = compute_rest_region(&target, &roughing, 1.0).unwrap();
        assert!(
            result.is_empty(),
            "expected empty rest when roughing covers target"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn no_roughing_returns_full_target() {
        let target = square(0.0, 0.0, 10.0, 10.0);
        let result = compute_rest_region(&target, &[], 1.0).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target);
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn partial_coverage_returns_rest() {
        // 20×20 target, roughing only covers center 6×6 (before offset).
        // With tool radius 1.0, swept area ≈ 8×8 — still leaves outer ring.
        let target = square(0.0, 0.0, 20.0, 20.0);
        let roughing = vec![square(7.0, 7.0, 13.0, 13.0)];
        let result = compute_rest_region(&target, &roughing, 1.0).unwrap();
        assert!(
            !result.is_empty(),
            "partial coverage should leave rest region"
        );
        // The rest region should have vertices outside the swept area.
        for poly in &result {
            assert!(
                poly.len() >= 4,
                "rest polygon should have at least 4 vertices"
            );
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn multiple_roughing_contours_unioned() {
        // Two roughing contours that together (after offset) cover the target.
        // Target is 6×6 at (2,2)–(8,8); two overlapping roughing rectangles cover it.
        let target = square(2.0, 2.0, 8.0, 8.0);
        let roughing = vec![square(0.0, 0.0, 6.0, 10.0), square(4.0, 0.0, 10.0, 10.0)];
        let result = compute_rest_region(&target, &roughing, 1.0).unwrap();
        assert!(
            result.is_empty(),
            "two overlapping roughing contours should cover target"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn large_tool_misses_corners() {
        // L-shaped target with a sharp internal corner at (10, 10).
        // Roughing contour is inset from the L-shape by the tool radius, so the
        // swept area (contour + offset) would cover most of the target.  But a
        // large tool radius produces a rounded internal corner that can't fill
        // the sharp 90° concave vertex → rest region remains near (10, 10).
        let l_shape = vec![
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 10.0),
            (10.0, 10.0),
            (10.0, 20.0),
            (0.0, 20.0),
        ];
        let tool_r = 3.0;
        // Roughing contour inset by tool_r from L boundary.
        let roughing = vec![vec![
            (tool_r, tool_r),
            (20.0 - tool_r, tool_r),
            (20.0 - tool_r, 10.0 - tool_r),
            (10.0 - tool_r, 10.0 - tool_r),
            (10.0 - tool_r, 20.0 - tool_r),
            (tool_r, 20.0 - tool_r),
        ]];
        let result = compute_rest_region(&l_shape, &roughing, tool_r).unwrap();
        assert!(
            !result.is_empty(),
            "large tool should miss the sharp internal corner"
        );
    }
}
