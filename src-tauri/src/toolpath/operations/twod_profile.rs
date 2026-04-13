//! Toolpath planning for Mode 2 (2D Profile) operations.

use crate::error::AppError;
use crate::models::{
    operation::{CutType, MillingDirection, Profile2dParams},
    Vec3,
};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

/// Plan a 2D profile toolpath from contour points.
///
/// # Arguments
/// - `params` – operation parameters (cut type, direction, Z levels, step-down)
/// - `tool_radius` – tool radius in mm
/// - `artwork_offset` – XY translation applied to every curve point before planning
/// - `safe_height` – Z height used for rapid retracts between passes
/// - `curve_points` – closed contour in Y-up, mm coordinates (Curve2d.points)
///
/// # Returns
/// An interleaved list of cutting and linking passes ready for post-processing.
pub fn plan_2d_profile(
    params: &Profile2dParams,
    tool_radius: f64,
    artwork_offset: [f64; 2],
    safe_height: f64,
    curve_points: &[[f64; 2]],
) -> Result<Vec<Pass>, AppError> {
    // 1. Apply artwork offset: translate every point by the artwork origin.
    let points_2d: Vec<(f64, f64)> = curve_points
        .iter()
        .map(|p| (p[0] + artwork_offset[0], p[1] + artwork_offset[1]))
        .collect();

    // 2. Compute tool path polygon via polygon offset (or identity for OnLine).
    //
    // Positive delta = outward (outside cut); negative delta = inward (inside cut).
    let tool_path: Vec<(f64, f64)> = match params.cut_type {
        CutType::OnLine => points_2d,
        CutType::Inside => {
            crate::geometry::poly_offset(&points_2d, -tool_radius, 0.05).map_err(|_| {
                AppError::InvalidInput(
                    "tool radius too large for inside cut on this curve".to_string(),
                )
            })?
        }
        CutType::Outside => crate::geometry::poly_offset(&points_2d, tool_radius, 0.05)
            .map_err(|e| AppError::InvalidInput(e.to_string()))?,
    };

    // 3. Winding order normalisation via shoelace signed area.
    //
    // Convention (matches standard climb/conventional milling semantics in a
    // Y-up coordinate system where the tool descends along -Z):
    //
    //   Cut type          Direction      Required winding
    //   ────────────────  ─────────────  ────────────────
    //   Outside / OnLine  Climb          CCW  (positive shoelace area)
    //   Outside / OnLine  Conventional   CW   (negative shoelace area)
    //   Inside            Climb          CW   (negative shoelace area)
    //   Inside            Conventional   CCW  (positive shoelace area)
    let want_ccw = matches!(
        (&params.cut_type, &params.direction),
        (CutType::Outside | CutType::OnLine, MillingDirection::Climb)
            | (CutType::Inside, MillingDirection::Conventional)
    );
    let tool_path = normalize_winding(tool_path, want_ccw);

    // 4. Z-level pass generation.
    let bottom_z = params.top_of_cut - params.depth_of_cut;
    let z_levels = compute_z_levels(params.top_of_cut, params.step_down, bottom_z);

    // 5. Build interleaved cutting + linking passes.
    if tool_path.is_empty() {
        return Ok(vec![]);
    }
    let mut passes: Vec<Pass> = Vec::with_capacity(z_levels.len() * 2);

    // Initial approach: rapid to the entry point at safe height, then descend.
    let entry_xy = tool_path[0];
    passes.push(Pass {
        kind: PassKind::Linking,
        cuts: vec![
            CutPoint {
                position: Vec3 { x: entry_xy.0, y: entry_xy.1, z: safe_height },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
            CutPoint {
                position: Vec3 { x: entry_xy.0, y: entry_xy.1, z: params.top_of_cut },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
        ],
    });

    for (idx, &z) in z_levels.iter().enumerate() {
        // Insert a linking pass between consecutive cutting passes.
        if idx > 0 {
            // The previous cutting pass closed at tool_path[0]; retract and
            // re-plunge at the same XY for the next Z level.
            let from_xy = tool_path[0];
            let to_xy = tool_path[0];
            passes.push(make_linking_pass(
                from_xy,
                to_xy,
                params.top_of_cut,
                safe_height,
            ));
        }

        // Build the cutting pass: every point is a Feed move.
        // The first point performs the entry plunge; subsequent points traverse
        // the contour at the same Z level.  The first point is repeated at the
        // end to close the polygon.
        let cuts: Vec<CutPoint> = tool_path
            .iter()
            .chain(std::iter::once(&tool_path[0]))
            .map(|&(x, y)| CutPoint {
                position: Vec3 { x, y, z },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: None,
            })
            .collect();

        passes.push(Pass {
            kind: PassKind::Cutting,
            cuts,
        });
    }

    // Final retract: raise tool to safe height after the last cutting pass.
    // Two points so the retract is visible in the line-geometry preview.
    let exit_xy = tool_path[0]; // last cutting pass closed at the entry point
    passes.push(Pass {
        kind: PassKind::Linking,
        cuts: vec![
            CutPoint {
                position: Vec3 { x: exit_xy.0, y: exit_xy.1, z: bottom_z },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
            CutPoint {
                position: Vec3 { x: exit_xy.0, y: exit_xy.1, z: safe_height },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
        ],
    });

    Ok(passes)
}

// ---------------------------------------------------------------------------
// Z-level helpers
// ---------------------------------------------------------------------------

/// Compute the ordered list of Z depths for each cutting pass.
///
/// Starts at `top_of_cut − step_down` and descends by `step_down` each pass.
/// The final entry is clamped to `bottom_z` so the deepest pass always lands
/// exactly at the programmed depth.  When `step_down ≥ (top_of_cut − bottom_z)`
/// a single pass at `bottom_z` is returned.
///
/// Uses a loop rather than `ceil(depth / step_down)` to avoid the case where
/// IEEE 754 rounding makes the ratio appear fractionally above an integer,
/// which would produce a spurious duplicate final pass at `bottom_z`.
fn compute_z_levels(top_of_cut: f64, step_down: f64, bottom_z: f64) -> Vec<f64> {
    let depth = top_of_cut - bottom_z;
    if depth <= 0.0 || step_down <= 0.0 {
        return vec![bottom_z];
    }
    let mut levels = Vec::new();
    let mut n = 1usize;
    loop {
        let z = (top_of_cut - step_down * n as f64).max(bottom_z);
        levels.push(z);
        if z <= bottom_z {
            break;
        }
        n += 1;
    }
    levels
}

// ---------------------------------------------------------------------------
// Winding order helpers
// ---------------------------------------------------------------------------

/// Compute the signed area of a closed polygon via the shoelace formula.
///
/// Positive → CCW (right-hand Y-up convention); negative → CW.
fn signed_area(points: &[(f64, f64)]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % n];
        sum += x1 * y2 - x2 * y1;
    }
    sum / 2.0
}

/// Reverse `points` if their winding does not match `want_ccw`.
fn normalize_winding(mut points: Vec<(f64, f64)>, want_ccw: bool) -> Vec<(f64, f64)> {
    let area = signed_area(&points);
    let is_ccw = area > 0.0;
    if is_ccw != want_ccw {
        points.reverse();
    }
    points
}

// ---------------------------------------------------------------------------
// Linking helpers
// ---------------------------------------------------------------------------

/// Build a simple plunge-linking pass between two consecutive cutting passes.
///
/// Sequence (all Rapid):
///   1. Retract to `safe_height` above the end of the previous cutting pass.
///   2. Traverse at `safe_height` to the XY start of the next cutting pass.
///   3. Descend to `top_of_cut` (the rapid positioning height, above material).
///
/// The actual feed plunge to cut depth is the first move of the following
/// cutting pass.  No arc lead-in / lead-out is generated here.
fn make_linking_pass(
    from_xy: (f64, f64),
    to_xy: (f64, f64),
    top_of_cut: f64,
    safe_height: f64,
) -> Pass {
    Pass {
        kind: PassKind::Linking,
        cuts: vec![
            // Retract to safe height at end of previous cutting pass.
            CutPoint {
                position: Vec3 {
                    x: from_xy.0,
                    y: from_xy.1,
                    z: safe_height,
                },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
            // Traverse to XY of next pass entry at safe height.
            CutPoint {
                position: Vec3 {
                    x: to_xy.0,
                    y: to_xy.1,
                    z: safe_height,
                },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
            // Descend to top_of_cut (rapid positioning, above material surface).
            CutPoint {
                position: Vec3 {
                    x: to_xy.0,
                    y: to_xy.1,
                    z: top_of_cut,
                },
                move_kind: MoveKind::Rapid,
                tool_orientation: None,
                feed_rate_override: None,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{CutType, MillingDirection, Profile2dParams};
    use uuid::Uuid;

    fn square_params(cut_type: CutType, direction: MillingDirection) -> Profile2dParams {
        Profile2dParams {
            curve_id: Uuid::nil(),
            cut_type,
            direction,
            top_of_cut: 0.0,
            depth_of_cut: 5.0,
            step_down: 5.0,
            feed_rate: 1000.0,
        }
    }

    /// CCW square (positive shoelace area): [0,0]→[10,0]→[10,10]→[0,10]
    fn ccw_square() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
    }

    /// CW square (negative shoelace area): [0,0]→[0,10]→[10,10]→[10,0]
    fn cw_square() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]]
    }

    // ── Single-pass ─────────────────────────────────────────────────────────

    /// When step_down >= depth_of_cut, exactly one cutting pass should be produced
    /// and the Z of all cut points must equal top_of_cut - depth_of_cut.
    #[test]
    fn single_pass_when_step_down_ge_depth() {
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square())
            .expect("plan should succeed");

        let cutting: Vec<&Pass> = passes
            .iter()
            .filter(|p| p.kind == PassKind::Cutting)
            .collect();

        assert_eq!(cutting.len(), 1, "expected exactly 1 cutting pass");
        let expected_z = params.top_of_cut - params.depth_of_cut;
        for cp in &cutting[0].cuts {
            assert!(
                (cp.position.z - expected_z).abs() < 1e-9,
                "z={} expected {}",
                cp.position.z,
                expected_z
            );
        }
    }

    // ── Multi-pass Z levels ─────────────────────────────────────────────────

    /// top_of_cut=0, step_down=2.5, depth=10 → 4 passes at -2.5, -5.0, -7.5, -10.0.
    #[test]
    fn multi_pass_z_levels() {
        let params = Profile2dParams {
            curve_id: Uuid::nil(),
            cut_type: CutType::OnLine,
            direction: MillingDirection::Climb,
            top_of_cut: 0.0,
            depth_of_cut: 10.0,
            step_down: 2.5,
            feed_rate: 1000.0,
        };
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square())
            .expect("plan should succeed");

        let z_values: Vec<f64> = passes
            .iter()
            .filter(|p| p.kind == PassKind::Cutting)
            .map(|p| p.cuts[0].position.z)
            .collect();

        assert_eq!(z_values.len(), 4, "expected 4 cutting passes");
        let expected = [-2.5, -5.0, -7.5, -10.0];
        for (got, exp) in z_values.iter().zip(expected.iter()) {
            assert!(
                (got - exp).abs() < 1e-9,
                "z level mismatch: got {got}, expected {exp}"
            );
        }
        // Final pass must land exactly at bottom_z.
        assert!(
            (z_values.last().unwrap() - (-10.0_f64)).abs() < 1e-9,
            "final pass must be exactly at bottom_z=-10.0"
        );
    }

    // ── Artwork offset ───────────────────────────────────────────────────────

    /// With offset [5,3], cutting pass points should have X>=5 and Y>=3.
    #[test]
    fn artwork_offset_applied() {
        let curve = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        let passes =
            plan_2d_profile(&params, 3.0, [5.0, 3.0], 10.0, &curve).expect("plan should succeed");

        for pass in passes.iter().filter(|p| p.kind == PassKind::Cutting) {
            for cp in &pass.cuts {
                assert!(
                    cp.position.x >= 5.0 - 1e-9,
                    "X={} should be >=5.0",
                    cp.position.x
                );
                assert!(
                    cp.position.y >= 3.0 - 1e-9,
                    "Y={} should be >=3.0",
                    cp.position.y
                );
            }
        }
    }

    // ── OnLine cut ───────────────────────────────────────────────────────────

    /// OnLine cut must not call poly_offset: the XY of the cutting pass should
    /// exactly equal the (offset-adjusted) input points, plus a closing point.
    #[test]
    fn on_line_uses_input_polygon_directly() {
        let curve = ccw_square();
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        let passes =
            plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &curve).expect("plan should succeed");

        let cutting = passes
            .iter()
            .find(|p| p.kind == PassKind::Cutting)
            .expect("at least one cutting pass");

        let got_xy: Vec<(f64, f64)> = cutting
            .cuts
            .iter()
            .map(|c| (c.position.x, c.position.y))
            .collect();

        // The pass should have N+1 points: the contour vertices plus a
        // closing point that repeats the first vertex.
        assert_eq!(
            got_xy.len(),
            curve.len() + 1,
            "cutting pass should close the polygon"
        );
        assert_eq!(
            got_xy.first(),
            got_xy.last(),
            "first and last point must match (closed contour)"
        );

        // All input curve points should appear in the pass (order may differ
        // due to winding normalisation).
        let mut got_unique: Vec<(f64, f64)> = got_xy[..got_xy.len() - 1].to_vec();
        got_unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut expected: Vec<(f64, f64)> = curve.iter().map(|p| (p[0], p[1])).collect();
        expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            got_unique, expected,
            "OnLine cut should use the input polygon points unchanged"
        );
    }

    // ── Winding order ────────────────────────────────────────────────────────

    /// CW input square + OnLine + Outside + Climb → cutting pass must be CCW
    /// (positive shoelace area).
    #[test]
    fn cw_input_outside_climb_gives_ccw_output() {
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &cw_square())
            .expect("plan should succeed");

        let cutting = passes
            .iter()
            .find(|p| p.kind == PassKind::Cutting)
            .expect("at least one cutting pass");

        let xy: Vec<(f64, f64)> = cutting
            .cuts
            .iter()
            .map(|c| (c.position.x, c.position.y))
            .collect();

        let area = signed_area(&xy);
        assert!(area > 0.0, "expected CCW (positive area), got area={area}");
    }

    /// CCW input square + OnLine + Outside + Conventional → cutting pass must be CW
    /// (negative shoelace area).
    #[test]
    fn ccw_input_outside_conventional_gives_cw_output() {
        let params = square_params(CutType::OnLine, MillingDirection::Conventional);
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square())
            .expect("plan should succeed");

        let cutting = passes
            .iter()
            .find(|p| p.kind == PassKind::Cutting)
            .expect("at least one cutting pass");

        let xy: Vec<(f64, f64)> = cutting
            .cuts
            .iter()
            .map(|c| (c.position.x, c.position.y))
            .collect();

        let area = signed_area(&xy);
        assert!(area < 0.0, "expected CW (negative area), got area={area}");
    }

    // ── No-bindings stubs ────────────────────────────────────────────────────

    /// Without Clipper2 bindings, inside and outside cuts must return an error
    /// while on-line cut succeeds.
    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn no_bindings_inside_cut_returns_error() {
        let params = square_params(CutType::Inside, MillingDirection::Climb);
        let result = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square());
        assert!(
            result.is_err(),
            "inside cut should return Err without bindings"
        );
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn no_bindings_outside_cut_returns_error() {
        let params = square_params(CutType::Outside, MillingDirection::Climb);
        let result = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square());
        assert!(
            result.is_err(),
            "outside cut should return Err without bindings"
        );
    }

    // ── Final retract ─────────────────────────────────────────────────────

    /// After the last cutting pass the toolpath must end with a linking pass
    /// that retracts the tool from cut depth to safe height.
    #[test]
    fn final_pass_retracts_to_safe_height() {
        let safe_height = 10.0;
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        // bottom_z = 0.0 - 5.0 = -5.0
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], safe_height, &ccw_square())
            .expect("plan should succeed");

        let last = passes.last().expect("should have at least one pass");
        assert_eq!(
            last.kind,
            PassKind::Linking,
            "last pass must be a linking (retract) pass"
        );
        assert_eq!(last.cuts.len(), 2, "retract pass needs 2 points for line geometry");
        let first_point = &last.cuts[0];
        assert!(
            (first_point.position.z - (-5.0)).abs() < 1e-9,
            "retract starts at bottom_z=-5.0, got Z={}",
            first_point.position.z
        );
        let final_point = &last.cuts[1];
        assert!(
            (final_point.position.z - safe_height).abs() < 1e-9,
            "retract ends at safe_height={}, got Z={}",
            safe_height,
            final_point.position.z
        );
    }

    /// Multi-pass: the final retract must still go from bottom depth to safe height.
    #[test]
    fn multi_pass_final_retract_to_safe_height() {
        let safe_height = 15.0;
        let params = Profile2dParams {
            curve_id: Uuid::nil(),
            cut_type: CutType::OnLine,
            direction: MillingDirection::Climb,
            top_of_cut: 0.0,
            depth_of_cut: 10.0,
            step_down: 2.5,
            feed_rate: 1000.0,
        };
        // bottom_z = 0.0 - 10.0 = -10.0
        let passes = plan_2d_profile(&params, 3.0, [0.0, 0.0], safe_height, &ccw_square())
            .expect("plan should succeed");

        let last = passes.last().expect("should have at least one pass");
        assert_eq!(
            last.kind,
            PassKind::Linking,
            "last pass must be a linking (retract) pass"
        );
        assert_eq!(last.cuts.len(), 2, "retract pass needs 2 points for line geometry");
        assert!(
            (last.cuts[0].position.z - (-10.0)).abs() < 1e-9,
            "retract starts at bottom_z=-10.0, got Z={}",
            last.cuts[0].position.z
        );
        assert!(
            (last.cuts[1].position.z - safe_height).abs() < 1e-9,
            "retract ends at safe_height={}, got Z={}",
            safe_height,
            last.cuts[1].position.z
        );
    }

    // ── No-bindings stubs ────────────────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn no_bindings_on_line_cut_succeeds() {
        let params = square_params(CutType::OnLine, MillingDirection::Climb);
        let result = plan_2d_profile(&params, 3.0, [0.0, 0.0], 10.0, &ccw_square());
        assert!(
            result.is_ok(),
            "on-line cut should succeed without bindings"
        );
    }
}
