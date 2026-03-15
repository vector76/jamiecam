//! Pencil milling algorithm.
//!
//! Traces concave corners and fillets by evaluating surface curvature on UV
//! grids. Points where the maximum principal curvature exceeds a threshold
//! (derived from the tool diameter) are grouped into connected components,
//! ordered into coherent curves, and offset along the surface normal to
//! produce cutting passes.

use crate::error::AppError;
use crate::geometry::OcctShape;
use crate::models::operation::PencilMillingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

// ── Public entry point ───────────────────────────────────────────────────────

/// Generate pencil milling passes for the given shape and params.
///
/// # Errors
/// - [`AppError::GeometryImport`] if no shape is loaded or OCCT is unavailable.
pub fn pencil_milling_passes(
    stock: &StockDefinition,
    params: &PencilMillingParams,
    _tool_diameter: f64,
    shape: Option<&OcctShape>,
) -> Result<Vec<Pass>, AppError> {
    let shape = shape
        .ok_or_else(|| AppError::GeometryImport("Shape required for pencil milling".into()))?;

    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (stock, params, shape);
        return Err(AppError::GeometryImport(
            "Shape required for pencil milling".into(),
        ));
    }

    #[cfg(cam_geometry_bindings)]
    {
        pencil_milling_inner(stock, params, shape)
    }
}

// ── Connected-component tracing ──────────────────────────────────────────────

/// Find connected components of `true` cells in a 2D boolean grid using BFS.
///
/// Each component is returned as a vector of `(row, col)` indices ordered by
/// walking adjacent cells to form a coherent curve.
fn trace_connected_components(grid: &[Vec<bool>]) -> Vec<Vec<(usize, usize)>> {
    let rows = grid.len();
    if rows == 0 {
        return Vec::new();
    }
    let cols = grid[0].len();
    if cols == 0 {
        return Vec::new();
    }

    let mut visited = vec![vec![false; cols]; rows];
    let mut components: Vec<Vec<(usize, usize)>> = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            if grid[r][c] && !visited[r][c] {
                // BFS to collect all cells in this component.
                let mut queue = std::collections::VecDeque::new();
                let mut cells = Vec::new();
                queue.push_back((r, c));
                visited[r][c] = true;

                while let Some((cr, cc)) = queue.pop_front() {
                    cells.push((cr, cc));

                    // 4-connected neighbors.
                    for (dr, dc) in [(-1i32, 0), (1, 0), (0, -1i32), (0, 1)] {
                        let nr = cr as i32 + dr;
                        let nc = cc as i32 + dc;
                        if nr >= 0 && nr < rows as i32 && nc >= 0 && nc < cols as i32 {
                            let (nr, nc) = (nr as usize, nc as usize);
                            if grid[nr][nc] && !visited[nr][nc] {
                                visited[nr][nc] = true;
                                queue.push_back((nr, nc));
                            }
                        }
                    }
                }

                // Order cells into a coherent curve by greedy nearest-neighbor walk.
                if cells.len() > 1 {
                    let ordered = order_cells_greedy(&cells);
                    components.push(ordered);
                } else {
                    components.push(cells);
                }
            }
        }
    }

    components
}

