//! Trochoidal adaptive clearing with multi-Z stepdown iteration.
//!
//! Generates cutting passes with trochoidal loop insertions where radial
//! engagement exceeds the optimal load. The top-level entry point
//! [`adaptive_clearing_passes`] iterates Z levels from the stock top down to
//! the target depth, delegating each level to [`adaptive_clear_at_z`].

use std::f64::consts::PI;

use crate::error::AppError;
use crate::geometry::{
    poly_boolean, poly_offset, shape_section_at_z, BoolOp, GeometryError, OcctShape,
};
use crate::models::operation::AdaptiveClearingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

use super::engagement::compute_engagement;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a polygon approximation of a circle (for swept-area computation).
fn circle_polygon(center: (f64, f64), radius: f64, segments: usize) -> Vec<(f64, f64)> {
    (0..segments)
        .map(|i| {
            let angle = 2.0 * PI * (i as f64) / (segments as f64);
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect()
}

/// Distance between two 2-D points.
fn dist2d(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    (dx * dx + dy * dy).sqrt()
}

/// Unit direction vector from `a` to `b`. Returns `None` for coincident points.
fn direction(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let d = dist2d(a, b);
    if d < 1e-12 {
        return None;
    }
    Some(((b.0 - a.0) / d, (b.1 - a.1) / d))
}

/// Generate a trochoidal loop as a series of short linear segments.
///
/// The loop is a circular arc detour that keeps radial engagement at or below
/// `optimal_load * tool_diameter`. The loop centre is offset perpendicular to
/// the cut direction.
fn trochoidal_loop(
    pos: (f64, f64),
    dir: (f64, f64),
    tool_radius: f64,
    optimal_load: f64,
    z: f64,
    base_feed: f64,
) -> Vec<CutPoint> {
    // Loop radius: sized so radial engagement stays at/below optimal_load.
    let loop_radius = tool_radius * (1.0 - optimal_load).max(0.1);

    // Perpendicular to cut direction (left-hand normal).
    let perp = (-dir.1, dir.0);

    // Loop centre is offset perpendicular to the path.
    let center = (pos.0 + perp.0 * loop_radius, pos.1 + perp.1 * loop_radius);

    let segments = 16usize;
    let start_angle = (pos.1 - center.1).atan2(pos.0 - center.0);

    // Trochoidal loops run at optimal_load engagement; scale feed accordingly.
    let feed = clamp_feed(base_feed, optimal_load, optimal_load);

    (0..=segments)
        .map(|i| {
            let angle = start_angle + 2.0 * PI * (i as f64) / (segments as f64);
            let x = center.0 + loop_radius * angle.cos();
            let y = center.1 + loop_radius * angle.sin();
            CutPoint {
                position: Vec3 { x, y, z },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: Some(feed),
            }
        })
        .collect()
}

/// Build a swept-area polygon for a tool moving from `a` to `b`.
fn swept_area(a: (f64, f64), b: (f64, f64), tool_radius: f64) -> Vec<(f64, f64)> {
    let dir = match direction(a, b) {
        Some(d) => d,
        None => return circle_polygon(a, tool_radius, 16),
    };

    let perp = (-dir.1 * tool_radius, dir.0 * tool_radius);
    let half_segments = 8usize;

    let mut pts = Vec::with_capacity(2 + 2 * half_segments);

    pts.push((a.0 + perp.0, a.1 + perp.1));
    pts.push((b.0 + perp.0, b.1 + perp.1));

    let start_b = perp.1.atan2(perp.0);
    for i in 1..half_segments {
        let angle = start_b - PI * (i as f64) / (half_segments as f64);
        pts.push((
            b.0 + tool_radius * angle.cos(),
            b.1 + tool_radius * angle.sin(),
        ));
    }

    pts.push((b.0 - perp.0, b.1 - perp.1));
    pts.push((a.0 - perp.0, a.1 - perp.1));

    let start_a = (-perp.1).atan2(-perp.0);
    for i in 1..half_segments {
        let angle = start_a - PI * (i as f64) / (half_segments as f64);
        pts.push((
            a.0 + tool_radius * angle.cos(),
            a.1 + tool_radius * angle.sin(),
        ));
    }

    pts
}

/// Compute clamped feed rate: `base_feed * (optimal_load / actual_engagement)`,
/// clamped to `[0.2 * base_feed, 1.5 * base_feed]`.
fn clamp_feed(base_feed: f64, optimal_load: f64, actual_engagement: f64) -> f64 {
    if actual_engagement <= 0.0 {
        // Zero engagement — caller should handle (rapid or skip).
        return base_feed;
    }
    let raw = base_feed * (optimal_load / actual_engagement);
    raw.clamp(0.2 * base_feed, 1.5 * base_feed)
}

// ── Multi-Z entry point ─────────────────────────────────────────────────────

/// Generate adaptive clearing passes across multiple Z levels.
///
/// Iterates from the stock top down to `stock_top_z - depth` in `stepdown`
/// increments, calling [`adaptive_clear_at_z`] at each level.
///
/// # Arguments
///
/// * `stock` — stock solid definition (used for top Z and XY boundary)
/// * `params` — adaptive clearing parameters (depth, stepdown, optimal_load, etc.)
/// * `tool_diameter` — cutter diameter in mm
/// * `shape` — optional 3-D shape for boundary extraction at each Z level
/// * `base_feed` — nominal feed rate in mm/min
///
/// # Errors
/// - [`AppError::InvalidInput`] if parameters are out of range.
pub fn adaptive_clearing_passes(
    stock: &StockDefinition,
    params: &AdaptiveClearingParams,
    tool_diameter: f64,
    shape: Option<&OcctShape>,
    base_feed: f64,
) -> Result<Vec<Pass>, AppError> {
    // ── Parameter validation ────────────────────────────────────────────
    if params.depth <= 0.0 {
        return Err(AppError::InvalidInput(
            "depth must be greater than zero".into(),
        ));
    }
    if params.stepdown <= 0.0 {
        return Err(AppError::InvalidInput(
            "stepdown must be greater than zero".into(),
        ));
    }
    if params.optimal_load <= 0.0 || params.optimal_load > 1.0 {
        return Err(AppError::InvalidInput(
            "optimal_load must be in range (0.0, 1.0]".into(),
        ));
    }
    if params.stepover_percent <= 0.0 || params.stepover_percent > 100.0 {
        return Err(AppError::InvalidInput(
            "stepover_percent must be in range (0, 100]".into(),
        ));
    }

    // ── Stock geometry ──────────────────────────────────────────────────
    let StockDefinition::Box(b) = stock;
    let stock_top_z = b.origin.z + b.height;

    let stock_boundary: Vec<(f64, f64)> = vec![
        (b.origin.x, b.origin.y),
        (b.origin.x + b.width, b.origin.y),
        (b.origin.x + b.width, b.origin.y + b.depth),
        (b.origin.x, b.origin.y + b.depth),
    ];

    let floor_z = stock_top_z - params.depth;
    let mut passes = Vec::new();
    let mut n = 0usize;

    // ── Stepdown iteration ──────────────────────────────────────────────
    loop {
        let z = (stock_top_z - n as f64 * params.stepdown).max(floor_z);

        let boundary: Vec<(f64, f64)> = if let Some(s) = shape {
            match shape_section_at_z(s, z) {
                Ok(loops) if loops.is_empty() => {
                    if z <= floor_z {
                        break;
                    }
                    n += 1;
                    continue;
                }
                Ok(loops) => loops[0].clone(),
                Err(GeometryError::NotImplemented) => stock_boundary.clone(),
                Err(e) => return Err(AppError::from(e)),
            }
        } else {
            stock_boundary.clone()
        };

        let level_passes = adaptive_clear_at_z(
            &boundary,
            z,
            tool_diameter,
            params.optimal_load,
            params.stepover_percent,
            base_feed,
        )?;
        passes.extend(level_passes);

        if z <= floor_z {
            break;
        }
        n += 1;
    }

    Ok(passes)
}

// ── Single-Z core ───────────────────────────────────────────────────────────

/// Generate adaptive clearing passes at a single Z level.
///
/// Produces cutting passes with trochoidal loop insertions where radial
/// engagement exceeds `optimal_load`. Each emitted [`CutPoint`] carries a
/// per-point `feed_rate_override` computed from the local engagement.
///
/// # Arguments
///
/// * `boundary` — closed 2-D polygon defining the region to clear
/// * `z` — Z height for all generated points
/// * `tool_diameter` — cutter diameter in mm
/// * `optimal_load` — target radial engagement fraction (0-1)
/// * `stepover_percent` — lateral step as percentage of tool diameter
/// * `base_feed` — nominal feed rate for per-point scaling
pub fn adaptive_clear_at_z(
    boundary: &[(f64, f64)],
    z: f64,
    tool_diameter: f64,
    optimal_load: f64,
    stepover_percent: f64,
    base_feed: f64,
) -> Result<Vec<Pass>, AppError> {
    let tool_radius = tool_diameter / 2.0;

    // Step 1: Offset boundary inward by tool radius to get machinable region.
    let machinable = match poly_offset(boundary, -tool_radius, 0.01) {
        Ok(m) => m,
        Err(_) => return Ok(Vec::new()), // Collapsed — no machinable area.
    };

    // Step 2: Generate concentric offset passes (pocket clearing pattern).
    let stepover = stepover_percent / 100.0 * tool_diameter;
    let mut contours: Vec<Vec<(f64, f64)>> = Vec::new();

    contours.push(machinable.clone());
    if stepover > f64::EPSILON {
        let mut current = machinable;
        while let Ok(next) = poly_offset(&current, -stepover, 0.01) {
            contours.push(next.clone());
            current = next;
        }
    }

    // Step 3: Initialize remaining-material polygon as the original boundary.
    let mut remaining = boundary.to_vec();

    // Step 4: Walk each contour, evaluate engagement, insert trochoidal loops,
    //         and compute per-point feed rate overrides.
    let mut passes = Vec::new();

    for contour in &contours {
        if contour.len() < 2 {
            continue;
        }

        let mut cuts: Vec<CutPoint> = Vec::new();

        for i in 0..contour.len() {
            let pos = contour[i];
            let next_pos = contour[(i + 1) % contour.len()];

            let dir = direction(pos, next_pos).unwrap_or((1.0, 0.0));

            let engagement = compute_engagement(pos, tool_radius, &remaining);

            if engagement <= 0.0 {
                // Zero engagement — material already cleared. Emit rapid traverse.
                cuts.push(CutPoint {
                    position: Vec3 {
                        x: pos.0,
                        y: pos.1,
                        z,
                    },
                    move_kind: MoveKind::Rapid,
                    tool_orientation: None,
                    feed_rate_override: None,
                });
            } else if engagement <= optimal_load {
                // Standard feed point with scaled feed rate.
                let feed = clamp_feed(base_feed, optimal_load, engagement);
                cuts.push(CutPoint {
                    position: Vec3 {
                        x: pos.0,
                        y: pos.1,
                        z,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                    feed_rate_override: Some(feed),
                });
            } else {
                // High engagement — insert trochoidal loop.
                let loop_pts = trochoidal_loop(pos, dir, tool_radius, optimal_load, z, base_feed);
                cuts.extend(loop_pts);
            }

            // Subtract swept area from remaining material.
            let swept = swept_area(pos, next_pos, tool_radius);
            if swept.len() >= 3 {
                if let Ok(diff) = poly_boolean(&remaining, &swept, BoolOp::Difference) {
                    if diff.len() >= 3 {
                        remaining = diff;
                    }
                }
            }
        }

        if !cuts.is_empty() {
            passes.push(Pass {
                kind: PassKind::Cutting,
                cuts,
            });
        }
    }

    Ok(passes)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stock::BoxDimensions;

    // ── Helpers ─────────────────────────────────────────────────────────

    fn make_box_stock(width: f64, depth: f64, height: f64) -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width,
            depth,
            height,
        })
    }

    fn make_params(
        depth: f64,
        stepdown: f64,
        optimal_load: f64,
        stepover_percent: f64,
    ) -> AdaptiveClearingParams {
        AdaptiveClearingParams {
            depth,
            stepdown,
            optimal_load,
            stepover_percent,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }
    }

    // ── Parameter validation tests (no cfg gating) ──────────────────────

    #[test]
    fn rejects_zero_depth() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(0.0, 2.0, 0.4, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_negative_depth() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(-1.0, 2.0, 0.4, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_zero_stepdown() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 0.0, 0.4, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_negative_stepdown() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, -1.0, 0.4, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_zero_optimal_load() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 0.0, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_optimal_load_over_one() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 1.1, 40.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn accepts_optimal_load_exactly_one() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 1.0, 40.0);
        // Should not error on validation (may still return empty if geometry collapses).
        let _ = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
    }

    #[test]
    fn rejects_zero_stepover_percent() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 0.4, 0.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_stepover_percent_over_100() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 0.4, 101.0);
        let result = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn accepts_stepover_percent_exactly_100() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 2.0, 0.4, 100.0);
        let _ = adaptive_clearing_passes(&stock, &params, 10.0, None, 1000.0);
    }

    // ── Feed rate clamping unit tests ───────────────────────────────────

    #[test]
    fn clamp_feed_at_optimal_load_returns_base() {
        let feed = clamp_feed(1000.0, 0.4, 0.4);
        assert!((feed - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn clamp_feed_low_engagement_caps_at_1_5x() {
        // Very low engagement → raw feed would be huge, clamped to 1.5x
        let feed = clamp_feed(1000.0, 0.4, 0.01);
        assert!((feed - 1500.0).abs() < 1e-6);
    }

    #[test]
    fn clamp_feed_high_engagement_floors_at_0_2x() {
        // Very high engagement → raw feed would be tiny, clamped to 0.2x
        let feed = clamp_feed(1000.0, 0.1, 10.0);
        assert!((feed - 200.0).abs() < 1e-6);
    }

    #[test]
    fn clamp_feed_zero_engagement_returns_base() {
        let feed = clamp_feed(1000.0, 0.4, 0.0);
        assert!((feed - 1000.0).abs() < 1e-6);
    }

    // ── Algorithm tests (require geometry bindings) ─────────────────────

    #[cfg(cam_geometry_bindings)]
    mod algorithm {
        use super::*;
        use crate::geometry::poly_offset;

        /// A 100x100 rectangle — wide open area.
        fn big_rect() -> Vec<(f64, f64)> {
            vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
        }

        /// A 20x20 rectangle — tight enough to force trochoidal loops at corners
        /// with a 10 mm tool.
        fn small_rect() -> Vec<(f64, f64)> {
            vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]
        }

        #[test]
        fn rectangular_region_produces_passes_with_trochoidal_at_corners() {
            let boundary = small_rect();
            let tool_diameter = 10.0;
            let optimal_load = 0.4;
            let stepover_percent = 40.0;

            let passes = adaptive_clear_at_z(
                &boundary,
                -5.0,
                tool_diameter,
                optimal_load,
                stepover_percent,
                1000.0,
            )
            .expect("should produce passes");

            assert!(!passes.is_empty(), "expected at least one pass");

            let total_points: usize = passes.iter().map(|p| p.cuts.len()).sum();
            let contour_vertices: usize = {
                let machinable = poly_offset(&boundary, -(tool_diameter / 2.0), 0.01).unwrap();
                let stepover = stepover_percent / 100.0 * tool_diameter;
                let mut count = machinable.len();
                let mut cur = machinable;
                while let Ok(next) = poly_offset(&cur, -stepover, 0.01) {
                    count += next.len();
                    cur = next;
                }
                count
            };

            assert!(
                total_points > contour_vertices,
                "trochoidal loops should add extra points: total={total_points}, base_contour={contour_vertices}"
            );
        }

        #[test]
        fn wide_open_area_produces_standard_linear_passes() {
            let boundary = big_rect();
            let tool_diameter = 6.0;
            let optimal_load = 0.5;
            let stepover_percent = 50.0;

            let passes = adaptive_clear_at_z(
                &boundary,
                -2.0,
                tool_diameter,
                optimal_load,
                stepover_percent,
                1500.0,
            )
            .expect("should produce passes");

            assert!(
                !passes.is_empty(),
                "expected non-empty passes for large region"
            );

            for pass in &passes {
                assert_eq!(pass.kind, PassKind::Cutting);
                assert!(!pass.cuts.is_empty());
            }
        }

        #[test]
        fn collapsed_region_returns_empty() {
            let boundary = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
            let passes = adaptive_clear_at_z(&boundary, 0.0, 50.0, 0.5, 50.0, 1000.0)
                .expect("collapsed region should return Ok(empty)");
            assert!(
                passes.is_empty(),
                "expected empty passes for collapsed region"
            );
        }

        #[test]
        fn narrow_slot_becomes_trochoidal() {
            let boundary = vec![(0.0, 0.0), (50.0, 0.0), (50.0, 12.0), (0.0, 12.0)];
            let tool_diameter = 10.0;
            let optimal_load = 0.3;

            let passes =
                adaptive_clear_at_z(&boundary, -1.0, tool_diameter, optimal_load, 40.0, 1000.0)
                    .expect("should not error");

            assert!(
                !passes.is_empty(),
                "narrow slot should still produce passes"
            );

            let total_points: usize = passes.iter().map(|p| p.cuts.len()).sum();
            let raw_vertices: usize = {
                let machinable = poly_offset(&boundary, -(tool_diameter / 2.0), 0.01).unwrap();
                machinable.len()
            };
            assert!(
                total_points > raw_vertices,
                "narrow slot should be mostly trochoidal: total={total_points}, base={raw_vertices}"
            );
        }

        #[test]
        fn all_z_values_match_requested() {
            let boundary = small_rect();
            let z = -3.5;
            let passes = adaptive_clear_at_z(&boundary, z, 8.0, 0.5, 50.0, 1000.0)
                .expect("should produce passes");

            for pass in &passes {
                for cut in &pass.cuts {
                    assert!(
                        (cut.position.z - z).abs() < 1e-12,
                        "z mismatch: expected {z}, got {}",
                        cut.position.z
                    );
                }
            }
        }

        // ── Per-point feed rate tests ───────────────────────────────────

        #[test]
        fn feed_rates_are_set_and_within_clamped_range() {
            let boundary = small_rect();
            let base_feed = 1000.0;
            let passes = adaptive_clear_at_z(&boundary, -1.0, 10.0, 0.4, 40.0, base_feed)
                .expect("should produce passes");

            let min_allowed = 0.2 * base_feed - 1e-6;
            let max_allowed = 1.5 * base_feed + 1e-6;

            for pass in &passes {
                for cut in &pass.cuts {
                    match cut.move_kind {
                        MoveKind::Feed => {
                            let feed = cut
                                .feed_rate_override
                                .expect("feed points must have feed_rate_override");
                            assert!(
                                feed >= min_allowed && feed <= max_allowed,
                                "feed {feed} out of clamped range [{min_allowed}, {max_allowed}]"
                            );
                        }
                        MoveKind::Rapid => {
                            assert!(
                                cut.feed_rate_override.is_none(),
                                "rapid points should not have feed_rate_override"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        #[test]
        fn feed_rates_vary_across_points() {
            // In a small rectangle, engagement varies between contours/positions
            // (corners vs straight segments), so feed rates should not all be
            // identical.
            let boundary = small_rect();
            let base_feed = 1000.0;
            let passes = adaptive_clear_at_z(&boundary, -1.0, 10.0, 0.4, 40.0, base_feed)
                .expect("should produce passes");

            let feeds: Vec<f64> = passes
                .iter()
                .flat_map(|p| p.cuts.iter())
                .filter_map(|c| c.feed_rate_override)
                .collect();

            assert!(feeds.len() >= 2, "expected multiple feed-rate points");

            let min = feeds.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = feeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (max - min) > 1e-6,
                "expected at least two distinct feed rates, but all are ~{min}"
            );
        }

        // ── Multi-Z stepdown tests ──────────────────────────────────────

        #[test]
        fn stepdown_produces_multiple_z_levels() {
            let stock = make_box_stock(50.0, 50.0, 10.0);
            let params = make_params(6.0, 2.0, 0.4, 40.0);
            let passes = adaptive_clearing_passes(&stock, &params, 6.0, None, 1000.0)
                .expect("should succeed");

            assert!(!passes.is_empty(), "expected non-empty passes");

            let mut z_set = std::collections::HashSet::new();
            for pass in &passes {
                for cut in &pass.cuts {
                    z_set.insert((cut.position.z * 1000.0) as i64);
                }
            }
            // stock_top_z=10, depth=6, stepdown=2 → z=10,8,6,4
            assert!(
                z_set.len() >= 4,
                "expected at least 4 Z levels, got {} ({z_set:?})",
                z_set.len()
            );
        }

        #[test]
        fn floor_z_always_machined_when_depth_not_multiple_of_stepdown() {
            let stock = make_box_stock(50.0, 50.0, 10.0);
            // depth=5, stepdown=2 → z=10,8,6,5 (floor clamped)
            let params = make_params(5.0, 2.0, 0.4, 40.0);
            let passes = adaptive_clearing_passes(&stock, &params, 6.0, None, 1000.0)
                .expect("should succeed");

            let floor_z_millis = ((10.0_f64 - 5.0) * 1000.0) as i64;
            let z_set: std::collections::HashSet<i64> = passes
                .iter()
                .flat_map(|p| p.cuts.iter())
                .map(|c| (c.position.z * 1000.0) as i64)
                .collect();
            assert!(
                z_set.contains(&floor_z_millis),
                "floor z=5 must be present; found z levels: {z_set:?}"
            );
        }

        #[test]
        fn pass_count_within_expected_bounds() {
            let stock = make_box_stock(50.0, 50.0, 10.0);
            let params = make_params(4.0, 2.0, 0.4, 40.0);
            let passes = adaptive_clearing_passes(&stock, &params, 8.0, None, 1000.0)
                .expect("should succeed");

            // 3 Z levels (10, 8, 6); each level has at least 1 pass.
            assert!(
                passes.len() >= 3,
                "expected at least 3 passes (one per Z level), got {}",
                passes.len()
            );
            // Upper bound: shouldn't be excessively many.
            assert!(
                passes.len() <= 100,
                "unexpectedly many passes: {}",
                passes.len()
            );
        }

        #[test]
        fn approximate_cutting_length_within_bounds() {
            let stock = make_box_stock(50.0, 50.0, 10.0);
            let params = make_params(4.0, 2.0, 0.4, 40.0);
            let passes = adaptive_clearing_passes(&stock, &params, 8.0, None, 1000.0)
                .expect("should succeed");

            // Compute total path length across all passes.
            let mut total_length = 0.0;
            for pass in &passes {
                for w in pass.cuts.windows(2) {
                    let dx = w[1].position.x - w[0].position.x;
                    let dy = w[1].position.y - w[0].position.y;
                    total_length += (dx * dx + dy * dy).sqrt();
                }
            }

            // Should be at least something (50x50 stock with 8mm tool).
            assert!(
                total_length > 10.0,
                "cutting length too short: {total_length}"
            );
            // And not absurdly long.
            assert!(
                total_length < 50000.0,
                "cutting length unreasonably long: {total_length}"
            );
        }
    }
}
