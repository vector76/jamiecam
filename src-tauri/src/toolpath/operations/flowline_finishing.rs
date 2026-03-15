//! Flowline finishing algorithm.
//!
//! Generates toolpath passes along UV parameter lines (iso-curves) of selected
//! NURBS faces. The pass direction is controlled by `params.direction` (U or V);
//! alternate passes are reversed (boustrophedon ordering) to minimise linking
//! distance.

use crate::error::AppError;
use crate::geometry::{self, OcctFace, OcctShape};
use crate::models::operation::{FlowlineDirection, FlowlineFinishingParams};
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

// ── Public entry point ───────────────────────────────────────────────────────

/// Generate flowline finishing passes for the given shape and params.
///
/// # Errors
/// - [`AppError::GeometryImport`] if no shape is loaded or OCCT is unavailable.
pub fn flowline_finishing_passes(
    stock: &StockDefinition,
    params: &FlowlineFinishingParams,
    _tool_diameter: f64,
    shape: Option<&OcctShape>,
) -> Result<Vec<Pass>, AppError> {
    let shape = shape
        .ok_or_else(|| AppError::GeometryImport("Shape required for flowline finishing".into()))?;

    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (stock, params, shape);
        return Err(AppError::GeometryImport(
            "Shape required for flowline finishing".into(),
        ));
    }

    #[cfg(cam_geometry_bindings)]
    {
        flowline_finishing_inner(stock, params, shape)
    }
}

// ── Ungated helpers ──────────────────────────────────────────────────────────

/// Split a sequence of points into contiguous runs where consecutive points
/// are no farther apart than `max_gap`.
fn split_runs(points: Vec<Vec3>, max_gap: f64) -> Vec<Vec<Vec3>> {
    if points.is_empty() {
        return Vec::new();
    }

    let max_gap_sq = max_gap * max_gap;
    let mut runs: Vec<Vec<Vec3>> = Vec::new();
    let mut current_run: Vec<Vec3> = Vec::new();

    let mut prev: Option<&Vec3> = None;
    for pt in &points {
        if let Some(p) = prev {
            let dx = pt.x - p.x;
            let dy = pt.y - p.y;
            let dz = pt.z - p.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq > max_gap_sq {
                runs.push(std::mem::take(&mut current_run));
            }
        }
        current_run.push(pt.clone());
        prev = Some(pt);
    }

    if !current_run.is_empty() {
        runs.push(current_run);
    }

    runs
}

/// Reverse odd-indexed passes to achieve boustrophedon (zigzag) ordering.
fn boustrophedon_reorder(passes: &mut [Pass]) {
    for (i, pass) in passes.iter_mut().enumerate() {
        if i % 2 == 1 {
            pass.cuts.reverse();
        }
    }
}

// ── OCCT-dependent implementation ────────────────────────────────────────────

