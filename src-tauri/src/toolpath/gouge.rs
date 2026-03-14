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
}