/// Order a set of grid cells into a coherent curve using greedy
/// nearest-neighbor traversal starting from the first cell.
fn order_cells_greedy(cells: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let n = cells.len();
    let mut used = vec![false; n];
    let mut ordered = Vec::with_capacity(n);

    // Start from the first cell.
    used[0] = true;
    ordered.push(cells[0]);

    for _ in 1..n {
        let (cr, cc) = *ordered.last().unwrap();
        let mut best_idx = None;
        let mut best_dist = f64::INFINITY;

        for (i, &(r, c)) in cells.iter().enumerate() {
            if used[i] {
                continue;
            }
            let dr = r as f64 - cr as f64;
            let dc = c as f64 - cc as f64;
            let dist = dr * dr + dc * dc;
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        if let Some(idx) = best_idx {
            used[idx] = true;
            ordered.push(cells[idx]);
        }
    }

    ordered
}

/// Remove passes whose total arc length is less than `min_length`.
fn filter_short_passes(passes: Vec<Pass>, min_length: f64) -> Vec<Pass> {
    passes
        .into_iter()
        .filter(|pass| {
            let length: f64 = pass
                .cuts
                .windows(2)
                .map(|w| {
                    let dx = w[1].position.x - w[0].position.x;
                    let dy = w[1].position.y - w[0].position.y;
                    let dz = w[1].position.z - w[0].position.z;
                    (dx * dx + dy * dy + dz * dz).sqrt()
                })
                .sum();
            length >= min_length
        })
        .collect()
}

// ── OCCT-dependent implementation ────────────────────────────────────────────

#[cfg(cam_geometry_bindings)]
fn pencil_milling_inner(
    _stock: &StockDefinition,
    params: &PencilMillingParams,
    shape: &OcctShape,
) -> Result<Vec<Pass>, AppError> {
    use crate::geometry::{self, OcctFace};

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

    // ── Step 2: Compute effective curvature threshold ────────────────────────
    let curvature_threshold = params
        .curvature_threshold
        .unwrap_or(2.0 / params.tool_diameter);

    let tool_radius = params.tool_diameter / 2.0;
    let offset = params.allowance + tool_radius;

    // ── Step 3: For each face, sample UV grid and find high-curvature points ──
    const GRID_SIZE: usize = 50;

    let mut all_passes: Vec<Pass> = Vec::new();

    for face in &selected_faces {
        let (umin, umax, vmin, vmax) = match geometry::face_uv_bounds(face) {
            Ok(bounds) => bounds,
            Err(_) => continue,
        };

        let u_span = umax - umin;
        let v_span = vmax - vmin;
        if u_span < 1e-12 || v_span < 1e-12 {
            continue;
        }

        // Sample curvature on a grid and mark high-curvature points.
        let mut concave_grid = vec![vec![false; GRID_SIZE]; GRID_SIZE];
        let mut any_concave = false;

        for (ir, row) in concave_grid.iter_mut().enumerate() {
            for (ic, cell) in row.iter_mut().enumerate() {
                let u = umin + u_span * (ir as f64) / ((GRID_SIZE - 1) as f64);
                let v = vmin + v_span * (ic as f64) / ((GRID_SIZE - 1) as f64);

                if let Ok(curv) = geometry::face_eval_curvature(face, u, v) {
                    let max_k = curv.k1.abs().max(curv.k2.abs());
                    if max_k > curvature_threshold {
                        *cell = true;
                        any_concave = true;
                    }
                }
            }
        }

        // Skip faces with no high-curvature regions.
        if !any_concave {
            continue;
        }

        // ── Step 4: Trace connected components ──────────────────────────────
        let components = trace_connected_components(&concave_grid);

        // ── Step 5: Build passes from traced curves ─────────────────────────
        for component in &components {
            if component.is_empty() {
                continue;
            }

            let mut cuts: Vec<CutPoint> = Vec::with_capacity(component.len());

            for &(ir, ic) in component {
                let u = umin + u_span * (ir as f64) / ((GRID_SIZE - 1) as f64);
                let v = vmin + v_span * (ic as f64) / ((GRID_SIZE - 1) as f64);

                let pos = match geometry::face_eval_point(face, u, v) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                // Offset along surface normal: allowance + tool radius compensation.
                let (px, py, pz) = if offset.abs() > f64::EPSILON {
                    if let Ok(n) = geometry::face_eval_normal(face, u, v) {
                        (
                            pos[0] + offset * n[0],
                            pos[1] + offset * n[1],
                            pos[2] + offset * n[2],
                        )
                    } else {
                        (pos[0], pos[1], pos[2])
                    }
                } else {
                    (pos[0], pos[1], pos[2])
                };

                cuts.push(CutPoint {
                    position: Vec3 {
                        x: px,
                        y: py,
                        z: pz,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: None,
                });
            }

            // Remove consecutive identical positions that arise when multiple
            // UV cells collapse to the same offset point (e.g. when hole
            // radius ≈ tool radius).
            cuts.dedup_by(|a, b| a.position == b.position);

            if !cuts.is_empty() {
                all_passes.push(Pass {
                    kind: PassKind::Cutting,
                    cuts,
                });
            }
        }
    }

    // ── Step 6: Filter short passes ─────────────────────────────────────────
    let passes = filter_short_passes(all_passes, params.min_pass_length);

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
        tool_diameter: f64,
        allowance: f64,
        min_pass_length: f64,
        curvature_threshold: Option<f64>,
        geometry: Option<Vec<String>>,
    ) -> PencilMillingParams {
        PencilMillingParams {
            allowance,
            tool_diameter,
            curvature_threshold,
            min_pass_length,
            geometry,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }
    }

    // ── Ungated: trace_connected_components tests ────────────────────────────

    #[test]
    fn trace_single_blob() {
        let grid = vec![
            vec![false, true, true],
            vec![false, true, false],
            vec![false, false, false],
        ];
        let components = trace_connected_components(&grid);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 3);
    }

    #[test]
    fn trace_multiple_blobs() {
        let grid = vec![
            vec![true, false, true],
            vec![false, false, false],
            vec![true, false, true],
        ];
        let components = trace_connected_components(&grid);
        // 4-connected: each corner is isolated.
        assert_eq!(components.len(), 4);
        for comp in &components {
            assert_eq!(comp.len(), 1);
        }
    }

    #[test]
    fn trace_diagonal_not_connected() {
        // Diagonal cells are NOT connected under 4-connectivity.
        let grid = vec![vec![true, false], vec![false, true]];
        let components = trace_connected_components(&grid);
        assert_eq!(components.len(), 2);
    }

    #[test]
    fn trace_empty_grid() {
        let grid = vec![vec![false, false], vec![false, false]];
        let components = trace_connected_components(&grid);
        assert!(components.is_empty());
    }

    #[test]
    fn trace_all_true_grid() {
        let grid = vec![vec![true, true], vec![true, true]];
        let components = trace_connected_components(&grid);
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 4);
    }

    #[test]
    fn trace_empty_rows() {
        let grid: Vec<Vec<bool>> = Vec::new();
        let components = trace_connected_components(&grid);
        assert!(components.is_empty());
    }

    // ── Ungated: filter_short_passes tests ──────────────────────────────────

    fn make_pass_with_length(length: f64) -> Pass {
        // Create a 2-point pass along X with the given length.
        Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                CutPoint {
                    position: Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: None,
                },
                CutPoint {
                    position: Vec3 {
                        x: length,
                        y: 0.0,
                        z: 0.0,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: None,
                },
            ],
        }
    }

    #[test]
    fn filter_removes_short_keeps_long() {
        let passes = vec![
            make_pass_with_length(1.0),
            make_pass_with_length(5.0),
            make_pass_with_length(0.5),
            make_pass_with_length(10.0),
        ];
        let filtered = filter_short_passes(passes, 2.0);
        assert_eq!(filtered.len(), 2);
        // The remaining passes should be the 5.0 and 10.0 length ones.
        assert!((filtered[0].cuts[1].position.x - 5.0).abs() < 1e-9);
        assert!((filtered[1].cuts[1].position.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn filter_keeps_all_when_none_short() {
        let passes = vec![make_pass_with_length(5.0), make_pass_with_length(10.0)];
        let filtered = filter_short_passes(passes, 1.0);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_removes_all_when_all_short() {
        let passes = vec![make_pass_with_length(0.1), make_pass_with_length(0.2)];
        let filtered = filter_short_passes(passes, 1.0);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_removes_single_point_pass() {
        // A pass with one cut has zero path length (windows(2) yields nothing).
        // It should be removed whenever min_length > 0.
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![CutPoint {
                position: Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: None,
            }],
        };
        let filtered = filter_short_passes(vec![pass], 1.0);
        assert!(
            filtered.is_empty(),
            "single-point pass has zero length and should be removed"
        );
    }

    // ── Tests without OCCT ──────────────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn returns_error_when_shape_is_none() {
        let stock = make_stock(0.0, 0.0, 0.0, 50.0, 50.0, 10.0);
        let params = make_params(6.0, 0.0, 1.0, None, None);
        let result = pencil_milling_passes(&stock, &params, 6.0, None);
        assert!(
            matches!(result, Err(AppError::GeometryImport(_))),
            "expected GeometryImport error when shape is None"
        );
    }

    // ── Tests that require OCCT ─────────────────────────────────────────────

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

        #[test]
        fn box_produces_zero_passes() {
            // A box has only flat faces — no concave corners to trace.
            let shape = load_box_shape();
            let stock = box_stock();
            let params = make_params(6.0, 0.0, 1.0, None, None);
            let passes =
                pencil_milling_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");
            assert!(
                passes.is_empty(),
                "flat box should produce zero pencil passes, got {}",
                passes.len()
            );
        }

        #[test]
        fn sphere_produces_passes() {
            // A sphere is curved everywhere — should produce passes.
            // Use an explicit low threshold since the default (2/diameter)
            // may exceed the sphere's curvature (1/radius).
            let shape = load_sphere_shape();
            let stock = sphere_stock();
            let params = make_params(6.0, 0.0, 0.1, Some(0.05), None);
            let passes =
                pencil_milling_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");
            assert!(
                !passes.is_empty(),
                "sphere should produce at least one pencil pass"
            );
        }

        #[test]
        fn higher_threshold_reduces_passes() {
            let shape = load_sphere_shape();
            let stock = sphere_stock();

            // Low threshold — more points exceed it.
            let params_low = make_params(6.0, 0.0, 0.1, Some(0.01), None);
            let passes_low = pencil_milling_passes(&stock, &params_low, 6.0, Some(&shape))
                .expect("low threshold should succeed");

            // High threshold — fewer points exceed it.
            let params_high = make_params(6.0, 0.0, 0.1, Some(10.0), None);
            let passes_high = pencil_milling_passes(&stock, &params_high, 6.0, Some(&shape))
                .expect("high threshold should succeed");

            let total_cuts_low: usize = passes_low.iter().map(|p| p.cuts.len()).sum();
            let total_cuts_high: usize = passes_high.iter().map(|p| p.cuts.len()).sum();

            assert!(
                total_cuts_high <= total_cuts_low,
                "higher threshold should produce fewer or equal cuts: low={total_cuts_low}, high={total_cuts_high}"
            );
        }

        #[test]
        fn min_pass_length_filters_short_passes() {
            let shape = load_sphere_shape();
            let stock = sphere_stock();

            // No minimum length filter.
            let params_no_min = make_params(6.0, 0.0, 0.0, None, None);
            let passes_no_min = pencil_milling_passes(&stock, &params_no_min, 6.0, Some(&shape))
                .expect("no-min should succeed");

            // Large minimum length filter.
            let params_large_min = make_params(6.0, 0.0, 1000.0, None, None);
            let passes_large_min =
                pencil_milling_passes(&stock, &params_large_min, 6.0, Some(&shape))
                    .expect("large-min should succeed");

            assert!(
                passes_large_min.len() <= passes_no_min.len(),
                "large min_pass_length should produce fewer or equal passes: no_min={}, large_min={}",
                passes_no_min.len(),
                passes_large_min.len()
            );
        }
    }
}
