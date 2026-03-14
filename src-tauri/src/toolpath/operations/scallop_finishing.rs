//! Scallop finishing algorithm.
//!
//! Generates surface-following scan-line passes with **adaptive stepover**
//! based on local surface curvature. At each prospective row position the
//! algorithm samples curvature, computes the stepover required to achieve the
//! requested scallop height, and clamps it to `[min_stepover, max_stepover]`.
//!
//! The pass direction is controlled by `params.direction_angle_deg`; alternate
//! passes are reversed (boustrophedon ordering) to minimise linking distance.

use crate::error::AppError;
use crate::geometry::{self, OcctFace, OcctShape};
use crate::models::operation::ScallopFinishingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

// ── Public entry point ───────────────────────────────────────────────────────

/// Generate scallop finishing passes for the given shape and params.
///
/// # Errors
/// - [`AppError::GeometryImport`] if no shape is loaded or OCCT is unavailable.
pub fn scallop_finishing_passes(
    stock: &StockDefinition,
    params: &ScallopFinishingParams,
    _tool_diameter: f64,
    shape: Option<&OcctShape>,
) -> Result<Vec<Pass>, AppError> {
    let shape = shape
        .ok_or_else(|| AppError::GeometryImport("Shape required for scallop finishing".into()))?;

    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (stock, params, shape);
        return Err(AppError::GeometryImport(
            "Shape required for scallop finishing".into(),
        ));
    }

    #[cfg(cam_geometry_bindings)]
    {
        scallop_finishing_inner(stock, params, shape)
    }
}

// ── Adaptive stepover calculation ────────────────────────────────────────────

/// Compute the stepover for a given scallop height and curvature.
///
/// `max_abs_curvature` is the maximum |k| sampled along the prospective row.
/// Returns the stepover distance (not clamped).
fn compute_stepover_for_curvature(
    tool_radius: f64,
    target_scallop_height: f64,
    max_abs_curvature: f64,
) -> f64 {
    let h = target_scallop_height;
    if max_abs_curvature.abs() < 1e-9 {
        // Flat surface: classic scallop formula.
        let disc = 2.0 * tool_radius * h - h * h;
        if disc <= 0.0 {
            return 0.0;
        }
        return 2.0 * disc.sqrt();
    }

    let surface_radius = 1.0 / max_abs_curvature;

    // Effective cutting radius depends on relative curvature of tool vs surface.
    // When surface_radius > tool_radius (gentle curvature), the concave formula
    // gives a larger effective radius → wider stepover.
    // When surface_radius <= tool_radius (tight curvature), the convex formula
    // gives a smaller effective radius → tighter stepover (conservative).
    let effective_radius = if surface_radius > tool_radius {
        tool_radius * surface_radius / (surface_radius - tool_radius)
    } else {
        tool_radius * surface_radius / (tool_radius + surface_radius)
    };

    let disc = 2.0 * effective_radius * h - h * h;
    if disc <= 0.0 {
        return 0.0;
    }
    2.0 * disc.sqrt()
}

// ── OCCT-dependent implementation ────────────────────────────────────────────

