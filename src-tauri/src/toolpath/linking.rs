//! Linking pass generation for toolpaths.
//!
//! Inserts [`PassKind::Linking`], [`PassKind::LeadIn`], and [`PassKind::LeadOut`] passes
//! around each cutting pass to produce safe tool transitions between depth levels.

use crate::models::Vec3;
use crate::toolpath::types::{CutPoint, LinkingParams, MoveKind, Pass, PassKind};

/// Default lead-in/lead-out ratio (as a fraction of tool diameter).
pub const DEFAULT_LEAD_RATIO: f64 = 0.4;

// ---------------------------------------------------------------------------
// Helical entry helpers
// ---------------------------------------------------------------------------

/// Maximum allowable chordal error for helix/arc approximation (mm).
const CHORDAL_ERROR: f64 = 0.01;

/// Chord length that achieves ≤ [`CHORDAL_ERROR`] for a given `radius`.
fn chord_len_for_radius(radius: f64) -> f64 {
    let e = CHORDAL_ERROR;
    2.0 * (2.0 * radius * e - e * e).sqrt()
}

/// Number of straight segments per full revolution for a given `radius`.
///
/// Returns 0 when `radius` is so small that the chordal-error formula would
/// require negative radicand (radius < CHORDAL_ERROR/2).
fn segments_per_revolution(radius: f64) -> usize {
    let e = CHORDAL_ERROR;
    if radius < e / 2.0 {
        return 0;
    }
    let cl = chord_len_for_radius(radius);
    let circ = 2.0 * std::f64::consts::PI * radius;
    (circ / cl).ceil() as usize
}

/// Produces a helical descent from `start_z` to `end_z` (end_z < start_z).
///
/// Each full revolution descends by `pitch`.  The helix begins at angle 0
/// (i.e. `center.0 + radius`, `center.1`) and winds counter-clockwise.
/// All moves are [`MoveKind::Feed`].
fn helical_descent_moves(
    center: (f64, f64),
    radius: f64,
    pitch: f64,
    start_z: f64,
    end_z: f64,
) -> Vec<CutPoint> {
    let total_depth = start_z - end_z;
    let spr_usize = segments_per_revolution(radius);
    if total_depth <= 0.0 || pitch <= 0.0 || spr_usize == 0 {
        return vec![];
    }
    let spr = spr_usize as f64;
    let total_revs = total_depth / pitch;
    let total_segs = (total_revs * spr).ceil().max(1.0) as usize;
    (1..=total_segs)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * (i as f64 / spr);
            let z = start_z - total_depth * (i as f64 / total_segs as f64);
            feed_point(Vec3 {
                x: center.0 + radius * angle.cos(),
                y: center.1 + radius * angle.sin(),
                z,
            })
        })
        .collect()
}

/// Full circle at `z` at `radius` from `center`, chord-approximated with
/// ≤ 0.01 mm chordal error.  Begins at `start_angle` and returns to it after
/// one full revolution — ensuring continuity with whatever motion preceded it.
fn cleanup_arc_moves(center: (f64, f64), radius: f64, z: f64, start_angle: f64) -> Vec<CutPoint> {
    let spr = segments_per_revolution(radius);
    (1..=spr)
        .map(|i| {
            let angle = start_angle + 2.0 * std::f64::consts::PI * (i as f64 / spr as f64);
            feed_point(Vec3 {
                x: center.0 + radius * angle.cos(),
                y: center.1 + radius * angle.sin(),
                z,
            })
        })
        .collect()
}

/// Centroid of cut-point XY positions.
///
/// For closed contours (last point ≈ first point) the closing duplicate is
/// excluded so it does not skew the centroid toward the first vertex.
fn centroid_xy(cuts: &[CutPoint]) -> (f64, f64) {
    // Exclude the closing duplicate if the contour is closed.
    let effective = if cuts.len() >= 2 {
        let first = &cuts[0].position;
        let last = &cuts[cuts.len() - 1].position;
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        if (dx * dx + dy * dy).sqrt() < 0.001 {
            &cuts[..cuts.len() - 1]
        } else {
            cuts
        }
    } else {
        cuts
    };
    let n = effective.len() as f64;
    let sx: f64 = effective.iter().map(|p| p.position.x).sum();
    let sy: f64 = effective.iter().map(|p| p.position.y).sum();
    (sx / n, sy / n)
}

