//! Revolution profile generation for cutting tools.
//!
//! Provides [`Tool::profile`] which returns the 2D outline that, when revolved
//! around the tool axis, produces the tool solid. Coordinates are (R, Z) with
//! Z = 0 at the tool tip and Z positive toward the spindle.

use super::tool::{Tool, ToolType};

impl Tool {
    /// Returns the tool's revolution profile as an ordered sequence of (radius, z)
    /// points, from tip (z=0) upward toward the shank.
    ///
    /// The profile is a polyline — straight segments between consecutive points.
    /// Curved regions (ball nose hemisphere, bull nose torus) are approximated
    /// with `segments_per_quarter` line segments per quarter-circle.
    ///
    /// A `segments_per_quarter` of 0 is clamped to 1.
    pub fn profile(&self, segments_per_quarter: u32) -> Vec<(f64, f64)> {
        let spq = segments_per_quarter.max(1);
        let r = self.diameter / 2.0;
        let shank_r = self.shank_diameter / 2.0;

        match self.tool_type {
            ToolType::FlatEndmill => self.profile_flat_endmill(r, shank_r),

            ToolType::Tap | ToolType::Reamer | ToolType::BoringBar | ToolType::ThreadMill => {
                // Cylindrical — same shape as straight FlatEndmill.
                profile_cylindrical(r, shank_r, self.cutting_length, self.overall_length)
            }

            ToolType::BallNose => self.profile_ball_nose(r, shank_r, spq),
            ToolType::BullNose => self.profile_bull_nose(r, shank_r, spq),
            ToolType::VBit => self.profile_vbit(r, shank_r),
            ToolType::Drill => self.profile_drill(r, shank_r),
            ToolType::CenterDrill => self.profile_center_drill(r, shank_r),
        }
    }

    /// Profile for FlatEndmill — handles both straight and tapered variants.
    fn profile_flat_endmill(&self, r: f64, shank_r: f64) -> Vec<(f64, f64)> {
        match self.taper_half_angle {
            Some(angle) => {
                let r_top = r + self.cutting_length * angle.to_radians().tan();
                let mut pts = Vec::with_capacity(4);
                pts.push((r, 0.0));
                pts.push((r_top, self.cutting_length));
                if (r_top - shank_r).abs() > f64::EPSILON {
                    pts.push((shank_r, self.cutting_length));
                }
                pts.push((shank_r, self.overall_length));
                pts
            }
            None => profile_cylindrical(r, shank_r, self.cutting_length, self.overall_length),
        }
    }

    /// Profile for BallNose — quarter-circle arc from tip to (R, R), then body + shank.
    fn profile_ball_nose(&self, r: f64, shank_r: f64, spq: u32) -> Vec<(f64, f64)> {
        let mut pts = Vec::with_capacity((spq as usize) + 4);
        // Quarter-circle arc from (0, 0) to (R, R)
        for i in 0..=spq {
            let angle = (i as f64) * std::f64::consts::FRAC_PI_2 / (spq as f64);
            pts.push((r * angle.sin(), r - r * angle.cos()));
        }
        // Straight body to cutting_length
        if self.cutting_length - r > f64::EPSILON {
            pts.push((r, self.cutting_length));
        }
        // Shank transition
        if (r - shank_r).abs() > f64::EPSILON {
            pts.push((shank_r, self.cutting_length));
        }
        pts.push((shank_r, self.overall_length));
        pts
    }

    /// Profile for BullNose — flat bottom, corner-radius arc, then body + shank.
    /// Handles both straight and tapered variants.
    fn profile_bull_nose(&self, r: f64, shank_r: f64, spq: u32) -> Vec<(f64, f64)> {
        let cr = self.corner_radius.unwrap_or(0.0);
        let mut pts = Vec::with_capacity((spq as usize) + 6);

        // Flat bottom from center to arc start (skip if corner fills entire radius)
        if r - cr > f64::EPSILON {
            pts.push((0.0, 0.0));
        }

        // Quarter-circle corner arc from (R-cr, 0) to (R, cr)
        for i in 0..=spq {
            let angle = (i as f64) * std::f64::consts::FRAC_PI_2 / (spq as f64);
            pts.push(((r - cr) + cr * angle.sin(), cr - cr * angle.cos()));
        }

        // Wall above arc — straight or tapered
        match self.taper_half_angle {
            Some(taper) => {
                let r_top = r + (self.cutting_length - cr) * taper.to_radians().tan();
                if self.cutting_length - cr > f64::EPSILON {
                    pts.push((r_top, self.cutting_length));
                }
                if (r_top - shank_r).abs() > f64::EPSILON {
                    pts.push((shank_r, self.cutting_length));
                }
            }
            None => {
                if self.cutting_length - cr > f64::EPSILON {
                    pts.push((r, self.cutting_length));
                }
                if (r - shank_r).abs() > f64::EPSILON {
                    pts.push((shank_r, self.cutting_length));
                }
            }
        }

        pts.push((shank_r, self.overall_length));
        pts
    }

