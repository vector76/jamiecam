//! Arc center computation for G2/G3 arc interpolation.

use crate::models::Vec3;

use super::types::{MotionSegment, ParseWarning, Plane, SegmentMetadata};

/// Compute arc center from IJK offsets relative to start position.
///
/// Missing I/J/K values default to 0.0. Only axes relevant to the active plane
/// are used; the linear axis is set to the start position's value.
pub(crate) fn resolve_ijk_center(
    start: &Vec3,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
    plane: &Plane,
) -> Vec3 {
    let i = i.unwrap_or(0.0);
    let j = j.unwrap_or(0.0);
    let k = k.unwrap_or(0.0);
    match plane {
        Plane::Xy => Vec3 {
            x: start.x + i,
            y: start.y + j,
            z: start.z,
        },
        Plane::Xz => Vec3 {
            x: start.x + i,
            y: start.y,
            z: start.z + k,
        },
        Plane::Yz => Vec3 {
            x: start.x,
            y: start.y + j,
            z: start.z + k,
        },
    }
}

/// Compute arc center from R (radius) value.
///
/// Positive R selects the minor arc (sweep ≤ 180°); negative R selects the
/// major arc (sweep > 180°, using |R|). Returns `None` if start == end (full
/// circle with R format is undefined) or if the radius is too small to span
/// the chord.
pub(crate) fn resolve_r_center(
    start: &Vec3,
    end: &Vec3,
    r: f64,
    clockwise: bool,
    plane: &Plane,
) -> Option<Vec3> {
    let (s0, s1) = plane_2d(start, plane);
    let (e0, e1) = plane_2d(end, plane);

    let dx = e0 - s0;
    let dy = e1 - s1;
    let d_sq = dx * dx + dy * dy;

    // Full circle: start == end in the arc plane.
    if d_sq < 1e-20 {
        return None;
    }

    let d = d_sq.sqrt();
    let abs_r = r.abs();

    let h_sq = abs_r * abs_r - d_sq / 4.0;
    if h_sq < 0.0 {
        return None; // radius too small to span chord
    }

    let h = h_sq.sqrt();
    let mid0 = (s0 + e0) / 2.0;
    let mid1 = (s1 + e1) / 2.0;

    // Right perpendicular of chord direction.
    let perp0 = dy / d;
    let perp1 = -dx / d;

    // CW + positive R → right perp (minor arc).
    // CCW + positive R → left perp (minor arc).
    // Flip for negative R (major arc).
    let sign = if clockwise ^ (r < 0.0) { 1.0 } else { -1.0 };

    let c0 = mid0 + sign * h * perp0;
    let c1 = mid1 + sign * h * perp1;

    Some(from_plane_2d(c0, c1, start, plane))
}

/// Validate that arc radii from center to start and center to end match.
///
/// Returns a warning if the difference exceeds 0.01mm. The `line` field of the
/// returned warning is set to 0; the caller should set it to the correct value.
pub(crate) fn validate_arc_radii(
    center: &Vec3,
    start: &Vec3,
    end: &Vec3,
    plane: &Plane,
) -> Option<ParseWarning> {
    let (c0, c1) = plane_2d(center, plane);
    let (s0, s1) = plane_2d(start, plane);
    let (e0, e1) = plane_2d(end, plane);

    let r_start = ((s0 - c0).powi(2) + (s1 - c1).powi(2)).sqrt();
    let r_end = ((e0 - c0).powi(2) + (e1 - c1).powi(2)).sqrt();

    let diff = (r_start - r_end).abs();
    if diff > 0.01 {
        Some(ParseWarning {
            line: 0,
            message: format!(
                "arc radii mismatch: start radius {:.4} vs end radius {:.4} (diff {:.4}mm)",
                r_start, r_end, diff
            ),
        })
    } else {
        None
    }
}

/// Resolve a G2/G3 arc: compute center, validate, and build the Arc segment.
///
/// IJK takes precedence over R when both are present (settled decision #12).
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_arc(
    start: &Vec3,
    end: &Vec3,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
    r: Option<f64>,
    clockwise: bool,
    plane: &Plane,
    feed_rate: f64,
    metadata: SegmentMetadata,
) -> (Option<MotionSegment>, Vec<ParseWarning>) {
    let line = metadata.source_line;
    let mut warnings = Vec::new();

    let has_ijk = i.is_some() || j.is_some() || k.is_some();

    let center = if has_ijk {
        // IJK takes precedence over R.
        resolve_ijk_center(start, i, j, k, plane)
    } else if let Some(r_val) = r {
        match resolve_r_center(start, end, r_val, clockwise, plane) {
            Some(c) => c,
            None => {
                let msg = if plane_2d_eq(start, end, plane) {
                    "full circle with R format not supported; use IJK"
                } else {
                    "arc radius too small to reach endpoint"
                };
                warnings.push(ParseWarning {
                    line,
                    message: msg.to_string(),
                });
                return (None, warnings);
            }
        }
    } else {
        warnings.push(ParseWarning {
            line,
            message: "arc with no center specified (missing I/J/K or R)".to_string(),
        });
        return (None, warnings);
    };

    // Validate radius match for IJK format.
    if has_ijk {
        if let Some(mut w) = validate_arc_radii(&center, start, end, plane) {
            w.line = line;
            warnings.push(w);
        }
    }

    let segment = MotionSegment::Arc {
        start: start.clone(),
        end: end.clone(),
        center,
        clockwise,
        plane: plane.clone(),
        feed_rate,
        metadata,
    };

    (Some(segment), warnings)
}

