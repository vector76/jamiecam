//! Trochoidal adaptive clearing at a single Z level.
//!
//! Generates cutting passes with trochoidal loop insertions where radial
//! engagement exceeds the optimal load. Designed as a self-contained callable
//! unit for composition with Z-level roughing.

use std::f64::consts::PI;

use crate::error::AppError;
use crate::geometry::{poly_boolean, poly_offset, BoolOp};
use crate::models::Vec3;
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
///
/// `pos`       — current tool position on the base path
/// `dir`       — unit direction along the base path at `pos`
/// `tool_radius` — half the tool diameter
/// `optimal_load` — target engagement fraction (0–1)
/// `z`         — current Z height
///
/// Returns a vec of (x, y) points forming the trochoidal loop.
fn trochoidal_loop(
    pos: (f64, f64),
    dir: (f64, f64),
    tool_radius: f64,
    optimal_load: f64,
    z: f64,
) -> Vec<CutPoint> {
    // Loop radius: sized so radial engagement stays at/below optimal_load.
    // A smaller loop reduces engagement; scale by (1 - optimal_load) so that
    // at 50% optimal load we get a half-diameter loop.
    let loop_radius = tool_radius * (1.0 - optimal_load).max(0.1);

    // Perpendicular to cut direction (left-hand normal).
    let perp = (-dir.1, dir.0);

    // Loop centre is offset perpendicular to the path.
    let center = (pos.0 + perp.0 * loop_radius, pos.1 + perp.1 * loop_radius);

    // Tessellate the circle into short segments. Start from the position
    // closest to `pos` (angle pointing back toward the path) and sweep a
    // full 2π.
    let segments = 16usize;
    let start_angle = (pos.1 - center.1).atan2(pos.0 - center.0);

    (0..=segments)
        .map(|i| {
            let angle = start_angle + 2.0 * PI * (i as f64) / (segments as f64);
            let x = center.0 + loop_radius * angle.cos();
            let y = center.1 + loop_radius * angle.sin();
            CutPoint {
                position: Vec3 { x, y, z },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
            }
        })
        .collect()
}

/// Build a swept-area polygon for a tool moving from `a` to `b`.
///
/// This is the Minkowski sum of the segment with a circle of `tool_radius`,
/// approximated as a rectangle capped with semicircles.
fn swept_area(a: (f64, f64), b: (f64, f64), tool_radius: f64) -> Vec<(f64, f64)> {
    let dir = match direction(a, b) {
        Some(d) => d,
        None => return circle_polygon(a, tool_radius, 16),
    };

    let perp = (-dir.1 * tool_radius, dir.0 * tool_radius);
    let half_segments = 8usize;

    let mut pts = Vec::with_capacity(2 + 2 * half_segments);

    // Side from a to b (offset +perp)
    pts.push((a.0 + perp.0, a.1 + perp.1));
    pts.push((b.0 + perp.0, b.1 + perp.1));

    // Semicircle around b
    let start_b = perp.1.atan2(perp.0);
    for i in 1..half_segments {
        let angle = start_b - PI * (i as f64) / (half_segments as f64);
        pts.push((
            b.0 + tool_radius * angle.cos(),
            b.1 + tool_radius * angle.sin(),
        ));
    }

    // Side from b to a (offset -perp)
    pts.push((b.0 - perp.0, b.1 - perp.1));
    pts.push((a.0 - perp.0, a.1 - perp.1));

    // Semicircle around a
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

// ── Core ─────────────────────────────────────────────────────────────────────

/// Generate adaptive clearing passes at a single Z level.
///
/// Produces cutting passes with trochoidal loop insertions where radial
/// engagement exceeds `optimal_load`. The `base_feed` parameter is reserved
/// for future per-point feed scaling and is currently unused.
///
/// # Arguments
///
/// * `boundary` — closed 2-D polygon defining the region to clear
/// * `z` — Z height for all generated points
/// * `tool_diameter` — cutter diameter in mm
/// * `optimal_load` — target radial engagement fraction (0–1)
/// * `stepover_percent` — lateral step as percentage of tool diameter
/// * `base_feed` — nominal feed rate (reserved for future feed scaling)
pub fn adaptive_clear_at_z(
    boundary: &[(f64, f64)],
    z: f64,
    tool_diameter: f64,
    optimal_load: f64,
    stepover_percent: f64,
    _base_feed: f64,
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
    // We use the full boundary (not the machinable region) because the tool
    // center walks along the machinable contour but the cutter extends to the
    // original stock edge — engagement must be measured against actual material.
    let mut remaining = boundary.to_vec();

    // Step 4: Walk each contour, evaluate engagement, insert trochoidal loops.
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

            if engagement <= optimal_load {
                // Standard feed point.
                cuts.push(CutPoint {
                    position: Vec3 {
                        x: pos.0,
                        y: pos.1,
                        z,
                    },
                    move_kind: MoveKind::Feed,
                    tool_orientation: None,
                });
            } else {
                // High engagement — insert trochoidal loop.
                let loop_pts = trochoidal_loop(pos, dir, tool_radius, optimal_load, z);
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
                // If boolean fails (e.g., fully consumed), remaining stays as-is
                // or becomes degenerate — engagement will read 0 for subsequent points.
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
#[cfg(cam_geometry_bindings)]
mod tests {
    use super::*;

    /// A 100×100 rectangle — wide open area.
    fn big_rect() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
    }

    /// A 20×20 rectangle — tight enough to force trochoidal loops at corners
    /// with a 10 mm tool.
    fn small_rect() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]
    }

    #[test]
    fn rectangular_region_produces_passes_with_trochoidal_at_corners() {
        let boundary = small_rect();
        let tool_diameter = 10.0;
        let optimal_load = 0.4; // 40% — corners will exceed this
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

        // Count total cut points — trochoidal loops add extra points beyond
        // the base contour vertex count, so total should exceed the number of
        // contour vertices.
        let total_points: usize = passes.iter().map(|p| p.cuts.len()).sum();
        let contour_vertices: usize = {
            // Reproduce the concentric offset count for comparison.
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
        // 100×100 with a 6 mm tool — centre is far from all walls,
        // so inner contours should have engagement <= optimal_load for most
        // points (engagement = 1.0 only for the outermost contour segments
        // near corners, but the majority of the interior is linear).
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

        // All passes should be Cutting kind.
        for pass in &passes {
            assert_eq!(pass.kind, PassKind::Cutting);
            assert!(!pass.cuts.is_empty());
        }
    }

    #[test]
    fn collapsed_region_returns_empty() {
        // Tool diameter 50 on a 10×10 region — tool radius 25 > half-width 5.
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
        // 50×12 slot with 10 mm tool — after offset by radius 5, only 2 mm
        // remains in Y, producing a very narrow machinable strip. The tool
        // is always near both walls so engagement is high → most or all points
        // should trigger trochoidal loops.
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

        // In a narrow slot the machinable contour has few vertices but high
        // engagement everywhere, so trochoidal loops should inflate the point
        // count well beyond the raw contour vertex count.
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

    #[test]
    fn zero_stepover_produces_single_contour_without_hanging() {
        // stepover_percent = 0 → stepover = 0; must not loop infinitely.
        let boundary = small_rect();
        let passes = adaptive_clear_at_z(&boundary, -1.0, 8.0, 0.5, 0.0, 1000.0)
            .expect("zero stepover should not error");

        // Should still produce at least the outermost contour.
        assert!(
            !passes.is_empty(),
            "expected at least one pass from the outermost contour"
        );
    }
}