    /// Profile for VBit — conical point, then transition to shank.
    fn profile_vbit(&self, r: f64, shank_r: f64) -> Vec<(f64, f64)> {
        let half_angle = self.included_angle.unwrap_or(90.0).to_radians() / 2.0;
        let cone_z = r / half_angle.tan();

        vec![
            (0.0, 0.0),
            (r, cone_z),
            (shank_r, self.cutting_length),
            (shank_r, self.overall_length),
        ]
    }

    /// Profile for Drill — conical point, cylindrical flute, then shank.
    fn profile_drill(&self, r: f64, shank_r: f64) -> Vec<(f64, f64)> {
        let half_angle = self.point_angle.unwrap_or(118.0).to_radians() / 2.0;
        let cone_z = r / half_angle.tan();

        let mut pts = Vec::with_capacity(5);
        pts.push((0.0, 0.0));
        pts.push((r, cone_z));
        // Cylindrical flute to cutting_length
        if self.cutting_length - cone_z > f64::EPSILON {
            pts.push((r, self.cutting_length));
        }
        // Shank transition
        if (r - shank_r).abs() > f64::EPSILON {
            pts.push((shank_r, self.cutting_length));
        }
        pts.push((shank_r, self.overall_length));
        pts
    }

    /// Profile for CenterDrill — pilot cone, pilot cylinder, step to body, then shank.
    fn profile_center_drill(&self, r: f64, shank_r: f64) -> Vec<(f64, f64)> {
        let pilot_r = self.pilot_diameter.unwrap_or(self.diameter * 0.3) / 2.0;
        let pilot_len = self.pilot_length.unwrap_or(self.cutting_length / 3.0);
        let half_angle = self.point_angle.unwrap_or(60.0).to_radians() / 2.0;
        let cone_z = pilot_r / half_angle.tan();

        let mut pts = Vec::with_capacity(7);
        pts.push((0.0, 0.0));
        pts.push((pilot_r, cone_z));
        // Cylindrical pilot to pilot_length
        if pilot_len - cone_z > f64::EPSILON {
            pts.push((pilot_r, pilot_len));
        }
        // Vertical step to body diameter
        if (r - pilot_r).abs() > f64::EPSILON {
            pts.push((r, pilot_len));
        }
        // Cylindrical body to cutting_length
        if self.cutting_length - pilot_len > f64::EPSILON {
            pts.push((r, self.cutting_length));
        }
        // Shank transition
        if (r - shank_r).abs() > f64::EPSILON {
            pts.push((shank_r, self.cutting_length));
        }
        pts.push((shank_r, self.overall_length));
        pts
    }
}

