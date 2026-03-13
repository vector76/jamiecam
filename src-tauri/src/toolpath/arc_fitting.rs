//! Arc fitting — replaces qualifying sequences of linear Feed moves with Arc moves.
//!
//! The algorithm scans for consecutive `MoveKind::Feed` moves at constant Z,
//! fits circles through sliding windows of 3 points, and replaces runs of 4+
//! points (3+ segments) that lie within `tolerance` of the fitted circle with a
//! single `MoveKind::Arc`.

use crate::models::Vec3;

use super::types::{CutPoint, MoveKind};

/// Replace qualifying linear chord sequences with arc moves.
///
/// Only consecutive `MoveKind::Feed` moves at a constant Z are considered.
/// At least 4 points (3 segments) must fit a circle within `tolerance` for
/// replacement to occur.
pub fn fit_arcs(cuts: Vec<CutPoint>, tolerance: f64) -> Vec<CutPoint> {
    if cuts.len() < 2 {
        return cuts;
    }

    let mut result: Vec<CutPoint> = Vec::with_capacity(cuts.len());
    let mut i = 0;

    while i < cuts.len() {
        // We need a previous point to define the arc start.
        // The first point, or any non-Feed point, is emitted as-is.
        if i == 0 || !matches!(cuts[i].move_kind, MoveKind::Feed) {
            result.push(cuts[i].clone());
            i += 1;
            continue;
        }

        // Try to collect a constant-Z run of Feed points starting at i,
        // using the point before i as the arc start.
        let start_idx = i - 1; // previous point (already emitted)
        let z = cuts[start_idx].position.z;

        // Check that the previous point is at the same Z
        if (cuts[i].position.z - z).abs() > 1e-9 {
            result.push(cuts[i].clone());
            i += 1;
            continue;
        }

        // Collect the run of consecutive Feed points at the same Z
        let mut run_end = i;
        while run_end < cuts.len()
            && matches!(cuts[run_end].move_kind, MoveKind::Feed)
            && (cuts[run_end].position.z - z).abs() < 1e-9
        {
            run_end += 1;
        }

        // run is cuts[start_idx..run_end], where start_idx is already emitted.
        // Feed points are cuts[i..run_end].
        let run_points = &cuts[start_idx..run_end]; // includes the start point

        if run_points.len() < 4 {
            // Not enough points for arc fitting (need 4 points = 3 segments)
            for cut in &cuts[i..run_end] {
                result.push(cut.clone());
            }
            i = run_end;
            continue;
        }

        // Try to fit arcs within this run
        let fitted = fit_arcs_in_run(run_points, tolerance);
        // fitted[0] corresponds to run_points[0] which is already emitted (start_idx),
        // so we skip it.
        for cp in fitted.into_iter().skip(1) {
            result.push(cp);
        }
        i = run_end;
    }

    result
}

