//! Raster (parallel) finishing algorithm.
//!
//! Generates surface-following scan-line passes by projecting sampled points
//! onto the model faces. The pass direction is controlled by
//! `params.direction_angle_deg`; scan lines are spaced by `params.stepover`.
//! Alternate passes are reversed (boustrophedon ordering) to minimise linking
//! distance.

use crate::error::AppError;
use crate::geometry::{self, OcctFace, OcctShape};
use crate::models::operation::ParallelFinishingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

// ── Public entry point ───────────────────────────────────────────────────────

/// Generate parallel (raster) finishing passes for the given shape and params.
///
/// # Errors
/// - [`AppError::GeometryImport`] if no shape is loaded or OCCT is unavailable.
pub fn parallel_finishing_passes(
    stock: &StockDefinition,
    params: &ParallelFinishingParams,
    _tool_diameter: f64,
    shape: Option<&OcctShape>,
) -> Result<Vec<Pass>, AppError> {
    let shape = shape.ok_or_else(|| AppError::GeometryImport("no model loaded".into()))?;

    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (stock, params, shape);
        return Err(AppError::GeometryImport("OCCT not available".into()));
    }

    #[cfg(cam_geometry_bindings)]
    {
        parallel_finishing_inner(stock, params, shape)
    }
}

// ── OCCT-dependent implementation ────────────────────────────────────────────

