//! Gouge detection and auto-lift correction for toolpaths.
//!
//! A gouge occurs when the tool cuts below the intended surface. This module
//! checks every cutting point against the model geometry and optionally
//! lifts offending points upward to eliminate gouges.

use serde::{Deserialize, Serialize};

use super::types::{Pass, PassKind};
use crate::error::AppError;
use crate::geometry::OcctShape;

/// A single point where the toolpath gouges into the model surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GougeViolation {
    pub position: [f64; 3],
    pub gouge_depth: f64,
    pub face_index: usize,
}

/// Result of a gouge check across an entire toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GougeCheckResult {
    pub violations: Vec<GougeViolation>,
    pub passed: bool,
}

/// Numerical tolerance for gouge depth comparisons.
const GOUGE_TOLERANCE: f64 = 1e-6;

/// Check all cutting passes for gouge violations against `shape`.
///
/// Returns a [`GougeCheckResult`] with every point that gouges into the
/// model surface beyond `allowance`.
///
/// # Tool types
/// - `"ball"` — contact Z is offset by half the tool diameter below the
///   toolpath point (the ball centre rides above the contact point).
/// - `"flat"` (or any other value) — contact Z equals the toolpath Z.
pub fn check_gouges(
    passes: &[Pass],
    shape: &OcctShape,
    tool_type: &str,
    tool_diameter: f64,
    allowance: f64,
) -> Result<GougeCheckResult, AppError> {
    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (passes, shape, tool_type, tool_diameter, allowance);
        return Err(AppError::GeometryImport("OCCT not available".into()));
    }

    #[cfg(cam_geometry_bindings)]
    {
        check_gouges_inner(passes, shape, tool_type, tool_diameter, allowance)
    }
}

/// Lift every gouging cutting point upward so it no longer violates.
///
/// Returns the number of corrections applied.
pub fn auto_lift_gouges(
    passes: &mut [Pass],
    shape: &OcctShape,
    tool_type: &str,
    tool_diameter: f64,
    allowance: f64,
) -> Result<usize, AppError> {
    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (passes, shape, tool_type, tool_diameter, allowance);
        return Err(AppError::GeometryImport("OCCT not available".into()));
    }

    #[cfg(cam_geometry_bindings)]
    {
        auto_lift_gouges_inner(passes, shape, tool_type, tool_diameter, allowance)
    }
}

// ── OCCT-dependent implementations ──────────────────────────────────────────

#[cfg(cam_geometry_bindings)]
fn contact_z(point_z: f64, tool_type: &str, tool_diameter: f64) -> f64 {
    match tool_type {
        "ball" => point_z - tool_diameter / 2.0,
        _ => point_z,
    }
}

/// Find the worst gouge depth across all faces for a single contact point.
///
/// Returns `Some((depth, face_index))` for the deepest violation, or `None`
/// if the point does not gouge.
#[cfg(cam_geometry_bindings)]
fn worst_gouge_at_point(
    x: f64,
    y: f64,
    cz: f64,
    faces: &[crate::geometry::OcctFace],
    allowance: f64,
) -> Option<(f64, usize)> {
    let mut worst: Option<(f64, usize)> = None;

    for (fi, face) in faces.iter().enumerate() {
        let (uv, _dist) = match crate::geometry::face_project_point(face, x, y, cz) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let surface_pt = match crate::geometry::face_eval_point(face, uv[0], uv[1]) {
            Ok(pt) => pt,
            Err(_) => continue,
        };
        let surface_z = surface_pt[2];
        if cz < surface_z + allowance - GOUGE_TOLERANCE {
            let depth = (surface_z + allowance) - cz;
            match &worst {
                Some((d, _)) if depth <= *d => {}
                _ => worst = Some((depth, fi)),
            }
        }
    }

    worst
}

#[cfg(cam_geometry_bindings)]
fn check_gouges_inner(
    passes: &[Pass],
    shape: &OcctShape,
    tool_type: &str,
    tool_diameter: f64,
    allowance: f64,
) -> Result<GougeCheckResult, AppError> {
    let faces = crate::geometry::shape_faces(shape)?;
    let mut violations = Vec::new();

    for pass in passes {
        if pass.kind != PassKind::Cutting {
            continue;
        }
        for pt in &pass.cuts {
            let pos = &pt.position;
            let cz = contact_z(pos.z, tool_type, tool_diameter);

            if let Some((depth, face_index)) =
                worst_gouge_at_point(pos.x, pos.y, cz, &faces, allowance)
            {
                violations.push(GougeViolation {
                    position: [pos.x, pos.y, pos.z],
                    gouge_depth: depth,
                    face_index,
                });
            }
        }
    }

    Ok(GougeCheckResult {
        passed: violations.is_empty(),
        violations,
    })
}