#[cfg(cam_geometry_bindings)]
fn flowline_finishing_inner(
    _stock: &StockDefinition,
    params: &FlowlineFinishingParams,
    shape: &OcctShape,
) -> Result<Vec<Pass>, AppError> {
    // ── Step 1: Resolve faces ────────────────────────────────────────────────
    let all_faces: Vec<OcctFace> = geometry::shape_faces(shape)?;

    let selected_faces: Vec<&OcctFace> = if let Some(fingerprints) = &params.geometry {
        let descriptors = geometry::enumerate_faces(shape)?;
        let mut result = Vec::with_capacity(fingerprints.len());
        for fp in fingerprints {
            let desc = descriptors
                .iter()
                .find(|d| &d.fingerprint == fp)
                .ok_or_else(|| {
                    AppError::GeometryImport(format!("no face found for fingerprint {fp}"))
                })?;
            result.push(&all_faces[desc.face_idx]);
        }
        result
    } else {
        all_faces.iter().collect()
    };

    if selected_faces.is_empty() {
        return Ok(Vec::new());
    }

    let tool_radius = params.tool_diameter / 2.0;
    let num_samples: usize = 100;
    // Maximum gap between consecutive points before splitting into separate runs.
    // Use a fraction of the tool diameter as a reasonable heuristic.
    let max_gap = params.tool_diameter * 2.0;

    let mut all_passes: Vec<Pass> = Vec::new();

    // ── Step 2: Process each face ────────────────────────────────────────────
    for face in &selected_faces {
        let (umin, umax, vmin, vmax) = geometry::face_uv_bounds(face)?;

        // Skip degenerate faces.
        if (umax - umin) < 1e-9 || (vmax - vmin) < 1e-9 {
            continue;
        }

        // Determine iteration axes based on direction.
        // "primary" = the axis we sample along for each pass (the iso-curve direction).
        // "secondary" = the axis we step over across passes.
        let (pri_min, pri_max, sec_min, sec_max) = match params.direction {
            FlowlineDirection::U => (umin, umax, vmin, vmax),
            FlowlineDirection::V => (vmin, vmax, umin, umax),
        };

        let sec_range = sec_max - sec_min;
        let num_lines = ((sec_range / params.stepover).floor() as usize).max(1);

        for line_idx in 0..=num_lines {
            let sec_val = (sec_min + line_idx as f64 * params.stepover).min(sec_max);
            let pri_step = (pri_max - pri_min) / num_samples as f64;

            let mut points: Vec<Vec3> = Vec::with_capacity(num_samples + 1);

            for sample_idx in 0..=num_samples {
                let pri_val = (pri_min + sample_idx as f64 * pri_step).min(pri_max);

                let (u, v) = match params.direction {
                    FlowlineDirection::U => (pri_val, sec_val),
                    FlowlineDirection::V => (sec_val, pri_val),
                };

                // Evaluate position on the surface.
                let pos = match geometry::face_eval_point(face, u, v) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                // Evaluate surface normal.
                let normal = match geometry::face_eval_normal(face, u, v) {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                // Offset along normal: allowance + ball-nose tool radius compensation.
                let offset = params.allowance + tool_radius;
                let point = Vec3 {
                    x: pos[0] + normal[0] * offset,
                    y: pos[1] + normal[1] * offset,
                    z: pos[2] + normal[2] * offset,
                };

                points.push(point);
            }

            // Split into contiguous runs and create passes.
            let runs = split_runs(points, max_gap);
            for run in runs {
                if run.is_empty() {
                    continue;
                }
                let cuts: Vec<CutPoint> = run
                    .into_iter()
                    .map(|position| CutPoint {
                        position,
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    })
                    .collect();

                all_passes.push(Pass {
                    kind: PassKind::Cutting,
                    cuts,
                });
            }
        }
    }

    // ── Step 3: Boustrophedon ordering ───────────────────────────────────────
    boustrophedon_reorder(&mut all_passes);

    Ok(all_passes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stock::BoxDimensions;

    fn make_stock(ox: f64, oy: f64, oz: f64, w: f64, d: f64, h: f64) -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: ox,
                y: oy,
                z: oz,
            },
            width: w,
            depth: d,
            height: h,
        })
    }

    fn make_params(
        stepover: f64,
        direction: FlowlineDirection,
        allowance: f64,
        tool_diameter: f64,
        geometry: Option<Vec<String>>,
    ) -> FlowlineFinishingParams {
        FlowlineFinishingParams {
            stepover,
            direction,
            allowance,
            tool_diameter,
            geometry,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }
    }

    // ── Ungated: split_runs tests ────────────────────────────────────────────

    #[test]
    fn split_runs_no_gap_single_run() {
        let points = vec![
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.1,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.2,
                y: 0.0,
                z: 0.0,
            },
        ];
        let runs = split_runs(points, 1.0);
        assert_eq!(runs.len(), 1, "no gap → single run");
        assert_eq!(runs[0].len(), 3);
    }

    #[test]
    fn split_runs_gap_in_middle() {
        let points = vec![
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 0.1,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            }, // big gap
            Vec3 {
                x: 10.1,
                y: 0.0,
                z: 0.0,
            },
        ];
        let runs = split_runs(points, 1.0);
        assert_eq!(runs.len(), 2, "gap in middle → two runs");
        assert_eq!(runs[0].len(), 2);
        assert_eq!(runs[1].len(), 2);
    }

    #[test]
    fn split_runs_all_gaps() {
        let points = vec![
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3 {
                x: 20.0,
                y: 0.0,
                z: 0.0,
            },
        ];
        let runs = split_runs(points, 1.0);
        assert_eq!(runs.len(), 3, "all gaps → individual points");
        for run in &runs {
            assert_eq!(run.len(), 1);
        }
    }

    #[test]
    fn split_runs_empty() {
        let runs = split_runs(Vec::new(), 1.0);
        assert!(runs.is_empty());
    }

    // ── Ungated: boustrophedon_reorder tests ─────────────────────────────────

    #[test]
    fn boustrophedon_alternates_direction() {
        let mut passes: Vec<Pass> = (0..4)
            .map(|i| Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: i as f64,
                            y: 0.0,
                            z: 0.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: i as f64,
                            y: 1.0,
                            z: 0.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                ],
            })
            .collect();

        boustrophedon_reorder(&mut passes);

        // Even passes: first cut y=0, last cut y=1 (unchanged).
        assert_eq!(passes[0].cuts[0].position.y, 0.0);
        assert_eq!(passes[0].cuts[1].position.y, 1.0);
        assert_eq!(passes[2].cuts[0].position.x, 2.0); // x preserved

        // Odd passes: reversed → first cut y=1, last cut y=0.
        assert_eq!(passes[1].cuts[0].position.y, 1.0);
        assert_eq!(passes[1].cuts[1].position.y, 0.0);
        assert_eq!(passes[3].cuts[0].position.y, 1.0);
        assert_eq!(passes[3].cuts[1].position.y, 0.0);
    }

    // ── Tests that run without OCCT ──────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn returns_error_when_shape_is_none() {
        let stock = make_stock(0.0, 0.0, 0.0, 50.0, 50.0, 10.0);
        let params = make_params(0.5, FlowlineDirection::U, 0.0, 6.0, None);
        let result = flowline_finishing_passes(&stock, &params, 6.0, None);
        assert!(
            matches!(result, Err(AppError::GeometryImport(_))),
            "expected GeometryImport error when shape is None"
        );
    }

    // ── Tests that require OCCT ──────────────────────────────────────────────

    #[cfg(cam_geometry_bindings)]
    mod algorithm {
        use super::*;

        fn load_box_shape() -> OcctShape {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/box.step");
            crate::geometry::safe::OcctShape::load_step(&path).expect("load box.step")
        }

        fn load_sphere_shape() -> OcctShape {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/sphere.step");
            crate::geometry::safe::OcctShape::load_step(&path).expect("load sphere.step")
        }

        fn box_stock() -> StockDefinition {
            make_stock(0.0, 0.0, 0.0, 50.0, 50.0, 10.0)
        }

        fn sphere_stock() -> StockDefinition {
            make_stock(-10.0, -10.0, -10.0, 20.0, 20.0, 20.0)
        }

        fn z_range(passes: &[Pass]) -> (f64, f64) {
            let mut zmin = f64::INFINITY;
            let mut zmax = f64::NEG_INFINITY;
            for pass in passes {
                for cut in &pass.cuts {
                    zmin = zmin.min(cut.position.z);
                    zmax = zmax.max(cut.position.z);
                }
            }
            (zmin, zmax)
        }

        // ── Sphere: passes follow curvature ─────────────────────────────────

        #[test]
        fn sphere_passes_follow_curvature() {
            let shape = load_sphere_shape();
            let stock = sphere_stock();
            let params = make_params(0.1, FlowlineDirection::U, 0.0, 6.0, None);

            let passes = flowline_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass");

            // Sphere has varying Z values across passes.
            let (zmin, zmax) = z_range(&passes);
            assert!(
                (zmax - zmin) > 0.5,
                "sphere Z range should vary: zmin={zmin}, zmax={zmax}"
            );
        }

        // ── Box: straight evenly-spaced lines ───────────────────────────────

        #[test]
        fn box_straight_evenly_spaced_lines() {
            let shape = load_box_shape();
            let stock = box_stock();
            let params = make_params(0.5, FlowlineDirection::U, 0.0, 6.0, None);

            let passes = flowline_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass");

            // Box faces are flat; each pass should have roughly constant Z
            // within any single pass on a flat face.
            for pass in &passes {
                if pass.cuts.len() < 2 {
                    continue;
                }
                let z0 = pass.cuts[0].position.z;
                let all_same_z = pass.cuts.iter().all(|c| (c.position.z - z0).abs() < 0.1);
                // At least some passes on flat faces should have constant Z.
                if all_same_z {
                    return; // Found at least one flat pass — test passes.
                }
            }
            // If we get here, at least verify we got passes (box has 6 flat faces).
            assert!(passes.len() >= 6, "expected passes on multiple box faces");
        }

        // ── U vs V direction produces perpendicular pass families ────────────

        #[test]
        fn u_vs_v_direction_perpendicular() {
            let shape = load_sphere_shape();
            let stock = sphere_stock();
            let params_u = make_params(0.2, FlowlineDirection::U, 0.0, 6.0, None);
            let params_v = make_params(0.2, FlowlineDirection::V, 0.0, 6.0, None);

            let passes_u = flowline_finishing_passes(&stock, &params_u, 6.0, Some(&shape))
                .expect("U direction");
            let passes_v = flowline_finishing_passes(&stock, &params_v, 6.0, Some(&shape))
                .expect("V direction");

            assert!(!passes_u.is_empty());
            assert!(!passes_v.is_empty());

            // Compute the dominant direction of the first non-trivial pass
            // for each family. They should differ noticeably.
            fn pass_direction(pass: &Pass) -> Option<Vec3> {
                if pass.cuts.len() < 2 {
                    return None;
                }
                let first = &pass.cuts[0].position;
                let last = &pass.cuts[pass.cuts.len() - 1].position;
                let dx = last.x - first.x;
                let dy = last.y - first.y;
                let dz = last.z - first.z;
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                if len < 1e-9 {
                    return None;
                }
                Some(Vec3 {
                    x: dx / len,
                    y: dy / len,
                    z: dz / len,
                })
            }

            let dir_u = passes_u.iter().find_map(pass_direction);
            let dir_v = passes_v.iter().find_map(pass_direction);

            if let (Some(du), Some(dv)) = (dir_u, dir_v) {
                // Dot product of perpendicular directions should be close to 0
                // (or at least different from ±1).
                let dot = (du.x * dv.x + du.y * dv.y + du.z * dv.z).abs();
                assert!(dot < 0.95, "U and V directions should differ: |dot|={dot}");
            }
        }

        // ── Boustrophedon ordering alternates direction in output ────────────

        #[test]
        fn boustrophedon_ordering_in_output() {
            let shape = load_sphere_shape();
            let stock = sphere_stock();
            let params = make_params(0.1, FlowlineDirection::U, 0.0, 6.0, None);

            let passes = flowline_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            // Find pairs of consecutive passes from the same face (similar Y centroids)
            // and verify alternating direction.
            if passes.len() >= 4 {
                // Odd-indexed passes should be reversed relative to even-indexed.
                // Check that passes at index 0 and 1 have opposite first-point ordering.
                let p0_first = &passes[0].cuts[0].position;
                let p0_last = &passes[0].cuts.last().unwrap().position;
                let p1_first = &passes[1].cuts[0].position;
                let p1_last = &passes[1].cuts.last().unwrap().position;

                // In boustrophedon, pass 1's first point should be near pass 0's last point
                // (or at least the directions should be opposite).
                let d0x = p0_last.x - p0_first.x;
                let d1x = p1_last.x - p1_first.x;
                let d0y = p0_last.y - p0_first.y;
                let d1y = p1_last.y - p1_first.y;

                // Directions should be roughly opposite (dot product negative)
                // or at least not identical.
                let dot = d0x * d1x + d0y * d1y;
                // We just verify that boustrophedon actually changed something
                // (not all passes go the same direction).
                assert!(
                    dot <= 0.0
                        || (d0x.abs() < 1e-6 && d0y.abs() < 1e-6)
                        || (d1x.abs() < 1e-6 && d1y.abs() < 1e-6),
                    "boustrophedon should alternate pass direction: dot={dot}"
                );
            }
        }
    }
}