/// Fit arcs within a constant-Z run of points.
/// `run[0]` is the start point (may be any MoveKind); `run[1..]` are all Feed.
/// Returns a new sequence starting from run[0].
fn fit_arcs_in_run(run: &[CutPoint], tolerance: f64) -> Vec<CutPoint> {
    let n = run.len();
    let mut result = Vec::with_capacity(n);
    result.push(run[0].clone());

    let mut i = 1; // index into run; we process Feed points starting here

    while i < n {
        // Try to fit an arc starting from run[i-1] (the previous point).
        // We need at least 3 more points (i, i+1, i+2) to have 3 segments.
        if i + 2 >= n {
            // Not enough remaining points for an arc
            for cut in &run[i..n] {
                result.push(cut.clone());
            }
            break;
        }

        // Fit a circle through the first 3 points of this candidate
        let p0 = &run[i - 1].position;
        let p1 = &run[i].position;
        let p2 = &run[i + 1].position;
        let p3 = &run[i + 2].position;

        let Some((cx, cy, r)) = fit_circle_3pt(p0, p1, p2) else {
            // Collinear — emit this point and advance
            result.push(run[i].clone());
            i += 1;
            continue;
        };

        // Check that p3 also lies on this circle
        let d3 = ((p3.x - cx).powi(2) + (p3.y - cy).powi(2)).sqrt();
        if (d3 - r).abs() > tolerance {
            // p3 doesn't fit — no arc here
            result.push(run[i].clone());
            i += 1;
            continue;
        }

        // Determine CW/CCW using center-relative cross products for consistency.
        let dir0 = direction_from_center(cx, cy, p0, p1);
        if dir0 == 0.0 {
            result.push(run[i].clone());
            i += 1;
            continue;
        }
        let clockwise = dir0 < 0.0;

        // Verify direction consistency for the initial seed points
        let dir1 = direction_from_center(cx, cy, p1, p2);
        let dir2 = direction_from_center(cx, cy, p2, p3);
        if (dir1 < 0.0) != clockwise || (dir2 < 0.0) != clockwise {
            result.push(run[i].clone());
            i += 1;
            continue;
        }

        // Extend the arc as far as possible
        let mut arc_end = i + 2; // inclusive index of last point in arc
        for k in (i + 3)..n {
            let pk = &run[k].position;
            let dk = ((pk.x - cx).powi(2) + (pk.y - cy).powi(2)).sqrt();
            if (dk - r).abs() > tolerance {
                break;
            }
            // Check direction consistency
            let prev = &run[k - 1].position;
            let dir = direction_from_center(cx, cy, prev, pk);
            if dir == 0.0 {
                break;
            }
            if (dir < 0.0) != clockwise {
                break;
            }
            arc_end = k;
        }

        // We have an arc from run[i-1] to run[arc_end]
        // Verify we have at least 3 segments (4 points including start)
        let seg_count = arc_end - (i - 1);
        if seg_count < 3 {
            result.push(run[i].clone());
            i += 1;
            continue;
        }

        // Emit a single Arc CutPoint.
        //
        // Convention (matches postprocessor/program.rs):
        //   position = arc START (used for IJK offset computation)
        //   end      = arc END   (emitted as X/Y/Z destination)
        let start_pos = &run[i - 1].position;
        let end_pos = &run[arc_end].position;
        let center = Vec3 {
            x: cx,
            y: cy,
            z: start_pos.z,
        };
        result.push(CutPoint {
            position: start_pos.clone(),
            move_kind: MoveKind::Arc {
                center,
                end: end_pos.clone(),
                clockwise,
            },
            tool_orientation: run[arc_end].tool_orientation.clone(),
            feed_rate_override: run[arc_end].feed_rate_override,
        });

        i = arc_end + 1;
    }

    result
}

