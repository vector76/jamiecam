//! Linking pass generation for toolpaths.
//!
//! Inserts [`PassKind::Linking`], [`PassKind::LeadIn`], and [`PassKind::LeadOut`] passes
//! around each cutting pass to produce safe tool transitions between depth levels.

use crate::models::Vec3;
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

const LEAD_RATIO: f64 = 0.4;

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
/// 2. [`PassKind::Linking`] — always present; three [`MoveKind::Rapid`] moves:
///    lift to `clearance_z`, traverse at `clearance_z`, descend to cutting depth.
/// 3. [`PassKind::LeadIn`] — skipped when the cutting pass has only one point.
/// 4. The cutting pass itself (unchanged).
pub fn link_passes(cutting_passes: Vec<Pass>, tool_diameter: f64, clearance_z: f64) -> Vec<Pass> {
    let lead_offset = LEAD_RATIO * tool_diameter;
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

        let result = link_passes(passes, 6.0, 5.0);

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

        let result = link_passes(passes, 6.0, 5.0);

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

        let result = link_passes(passes, 6.0, 5.0);

        for pass in &result {
            assert!(
                pass.kind != PassKind::LeadIn && pass.kind != PassKind::LeadOut,
                "unexpected {:?} pass for a 1-point cutting pass",
                pass.kind
            );
        }
    }
}