#[cfg(cam_geometry_bindings)]
fn parallel_finishing_inner(
    stock: &StockDefinition,
    params: &ParallelFinishingParams,
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

    // ── Step 2: Compute XYZ bounding box ─────────────────────────────────────
    let StockDefinition::Box(b) = stock;
    let stock_top = b.origin.z + b.height;

    let mut xmin = b.origin.x;
    let mut xmax = b.origin.x + b.width;
    let mut ymin = b.origin.y;
    let mut ymax = b.origin.y + b.depth;

    const GRID: usize = 5;
    for face in &selected_faces {
        if let Ok((umin, umax, vmin, vmax)) = geometry::face_uv_bounds(face) {
            for iu in 0..GRID {
                for iv in 0..GRID {
                    let u = umin + (umax - umin) * (iu as f64) / ((GRID - 1) as f64);
                    let v = vmin + (vmax - vmin) * (iv as f64) / ((GRID - 1) as f64);
                    if let Ok([x, y, _z]) = geometry::face_eval_point(face, u, v) {
                        xmin = xmin.min(x);
                        xmax = xmax.max(x);
                        ymin = ymin.min(y);
                        ymax = ymax.max(y);
                    }
                }
            }
        }
    }

    // ── Step 3: Build rotated scan frame ─────────────────────────────────────
    let angle = params.direction_angle_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    // scan axis = (cos_a, sin_a), perp axis = (-sin_a, cos_a)

    let corners = [(xmin, ymin), (xmax, ymin), (xmax, ymax), (xmin, ymax)];
    let mut scan_min = f64::INFINITY;
    let mut scan_max = f64::NEG_INFINITY;
    let mut perp_min = f64::INFINITY;
    let mut perp_max = f64::NEG_INFINITY;

    for (cx, cy) in corners {
        let scan = cx * cos_a + cy * sin_a;
        let perp = -cx * sin_a + cy * cos_a;
        scan_min = scan_min.min(scan);
        scan_max = scan_max.max(scan);
        perp_min = perp_min.min(perp);
        perp_max = perp_max.max(perp);
    }

    // ── Step 4: Generate scan-line perp positions ────────────────────────────
    let stepover = params.stepover;
    if stepover <= 0.0 {
        return Ok(Vec::new());
    }

    let mut perp_positions: Vec<f64> = Vec::new();
    let mut p = perp_min;
    loop {
        perp_positions.push(p.min(perp_max));
        if p >= perp_max {
            break;
        }
        p += stepover;
    }

    // ── Steps 5-6: Sample each scan line, project onto faces, apply allowance ─
    let sample_spacing = stepover / 10.0;
    let tolerance = stepover * 2.0;
    // Keep the probe height strictly within the tolerance band so that
    // `dist < tolerance` is always satisfied for any face within reach.
    // Capping at 1.0 mm prevents the probe from being too far above the surface
    // for large-stepover passes.
    let probe_height = stepover.min(1.0);

    struct SurfacePoint {
        x: f64,
        y: f64,
        z: f64,
        scan_coord: f64,
    }

    let z_probe = stock_top + probe_height;
    let mut scan_lines: Vec<Vec<SurfacePoint>> = Vec::with_capacity(perp_positions.len());

    for perp_pos in &perp_positions {
        let mut line_points: Vec<SurfacePoint> = Vec::new();

        let mut scan = scan_min;
        loop {
            let s = scan.min(scan_max);
            // Convert scan/perp frame back to model XY.
            let x = s * cos_a - perp_pos * sin_a;
            let y = s * sin_a + perp_pos * cos_a;

            // Project onto all selected faces; keep the closest within tolerance.
            let mut best_dist = f64::INFINITY;
            let mut best_point: Option<(f64, f64, f64, f64, f64, usize)> = None; // (sx,sy,sz,u,v,fi)

            for (fi, face) in selected_faces.iter().enumerate() {
                if let Ok(([u, v], dist)) = geometry::face_project_point(face, x, y, z_probe) {
                    if dist < tolerance && dist < best_dist {
                        if let Ok(pts) = geometry::face_eval_point(face, u, v) {
                            best_dist = dist;
                            best_point = Some((pts[0], pts[1], pts[2], u, v, fi));
                        }
                    }
                }
            }

            if let Some((sx, sy, sz, u, v, fi)) = best_point {
                // Reject surface points that are above the stock top — these
                // come from vertical faces whose underlying surface extends above
                // the stock top when the probe is near an edge.
                if sz > stock_top + 1e-6 {
                    if s >= scan_max {
                        break;
                    }
                    scan += sample_spacing;
                    continue;
                }

                // Apply allowance offset along the surface normal.
                let (px, py, pz) = if params.allowance.abs() > f64::EPSILON {
                    if let Ok(n) = geometry::face_eval_normal(selected_faces[fi], u, v) {
                        (
                            sx + params.allowance * n[0],
                            sy + params.allowance * n[1],
                            sz + params.allowance * n[2],
                        )
                    } else {
                        (sx, sy, sz)
                    }
                } else {
                    (sx, sy, sz)
                };

                line_points.push(SurfacePoint {
                    x: px,
                    y: py,
                    z: pz,
                    scan_coord: s,
                });
            }

            if s >= scan_max {
                break;
            }
            scan += sample_spacing;
        }

        scan_lines.push(line_points);
    }

    // ── Step 7: Build contiguous sub-passes by grouping runs ─────────────────
    let gap_threshold = stepover * 3.0;

    // `perp_line_idx` is the scan-line index (0, 1, 2, …) used for
    // boustrophedon direction: all sub-passes on the same scan line share
    // this index so they all travel in the same direction even when a single
    // scan line is split into multiple runs by a surface hole.
    struct ScanPass {
        perp_line_idx: usize,
        pass: Pass,
    }

    let mut all_passes: Vec<ScanPass> = Vec::new();

    for (line_idx, mut pts) in scan_lines.into_iter().enumerate() {
        if pts.is_empty() {
            continue;
        }

        pts.sort_by(|a, b| a.scan_coord.partial_cmp(&b.scan_coord).unwrap());

        let mut run_start = 0usize;
        while run_start < pts.len() {
            let mut run_end = run_start + 1;
            while run_end < pts.len() {
                let dx = pts[run_end].x - pts[run_end - 1].x;
                let dy = pts[run_end].y - pts[run_end - 1].y;
                let dz = pts[run_end].z - pts[run_end - 1].z;
                let gap = (dx * dx + dy * dy + dz * dz).sqrt();
                if gap > gap_threshold {
                    break;
                }
                run_end += 1;
            }

            let cuts: Vec<CutPoint> = pts[run_start..run_end]
                .iter()
                .map(|sp| CutPoint {
                    position: Vec3 {
                        x: sp.x,
                        y: sp.y,
                        z: sp.z,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: None,
                })
                .collect();

            if !cuts.is_empty() {
                all_passes.push(ScanPass {
                    perp_line_idx: line_idx,
                    pass: Pass {
                        kind: PassKind::Cutting,
                        cuts,
                    },
                });
            }

            run_start = run_end;
        }
    }

    // ── Step 8: Boustrophedon ordering ───────────────────────────────────────
    // scan_lines was already built in ascending perp order, so all_passes is
    // already sorted by perp_line_idx.  The explicit sort is defensive.
    all_passes.sort_by_key(|sp| sp.perp_line_idx);

    // ── Step 9: Return passes ────────────────────────────────────────────────
    let passes: Vec<Pass> = all_passes
        .into_iter()
        .map(|mut sp| {
            if sp.perp_line_idx % 2 == 1 {
                sp.pass.cuts.reverse();
            }
            sp.pass
        })
        .collect();

    Ok(passes)
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
        direction_angle_deg: f64,
        allowance: f64,
        geometry: Option<Vec<String>>,
    ) -> ParallelFinishingParams {
        ParallelFinishingParams {
            stepover,
            direction_angle_deg,
            allowance,
            geometry,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }
    }

    // ── Tests that run without OCCT ──────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn returns_error_when_shape_is_none() {
        let stock = make_stock(0.0, 0.0, 0.0, 50.0, 50.0, 10.0);
        let params = make_params(2.0, 0.0, 0.0, None);
        let result = parallel_finishing_passes(&stock, &params, 6.0, None);
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

        // 1. Flat surface / basic.
        #[test]
        fn flat_surface_basic() {
            let shape = load_box_shape();
            let stock = box_stock();
            let params = make_params(2.0, 0.0, 0.0, None);

            let passes = parallel_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass");

            let StockDefinition::Box(ref b) = stock;
            let zmin_stock = b.origin.z;
            let zmax_stock = b.origin.z + b.height;
            let (zmin, zmax) = z_range(&passes);
            assert!(
                zmin >= zmin_stock - 1e-3,
                "z should be within stock range (below): zmin={zmin}, stock_zmin={zmin_stock}"
            );
            assert!(
                zmax <= zmax_stock + 1e-3,
                "z should be within stock range (above): zmax={zmax}, stock_zmax={zmax_stock}"
            );

            // Pass count ≈ depth / stepover ± 1.
            let expected_lines = (b.depth / 2.0).round() as usize;
            let pass_count = passes.len();
            assert!(
                pass_count >= expected_lines.saturating_sub(1),
                "too few passes: {pass_count}, expected ~{expected_lines}"
            );
            assert!(
                pass_count <= expected_lines + 1,
                "too many passes: {pass_count}, expected ~{expected_lines}"
            );
        }

        // 1b. Default (small) stepover produces passes.
        //
        // Regression: with probe_height = stock_top + 1.0 and tolerance =
        // stepover * 2.0 = 1.0 the strict `dist < tolerance` check
        // (1.0 < 1.0 = false) would silently return zero passes.
        #[test]
        fn default_small_stepover_produces_passes() {
            let shape = load_box_shape();
            let stock = box_stock();
            // Use the model-default stepover of 0.5 mm.
            let params = make_params(0.5, 0.0, 0.0, None);
            let passes = parallel_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");
            assert!(
                !passes.is_empty(),
                "default stepover=0.5 should produce passes, got 0"
            );
        }

        // 2. Stepover scaling.
        #[test]
        fn stepover_scaling() {
            let shape = load_box_shape();
            let stock = box_stock();

            let fine = parallel_finishing_passes(
                &stock,
                &make_params(1.0, 0.0, 0.0, None),
                6.0,
                Some(&shape),
            )
            .expect("fine should succeed");
            let coarse = parallel_finishing_passes(
                &stock,
                &make_params(5.0, 0.0, 0.0, None),
                6.0,
                Some(&shape),
            )
            .expect("coarse should succeed");

            assert!(
                fine.len() > coarse.len(),
                "finer stepover should produce more passes: fine={}, coarse={}",
                fine.len(),
                coarse.len()
            );
        }

        // 3. Allowance offset.
        #[test]
        fn allowance_offset() {
            let shape = load_box_shape();
            let stock = box_stock();

            let zero_passes = parallel_finishing_passes(
                &stock,
                &make_params(2.0, 0.0, 0.0, None),
                6.0,
                Some(&shape),
            )
            .expect("zero allowance should succeed");
            let allow_passes = parallel_finishing_passes(
                &stock,
                &make_params(2.0, 0.0, 0.1, None),
                6.0,
                Some(&shape),
            )
            .expect("allowance should succeed");

            let (zmin_zero, _) = z_range(&zero_passes);

            for pass in &allow_passes {
                for cut in &pass.cuts {
                    assert!(
                        cut.position.z >= zmin_zero - 1e-6,
                        "allowance point z={} below zero-run zmin={}",
                        cut.position.z,
                        zmin_zero
                    );
                }
            }
        }

        // 4. Curved surface (sphere).
        #[test]
        fn curved_surface_sphere() {
            let shape = load_sphere_shape();
            let stock = make_stock(-10.0, -10.0, -10.0, 20.0, 20.0, 20.0);
            let params = make_params(5.0, 0.0, 0.0, None);

            let passes = parallel_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass on sphere");

            let (zmin, zmax) = z_range(&passes);
            assert!(
                (zmax - zmin) > 1.0,
                "Z values should vary on a sphere (range={:.3})",
                zmax - zmin
            );
        }

        // 5. Direction 45°.
        #[test]
        fn direction_45_degrees() {
            let shape = load_box_shape();
            let stock = box_stock();

            let passes_0 = parallel_finishing_passes(
                &stock,
                &make_params(2.0, 0.0, 0.0, None),
                6.0,
                Some(&shape),
            )
            .expect("0° should succeed");
            let passes_45 = parallel_finishing_passes(
                &stock,
                &make_params(2.0, 45.0, 0.0, None),
                6.0,
                Some(&shape),
            )
            .expect("45° should succeed");

            assert!(!passes_45.is_empty(), "45° run should produce passes");

            // First pass of 45° run: both dx and dy should be non-zero and similar.
            if let Some(pass) = passes_45.first() {
                if pass.cuts.len() >= 2 {
                    let dx = (pass.cuts[1].position.x - pass.cuts[0].position.x).abs();
                    let dy = (pass.cuts[1].position.y - pass.cuts[0].position.y).abs();
                    assert!(dx > 1e-6, "expected non-zero dx in 45° first pass");
                    assert!(dy > 1e-6, "expected non-zero dy in 45° first pass");
                    let ratio = if dx > dy { dx / dy } else { dy / dx };
                    assert!(
                        ratio < 3.0,
                        "dx={dx:.4} and dy={dy:.4} should be similar magnitude for 45°"
                    );
                }
            }

            // First pass of 0° run: dy should be negligible compared to dx.
            if let Some(pass) = passes_0.first() {
                if pass.cuts.len() >= 2 {
                    let dx = (pass.cuts[1].position.x - pass.cuts[0].position.x).abs();
                    let dy = (pass.cuts[1].position.y - pass.cuts[0].position.y).abs();
                    if dx > 1e-6 {
                        assert!(
                            dy / dx < 0.1,
                            "0° pass should be primarily X-direction: dx={dx:.4}, dy={dy:.4}"
                        );
                    }
                }
            }
        }
    }
}
