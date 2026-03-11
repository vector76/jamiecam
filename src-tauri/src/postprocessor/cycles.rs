use crate::toolpath::types::{MoveKind, Pass, PassKind, DEFAULT_CLEARANCE_OFFSET};

/// Detect whether a `Pass` carries a drill cycle signature.
///
/// A `Cutting` pass is considered a drill pass when:
/// - `cuts` has an odd count ≥ 3 (1 + 2N for N ≥ 1)
/// - `cuts[0]` is `Rapid` (R-plane approach)
/// - Subsequent points alternate Feed (odd indices) and Rapid (even indices)
/// - All cut-points share the same XY position (within 1e-9 mm tolerance)
pub fn is_drill_cutting_pass(pass: &Pass) -> bool {
    if pass.kind != PassKind::Cutting {
        return false;
    }
    let cuts = &pass.cuts;
    let n = cuts.len();
    if n < 3 || n % 2 == 0 {
        return false;
    }
    if cuts[0].move_kind != MoveKind::Rapid {
        return false;
    }
    let x0 = cuts[0].position.x;
    let y0 = cuts[0].position.y;
    for (i, cut) in cuts.iter().enumerate().skip(1) {
        // Check XY uniformity
        if (cut.position.x - x0).abs() > 1e-9 || (cut.position.y - y0).abs() > 1e-9 {
            return false;
        }
        // Check alternating move kinds: odd indices are Feed, even indices are Rapid
        let expected_feed = i % 2 == 1;
        let is_feed = cut.move_kind == MoveKind::Feed;
        let is_rapid = cut.move_kind == MoveKind::Rapid;
        if expected_feed && !is_feed {
            return false;
        }
        if !expected_feed && !is_rapid {
            return false;
        }
    }
    true
}

/// Classification of a drill cycle.
#[derive(Debug, Clone, PartialEq)]
pub enum DrillCycleKind {
    /// Simple drill: 3 cut-points `[Rapid, Feed, Rapid]`.
    Simple,
    /// Peck drill: 1 + 2N cut-points (N ≥ 2); `increment` is depth per peck.
    Peck { increment: f64 },
}

/// Extracted parameters for a drill cycle pass.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillCycleParams {
    /// Whether this is a simple drill or a peck cycle.
    pub kind: DrillCycleKind,
    /// Z height of `cuts[0]` — the R-plane (clearance approach height).
    pub r_plane_z: f64,
    /// Z height of the deepest Feed point (`cuts[cuts.len() - 2]`).
    pub drill_depth_z: f64,
}

/// Classify a `Pass` as a drill cycle and extract its parameters.
///
/// Returns `None` if the pass does not satisfy `is_drill_cutting_pass`.
pub fn classify_drill_pass(pass: &Pass) -> Option<DrillCycleParams> {
    if !is_drill_cutting_pass(pass) {
        return None;
    }
    let cuts = &pass.cuts;
    let r_plane_z = cuts[0].position.z;
    let drill_depth_z = cuts[cuts.len() - 2].position.z;
    let kind = if cuts.len() == 3 {
        DrillCycleKind::Simple
    } else {
        // Peck increment: stock_top_z - first_peck_z
        // stock_top_z is reconstructed as (r_plane_z - DEFAULT_CLEARANCE_OFFSET)
        let stock_top_z = r_plane_z - DEFAULT_CLEARANCE_OFFSET;
        let first_peck_z = cuts[1].position.z;
        let increment = stock_top_z - first_peck_z;
        DrillCycleKind::Peck { increment }
    };
    Some(DrillCycleParams {
        kind,
        r_plane_z,
        drill_depth_z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;
    use crate::toolpath::types::{CutPoint, Pass, PassKind};

    fn make_cut(x: f64, y: f64, z: f64, move_kind: MoveKind) -> CutPoint {
        CutPoint {
            position: Vec3 { x, y, z },
            move_kind,
            tool_orientation: None,
        }
    }

    #[test]
    fn test_is_drill_pass_simple() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(10.0, 20.0, 15.0, MoveKind::Rapid),
                make_cut(10.0, 20.0, 2.0, MoveKind::Feed),
                make_cut(10.0, 20.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(is_drill_cutting_pass(&pass));
        let params = classify_drill_pass(&pass).unwrap();
        assert_eq!(params.kind, DrillCycleKind::Simple);
        assert_eq!(params.r_plane_z, 15.0);
        assert_eq!(params.drill_depth_z, 2.0);
    }

    #[test]
    fn test_is_drill_pass_peck() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 7.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 4.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 1.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(is_drill_cutting_pass(&pass));
        let params = classify_drill_pass(&pass).unwrap();
        assert_eq!(params.r_plane_z, 15.0);
        assert_eq!(params.drill_depth_z, 1.0);
        match params.kind {
            DrillCycleKind::Peck { increment } => {
                assert!(
                    (increment - 3.0).abs() < 1e-9,
                    "expected increment ~3.0, got {increment}"
                );
            }
            _ => panic!("expected Peck variant"),
        }
    }

    #[test]
    fn test_is_not_drill_pass_mixed_xy() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(1.0, 0.0, 2.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(!is_drill_cutting_pass(&pass));
    }

    #[test]
    fn test_is_not_drill_pass_wrong_count() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 7.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 4.0, MoveKind::Feed),
            ],
        };
        assert!(!is_drill_cutting_pass(&pass));
    }
}