#[cfg(cam_geometry_bindings)]
fn auto_lift_gouges_inner(
    passes: &mut [Pass],
    shape: &OcctShape,
    tool_type: &str,
    tool_diameter: f64,
    allowance: f64,
) -> Result<usize, AppError> {
    let faces = crate::geometry::shape_faces(shape)?;
    let mut corrections = 0usize;

    for pass in passes.iter_mut() {
        if pass.kind != PassKind::Cutting {
            continue;
        }
        for pt in pass.cuts.iter_mut() {
            let pos = &pt.position;
            let cz = contact_z(pos.z, tool_type, tool_diameter);

            if let Some((depth, _)) = worst_gouge_at_point(pos.x, pos.y, cz, &faces, allowance) {
                pt.position.z += depth;
                corrections += 1;
            }
        }
    }

    Ok(corrections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;
    use crate::toolpath::types::{CutPoint, MoveKind};

    /// Build a simple vector of cutting passes from raw XYZ positions.
    fn make_test_passes(positions: &[[f64; 3]]) -> Vec<Pass> {
        vec![Pass {
            kind: PassKind::Cutting,
            cuts: positions
                .iter()
                .map(|&[x, y, z]| CutPoint {
                    position: Vec3 { x, y, z },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: None,
                })
                .collect(),
        }]
    }

    #[test]
    fn gouge_violation_serde_round_trip() {
        let v = GougeViolation {
            position: [1.0, 2.0, 3.0],
            gouge_depth: 0.5,
            face_index: 2,
        };
        let json = serde_json::to_string(&v).expect("serialize GougeViolation");
        let recovered: GougeViolation =
            serde_json::from_str(&json).expect("deserialize GougeViolation");
        assert_eq!(recovered.position, [1.0, 2.0, 3.0]);
        assert!((recovered.gouge_depth - 0.5).abs() < f64::EPSILON);
        assert_eq!(recovered.face_index, 2);
    }

    #[test]
    fn gouge_violation_fields_are_camel_case() {
        let v = GougeViolation {
            position: [0.0, 0.0, 0.0],
            gouge_depth: 1.0,
            face_index: 0,
        };
        let value = serde_json::to_value(&v).expect("serialize to value");
        assert!(value.get("gougeDepth").is_some(), "missing gougeDepth");
        assert!(value.get("faceIndex").is_some(), "missing faceIndex");
    }

    #[test]
    fn gouge_check_result_serde_round_trip() {
        let r = GougeCheckResult {
            violations: vec![GougeViolation {
                position: [10.0, 20.0, -1.0],
                gouge_depth: 0.3,
                face_index: 1,
            }],
            passed: false,
        };
        let json = serde_json::to_string(&r).expect("serialize GougeCheckResult");
        let recovered: GougeCheckResult =
            serde_json::from_str(&json).expect("deserialize GougeCheckResult");
        assert!(!recovered.passed);
        assert_eq!(recovered.violations.len(), 1);
    }

    #[test]
    fn gouge_check_result_passed_when_empty() {
        let r = GougeCheckResult {
            violations: vec![],
            passed: true,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let recovered: GougeCheckResult = serde_json::from_str(&json).expect("deserialize");
        assert!(recovered.passed);
        assert!(recovered.violations.is_empty());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn check_gouges_returns_error_without_occt() {
        use crate::geometry::OcctShape;

        let shape = OcctShape::new_for_test(0);
        let result = check_gouges(&[], &shape, "flat", 10.0, 0.0);
        assert!(result.is_err());
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn auto_lift_returns_error_without_occt() {
        use crate::geometry::OcctShape;

        let shape = OcctShape::new_for_test(0);
        let result = auto_lift_gouges(&mut [], &shape, "flat", 10.0, 0.0);
        assert!(result.is_err());
    }

    // ── OCCT-dependent unit tests ────────────────────────────────────────────

    #[cfg(cam_geometry_bindings)]
    fn load_box_shape() -> crate::geometry::OcctShape {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
        crate::geometry::OcctShape::load_step(&path).expect("load box.step")
    }

    /// (a) Clean path — points well above the top face produce no violations.
    #[cfg(cam_geometry_bindings)]
    #[test]
    fn clean_path_no_gouges() {
        let shape = load_box_shape();
        let (xmin, ymin, _zmin, xmax, ymax, zmax) = shape.bounding_box();

        // Place points 10mm above the top face, well inside the XY footprint.
        let cx = (xmin + xmax) / 2.0;
        let cy = (ymin + ymax) / 2.0;
        let positions = [
            [cx - 1.0, cy, zmax + 10.0],
            [cx, cy, zmax + 10.0],
            [cx + 1.0, cy, zmax + 10.0],
        ];
        let passes = make_test_passes(&positions);

        let result = check_gouges(&passes, &shape, "flat", 6.0, 0.0).expect("check_gouges");
        assert!(
            result.passed,
            "expected no violations, got {}",
            result.violations.len()
        );
        assert!(result.violations.is_empty());
    }

    /// (b) Known gouges — points below surface produce violations with expected depths.
    #[cfg(cam_geometry_bindings)]
    #[test]
    fn known_gouges_detected() {
        let shape = load_box_shape();
        let (xmin, ymin, _zmin, xmax, ymax, zmax) = shape.bounding_box();

        let gouge_amount = 0.5;
        // Place points 0.5mm below the top face, well inside the XY footprint.
        let cx = (xmin + xmax) / 2.0;
        let cy = (ymin + ymax) / 2.0;
        let positions = [
            [cx - 1.0, cy, zmax - gouge_amount],
            [cx, cy, zmax - gouge_amount],
            [cx + 1.0, cy, zmax - gouge_amount],
        ];
        let passes = make_test_passes(&positions);

        let result = check_gouges(&passes, &shape, "flat", 6.0, 0.0).expect("check_gouges");
        assert!(!result.passed, "expected violations for sub-surface points");
        assert_eq!(result.violations.len(), positions.len());

        for v in &result.violations {
            assert!(
                (v.gouge_depth - gouge_amount).abs() < 0.05,
                "expected gouge depth ~{gouge_amount}, got {:.4}",
                v.gouge_depth
            );
        }
    }

    /// (c) Auto-lift corrects gouging points so re-check passes.
    #[cfg(cam_geometry_bindings)]
    #[test]
    fn auto_lift_fixes_gouges() {
        let shape = load_box_shape();
        let (xmin, ymin, _zmin, xmax, ymax, zmax) = shape.bounding_box();

        let cx = (xmin + xmax) / 2.0;
        let cy = (ymin + ymax) / 2.0;
        let positions = [
            [cx - 1.0, cy, zmax - 0.5],
            [cx, cy, zmax - 0.5],
            [cx + 1.0, cy, zmax - 0.5],
        ];
        let mut passes = make_test_passes(&positions);

        let corrections =
            auto_lift_gouges(&mut passes, &shape, "flat", 6.0, 0.0).expect("auto_lift");
        assert!(corrections > 0, "expected corrections on gouging path");

        let result =
            check_gouges(&passes, &shape, "flat", 6.0, 0.0).expect("check_gouges after lift");
        assert!(
            result.passed,
            "expected zero violations after auto-lift, got {}",
            result.violations.len()
        );
    }

    /// (d) Tool type handling — "ball" has a lower contact Z than "flat" for the
    ///     same toolpath point, so it detects more/deeper gouges.
    #[cfg(cam_geometry_bindings)]
    #[test]
    fn ball_vs_flat_tool_type() {
        let shape = load_box_shape();
        let (xmin, ymin, _zmin, xmax, ymax, zmax) = shape.bounding_box();

        let tool_diameter = 6.0;
        // Place points exactly at the top face Z.  A flat tool has contact_z = z
        // (right at the surface — no gouge), while a ball tool has
        // contact_z = z - radius = z - 3.0 (well below the surface — gouge).
        let cx = (xmin + xmax) / 2.0;
        let cy = (ymin + ymax) / 2.0;
        let positions = [[cx - 1.0, cy, zmax], [cx + 1.0, cy, zmax]];
        let passes = make_test_passes(&positions);

        let flat_result = check_gouges(&passes, &shape, "flat", tool_diameter, 0.0).expect("flat");
        let ball_result = check_gouges(&passes, &shape, "ball", tool_diameter, 0.0).expect("ball");

        // Flat tool sits right on the surface — should pass.
        assert!(
            flat_result.passed,
            "flat tool at surface Z should not gouge, got {} violations",
            flat_result.violations.len()
        );

        // Ball tool contact point is 3mm below Z — should gouge.
        assert!(
            !ball_result.passed,
            "ball tool should detect gouges (contact Z is below surface)"
        );
        assert!(
            ball_result.violations.len() > flat_result.violations.len(),
            "ball tool should have more violations than flat"
        );
    }

    /// (e) Allowance respect — points above raw surface but below surface + allowance
    ///     are flagged as violations.
    #[cfg(cam_geometry_bindings)]
    #[test]
    fn allowance_respected() {
        let shape = load_box_shape();
        let (xmin, ymin, _zmin, xmax, ymax, zmax) = shape.bounding_box();

        let allowance = 1.0;
        // Place points 0.5mm above the raw surface, well inside the XY footprint:
        // above the surface itself but below surface + allowance (= zmax + 1.0).
        let cx = (xmin + xmax) / 2.0;
        let cy = (ymin + ymax) / 2.0;
        let positions = [[cx - 1.0, cy, zmax + 0.5], [cx + 1.0, cy, zmax + 0.5]];
        let passes = make_test_passes(&positions);

        // With zero allowance these should be clean.
        let no_allowance = check_gouges(&passes, &shape, "flat", 6.0, 0.0).expect("zero allowance");
        assert!(
            no_allowance.passed,
            "points above surface should pass with zero allowance"
        );

        // With allowance=1.0 these points are 0.5mm below the effective surface.
        let with_allowance =
            check_gouges(&passes, &shape, "flat", 6.0, allowance).expect("with allowance");
        assert!(
            !with_allowance.passed,
            "points between raw surface and surface+allowance should violate"
        );
        for v in &with_allowance.violations {
            // Depth must be positive and at most the full allowance value.
            // (Side-face projections can report depth == allowance.)
            assert!(
                v.gouge_depth > 0.0 && v.gouge_depth <= allowance + 1e-6,
                "expected gouge depth in (0, {allowance}], got {:.4}",
                v.gouge_depth
            );
        }
    }
}