// --- Private helpers ---

/// Extract 2D coordinates from a Vec3 in the arc plane.
///
/// Axis ordering preserves CW/CCW semantics across all planes:
/// - G17 (XY): (x, y)
/// - G18 (XZ): (z, x) — ZX plane order
/// - G19 (YZ): (y, z)
fn plane_2d(v: &Vec3, plane: &Plane) -> (f64, f64) {
    match plane {
        Plane::Xy => (v.x, v.y),
        Plane::Xz => (v.z, v.x),
        Plane::Yz => (v.y, v.z),
    }
}

/// Convert 2D plane coordinates back to Vec3, using a reference point for the
/// linear axis value.
fn from_plane_2d(a0: f64, a1: f64, reference: &Vec3, plane: &Plane) -> Vec3 {
    match plane {
        Plane::Xy => Vec3 {
            x: a0,
            y: a1,
            z: reference.z,
        },
        Plane::Xz => Vec3 {
            x: a1,
            y: reference.y,
            z: a0,
        },
        Plane::Yz => Vec3 {
            x: reference.x,
            y: a0,
            z: a1,
        },
    }
}

/// Check if two points are equal in the arc plane (within floating-point tolerance).
fn plane_2d_eq(a: &Vec3, b: &Vec3, plane: &Plane) -> bool {
    let (a0, a1) = plane_2d(a, plane);
    let (b0, b1) = plane_2d(b, plane);
    (a0 - b0).abs() < 1e-10 && (a1 - b1).abs() < 1e-10
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode_parser::{FeedMode, SpindleDir};

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    fn test_metadata(line: usize) -> SegmentMetadata {
        SegmentMetadata {
            source_line: line,
            tool_number: 1,
            spindle_speed: 10000.0,
            spindle_dir: SpindleDir::Cw,
            feed_mode: FeedMode::PerMinute,
        }
    }

    // --- resolve_ijk_center ---

    #[test]
    fn ijk_center_g17() {
        let start = v(10.0, 20.0, 5.0);
        let center = resolve_ijk_center(&start, Some(5.0), Some(-10.0), None, &Plane::Xy);
        assert_eq!(center, v(15.0, 10.0, 5.0));
    }

    #[test]
    fn ijk_center_g18() {
        let start = v(10.0, 20.0, 30.0);
        let center = resolve_ijk_center(&start, Some(5.0), None, Some(-10.0), &Plane::Xz);
        assert_eq!(center, v(15.0, 20.0, 20.0));
    }

    #[test]
    fn ijk_center_g19() {
        let start = v(10.0, 20.0, 30.0);
        let center = resolve_ijk_center(&start, None, Some(5.0), Some(-10.0), &Plane::Yz);
        assert_eq!(center, v(10.0, 25.0, 20.0));
    }

    #[test]
    fn ijk_missing_defaults_to_zero() {
        let start = v(10.0, 20.0, 5.0);
        let center = resolve_ijk_center(&start, Some(5.0), None, None, &Plane::Xy);
        assert_eq!(center, v(15.0, 20.0, 5.0));
    }

    // --- resolve_r_center ---

    #[test]
    fn r_center_minor_arc_cw() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let center = resolve_r_center(&start, &end, 10.0, true, &Plane::Xy).unwrap();
        assert!((center.x - 10.0).abs() < 1e-9);
        assert!((center.y - 10.0).abs() < 1e-9);
        assert_eq!(center.z, 0.0);
    }

    #[test]
    fn r_center_minor_arc_ccw() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let center = resolve_r_center(&start, &end, 10.0, false, &Plane::Xy).unwrap();
        assert!((center.x).abs() < 1e-9);
        assert!((center.y).abs() < 1e-9);
        assert_eq!(center.z, 0.0);
    }

    #[test]
    fn r_center_major_arc_cw() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let center = resolve_r_center(&start, &end, -10.0, true, &Plane::Xy).unwrap();
        assert!((center.x).abs() < 1e-9);
        assert!((center.y).abs() < 1e-9);
    }

    #[test]
    fn r_center_major_arc_ccw() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let center = resolve_r_center(&start, &end, -10.0, false, &Plane::Xy).unwrap();
        assert!((center.x - 10.0).abs() < 1e-9);
        assert!((center.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn r_center_full_circle_returns_none() {
        let p = v(10.0, 0.0, 0.0);
        assert!(resolve_r_center(&p, &p, 10.0, true, &Plane::Xy).is_none());
    }

    #[test]
    fn r_center_radius_too_small_returns_none() {
        let start = v(0.0, 0.0, 0.0);
        let end = v(20.0, 0.0, 0.0);
        assert!(resolve_r_center(&start, &end, 5.0, true, &Plane::Xy).is_none());
    }

    // --- Full circle with IJK ---

    #[test]
    fn full_circle_ijk_succeeds() {
        let start = v(10.0, 0.0, 0.0);
        let center = resolve_ijk_center(&start, Some(-10.0), Some(0.0), None, &Plane::Xy);
        assert_eq!(center, v(0.0, 0.0, 0.0));
        // start == end for full circle; both radii match
        let warning = validate_arc_radii(&center, &start, &start, &Plane::Xy);
        assert!(warning.is_none());
    }

    // --- validate_arc_radii ---

    #[test]
    fn validate_matching_radii() {
        let center = v(0.0, 0.0, 0.0);
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        assert!(validate_arc_radii(&center, &start, &end, &Plane::Xy).is_none());
    }

    #[test]
    fn validate_mismatched_radii() {
        let center = v(0.0, 0.0, 0.0);
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.02, 0.0);
        let w = validate_arc_radii(&center, &start, &end, &Plane::Xy);
        assert!(w.is_some());
        assert!(w.unwrap().message.contains("radii mismatch"));
    }

    #[test]
    fn validate_within_tolerance() {
        let center = v(0.0, 0.0, 0.0);
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.005, 0.0);
        assert!(validate_arc_radii(&center, &start, &end, &Plane::Xy).is_none());
    }

    // --- resolve_arc ---

    #[test]
    fn resolve_arc_ijk() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let (seg, warnings) = resolve_arc(
            &start,
            &end,
            Some(-10.0),
            Some(0.0),
            None,
            None,
            true,
            &Plane::Xy,
            300.0,
            test_metadata(1),
        );
        assert!(seg.is_some());
        match seg.unwrap() {
            MotionSegment::Arc {
                center, clockwise, ..
            } => {
                assert_eq!(center, v(0.0, 0.0, 0.0));
                assert!(clockwise);
            }
            other => panic!("expected Arc, got {:?}", other),
        }
        assert!(warnings.is_empty());
    }

    #[test]
    fn resolve_arc_r_format() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let (seg, warnings) = resolve_arc(
            &start,
            &end,
            None,
            None,
            None,
            Some(10.0),
            false,
            &Plane::Xy,
            300.0,
            test_metadata(1),
        );
        assert!(seg.is_some());
        match seg.unwrap() {
            MotionSegment::Arc {
                center, clockwise, ..
            } => {
                assert!((center.x).abs() < 1e-9);
                assert!((center.y).abs() < 1e-9);
                assert!(!clockwise);
            }
            other => panic!("expected Arc, got {:?}", other),
        }
        assert!(warnings.is_empty());
    }

    #[test]
    fn resolve_arc_no_center_warning() {
        let start = v(0.0, 0.0, 0.0);
        let end = v(10.0, 0.0, 0.0);
        let (seg, warnings) = resolve_arc(
            &start,
            &end,
            None,
            None,
            None,
            None,
            true,
            &Plane::Xy,
            300.0,
            test_metadata(1),
        );
        assert!(seg.is_none());
        assert!(warnings
            .iter()
            .any(|w| w.message.contains("no center specified")));
    }

    #[test]
    fn resolve_arc_ijk_precedence_over_r() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 0.0);
        let (seg, warnings) = resolve_arc(
            &start,
            &end,
            Some(-10.0),
            Some(0.0),
            None,
            Some(999.0),
            true,
            &Plane::Xy,
            300.0,
            test_metadata(1),
        );
        assert!(seg.is_some());
        match seg.unwrap() {
            MotionSegment::Arc { center, .. } => {
                assert_eq!(center, v(0.0, 0.0, 0.0));
            }
            other => panic!("expected Arc, got {:?}", other),
        }
        assert!(warnings.is_empty());
    }

    // --- Helical arc ---

    #[test]
    fn helical_arc_preserves_3d_positions() {
        let start = v(10.0, 0.0, 0.0);
        let end = v(0.0, 10.0, 5.0);
        let (seg, _) = resolve_arc(
            &start,
            &end,
            Some(-10.0),
            Some(0.0),
            None,
            None,
            true,
            &Plane::Xy,
            300.0,
            test_metadata(1),
        );
        match seg.unwrap() {
            MotionSegment::Arc {
                start: s,
                end: e,
                center,
                ..
            } => {
                assert_eq!(s, v(10.0, 0.0, 0.0));
                assert_eq!(e, v(0.0, 10.0, 5.0));
                // Center linear axis (Z for G17) = start's Z
                assert_eq!(center.z, 0.0);
                assert_eq!(center.x, 0.0);
                assert_eq!(center.y, 0.0);
            }
            other => panic!("expected Arc, got {:?}", other),
        }
    }
}
