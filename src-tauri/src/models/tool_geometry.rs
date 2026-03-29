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
        let _spq = segments_per_quarter.max(1);
        let r = self.diameter / 2.0;
        let shank_r = self.shank_diameter / 2.0;

        match self.tool_type {
            ToolType::FlatEndmill => self.profile_flat_endmill(r, shank_r),

            ToolType::Tap | ToolType::Reamer | ToolType::BoringBar | ToolType::ThreadMill => {
                // Cylindrical — same shape as straight FlatEndmill.
                profile_cylindrical(r, shank_r, self.cutting_length, self.overall_length)
            }

            ToolType::BallNose => {
                todo!("BallNose profile")
            }
            ToolType::BullNose => {
                todo!("BullNose profile")
            }
            ToolType::VBit => {
                todo!("VBit profile")
            }
            ToolType::Drill => {
                todo!("Drill profile")
            }
            ToolType::CenterDrill => {
                todo!("CenterDrill profile")
            }
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
}