#[cfg(cam_geometry_bindings)]
fn scallop_finishing_inner(
    stock: &StockDefinition,
    params: &ScallopFinishingParams,
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

    // ── Step 4: Generate scan rows with adaptive spacing ─────────────────────
    if params.max_stepover <= 0.0 || params.min_stepover <= 0.0 {
        return Ok(Vec::new());
    }

    let mut perp_positions: Vec<f64> = Vec::new();
    let mut perp = perp_min;

    // Number of curvature sample points along each prospective row.
    const CURVATURE_SAMPLES: usize = 5;
    let curvature_tolerance = params.max_stepover * 2.0;
    let curvature_probe_z = stock_top + params.max_stepover.min(1.0);

    loop {
        perp_positions.push(perp.min(perp_max));
        if perp >= perp_max {
            break;
        }

        // Sample curvature along the prospective next row to decide stepover.
        let next_perp_approx = (perp + params.min_stepover).min(perp_max);
        let mut max_abs_k: f64 = 0.0;

        for si in 0..CURVATURE_SAMPLES {
            let s = scan_min
                + (scan_max - scan_min) * (si as f64) / ((CURVATURE_SAMPLES - 1).max(1) as f64);
            let x = s * cos_a - next_perp_approx * sin_a;
            let y = s * sin_a + next_perp_approx * cos_a;

            // Project onto closest face and evaluate curvature.
            for face in &selected_faces {
                if let Ok(([u, v], dist)) =
                    geometry::face_project_point(face, x, y, curvature_probe_z)
                {
                    if dist < curvature_tolerance {
                        if let Ok(curv) = geometry::face_eval_curvature(face, u, v) {
                            let k = curv.k1.abs().max(curv.k2.abs());
                            max_abs_k = max_abs_k.max(k);
                        }
                    }
                }
            }
        }

        let stepover = compute_stepover_for_curvature(
            params.tool_radius,
            params.target_scallop_height,
            max_abs_k,
        )
        .clamp(params.min_stepover, params.max_stepover);

        perp += stepover;
    }

    // ── Steps 5-6: Sample each scan line, project onto faces, apply allowance ─
    let sample_spacing = params.min_stepover / 10.0;
    let tolerance = params.max_stepover * 2.0;
    let probe_height = params.max_stepover.min(1.0);

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
            let x = s * cos_a - perp_pos * sin_a;
            let y = s * sin_a + perp_pos * cos_a;

            let mut best_dist = f64::INFINITY;
            let mut best_point: Option<(f64, f64, f64, f64, f64, usize)> = None;

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
                if sz > stock_top + 1e-6 {
                    if s >= scan_max {
                        break;
                    }
                    scan += sample_spacing;
                    continue;
                }

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
    let gap_threshold = params.max_stepover * 3.0;

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
    all_passes.sort_by_key(|sp| sp.perp_line_idx);

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
        target_scallop_height: f64,
        min_stepover: f64,
        max_stepover: f64,
        direction_angle_deg: f64,
        allowance: f64,
        tool_radius: f64,
        geometry: Option<Vec<String>>,
    ) -> ScallopFinishingParams {
        ScallopFinishingParams {
            target_scallop_height,
            min_stepover,
            max_stepover,
            direction_angle_deg,
            allowance,
            tool_radius,
            geometry,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }
    }

    // ── Unit tests for stepover computation ──────────────────────────────────

    #[test]
    fn flat_surface_stepover() {
        // For a flat surface (k=0), scallop formula: 2*sqrt(2*R*h - h^2)
        let tool_r = 3.0;
        let h = 0.01;
        let expected = 2.0 * (2.0_f64 * tool_r * h - h * h).sqrt();
        let result = compute_stepover_for_curvature(tool_r, h, 0.0);
        assert!(
            (result - expected).abs() < 1e-9,
            "flat stepover: got {result}, expected {expected}"
        );
    }

    #[test]
    fn high_curvature_smaller_stepover() {
        // High curvature (surface_radius=2 < tool_radius=3) → convex branch
        // effective_r = 3*2/(3+2) = 1.2 < tool_r → smaller stepover.
        let tool_r = 3.0;
        let h = 0.01;
        let flat = compute_stepover_for_curvature(tool_r, h, 0.0);
        let curved = compute_stepover_for_curvature(tool_r, h, 0.5); // surface_radius=2
        assert!(
            curved < flat,
            "high-curvature stepover ({curved}) should be less than flat ({flat})"
        );
    }

    #[test]
    fn low_curvature_larger_stepover() {
        // Low curvature (surface_radius=10 > tool_radius=3) → concave branch
        // effective_r = 3*10/(10-3) = 30/7 ≈ 4.29 > tool_r → larger stepover.
        let tool_r = 3.0;
        let h = 0.01;
        let flat = compute_stepover_for_curvature(tool_r, h, 0.0);
        let curved = compute_stepover_for_curvature(tool_r, h, 0.1); // surface_radius=10
        assert!(
            curved > flat,
            "low-curvature stepover ({curved}) should be greater than flat ({flat})"
        );
    }

    #[test]
    fn zero_scallop_height_returns_zero() {
        let result = compute_stepover_for_curvature(3.0, 0.0, 0.0);
        assert!(
            result.abs() < 1e-9,
            "zero scallop height should give zero stepover"
        );
    }

    // ── Tests that run without OCCT ──────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn returns_error_when_shape_is_none() {
        let stock = make_stock(0.0, 0.0, 0.0, 50.0, 50.0, 10.0);
        let params = make_params(0.01, 0.2, 3.0, 0.0, 0.0, 3.0, None);
        let result = scallop_finishing_passes(&stock, &params, 6.0, None);
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

        #[test]
        fn flat_surface_produces_passes() {
            let shape = load_box_shape();
            let stock = box_stock();
            let params = make_params(0.01, 0.5, 3.0, 0.0, 0.0, 3.0, None);

            let passes = scallop_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass");

            let StockDefinition::Box(ref b) = stock;
            let stock_top = b.origin.z + b.height;
            let (zmin, zmax) = z_range(&passes);
            assert!(zmin >= b.origin.z - 1e-3);
            assert!(zmax <= stock_top + 1e-3);
        }

        #[test]
        fn curved_surface_produces_passes() {
            let shape = load_sphere_shape();
            let stock = make_stock(-10.0, -10.0, -10.0, 20.0, 20.0, 20.0);
            let params = make_params(0.01, 0.5, 5.0, 0.0, 0.0, 3.0, None);

            let passes = scallop_finishing_passes(&stock, &params, 6.0, Some(&shape))
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected at least one pass on sphere");

            let (zmin, zmax) = z_range(&passes);
            assert!(
                (zmax - zmin) > 1.0,
                "Z values should vary on a sphere (range={:.3})",
                zmax - zmin
            );
        }
    }
}