/// Build the cylindrical (straight, non-tapered) profile shared by several tool types.
///
/// Shape: `(R, 0)` → `(R, cutting_length)` → `(shank_R, cutting_length)` → `(shank_R, overall_length)`
///
/// When `shank_r == r`, the zero-length radial transition is omitted.
fn profile_cylindrical(
    r: f64,
    shank_r: f64,
    cutting_length: f64,
    overall_length: f64,
) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(4);
    pts.push((r, 0.0));
    pts.push((r, cutting_length));
    if (r - shank_r).abs() > f64::EPSILON {
        pts.push((shank_r, cutting_length));
    }
    pts.push((shank_r, overall_length));
    pts
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Helper: build a resolved tool of the given type.
    fn make_resolved(tool_type: ToolType, diameter: f64) -> Tool {
        let mut tool = Tool {
            id: Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap(),
            name: "test".to_string(),
            tool_type,
            material: "carbide".to_string(),
            diameter,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 0.0,
            shank_diameter: 0.0,
            overall_length: 0.0,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        };
        tool.resolve_defaults();
        tool
    }

    fn assert_approx(actual: f64, expected: f64, tol: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {}, got {} (diff {})",
            msg,
            expected,
            actual,
            (actual - expected).abs()
        );
    }

    // ---- FlatEndmill straight ----

    #[test]
    fn flat_endmill_straight_profile() {
        let tool = make_resolved(ToolType::FlatEndmill, 10.0);
        // diameter=10, cutting_length=30, shank_diameter=10, overall_length=90
        let pts = tool.profile(1);

        // shank_diameter == diameter → no transition step
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0], (5.0, 0.0)); // tip
        assert_eq!(pts[1], (5.0, 30.0)); // top of cutting
        assert_eq!(pts[2], (5.0, 90.0)); // top of shank
    }

    #[test]
    fn flat_endmill_straight_different_shank() {
        let mut tool = make_resolved(ToolType::FlatEndmill, 10.0);
        tool.shank_diameter = 12.0;
        let pts = tool.profile(1);

        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (5.0, 0.0));
        assert_eq!(pts[1], (5.0, 30.0));
        assert_eq!(pts[2], (6.0, 30.0)); // shank_R = 6
        assert_eq!(pts[3], (6.0, 90.0));
    }

    // ---- FlatEndmill tapered ----

    #[test]
    fn flat_endmill_tapered_profile() {
        let mut tool = make_resolved(ToolType::FlatEndmill, 10.0);
        tool.taper_half_angle = Some(5.0); // 5 degrees
        tool.shank_diameter = 12.0;
        let pts = tool.profile(1);

        let r = 5.0;
        let r_top = r + 30.0 * 5.0_f64.to_radians().tan();

        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (r, 0.0));
        assert_approx(pts[1].0, r_top, 1e-10, "tapered R_top");
        assert_eq!(pts[1].1, 30.0);
        assert_eq!(pts[2], (6.0, 30.0)); // shank transition
        assert_eq!(pts[3], (6.0, 90.0));
    }

    #[test]
    fn flat_endmill_tapered_same_shank() {
        // When R_top ~= shank_R, the transition is skipped.
        let mut tool = make_resolved(ToolType::FlatEndmill, 10.0);
        tool.taper_half_angle = Some(5.0);
        // Set shank to match R_top exactly.
        let r_top = 5.0 + 30.0 * 5.0_f64.to_radians().tan();
        tool.shank_diameter = r_top * 2.0;
        let pts = tool.profile(1);

        // Should skip the transition: 3 points total
        assert_eq!(pts.len(), 3);
        assert_approx(pts[1].0, r_top, 1e-10, "R_top");
        assert_approx(pts[2].0, r_top, 1e-10, "shank matches R_top");
    }

    // ---- Tap (cylindrical) ----

    #[test]
    fn tap_profile_matches_straight_flat_endmill() {
        let flat = make_resolved(ToolType::FlatEndmill, 10.0);
        let tap = make_resolved(ToolType::Tap, 10.0);

        let flat_pts = flat.profile(1);
        let tap_pts = tap.profile(1);
        assert_eq!(flat_pts, tap_pts);
    }

    // ---- Profile invariants ----

    #[test]
    fn profile_invariants_simple_types() {
        let types = [
            ToolType::FlatEndmill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        for tt in &types {
            let tool = make_resolved(tt.clone(), 10.0);
            let pts = tool.profile(1);

            // First point z == 0
            assert_eq!(pts[0].1, 0.0, "{:?}: first z == 0", tt);

            // Last point z == overall_length
            assert_eq!(
                pts.last().unwrap().1,
                tool.overall_length,
                "{:?}: last z == overall_length",
                tt
            );

            // All R >= 0
            for (i, &(r, _)) in pts.iter().enumerate() {
                assert!(r >= 0.0, "{:?}: R >= 0 at point {}", tt, i);
            }

            // Z monotonically non-decreasing
            for w in pts.windows(2) {
                assert!(
                    w[1].1 >= w[0].1,
                    "{:?}: Z non-decreasing: {} -> {}",
                    tt,
                    w[0].1,
                    w[1].1
                );
            }
        }
    }

    #[test]
    fn profile_invariants_tapered_flat_endmill() {
        let mut tool = make_resolved(ToolType::FlatEndmill, 10.0);
        tool.taper_half_angle = Some(3.0);
        tool.shank_diameter = 14.0;
        let pts = tool.profile(1);

        assert_eq!(pts[0].1, 0.0);
        assert_eq!(pts.last().unwrap().1, tool.overall_length);
        for (i, &(r, _)) in pts.iter().enumerate() {
            assert!(r >= 0.0, "R >= 0 at point {}", i);
        }
        for w in pts.windows(2) {
            assert!(w[1].1 >= w[0].1);
        }
    }

    #[test]
    fn segments_per_quarter_zero_clamped() {
        let tool = make_resolved(ToolType::FlatEndmill, 10.0);
        // Should not panic — 0 is clamped to 1.
        let pts = tool.profile(0);
        assert!(!pts.is_empty());
    }

    // ---- BallNose ----

    #[test]
    fn ball_nose_profile_spq1() {
        let tool = make_resolved(ToolType::BallNose, 10.0);
        // R=5, cutting_length=30, shank_diameter=10 (shank_R=5), overall_length=90
        let pts = tool.profile(1);

        // spq=1: arc has 2 points (start + end), then body, then shank end
        assert_eq!(pts.len(), 4);
        assert_approx(pts[0].0, 0.0, 1e-10, "arc start R");
        assert_approx(pts[0].1, 0.0, 1e-10, "arc start Z");
        assert_approx(pts[1].0, 5.0, 1e-10, "arc end R");
        assert_approx(pts[1].1, 5.0, 1e-10, "arc end Z");
        assert_eq!(pts[2], (5.0, 30.0)); // body
        assert_eq!(pts[3], (5.0, 90.0)); // shank end
    }

    #[test]
    fn ball_nose_profile_spq4() {
        let tool = make_resolved(ToolType::BallNose, 10.0);
        let pts = tool.profile(4);

        // 5 arc points (i=0..=4) + body + shank end = 7
        assert_eq!(pts.len(), 7);

        // First arc point at (0, 0)
        assert_approx(pts[0].0, 0.0, 1e-10, "arc[0] R");
        assert_approx(pts[0].1, 0.0, 1e-10, "arc[0] Z");

        // Last arc point at (R, R)
        assert_approx(pts[4].0, 5.0, 1e-10, "arc[4] R");
        assert_approx(pts[4].1, 5.0, 1e-10, "arc[4] Z");

        // All arc points lie on the quarter-circle of radius R=5
        let r = 5.0;
        for i in 0..=4 {
            let (pr, pz) = pts[i];
            let dist = (pr * pr + (pz - r) * (pz - r)).sqrt();
            assert_approx(dist, r, 1e-10, &format!("arc[{}] on circle", i));
        }

        // Arc R values are monotonically increasing
        for i in 0..4 {
            assert!(
                pts[i + 1].0 >= pts[i].0,
                "arc R non-decreasing: {} -> {}",
                pts[i].0,
                pts[i + 1].0
            );
        }
    }

    // ---- BullNose straight ----

    #[test]
    fn bull_nose_straight_profile() {
        let tool = make_resolved(ToolType::BullNose, 10.0);
        // R=5, cr=1 (10*0.1), cutting_length=30, shank_R=5, overall_length=90
        let pts = tool.profile(1);

        // (0,0) flat bottom, arc start (4,0), arc end (5,1), body (5,30), shank (5,90)
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0], (0.0, 0.0)); // flat bottom center
        assert_approx(pts[1].0, 4.0, 1e-10, "arc start R");
        assert_approx(pts[1].1, 0.0, 1e-10, "arc start Z");
        assert_approx(pts[2].0, 5.0, 1e-10, "arc end R");
        assert_approx(pts[2].1, 1.0, 1e-10, "arc end Z");
        assert_eq!(pts[3], (5.0, 30.0)); // straight wall top
        assert_eq!(pts[4], (5.0, 90.0)); // shank end
    }

    // ---- BullNose tapered ----

    #[test]
    fn bull_nose_tapered_profile() {
        let mut tool = make_resolved(ToolType::BullNose, 10.0);
        tool.taper_half_angle = Some(5.0); // 5 degrees
        tool.shank_diameter = 16.0; // shank_R = 8
        let pts = tool.profile(1);

        let r = 5.0;
        let cr = 1.0;
        let r_top = r + (30.0 - cr) * 5.0_f64.to_radians().tan();

        // (0,0), arc start (4,0), arc end (5,1), (r_top,30), (8,30), (8,90)
        assert_eq!(pts.len(), 6);
        assert_eq!(pts[0], (0.0, 0.0));
        // Arc endpoints are NOT tapered
        assert_approx(pts[1].0, 4.0, 1e-10, "arc start R (not tapered)");
        assert_approx(pts[2].0, 5.0, 1e-10, "arc end R (not tapered)");
        assert_approx(pts[2].1, 1.0, 1e-10, "arc end Z");
        // Tapered wall
        assert_approx(pts[3].0, r_top, 1e-10, "tapered R_top");
        assert_eq!(pts[3].1, 30.0);
        // Shank transition
        assert_eq!(pts[4], (8.0, 30.0));
        assert_eq!(pts[5], (8.0, 90.0));
    }

    // ---- VBit ----

    #[test]
    fn vbit_profile() {
        let tool = make_resolved(ToolType::VBit, 10.0);
        // R=5, included_angle=90°, half_angle=45°, cone_z=5/tan(45°)=5
        // shank_R=5, cutting_length=30, overall_length=90
        let pts = tool.profile(1);

        let half_angle = 45.0_f64.to_radians();
        let cone_z = 5.0 / half_angle.tan();

        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (0.0, 0.0));
        assert_approx(pts[1].0, 5.0, 1e-10, "cone base R");
        assert_approx(pts[1].1, cone_z, 1e-10, "cone base Z");
        assert_eq!(pts[2], (5.0, 30.0)); // shank_R == R
        assert_eq!(pts[3], (5.0, 90.0));
    }

    // ---- Drill ----

    #[test]
    fn drill_profile() {
        let tool = make_resolved(ToolType::Drill, 10.0);
        // R=5, point_angle=118°, half_angle=59°, shank_R=5
        let pts = tool.profile(1);

        let half_angle = 59.0_f64.to_radians();
        let cone_z = 5.0 / half_angle.tan();

        // (0,0), cone end, cylindrical flute to cutting_length, shank end
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], (0.0, 0.0));
        assert_approx(pts[1].0, 5.0, 1e-10, "cone R");
        assert_approx(pts[1].1, cone_z, 1e-10, "cone Z");
        assert_eq!(pts[2], (5.0, 30.0)); // flute top (R == shank_R, no transition)
        assert_eq!(pts[3], (5.0, 90.0));
    }

    #[test]
    fn drill_profile_different_shank() {
        let mut tool = make_resolved(ToolType::Drill, 10.0);
        tool.shank_diameter = 12.0; // shank_R = 6
        let pts = tool.profile(1);

        let half_angle = 59.0_f64.to_radians();
        let cone_z = 5.0 / half_angle.tan();

        // (0,0), cone, flute, shank transition, shank end
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0], (0.0, 0.0));
        assert_approx(pts[1].1, cone_z, 1e-10, "cone Z");
        assert_eq!(pts[2], (5.0, 30.0)); // flute top
        assert_eq!(pts[3], (6.0, 30.0)); // shank transition
        assert_eq!(pts[4], (6.0, 90.0));
    }

    // ---- CenterDrill ----

    #[test]
    fn center_drill_profile() {
        let tool = make_resolved(ToolType::CenterDrill, 10.0);
        // R=5, point_angle=60°, pilot_diameter=3 (pilot_R=1.5),
        // pilot_length=10, shank_R=5, cutting_length=30, overall_length=90
        let pts = tool.profile(1);

        let pilot_r = 1.5;
        let half_angle = 30.0_f64.to_radians();
        let cone_z = pilot_r / half_angle.tan();

        // (0,0), pilot cone, pilot cyl, step to R, body, shank end
        assert_eq!(pts.len(), 6);
        assert_eq!(pts[0], (0.0, 0.0)); // tip
        assert_approx(pts[1].0, pilot_r, 1e-10, "pilot cone R");
        assert_approx(pts[1].1, cone_z, 1e-10, "pilot cone Z");
        assert_approx(pts[2].0, pilot_r, 1e-10, "pilot cyl R");
        assert_eq!(pts[2].1, 10.0); // pilot_length
        assert_eq!(pts[3], (5.0, 10.0)); // vertical step to body
        assert_eq!(pts[4], (5.0, 30.0)); // body top
        assert_eq!(pts[5], (5.0, 90.0)); // shank end (R == shank_R)
    }

    // ---- Profile invariants across ALL tool types ----

    #[test]
    fn profile_invariants_all_types() {
        let types = [
            ToolType::FlatEndmill,
            ToolType::BallNose,
            ToolType::BullNose,
            ToolType::VBit,
            ToolType::Drill,
            ToolType::CenterDrill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        for tt in &types {
            let tool = make_resolved(tt.clone(), 10.0);
            let pts = tool.profile(4);

            // First point z == 0
            assert_eq!(pts[0].1, 0.0, "{:?}: first z == 0", tt);

            // Last point z == overall_length
            assert_eq!(
                pts.last().unwrap().1,
                tool.overall_length,
                "{:?}: last z == overall_length",
                tt
            );

            // All R >= 0
            for (i, &(r, _)) in pts.iter().enumerate() {
                assert!(r >= 0.0, "{:?}: R >= 0 at point {}", tt, i);
            }

            // Z monotonically non-decreasing
            for w in pts.windows(2) {
                assert!(
                    w[1].1 >= w[0].1,
                    "{:?}: Z non-decreasing: {} -> {}",
                    tt,
                    w[0].1,
                    w[1].1
                );
            }
        }
    }
}