/// Minimum Euclidean XY distance from `center` to any cut-point.
fn min_dist_to_centroid(center: (f64, f64), cuts: &[CutPoint]) -> f64 {
    cuts.iter()
        .map(|p| {
            let dx = p.position.x - center.0;
            let dy = p.position.y - center.1;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(f64::MAX, f64::min)
}

/// Returns `true` when the first and last cut points of `pass` are within
/// 0.001 mm in XY — i.e. the contour is closed.
fn is_closed_contour(pass: &Pass) -> bool {
    if pass.cuts.len() < 2 {
        return false;
    }
    let first = &pass.cuts[0].position;
    let last = &pass.cuts[pass.cuts.len() - 1].position;
    let dx = first.x - last.x;
    let dy = first.y - last.y;
    (dx * dx + dy * dy).sqrt() < 0.001
}

fn vec3_sub(a: &Vec3, b: &Vec3) -> Vec3 {
    Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn vec3_add(a: &Vec3, b: &Vec3) -> Vec3 {
    Vec3 {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn vec3_scale(v: &Vec3, s: f64) -> Vec3 {
    Vec3 {
        x: v.x * s,
        y: v.y * s,
        z: v.z * s,
    }
}

fn normalize(v: Vec3) -> Vec3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len < 1e-12 {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    } else {
        vec3_scale(&v, 1.0 / len)
    }
}

fn rapid_point(position: Vec3) -> CutPoint {
    CutPoint {
        position,
        move_kind: MoveKind::Rapid,
        tool_orientation: None,
    }
}

fn feed_point(position: Vec3) -> CutPoint {
    CutPoint {
        position,
        move_kind: MoveKind::Feed,
        tool_orientation: None,
    }
}

/// Returns the XY approach coordinates for the LeadIn of `pass`.
///
/// The approach point sits `lead_offset` behind the first cut point along the
/// head tangent (direction from point 0 to point 1).  Falls back to the first
/// cut point's XY when the pass has only one point (no LeadIn is generated in
/// that case, but the Linking still needs a descent target).
fn lead_in_approach_xy(pass: &Pass, lead_offset: f64) -> (f64, f64) {
    if pass.cuts.len() > 1 {
        let p0 = &pass.cuts[0].position;
        let p1 = &pass.cuts[1].position;
        let head_tangent = normalize(vec3_sub(p1, p0));
        let approach = vec3_sub(p0, &vec3_scale(&head_tangent, lead_offset));
        (approach.x, approach.y)
    } else {
        let p0 = &pass.cuts[0].position;
        (p0.x, p0.y)
    }
}

/// Inserts linking, lead-in, and lead-out passes around each cutting pass.
///
/// For every cutting pass the output order is:
/// 1. [`PassKind::LeadOut`] — skipped for the first pass and when the previous
///    pass has only one point.
/// 2. [`PassKind::Linking`] — always present.  For a standard plunge: three
///    [`MoveKind::Rapid`] moves (lift, traverse, descend).  When
///    `params.helical_entry_radius` is set and the contour is closed: two
///    Rapid moves followed by helical-descent and cleanup-arc [`MoveKind::Feed`]
///    moves, provided the helix fits inside the cut polygon; otherwise falls
///    back to the standard plunge.
/// 3. [`PassKind::LeadIn`] — skipped when the cutting pass has only one point.
/// 4. The cutting pass itself (unchanged).
pub fn link_passes(cutting_passes: Vec<Pass>, params: &LinkingParams) -> Vec<Pass> {
    let lead_offset = params.lead_ratio * params.tool_diameter;
    let clearance_z = params.clearance_z;
    let mut result = Vec::new();

    for (i, pass) in cutting_passes.iter().enumerate() {
        // Compute LeadOut position once; it is reused both to emit the LeadOut
        // pass and to determine the Linking lift-from XY.
        let lead_out_pos: Option<Vec3> = if i > 0 {
            let prev = &cutting_passes[i - 1];
            let n = prev.cuts.len();
            if n > 1 {
                let tail = &prev.cuts[n - 1].position;
                let pre_tail = &prev.cuts[n - 2].position;
                let tangent = normalize(vec3_sub(tail, pre_tail));
                Some(vec3_add(tail, &vec3_scale(&tangent, lead_offset)))
            } else {
                None
            }
        } else {
            None
        };

        // 1. LeadOut — based on the tail of the previous cutting pass.
        if let Some(ref pos) = lead_out_pos {
            result.push(Pass {
                kind: PassKind::LeadOut,
                cuts: vec![feed_point(pos.clone())],
            });
        }

        // 2. Linking — lift, traverse, descend.
        //
        // The lift-from XY is the end of the LeadOut (if generated) or the tail
        // of the previous cutting pass.  For the very first pass there is no
        // predecessor, so the lift-from XY equals the approach XY, collapsing
        // the first two rapid moves to the same position — intentional and safe.
        let (approach_x, approach_y) = lead_in_approach_xy(pass, lead_offset);
        let cutting_z = pass.cuts[0].position.z;

        let (from_x, from_y) = match &lead_out_pos {
            Some(pos) => (pos.x, pos.y),
            None if i > 0 => {
                let prev = &cutting_passes[i - 1];
                let tail = &prev.cuts[prev.cuts.len() - 1].position;
                (tail.x, tail.y)
            }
            None => (approach_x, approach_y),
        };

        // Decide whether to use helical entry for this pass.
        let use_helical = params.helical_entry_radius.is_some() && is_closed_contour(pass);

        if use_helical {
            let radius = params.helical_entry_radius.unwrap();
            let pitch = params
                .helical_entry_pitch
                .unwrap_or(params.tool_diameter / 3.0);

            let center = centroid_xy(&pass.cuts);
            let min_dist = min_dist_to_centroid(center, &pass.cuts);

            if radius < min_dist {
                // Helix fits: helix entry point is at angle=0 on the circle.
                let helix_x = center.0 + radius;
                let helix_y = center.1;

                let mut linking_cuts = vec![
                    rapid_point(Vec3 {
                        x: from_x,
                        y: from_y,
                        z: clearance_z,
                    }),
                    rapid_point(Vec3 {
                        x: helix_x,
                        y: helix_y,
                        z: clearance_z,
                    }),
                ];
                let helix_moves =
                    helical_descent_moves(center, radius, pitch, clearance_z, cutting_z);
                // Derive the cleanup arc's start angle from the helix endpoint so
                // there is no positional discontinuity when total_segs is not an
                // integer multiple of segments_per_revolution.
                let helix_end_angle = helix_moves.last().map_or(0.0, |p| {
                    let dx = p.position.x - center.0;
                    let dy = p.position.y - center.1;
                    dy.atan2(dx)
                });
                linking_cuts.extend(helix_moves);
                linking_cuts.extend(cleanup_arc_moves(
                    center,
                    radius,
                    cutting_z,
                    helix_end_angle,
                ));

                result.push(Pass {
                    kind: PassKind::Linking,
                    cuts: linking_cuts,
                });
            } else {
                // Helix doesn't fit; fall back to straight plunge with a warning.
                tracing::warn!(
                    "helical entry: radius too large for pocket, falling back to plunge"
                );
                result.push(Pass {
                    kind: PassKind::Linking,
                    cuts: vec![
                        rapid_point(Vec3 {
                            x: from_x,
                            y: from_y,
                            z: clearance_z,
                        }),
                        rapid_point(Vec3 {
                            x: approach_x,
                            y: approach_y,
                            z: clearance_z,
                        }),
                        rapid_point(Vec3 {
                            x: approach_x,
                            y: approach_y,
                            z: cutting_z,
                        }),
                    ],
                });
            }
        } else {
            result.push(Pass {
                kind: PassKind::Linking,
                cuts: vec![
                    rapid_point(Vec3 {
                        x: from_x,
                        y: from_y,
                        z: clearance_z,
                    }),
                    rapid_point(Vec3 {
                        x: approach_x,
                        y: approach_y,
                        z: clearance_z,
                    }),
                    rapid_point(Vec3 {
                        x: approach_x,
                        y: approach_y,
                        z: cutting_z,
                    }),
                ],
            });
        }

        // 3. LeadIn — feed from approach position to the first cut point.
        if pass.cuts.len() > 1 {
            result.push(Pass {
                kind: PassKind::LeadIn,
                cuts: vec![feed_point(pass.cuts[0].position.clone())],
            });
        }

        // 4. Cutting pass (unchanged).
        result.push(pass.clone());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;
    use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

    fn make_pass(points: &[(f64, f64, f64)]) -> Pass {
        Pass {
            kind: PassKind::Cutting,
            cuts: points
                .iter()
                .map(|&(x, y, z)| CutPoint {
                    position: Vec3 { x, y, z },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                })
                .collect(),
        }
    }

    #[test]
    fn link_two_passes_produces_correct_sequence() {
        let passes = vec![
            make_pass(&[(0.0, 0.0, -5.0), (10.0, 0.0, -5.0), (20.0, 0.0, -5.0)]),
            make_pass(&[(0.0, 5.0, -10.0), (10.0, 5.0, -10.0), (20.0, 5.0, -10.0)]),
        ];

        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        let kinds: Vec<&PassKind> = result.iter().map(|p| &p.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &PassKind::Linking,
                &PassKind::LeadIn,
                &PassKind::Cutting,
                &PassKind::LeadOut,
                &PassKind::Linking,
                &PassKind::LeadIn,
                &PassKind::Cutting,
            ]
        );
    }

    #[test]
    fn linking_pass_contains_rapid_moves() {
        let passes = vec![
            make_pass(&[(0.0, 0.0, -5.0), (10.0, 0.0, -5.0), (20.0, 0.0, -5.0)]),
            make_pass(&[(0.0, 5.0, -10.0), (10.0, 5.0, -10.0), (20.0, 5.0, -10.0)]),
        ];

        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        for pass in &result {
            if pass.kind == PassKind::Linking {
                for cut in &pass.cuts {
                    assert_eq!(
                        cut.move_kind,
                        MoveKind::Rapid,
                        "all moves in a Linking pass must be Rapid"
                    );
                }
            }
        }
    }

    #[test]
    fn single_point_pass_skips_lead_geometry() {
        let passes = vec![make_pass(&[(5.0, 5.0, -3.0)])];

        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        for pass in &result {
            assert!(
                pass.kind != PassKind::LeadIn && pass.kind != PassKind::LeadOut,
                "unexpected {:?} pass for a 1-point cutting pass",
                pass.kind
            );
        }
    }

    // -----------------------------------------------------------------------
    // Helical entry tests
    // -----------------------------------------------------------------------

    /// Build a closed square contour at the given Z (last point == first point).
    fn make_closed_pass(z: f64) -> Pass {
        // 10 mm square centred at origin; cut points already inset by tool_radius.
        make_pass(&[
            (5.0, 5.0, z),
            (-5.0, 5.0, z),
            (-5.0, -5.0, z),
            (5.0, -5.0, z),
            (5.0, 5.0, z), // close
        ])
    }

    #[test]
    fn helical_descent_z_is_monotonically_decreasing() {
        let moves = helical_descent_moves((0.0, 0.0), 2.0, 1.0, 5.0, -5.0);
        assert!(!moves.is_empty());
        let mut prev_z = 5.0_f64;
        for m in &moves {
            assert!(
                m.position.z <= prev_z,
                "Z not monotonically decreasing: {} after {}",
                m.position.z,
                prev_z
            );
            prev_z = m.position.z;
        }
        // Final point must reach end_z.
        let last_z = moves.last().unwrap().position.z;
        assert!(
            (last_z - (-5.0)).abs() < 1e-9,
            "last Z should be end_z, got {}",
            last_z
        );
    }

    #[test]
    fn helical_descent_xy_on_circle() {
        let center = (1.0, 2.0);
        let radius = 3.0;
        let moves = helical_descent_moves(center, radius, 1.0, 0.0, -4.0);
        assert!(!moves.is_empty());
        for m in &moves {
            let dx = m.position.x - center.0;
            let dy = m.position.y - center.1;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(
                (dist - radius).abs() < 1e-9,
                "XY distance from center should be radius {}, got {}",
                radius,
                dist
            );
        }
    }

    #[test]
    fn helical_descent_pitch_controls_revolution_count() {
        let depth = 10.0_f64;
        let radius = 2.0_f64;
        let spr = segments_per_revolution(radius);

        // With pitch = depth/2 we expect approximately 2 full revolutions.
        let moves_half = helical_descent_moves((0.0, 0.0), radius, depth / 2.0, 0.0, -depth);
        // With pitch = depth we expect approximately 1 full revolution.
        let moves_full = helical_descent_moves((0.0, 0.0), radius, depth, 0.0, -depth);

        // Two-revolution case should have roughly twice as many segments.
        let expected_min_half = spr as f64 * 1.8; // at least ~2 revolutions
        let expected_max_full = spr as f64 * 1.2; // at most ~1 revolution
        assert!(
            moves_half.len() as f64 >= expected_min_half,
            "expected ≥{} segments for 2-rev helix, got {}",
            expected_min_half,
            moves_half.len()
        );
        assert!(
            moves_full.len() as f64 <= expected_max_full,
            "expected ≤{} segments for 1-rev helix, got {}",
            expected_max_full,
            moves_full.len()
        );
        // Two-rev should have more segments than one-rev.
        assert!(
            moves_half.len() > moves_full.len(),
            "more revolutions should produce more segments"
        );
    }

    #[test]
    fn helical_entry_none_falls_back_to_plunge() {
        let passes = vec![make_closed_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None, // no helical entry
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        // With helical_entry_radius=None, Linking pass should have exactly 3 Rapid moves.
        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert_eq!(
            linking.cuts.len(),
            3,
            "plunge linking should have 3 rapid points"
        );
        for cut in &linking.cuts {
            assert_eq!(
                cut.move_kind,
                MoveKind::Rapid,
                "plunge linking moves should be Rapid"
            );
        }
    }

    #[test]
    fn helical_entry_used_for_closed_contour() {
        let passes = vec![make_closed_pass(-5.0)];
        let radius = 2.0_f64;
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: Some(radius),
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        // With helical entry, the linking pass should have more than 3 points.
        assert!(
            linking.cuts.len() > 3,
            "helical linking pass should have more than 3 points, got {}",
            linking.cuts.len()
        );
        // First two moves are Rapid (lift + traverse to helix start).
        assert_eq!(linking.cuts[0].move_kind, MoveKind::Rapid);
        assert_eq!(linking.cuts[1].move_kind, MoveKind::Rapid);
        // Remaining moves are Feed (helix + cleanup arc).
        for cut in &linking.cuts[2..] {
            assert_eq!(
                cut.move_kind,
                MoveKind::Feed,
                "helical descent/cleanup moves should be Feed"
            );
        }
    }

    #[test]
    fn cleanup_arc_starts_at_helix_endpoint() {
        // Use a pitch that does NOT divide evenly into the depth so the helix
        // ends at a non-zero angle.  Verify the first cleanup arc point is
        // within ε of the last helix point in XY (same Z aside).
        let passes = vec![make_closed_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: Some(2.0),
            helical_entry_pitch: Some(3.0), // depth=10, pitch=3 → non-integer revolutions
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);
        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();

        // Separate the helix descent moves from the cleanup arc moves.
        // Rapids are first two; feeds follow. Within the feeds, Z stops at
        // cutting_z when the cleanup arc begins.
        let feed_moves: Vec<_> = linking
            .cuts
            .iter()
            .filter(|c| c.move_kind == MoveKind::Feed)
            .collect();
        assert!(!feed_moves.is_empty());

        // Find where Z first reaches cutting_z = -5.0 — that's the boundary
        // between helix and cleanup arc.
        let cutting_z = -5.0_f64;
        let arc_start_idx = feed_moves
            .iter()
            .position(|c| (c.position.z - cutting_z).abs() < 1e-9)
            .expect("at least one point should be at cutting_z");

        // The point just before the arc (last helix point) and the first arc
        // point should have continuous XY (≤ one chord length apart).
        if arc_start_idx > 0 {
            let last_helix = &feed_moves[arc_start_idx - 1];
            let first_arc = &feed_moves[arc_start_idx];
            let dx = first_arc.position.x - last_helix.position.x;
            let dy = first_arc.position.y - last_helix.position.y;
            let gap = (dx * dx + dy * dy).sqrt();
            let chord = chord_len_for_radius(2.0);
            assert!(
                gap <= chord + 1e-9,
                "XY gap between last helix point and first cleanup arc point ({gap:.6}) \
                 exceeds chord length ({chord:.6}) — discontinuity"
            );
        }
    }

    #[test]
    fn helical_entry_falls_back_when_radius_too_large() {
        // Radius larger than min distance from centroid to any cut point → plunge fallback.
        let passes = vec![make_closed_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: Some(100.0), // way too large
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        // Should fall back: linking pass has exactly 3 Rapid moves.
        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert_eq!(
            linking.cuts.len(),
            3,
            "oversized radius should fall back to 3-point plunge, got {}",
            linking.cuts.len()
        );
        for cut in &linking.cuts {
            assert_eq!(cut.move_kind, MoveKind::Rapid);
        }
    }
}