/// Fit a circle through 3 points (2D, using x and y only).
/// Returns `Some((cx, cy, radius))` or `None` if collinear.
fn fit_circle_3pt(p1: &Vec3, p2: &Vec3, p3: &Vec3) -> Option<(f64, f64, f64)> {
    let ax = p1.x;
    let ay = p1.y;
    let bx = p2.x;
    let by = p2.y;
    let cx = p3.x;
    let cy = p3.y;

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None; // collinear
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// Cross product of (prev - center) × (next - center) in Z.
/// Positive = CCW rotation, negative = CW rotation.
fn direction_from_center(cx: f64, cy: f64, prev: &Vec3, next: &Vec3) -> f64 {
    let ax = prev.x - cx;
    let ay = prev.y - cy;
    let bx = next.x - cx;
    let by = next.y - cy;
    ax * by - ay * bx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn feed_point(x: f64, y: f64, z: f64) -> CutPoint {
        CutPoint {
            position: Vec3 { x, y, z },
            move_kind: MoveKind::Feed,
            tool_orientation: None,
            feed_rate_override: None,
        }
    }

    fn rapid_point(x: f64, y: f64, z: f64) -> CutPoint {
        CutPoint {
            position: Vec3 { x, y, z },
            move_kind: MoveKind::Rapid,
            tool_orientation: None,
            feed_rate_override: None,
        }
    }

    /// Create an Arc CutPoint.  Convention: position = arc start, end = arc end.
    fn arc_point(
        start_x: f64,
        start_y: f64,
        start_z: f64,
        end_x: f64,
        end_y: f64,
        end_z: f64,
        cx: f64,
        cy: f64,
        cz: f64,
        cw: bool,
    ) -> CutPoint {
        CutPoint {
            position: Vec3 {
                x: start_x,
                y: start_y,
                z: start_z,
            },
            move_kind: MoveKind::Arc {
                center: Vec3 {
                    x: cx,
                    y: cy,
                    z: cz,
                },
                end: Vec3 {
                    x: end_x,
                    y: end_y,
                    z: end_z,
                },
                clockwise: cw,
            },
            tool_orientation: None,
            feed_rate_override: None,
        }
    }

    /// Generate points on a circle (CCW by default).
    fn circle_points(
        cx: f64,
        cy: f64,
        r: f64,
        z: f64,
        n: usize,
        start_angle: f64,
        sweep: f64,
    ) -> Vec<CutPoint> {
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let t = start_angle + sweep * (i as f64) / ((n - 1) as f64);
            pts.push(feed_point(cx + r * t.cos(), cy + r * t.sin(), z));
        }
        pts
    }

    #[test]
    fn points_on_known_circle_produce_single_arc() {
        // CCW quarter circle: center (0,0), r=10, z=-5
        let mut pts = vec![rapid_point(10.0, 0.0, -5.0)]; // start (rapid)
        let arc_pts = circle_points(0.0, 0.0, 10.0, -5.0, 5, 0.0, PI / 2.0);
        // First point coincides with start, so skip it
        pts.extend(arc_pts.into_iter().skip(1));

        let result = fit_arcs(pts, 0.01);

        // Should be: rapid start + 1 arc point
        assert_eq!(result.len(), 2, "expected 2 points, got {}", result.len());
        assert!(matches!(result[0].move_kind, MoveKind::Rapid));
        match &result[1].move_kind {
            MoveKind::Arc {
                center, clockwise, ..
            } => {
                assert!(!clockwise, "expected CCW arc");
                assert!(
                    (center.x).abs() < 0.1,
                    "center.x should be ~0, got {}",
                    center.x
                );
                assert!(
                    (center.y).abs() < 0.1,
                    "center.y should be ~0, got {}",
                    center.y
                );
            }
            other => panic!("expected Arc, got {:?}", other),
        }
    }

    #[test]
    fn cw_circle_detected_correctly() {
        // CW quarter circle: center (0,0), r=10, z=0
        // CW = decreasing angle
        let mut pts = vec![rapid_point(10.0, 0.0, 0.0)];
        let arc_pts = circle_points(0.0, 0.0, 10.0, 0.0, 5, 0.0, -PI / 2.0);
        pts.extend(arc_pts.into_iter().skip(1));

        let result = fit_arcs(pts, 0.01);

        assert_eq!(result.len(), 2);
        match &result[1].move_kind {
            MoveKind::Arc { clockwise, .. } => {
                assert!(*clockwise, "expected CW arc");
            }
            other => panic!("expected Arc, got {:?}", other),
        }
    }

    #[test]
    fn mixed_straight_and_curved_only_curve_replaced() {
        // 3 straight Feed points + 5 curved points on a circle
        let mut pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            feed_point(1.0, 0.0, 0.0),
            feed_point(2.0, 0.0, 0.0),
            feed_point(3.0, 0.0, 0.0),
        ];
        // Now add arc points: center (3, 5), r=5, starting from (3, 0)
        let r = 5.0;
        let cx = 3.0;
        let cy = 5.0;
        let start_angle = -PI / 2.0; // (3, 0) relative to center (3, 5)
        let arc_pts = circle_points(cx, cy, r, 0.0, 6, start_angle, PI / 2.0);
        // arc_pts[0] should be close to (3, 0), which is already pts[3]
        pts.extend(arc_pts.into_iter().skip(1));

        let result = fit_arcs(pts, 0.05);

        // The straight section should remain as Feed moves
        // The curved section should become an Arc
        let feed_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Feed))
            .count();
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();

        assert!(feed_count >= 2, "straight feeds should be preserved");
        assert_eq!(arc_count, 1, "curved portion should become 1 arc");
    }

    #[test]
    fn tolerance_boundary_just_inside_merges() {
        // 8 points on a perfect circle, plus one point perturbed radially
        // by less than tolerance. All should merge into a single arc.
        let tol = 0.1;
        let r = 10.0;
        let n = 8;
        let mut pts = vec![rapid_point(r, 0.0, 0.0)];
        for i in 1..n {
            let t = PI / 2.0 * (i as f64) / ((n - 1) as f64);
            // Perturb only one point (the 4th), by less than tolerance
            let pr = if i == 4 { r + 0.05 } else { r };
            pts.push(feed_point(pr * t.cos(), pr * t.sin(), 0.0));
        }

        let result = fit_arcs(pts, tol);
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();
        assert_eq!(arc_count, 1, "points within tolerance should merge to arc");
    }

    #[test]
    fn tolerance_boundary_just_outside_breaks_arc() {
        // Circle r=10 with one point way outside tolerance
        let tol = 0.01;
        let r = 10.0;
        let mut pts = vec![rapid_point(r, 0.0, 0.0)];
        for i in 1..=4 {
            let t = PI / 2.0 * (i as f64) / 4.0;
            let pr = if i == 3 { r + 0.5 } else { r }; // point 3 is way off
            pts.push(feed_point(pr * t.cos(), pr * t.sin(), 0.0));
        }

        let result = fit_arcs(pts, tol);
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();
        assert_eq!(
            arc_count, 0,
            "out-of-tolerance point should prevent arc fitting"
        );
    }

    #[test]
    fn fewer_than_3_segments_no_replacement() {
        // Only 3 points = 2 segments (not enough)
        let pts = vec![
            rapid_point(10.0, 0.0, 0.0),
            feed_point(0.0, 10.0, 0.0),
            feed_point(-10.0, 0.0, 0.0),
        ];

        let result = fit_arcs(pts.clone(), 0.01);
        assert_eq!(result.len(), pts.len(), "should pass through unchanged");
        for p in &result {
            assert!(
                !matches!(p.move_kind, MoveKind::Arc { .. }),
                "should not create arcs with < 3 segments"
            );
        }
    }

    #[test]
    fn z_change_breaks_detection() {
        // Points on a circle but with a Z change in the middle
        let r = 10.0;
        let mut pts = vec![rapid_point(r, 0.0, 0.0)];
        for i in 1..=6 {
            let t = PI * (i as f64) / 6.0;
            let z = if i >= 4 { -1.0 } else { 0.0 }; // Z changes at point 4
            pts.push(feed_point(r * t.cos(), r * t.sin(), z));
        }

        let result = fit_arcs(pts, 0.01);
        // No arc should span the Z change
        for p in &result {
            if let MoveKind::Arc { center, end, .. } = &p.move_kind {
                assert!(
                    (center.z - end.z).abs() < 1e-9,
                    "arc should not cross Z levels"
                );
            }
        }
    }

    #[test]
    fn full_360_circle_detection() {
        // Full circle: 13 points (first = last to close)
        let r = 10.0;
        let n = 13;
        let mut pts = vec![rapid_point(r, 0.0, 0.0)];
        for i in 1..n {
            let t = 2.0 * PI * (i as f64) / ((n - 1) as f64);
            pts.push(feed_point(r * t.cos(), r * t.sin(), 0.0));
        }
        // Close the circle: last point = first arc point
        pts.push(feed_point(r, 0.0, 0.0));

        let result = fit_arcs(pts, 0.01);
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();
        assert!(arc_count >= 1, "full circle should produce at least 1 arc");
    }

    #[test]
    fn existing_arc_moves_pass_through() {
        let pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            // Arc: start (0,0,0), end (10,0,0), center (5,0,0), CW
            arc_point(0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 5.0, 0.0, 0.0, true),
            feed_point(20.0, 0.0, 0.0),
        ];

        let result = fit_arcs(pts.clone(), 0.01);
        assert_eq!(result.len(), 3);
        assert!(matches!(result[1].move_kind, MoveKind::Arc { .. }));
    }

    #[test]
    fn straight_collinear_points_no_false_positive() {
        // All points on a straight line
        let pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            feed_point(1.0, 0.0, 0.0),
            feed_point(2.0, 0.0, 0.0),
            feed_point(3.0, 0.0, 0.0),
            feed_point(4.0, 0.0, 0.0),
            feed_point(5.0, 0.0, 0.0),
        ];

        let result = fit_arcs(pts.clone(), 0.01);
        for p in &result {
            assert!(
                !matches!(p.move_kind, MoveKind::Arc { .. }),
                "collinear points should not produce an arc"
            );
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = fit_arcs(vec![], 0.01);
        assert!(result.is_empty());
    }

    #[test]
    fn single_point_returns_unchanged() {
        let pts = vec![feed_point(1.0, 2.0, 3.0)];
        let result = fit_arcs(pts.clone(), 0.01);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn all_rapid_moves_pass_through() {
        let pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            rapid_point(10.0, 0.0, 0.0),
            rapid_point(10.0, 10.0, 0.0),
        ];
        let result = fit_arcs(pts.clone(), 0.01);
        assert_eq!(result.len(), 3);
        for p in &result {
            assert!(matches!(p.move_kind, MoveKind::Rapid));
        }
    }

    #[test]
    fn dwell_moves_pass_through() {
        let pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            CutPoint {
                position: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                move_kind: MoveKind::Dwell { seconds: 1.0 },
                tool_orientation: None,
                feed_rate_override: None,
            },
            feed_point(10.0, 0.0, 0.0),
        ];
        let result = fit_arcs(pts.clone(), 0.01);
        assert_eq!(result.len(), 3);
        assert!(matches!(result[1].move_kind, MoveKind::Dwell { .. }));
    }

    #[test]
    fn arc_center_and_end_are_correct() {
        // Half circle CCW: center (0,0), r=5, from (5,0) to (-5,0)
        let r = 5.0;
        let n = 9; // 8 segments
        let mut pts = vec![rapid_point(r, 0.0, -2.0)];
        for i in 1..n {
            let t = PI * (i as f64) / ((n - 1) as f64);
            pts.push(feed_point(r * t.cos(), r * t.sin(), -2.0));
        }

        let result = fit_arcs(pts, 0.01);
        assert_eq!(result.len(), 2);

        // position = arc start (should be near the rapid start point (5, 0))
        let arc_pt = &result[1];
        assert!(
            (arc_pt.position.x - r).abs() < 0.1,
            "position.x (arc start) should be ~5, got {}",
            arc_pt.position.x
        );
        assert!(
            (arc_pt.position.y).abs() < 0.1,
            "position.y (arc start) should be ~0, got {}",
            arc_pt.position.y
        );

        match &arc_pt.move_kind {
            MoveKind::Arc {
                center,
                end,
                clockwise,
            } => {
                assert!(!clockwise, "should be CCW");
                assert!(
                    (center.x).abs() < 0.1 && (center.y).abs() < 0.1,
                    "center should be near origin, got ({}, {})",
                    center.x,
                    center.y
                );
                // End should be near (-5, 0)
                assert!(
                    (end.x - (-r)).abs() < 0.1,
                    "end.x should be ~-5, got {}",
                    end.x
                );
                assert!((end.y).abs() < 0.1, "end.y should be ~0, got {}", end.y);
                assert!(
                    (end.z - (-2.0)).abs() < 1e-9,
                    "end.z should be -2, got {}",
                    end.z
                );
            }
            other => panic!("expected Arc, got {:?}", other),
        }
    }

    #[test]
    fn all_feed_points_on_circle_no_leading_rapid() {
        // No rapid start — first point is also Feed
        let r = 10.0;
        let n = 6;
        let mut pts = Vec::new();
        for i in 0..n {
            let t = PI / 2.0 * (i as f64) / ((n - 1) as f64);
            pts.push(feed_point(r * t.cos(), r * t.sin(), 0.0));
        }

        let result = fit_arcs(pts, 0.01);
        // First point emitted as-is (Feed), rest become arc
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].move_kind, MoveKind::Feed));
        assert!(matches!(result[1].move_kind, MoveKind::Arc { .. }));
    }

    #[test]
    fn arc_after_dwell_interruption() {
        // Dwell interrupts a run of Feed moves; arc should still be detected
        // in the second run.
        let r = 10.0;
        let mut pts = vec![
            rapid_point(0.0, 0.0, 0.0),
            feed_point(1.0, 0.0, 0.0),
            CutPoint {
                position: Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                move_kind: MoveKind::Dwell { seconds: 0.5 },
                tool_orientation: None,
                feed_rate_override: None,
            },
        ];
        // After dwell, add arc-forming points. Dwell position (1,0) is the
        // arc start; the arc is centered at (1, r) with radius r.
        let cx = 1.0;
        let cy = r;
        for i in 1..=6 {
            let t = -PI / 2.0 + PI / 2.0 * (i as f64) / 6.0;
            pts.push(feed_point(cx + r * t.cos(), cy + r * t.sin(), 0.0));
        }

        let result = fit_arcs(pts, 0.05);
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();
        assert_eq!(arc_count, 1, "should detect arc after dwell interruption");
    }

    #[test]
    fn two_arcs_different_radii() {
        // Two consecutive arcs with different radii should produce 2 arc moves.
        let mut pts = vec![rapid_point(10.0, 0.0, 0.0)];
        // Arc 1: center (0,0), r=10, quarter circle CCW
        for i in 1..=5 {
            let t = PI / 2.0 * (i as f64) / 5.0;
            pts.push(feed_point(10.0 * t.cos(), 10.0 * t.sin(), 0.0));
        }
        // Arc 2: center (0,5), r=5, from (0,10) continuing CCW
        // (0,10) is 5 above center (0,5), so start angle = PI/2
        for i in 1..=5 {
            let t = PI / 2.0 + PI / 2.0 * (i as f64) / 5.0;
            pts.push(feed_point(5.0 * t.cos(), 5.0 + 5.0 * t.sin(), 0.0));
        }

        let result = fit_arcs(pts, 0.05);
        let arc_count = result
            .iter()
            .filter(|p| matches!(p.move_kind, MoveKind::Arc { .. }))
            .count();
        assert_eq!(
            arc_count, 2,
            "two arcs with different radii should produce 2 arc moves"
        );
    }
}
