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
/// yield a zero or negative radicand (radius ≤ CHORDAL_ERROR/2).  At exactly
/// `radius == CHORDAL_ERROR/2` the chord length is zero, which would produce
/// +inf and then usize::MAX via Rust's saturating float-to-int cast.
fn segments_per_revolution(radius: f64) -> usize {
    let e = CHORDAL_ERROR;
    if radius <= e / 2.0 {
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

// ---------------------------------------------------------------------------
// Arc lead-in / lead-out helpers
// ---------------------------------------------------------------------------

/// Number of segments to chord-approximate a quarter-circle arc with
/// ≤ [`CHORDAL_ERROR`] error.  Returns 0 for degenerate (tiny) radii.
fn arc_quarter_segments(radius: f64) -> usize {
    if radius <= CHORDAL_ERROR / 2.0 {
        return 0;
    }
    (std::f64::consts::FRAC_PI_2 / (1.0 - CHORDAL_ERROR / radius).max(-1.0).acos()).ceil() as usize
}

/// Quarter-circle arc approach into `first_cut_point` at `cut_z`.
///
/// The arc is in the XY plane, tangent to the direction from `first_cut_point`
/// toward `second_cut_point`.  The center is offset to the left of the travel
/// direction, so the arc sweeps CCW from outside the material into
/// `first_cut_point`.  All returned moves are [`MoveKind::Feed`].
///
/// Segment count: `(π/2 / acos(1 − CHORDAL_ERROR/radius)).ceil()`.
fn arc_approach_moves(
    first_cut_point: (f64, f64, f64),
    second_cut_point: (f64, f64, f64),
    radius: f64,
    cut_z: f64,
) -> Vec<CutPoint> {
    let n_segs = arc_quarter_segments(radius);
    if n_segs == 0 {
        return vec![];
    }

    let dx = second_cut_point.0 - first_cut_point.0;
    let dy = second_cut_point.1 - first_cut_point.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return vec![];
    }
    let (tx, ty) = (dx / len, dy / len);
    // Left normal to the cutting direction.
    let (nx, ny) = (-ty, tx);

    // Circle center: to the left of the entry tangent at first_cut_point.
    let cx = first_cut_point.0 + radius * nx;
    let cy = first_cut_point.1 + radius * ny;

    // Angle of first_cut_point relative to center.
    // first_cut_point - center = -radius * (nx, ny) = (radius*ty, -radius*tx)
    let alpha = (-radius * tx).atan2(radius * ty);

    // Arc: CCW from (alpha - π/2) to alpha.
    let start_angle = alpha - std::f64::consts::FRAC_PI_2;

    (1..=n_segs)
        .map(|i| {
            let t = i as f64 / n_segs as f64;
            let angle = start_angle + std::f64::consts::FRAC_PI_2 * t;
            feed_point(Vec3 {
                x: cx + radius * angle.cos(),
                y: cy + radius * angle.sin(),
                z: cut_z,
            })
        })
        .collect()
}

/// Quarter-circle arc departure from `last_cut_point` at `cut_z`.
///
/// Symmetric to [`arc_approach_moves`]: the arc sweeps CCW starting at
/// `last_cut_point`, tangent to the direction from `second_to_last_cut_point`
/// toward `last_cut_point`, ending at a point geometrically outside the
/// original boundary.  All returned moves are [`MoveKind::Feed`].
fn arc_departure_moves(
    last_cut_point: (f64, f64, f64),
    second_to_last_cut_point: (f64, f64, f64),
    radius: f64,
    cut_z: f64,
) -> Vec<CutPoint> {
    let n_segs = arc_quarter_segments(radius);
    if n_segs == 0 {
        return vec![];
    }

    let dx = last_cut_point.0 - second_to_last_cut_point.0;
    let dy = last_cut_point.1 - second_to_last_cut_point.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return vec![];
    }
    let (tx, ty) = (dx / len, dy / len);
    let (nx, ny) = (-ty, tx);

    let cx = last_cut_point.0 + radius * nx;
    let cy = last_cut_point.1 + radius * ny;

    // Angle of last_cut_point relative to center.
    let alpha = (last_cut_point.1 - cy).atan2(last_cut_point.0 - cx);

    // Arc: CCW from alpha to (alpha + π/2).
    (1..=n_segs)
        .map(|i| {
            let t = i as f64 / n_segs as f64;
            let angle = alpha + std::f64::consts::FRAC_PI_2 * t;
            feed_point(Vec3 {
                x: cx + radius * angle.cos(),
                y: cy + radius * angle.sin(),
                z: cut_z,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Ramp entry helpers
// ---------------------------------------------------------------------------

/// Produces linear feed moves descending from `retract_z` to `cut_z` along
/// the XY direction from `xy_start` toward `xy_end`.
///
/// The required horizontal distance is `depth / tan(angle_deg)`.  When the
/// segment is shorter than that distance the ramp is clamped to the segment
/// length (steepening the effective angle) and a warning is logged.
/// The number of steps is chosen so that individual Z increments ≤ 0.5 mm.
fn ramp_descent_moves(
    xy_start: (f64, f64),
    xy_end: (f64, f64),
    retract_z: f64,
    cut_z: f64,
    angle_deg: f64,
) -> Vec<CutPoint> {
    let depth = (retract_z - cut_z).abs();
    // angle_deg must be in (0°, 90°): at 0° the ramp is horizontal (infinite
    // length), at ≥ 90° tan becomes zero or negative and required_horiz would
    // be infinite or negative, producing moves in the wrong direction.
    if depth <= 0.0 || angle_deg <= 0.0 || angle_deg >= 90.0 {
        return vec![];
    }

    let dx = xy_end.0 - xy_start.0;
    let dy = xy_end.1 - xy_start.1;
    let seg_len = (dx * dx + dy * dy).sqrt();

    let required_horiz = depth / angle_deg.to_radians().tan();
    let actual_horiz = if required_horiz > seg_len {
        tracing::warn!("ramp entry: segment too short, steepening angle");
        seg_len
    } else {
        required_horiz
    };

    let n_steps = ((depth / 0.5).ceil() as usize).max(1);
    let (dir_x, dir_y) = if seg_len > 1e-12 {
        (dx / seg_len, dy / seg_len)
    } else {
        (1.0, 0.0)
    };

    (1..=n_steps)
        .map(|i| {
            let t = i as f64 / n_steps as f64;
            let z = retract_z - depth * t;
            let horiz = actual_horiz * t;
            feed_point(Vec3 {
                x: xy_start.0 + dir_x * horiz,
                y: xy_start.1 + dir_y * horiz,
                z,
            })
        })
        .collect()
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
///    back to the standard plunge.  When `params.ramp_entry_angle_deg` is set
///    and the contour is open (first ≠ last point): two Rapid moves followed
///    by ramp-descent [`MoveKind::Feed`] moves along the first segment direction.
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
        //
        // `lead_out_actual_xy` is the actual XY endpoint of the LeadOut pass
        // (arc end or straight-feed end) and is used as the Linking lift-from
        // position so the path is continuous.
        let mut lead_out_actual_xy: Option<(f64, f64)> = None;
        if let Some(ref pos) = lead_out_pos {
            let lead_out_cuts = if let Some(arc_r) = params.arc_lead_out_radius {
                let prev = &cutting_passes[i - 1];
                let n = prev.cuts.len();
                // n > 1 is guaranteed because lead_out_pos is Some only when n > 1.
                let tail = &prev.cuts[n - 1].position;
                let pre_tail = &prev.cuts[n - 2].position;
                arc_departure_moves(
                    (tail.x, tail.y, tail.z),
                    (pre_tail.x, pre_tail.y, pre_tail.z),
                    arc_r,
                    tail.z,
                )
            } else {
                vec![]
            };
            // Fall back to a straight feed when arc is degenerate (e.g. coincident
            // points) so the LeadOut pass is never silently omitted.
            let lead_out_cuts = if lead_out_cuts.is_empty() {
                vec![feed_point(pos.clone())]
            } else {
                lead_out_cuts
            };
            lead_out_actual_xy = lead_out_cuts.last().map(|c| (c.position.x, c.position.y));
            result.push(Pass {
                kind: PassKind::LeadOut,
                cuts: lead_out_cuts,
            });
        }

        // 2. Linking — lift, traverse, descend.
        //
        // The lift-from XY is the actual end of the LeadOut (if generated) or
        // the tail of the previous cutting pass.  For the very first pass there
        // is no predecessor, so the lift-from XY equals the approach XY,
        // collapsing the first two rapid moves to the same position —
        // intentional and safe.
        let (approach_x, approach_y) = lead_in_approach_xy(pass, lead_offset);
        let cutting_z = pass.cuts[0].position.z;

        let (from_x, from_y) = match lead_out_actual_xy {
            Some((x, y)) => (x, y),
            None if i > 0 => {
                let prev = &cutting_passes[i - 1];
                let tail = &prev.cuts[prev.cuts.len() - 1].position;
                (tail.x, tail.y)
            }
            None => (approach_x, approach_y),
        };

        // Decide whether to use helical or ramp entry for this pass.
        let use_helical = params.helical_entry_radius.is_some() && is_closed_contour(pass);
        let use_ramp = !use_helical
            && params.ramp_entry_angle_deg.is_some()
            && pass.cuts.len() >= 2
            && !is_closed_contour(pass);

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
        } else if use_ramp {
            let angle_deg = params.ramp_entry_angle_deg.unwrap();
            let first_cut = &pass.cuts[0].position;
            let ramp_moves = ramp_descent_moves(
                (approach_x, approach_y),
                (first_cut.x, first_cut.y),
                clearance_z,
                cutting_z,
                angle_deg,
            );
            if ramp_moves.is_empty() {
                // Invalid angle or zero depth — fall back to straight plunge.
                tracing::warn!(
                    "ramp entry: invalid angle {angle_deg}° or zero depth, falling back to plunge"
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
            } else {
                let mut linking_cuts = vec![
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
                ];
                linking_cuts.extend(ramp_moves);
                result.push(Pass {
                    kind: PassKind::Linking,
                    cuts: linking_cuts,
                });
            }
        } else {
            // For arc lead-in, descend to the arc start S = P − R·T + R·N so
            // the Linking plunge ends exactly where the arc approach begins,
            // keeping the path continuous at cutting depth.
            let (desc_x, desc_y) =
                if let Some(arc_r) = params.arc_lead_in_radius.filter(|_| pass.cuts.len() >= 2) {
                    let p0 = &pass.cuts[0].position;
                    let p1 = &pass.cuts[1].position;
                    let ddx = p1.x - p0.x;
                    let ddy = p1.y - p0.y;
                    let dlen = (ddx * ddx + ddy * ddy).sqrt();
                    if dlen >= 1e-12 {
                        let (tx, ty) = (ddx / dlen, ddy / dlen);
                        let (nx, ny) = (-ty, tx); // left perpendicular
                        (
                            p0.x - arc_r * tx + arc_r * nx,
                            p0.y - arc_r * ty + arc_r * ny,
                        )
                    } else {
                        (approach_x, approach_y)
                    }
                } else {
                    (approach_x, approach_y)
                };
            result.push(Pass {
                kind: PassKind::Linking,
                cuts: vec![
                    rapid_point(Vec3 {
                        x: from_x,
                        y: from_y,
                        z: clearance_z,
                    }),
                    rapid_point(Vec3 {
                        x: desc_x,
                        y: desc_y,
                        z: clearance_z,
                    }),
                    rapid_point(Vec3 {
                        x: desc_x,
                        y: desc_y,
                        z: cutting_z,
                    }),
                ],
            });
        }

        // 3. LeadIn — feed from approach position to the first cut point.
        if pass.cuts.len() > 1 {
            let lead_in_cuts = if let Some(arc_r) = params.arc_lead_in_radius {
                let p0 = &pass.cuts[0].position;
                let p1 = &pass.cuts[1].position;
                arc_approach_moves((p0.x, p0.y, p0.z), (p1.x, p1.y, p1.z), arc_r, cutting_z)
            } else {
                vec![]
            };
            // Fall back to a straight feed when arc is degenerate so the
            // LeadIn pass is never silently omitted.
            let lead_in_cuts = if lead_in_cuts.is_empty() {
                vec![feed_point(pass.cuts[0].position.clone())]
            } else {
                lead_in_cuts
            };
            result.push(Pass {
                kind: PassKind::LeadIn,
                cuts: lead_in_cuts,
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
        // ends at a non-zero angle.  The boundary between helix moves and
        // cleanup-arc moves is at index `total_segs` in the feed-move list.
        // Verify no positional discontinuity across that boundary.
        let radius = 2.0_f64;
        let pitch = 3.0_f64;
        let clearance_z = 5.0_f64;
        let cutting_z = -5.0_f64;

        let passes = vec![make_closed_pass(cutting_z)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: Some(radius),
            helical_entry_pitch: Some(pitch), // depth=10, pitch=3 → non-integer revolutions
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);
        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();

        let feed_moves: Vec<_> = linking
            .cuts
            .iter()
            .filter(|c| c.move_kind == MoveKind::Feed)
            .collect();

        // Compute the expected helix segment count identically to the
        // production code so we can index the exact helix→arc boundary.
        let spr = segments_per_revolution(radius);
        let total_depth = clearance_z - cutting_z;
        let total_segs = ((total_depth / pitch) * spr as f64).ceil().max(1.0) as usize;

        assert_eq!(
            feed_moves.len(),
            total_segs + spr,
            "expected {total_segs} helix + {spr} arc feed moves"
        );

        // Last helix point must be at cutting_z.
        assert!(
            (feed_moves[total_segs - 1].position.z - cutting_z).abs() < 1e-9,
            "last helix point should be at cutting_z"
        );

        // The XY gap between the last helix point and the first cleanup-arc
        // point must be ≤ one chord length (i.e. no jump back to angle=0).
        let last_helix = &feed_moves[total_segs - 1];
        let first_arc = &feed_moves[total_segs];
        let dx = first_arc.position.x - last_helix.position.x;
        let dy = first_arc.position.y - last_helix.position.y;
        let gap = (dx * dx + dy * dy).sqrt();
        let chord = chord_len_for_radius(radius);
        assert!(
            gap <= chord + 1e-9,
            "XY gap at helix→arc boundary ({gap:.6} mm) exceeds chord length \
             ({chord:.6} mm) — cleanup arc jumped to wrong start angle"
        );
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

    // -----------------------------------------------------------------------
    // Ramp entry tests
    // -----------------------------------------------------------------------

    /// Open contour: a simple line segment along X at cut_z.
    fn make_open_pass(z: f64) -> Pass {
        make_pass(&[(0.0, 0.0, z), (20.0, 0.0, z), (40.0, 0.0, z)])
    }

    #[test]
    fn ramp_moves_span_retract_to_cut_z() {
        let retract_z = 5.0_f64;
        let cut_z = -5.0_f64;
        let moves = ramp_descent_moves((0.0, 0.0), (100.0, 0.0), retract_z, cut_z, 15.0);
        assert!(!moves.is_empty());
        let first_z = moves[0].position.z;
        let last_z = moves.last().unwrap().position.z;
        assert!(
            first_z < retract_z,
            "first Z should be below retract_z, got {first_z}"
        );
        assert!(
            (last_z - cut_z).abs() < 1e-9,
            "last Z should equal cut_z {cut_z}, got {last_z}"
        );
    }

    #[test]
    fn ramp_z_values_decrease_monotonically() {
        let moves = ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, 15.0);
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
    }

    #[test]
    fn ramp_horizontal_distance_consistent_with_angle_and_depth() {
        let retract_z = 5.0_f64;
        let cut_z = -5.0_f64;
        let angle_deg = 15.0_f64;
        let xy_start = (0.0_f64, 0.0_f64);
        let xy_end = (200.0_f64, 0.0_f64); // long enough to not clamp

        let moves = ramp_descent_moves(xy_start, xy_end, retract_z, cut_z, angle_deg);
        assert!(!moves.is_empty());

        let depth = (retract_z - cut_z).abs();
        let expected_horiz = depth / angle_deg.to_radians().tan();
        let last = moves.last().unwrap();
        let actual_horiz = (last.position.x - xy_start.0).abs();

        assert!(
            (actual_horiz - expected_horiz).abs() < 1e-9,
            "expected horizontal distance {expected_horiz:.6}, got {actual_horiz:.6}"
        );
    }

    #[test]
    fn ramp_entry_none_falls_back_to_plunge() {
        let passes = vec![make_open_pass(-5.0)];
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

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert_eq!(
            linking.cuts.len(),
            3,
            "plunge should have 3 points, got {}",
            linking.cuts.len()
        );
        for cut in &linking.cuts {
            assert_eq!(
                cut.move_kind,
                MoveKind::Rapid,
                "plunge moves should be Rapid"
            );
        }
    }

    #[test]
    fn ramp_short_segment_clamps_and_still_reaches_cut_z() {
        // Segment is only 2 mm long; a 15° ramp over 10 mm depth needs ~37 mm.
        let retract_z = 5.0_f64;
        let cut_z = -5.0_f64;
        // warning is emitted via tracing::warn! inside ramp_descent_moves
        let moves = ramp_descent_moves((0.0, 0.0), (2.0, 0.0), retract_z, cut_z, 15.0);
        assert!(
            !moves.is_empty(),
            "should produce moves even for short segment"
        );

        let last_z = moves.last().unwrap().position.z;
        assert!(
            (last_z - cut_z).abs() < 1e-9,
            "last Z should still reach cut_z {cut_z} even when segment is short, got {last_z}"
        );

        let last_x = moves.last().unwrap().position.x;
        assert!(
            last_x <= 2.0 + 1e-9,
            "ramp end should be clamped to segment length, got x={last_x}"
        );
    }

    #[test]
    fn ramp_invalid_angles_produce_no_moves() {
        // angle_deg ≥ 90° → tan goes zero or negative, ramp would travel
        // backward or be infinite; must return empty rather than bad moves.
        assert!(ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, 90.0).is_empty());
        assert!(ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, 91.0).is_empty());
        assert!(ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, 180.0).is_empty());
        // angle_deg ≤ 0 was already guarded but verify too.
        assert!(ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, 0.0).is_empty());
        assert!(ramp_descent_moves((0.0, 0.0), (100.0, 0.0), 5.0, -5.0, -5.0).is_empty());
    }

    #[test]
    fn ramp_zero_length_segment_stays_at_start_xy() {
        // When xy_start == xy_end the ramp must not wander off in an arbitrary
        // direction; all XY positions should stay at xy_start (vertical plunge).
        let xy_start = (3.0_f64, 7.0_f64);
        let moves = ramp_descent_moves(xy_start, xy_start, 5.0, -5.0, 15.0);
        assert!(!moves.is_empty());
        for m in &moves {
            assert!(
                (m.position.x - xy_start.0).abs() < 1e-9
                    && (m.position.y - xy_start.1).abs() < 1e-9,
                "zero-length segment ramp moved in XY: ({}, {})",
                m.position.x,
                m.position.y
            );
        }
        // Must still reach cut_z.
        let last_z = moves.last().unwrap().position.z;
        assert!(
            (last_z - (-5.0)).abs() < 1e-9,
            "last Z should be cut_z, got {last_z}"
        );
    }

    #[test]
    fn ramp_entry_used_for_open_contour() {
        let passes = vec![make_open_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: Some(15.0),
        };
        let result = link_passes(passes, &params);

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert!(
            linking.cuts.len() > 3,
            "ramp linking pass should have more than 3 points, got {}",
            linking.cuts.len()
        );
        assert_eq!(linking.cuts[0].move_kind, MoveKind::Rapid);
        assert_eq!(linking.cuts[1].move_kind, MoveKind::Rapid);
        for cut in &linking.cuts[2..] {
            assert_eq!(cut.move_kind, MoveKind::Feed, "ramp moves should be Feed");
        }
        let last_z = linking.cuts.last().unwrap().position.z;
        assert!(
            (last_z - (-5.0)).abs() < 1e-9,
            "last ramp move should reach cutting_z, got {last_z}"
        );
    }

    #[test]
    fn ramp_entry_non_clamped_reaches_cut_z() {
        // Use a steep angle (80°) so required_horiz ≈ 1.76 mm, well under the
        // lead_offset of 2.4 mm (0.4 × 6.0).  This exercises the code path in
        // link_passes where actual_horiz == required_horiz (no clamping).
        let passes = vec![make_open_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: Some(80.0),
        };
        let result = link_passes(passes, &params);

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        // 2 rapids + at least 1 ramp feed move.
        assert!(linking.cuts.len() > 2, "should have ramp feed moves");
        assert_eq!(linking.cuts[0].move_kind, MoveKind::Rapid);
        assert_eq!(linking.cuts[1].move_kind, MoveKind::Rapid);
        for cut in &linking.cuts[2..] {
            assert_eq!(cut.move_kind, MoveKind::Feed);
        }
        let last_z = linking.cuts.last().unwrap().position.z;
        assert!(
            (last_z - (-5.0)).abs() < 1e-9,
            "non-clamped ramp must still reach cutting_z, got {last_z}"
        );
    }

    #[test]
    fn ramp_invalid_angle_falls_back_to_plunge_in_link_passes() {
        // An out-of-range angle (≥ 90°) should not leave the linking pass without
        // a Z descent — it must fall back to the standard 3-point plunge.
        let passes = vec![make_open_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: Some(90.0),
        };
        let result = link_passes(passes, &params);
        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert_eq!(linking.cuts.len(), 3, "should fall back to 3-point plunge");
        let last_z = linking.cuts.last().unwrap().position.z;
        assert!(
            (last_z - (-5.0)).abs() < 1e-9,
            "fallback plunge must reach cutting_z, got {last_z}"
        );
        for cut in &linking.cuts {
            assert_eq!(cut.move_kind, MoveKind::Rapid);
        }
    }

    // -----------------------------------------------------------------------
    // Arc lead-in / lead-out tests
    // -----------------------------------------------------------------------

    #[test]
    fn arc_approach_moves_none_radius_gives_straight_lead_in() {
        let passes = vec![make_open_pass(-5.0)];
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
        let lead_in = result.iter().find(|p| p.kind == PassKind::LeadIn).unwrap();
        // Straight lead-in: single feed move to first cut point.
        assert_eq!(lead_in.cuts.len(), 1);
        assert_eq!(lead_in.cuts[0].move_kind, MoveKind::Feed);
    }

    #[test]
    fn arc_approach_moves_some_radius_gives_arc_lead_in() {
        let passes = vec![make_open_pass(-5.0)];
        let arc_r = 3.0_f64;
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: Some(arc_r),
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);
        let lead_in = result.iter().find(|p| p.kind == PassKind::LeadIn).unwrap();
        // Arc lead-in: multiple feed moves.
        assert!(
            lead_in.cuts.len() > 1,
            "arc lead-in should have more than 1 move, got {}",
            lead_in.cuts.len()
        );
        // All arc moves must be Feed.
        for cut in &lead_in.cuts {
            assert_eq!(cut.move_kind, MoveKind::Feed, "arc move must be Feed");
        }
    }

    #[test]
    fn arc_approach_last_move_ends_at_first_cut_point() {
        let first = (0.0_f64, 0.0_f64, -5.0_f64);
        let second = (10.0_f64, 0.0_f64, -5.0_f64);
        let radius = 2.0_f64;
        let moves = arc_approach_moves(first, second, radius, first.2);
        assert!(!moves.is_empty());
        let last = moves.last().unwrap();
        let dx = last.position.x - first.0;
        let dy = last.position.y - first.1;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!(
            dist < 1e-9,
            "last arc approach move should be at first_cut_point, distance = {dist}"
        );
    }

    #[test]
    fn arc_approach_first_move_is_outside_first_cut_point() {
        let first = (0.0_f64, 0.0_f64, -5.0_f64);
        let second = (10.0_f64, 0.0_f64, -5.0_f64);
        let radius = 2.0_f64;
        let moves = arc_approach_moves(first, second, radius, first.2);
        assert!(!moves.is_empty());
        let start = &moves[0];
        let dx = start.position.x - first.0;
        let dy = start.position.y - first.1;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!(
            dist >= radius - 1e-9,
            "first arc approach move should be ≥ radius ({radius}) from first_cut_point, got {dist}"
        );
    }

    #[test]
    fn arc_approach_all_moves_are_feed() {
        let first = (5.0_f64, 3.0_f64, -2.0_f64);
        let second = (15.0_f64, 3.0_f64, -2.0_f64);
        let moves = arc_approach_moves(first, second, 1.5, first.2);
        assert!(!moves.is_empty());
        for m in &moves {
            assert_eq!(m.move_kind, MoveKind::Feed);
        }
    }

    #[test]
    fn arc_departure_none_radius_gives_straight_lead_out() {
        let passes = vec![make_open_pass(-5.0), make_open_pass(-10.0)];
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
        let lead_out = result.iter().find(|p| p.kind == PassKind::LeadOut).unwrap();
        // Straight lead-out: single feed move.
        assert_eq!(lead_out.cuts.len(), 1);
        assert_eq!(lead_out.cuts[0].move_kind, MoveKind::Feed);
    }

    #[test]
    fn arc_departure_some_radius_gives_arc_lead_out() {
        let passes = vec![make_open_pass(-5.0), make_open_pass(-10.0)];
        let arc_r = 3.0_f64;
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: Some(arc_r),
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);
        let lead_out = result.iter().find(|p| p.kind == PassKind::LeadOut).unwrap();
        assert!(
            lead_out.cuts.len() > 1,
            "arc lead-out should have more than 1 move, got {}",
            lead_out.cuts.len()
        );
        for cut in &lead_out.cuts {
            assert_eq!(cut.move_kind, MoveKind::Feed, "arc move must be Feed");
        }
    }

    #[test]
    fn arc_departure_all_moves_are_feed() {
        let last = (20.0_f64, 0.0_f64, -5.0_f64);
        let pre_last = (10.0_f64, 0.0_f64, -5.0_f64);
        let moves = arc_departure_moves(last, pre_last, 2.0, last.2);
        assert!(!moves.is_empty());
        for m in &moves {
            assert_eq!(m.move_kind, MoveKind::Feed);
        }
    }

    /// The Linking pass should end at the arc start S so the path is
    /// continuous at cutting depth: no implicit XY jump between Linking and
    /// LeadIn.
    #[test]
    fn arc_lead_in_linking_ends_at_arc_start() {
        // Open pass along X; first cut at origin, second at (20, 0).
        let passes = vec![make_open_pass(-5.0)];
        let arc_r = 3.0_f64;
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: Some(arc_r),
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        let lead_in = result.iter().find(|p| p.kind == PassKind::LeadIn).unwrap();

        // The Linking last rapid point and the arc start (one chord before the
        // first arc move) must be at the same XY — verified by comparing the
        // linking descent XY against the expected S = P - R·T + R·N.
        // P = (0,0), T = (1,0), N = (0,1) → S = (-R, R).
        let linking_end = linking.cuts.last().unwrap();
        assert!(
            (linking_end.position.x - (-arc_r)).abs() < 1e-9
                && (linking_end.position.y - arc_r).abs() < 1e-9,
            "linking should descend to arc start S=({},{}) but got ({},{})",
            -arc_r,
            arc_r,
            linking_end.position.x,
            linking_end.position.y
        );

        // The last arc approach move must end at the first cut point (0,0).
        let arc_end = lead_in.cuts.last().unwrap();
        assert!(
            arc_end.position.x.abs() < 1e-9 && arc_end.position.y.abs() < 1e-9,
            "arc lead-in last point should be at first cut (0,0), got ({},{})",
            arc_end.position.x,
            arc_end.position.y
        );
    }

    /// After an arc lead-out, the next Linking pass must rapid from the actual
    /// arc endpoint, not the old straight-offset position.
    #[test]
    fn arc_lead_out_linking_lift_from_arc_end() {
        let passes = vec![make_open_pass(-5.0), make_open_pass(-10.0)];
        let arc_r = 3.0_f64;
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: Some(arc_r),
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = link_passes(passes, &params);

        // Find the LeadOut and the Linking that follows it.
        let lead_out = result.iter().find(|p| p.kind == PassKind::LeadOut).unwrap();
        let second_linking = result
            .iter()
            .filter(|p| p.kind == PassKind::Linking)
            .nth(1)
            .unwrap();

        // The arc departure ends at some point E.
        let arc_end = lead_out.cuts.last().unwrap();
        // The second linking lift-from (first rapid point) must equal E.
        let lift_from = &second_linking.cuts[0];
        assert!(
            (lift_from.position.x - arc_end.position.x).abs() < 1e-9
                && (lift_from.position.y - arc_end.position.y).abs() < 1e-9,
            "linking lift-from ({},{}) should match arc lead-out end ({},{})",
            lift_from.position.x,
            lift_from.position.y,
            arc_end.position.x,
            arc_end.position.y
        );
    }

    #[test]
    fn ramp_not_applied_for_closed_contour() {
        let passes = vec![make_closed_pass(-5.0)];
        let params = LinkingParams {
            tool_diameter: 6.0,
            clearance_z: 5.0,
            lead_ratio: 0.4,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: Some(15.0),
        };
        let result = link_passes(passes, &params);

        let linking = result.iter().find(|p| p.kind == PassKind::Linking).unwrap();
        assert_eq!(
            linking.cuts.len(),
            3,
            "closed contour should fall back to plunge, got {}",
            linking.cuts.len()
        );
        for cut in &linking.cuts {
            assert_eq!(cut.move_kind, MoveKind::Rapid);
        }
    }
}
