//! Radial engagement computation for adaptive clearing.
//!
//! Determines the fraction of the tool diameter that is engaged with material
//! at a given position. This drives trochoidal loop insertion and feed rate
//! scaling.

use std::f64::consts::PI;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a polygon approximation of a circle.
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

/// Minimum distance from point `p` to the line segment `a`–`b`.
fn point_to_segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-18 {
        // Degenerate segment (a == b).
        let ex = p.0 - a.0;
        let ey = p.1 - a.1;
        return (ex * ex + ey * ey).sqrt();
    }
    let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = a.0 + t * dx;
    let proj_y = a.1 + t * dy;
    let ex = p.0 - proj_x;
    let ey = p.1 - proj_y;
    (ex * ex + ey * ey).sqrt()
}

/// Minimum distance from `point` to any edge of the polygon.
fn min_distance_to_boundary(point: (f64, f64), poly: &[(f64, f64)]) -> f64 {
    if poly.is_empty() {
        return f64::INFINITY;
    }
    let mut min_d = f64::INFINITY;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let d = point_to_segment_distance(point, a, b);
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

/// Winding-number point-in-polygon test. Returns true if `point` is inside
/// the closed polygon.
fn point_in_polygon(point: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut winding: i32 = 0;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if a.1 <= point.1 {
            if b.1 > point.1 {
                // Upward crossing.
                if cross(a, b, point) > 0.0 {
                    winding += 1;
                }
            }
        } else if b.1 <= point.1 {
            // Downward crossing.
            if cross(a, b, point) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

/// 2D cross product of vectors (b-a) and (p-a).
fn cross(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

/// Compute area of a simple polygon using the shoelace formula.
fn polygon_area(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += poly[i].0 * poly[j].1;
        area -= poly[j].0 * poly[i].1;
    }
    (area / 2.0).abs()
}

// ── Core ─────────────────────────────────────────────────────────────────────

/// Compute radial engagement fraction at a given tool position.
///
/// Returns a value in 0.0–1.0 representing the fraction of the tool diameter
/// engaged with `material_boundary`. A value of 0.0 means the tool is in free
/// air; 1.0 means fully embedded (slotting).
///
/// **Fast path:** Uses point-in-polygon and point-to-boundary-distance to
/// detect the trivial cases (tool completely outside or completely inside with
/// clearance) without any polygon boolean operations.
///
/// **Detailed path** (requires `cam_geometry_bindings`): Approximates the tool
/// circle as a 32-vertex polygon, intersects it with the material boundary via
/// Clipper2, and derives the engagement fraction from the area ratio.
pub fn compute_engagement(
    tool_center: (f64, f64),
    tool_radius: f64,
    material_boundary: &[(f64, f64)],
) -> f64 {
    if material_boundary.len() < 3 || tool_radius <= 0.0 {
        return 0.0;
    }

    let inside = point_in_polygon(tool_center, material_boundary);
    let dist = min_distance_to_boundary(tool_center, material_boundary);

    // Fast path: tool entirely outside material.
    if !inside && dist >= tool_radius {
        return 0.0;
    }

    // Fast path: tool entirely inside material (all of the tool circle fits).
    if inside && dist >= tool_radius {
        return 1.0;
    }

    // Detailed path — requires Clipper2 bindings.
    #[cfg(cam_geometry_bindings)]
    {
        compute_engagement_detailed(tool_center, tool_radius, material_boundary)
    }

    // Fallback without bindings: return 1.0 (conservative — assumes full
    // engagement so the caller will apply trochoidal loops).
    #[cfg(not(cam_geometry_bindings))]
    {
        1.0
    }
}

/// Detailed engagement via polygon intersection (Clipper2 required).
#[cfg(cam_geometry_bindings)]
fn compute_engagement_detailed(
    tool_center: (f64, f64),
    tool_radius: f64,
    material_boundary: &[(f64, f64)],
) -> f64 {
    use crate::geometry::clipper::{poly_boolean, BoolOp};

    const SEGMENTS: usize = 32;
    let tool_poly = circle_polygon(tool_center, tool_radius, SEGMENTS);
    let tool_area = PI * tool_radius * tool_radius;

    match poly_boolean(&tool_poly, material_boundary, BoolOp::Intersection) {
        Ok(intersection) => {
            let intersection_area = polygon_area(&intersection);
            (intersection_area / tool_area).clamp(0.0, 1.0)
        }
        // No intersection result means the polygons don't overlap at all.
        Err(_) => 0.0,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Large rectangle for test material.
    fn big_rect() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]
    }

    // ── Fast-path tests (no bindings required) ───────────────────────────

    #[test]
    fn fast_path_tool_clearly_outside() {
        let material = big_rect();
        // Tool center far from material.
        let e = compute_engagement((200.0, 200.0), 5.0, &material);
        assert!(
            (e - 0.0).abs() < 1e-9,
            "tool in free air should have 0 engagement, got {e}"
        );
    }

    #[test]
    fn fast_path_tool_clearly_inside() {
        let material = big_rect();
        // Tool center deep inside with lots of clearance.
        let e = compute_engagement((50.0, 50.0), 5.0, &material);
        assert!(
            (e - 1.0).abs() < 1e-9,
            "tool fully embedded should have 1.0 engagement, got {e}"
        );
    }

    #[test]
    fn empty_boundary_returns_zero() {
        assert!((compute_engagement((0.0, 0.0), 5.0, &[]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn zero_radius_returns_zero() {
        let material = big_rect();
        assert!((compute_engagement((50.0, 50.0), 0.0, &material) - 0.0).abs() < 1e-9);
    }

    // ── Helper tests ─────────────────────────────────────────────────────

    #[test]
    fn circle_polygon_correct_vertex_count() {
        let poly = circle_polygon((0.0, 0.0), 10.0, 32);
        assert_eq!(poly.len(), 32);
    }

    #[test]
    fn circle_polygon_vertices_on_circle() {
        let center = (5.0, 7.0);
        let r = 3.0;
        let poly = circle_polygon(center, r, 64);
        for (x, y) in &poly {
            let dist = ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt();
            assert!(
                (dist - r).abs() < 1e-10,
                "vertex ({x}, {y}) distance {dist} != radius {r}"
            );
        }
    }

    #[test]
    fn point_to_segment_basic() {
        // Point (1, 1) to segment (0,0)-(2,0) → distance 1.
        let d = point_to_segment_distance((1.0, 1.0), (0.0, 0.0), (2.0, 0.0));
        assert!((d - 1.0).abs() < 1e-10);
    }

    #[test]
    fn point_in_polygon_inside() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon((5.0, 5.0), &square));
    }

    #[test]
    fn point_in_polygon_outside() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(!point_in_polygon((15.0, 5.0), &square));
    }

    #[test]
    fn polygon_area_square() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!((polygon_area(&square) - 100.0).abs() < 1e-10);
    }

    // ── Detailed-path tests (Clipper2 bindings only) ─────────────────────

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detailed_tool_nearly_inside() {
        // Center at (4.9, 50), radius 5. Distance to x=0 edge is 4.9 < 5,
        // so the fast path doesn't apply and we go through Clipper2.
        // The circle barely clips outside → engagement very close to 1.0.
        let material = big_rect();
        let e = compute_engagement((4.9, 50.0), 5.0, &material);
        assert!(
            e > 0.95 && e <= 1.0,
            "tool nearly inside: expected >0.95, got {e}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detailed_tool_barely_overlapping() {
        // Center at (-4.9, 50), radius 5. Outside material, distance to x=0
        // edge is 4.9 < 5, so the fast path doesn't apply. The circle barely
        // clips inside → engagement very close to 0.0.
        let material = big_rect();
        let e = compute_engagement((-4.9, 50.0), 5.0, &material);
        assert!(
            e >= 0.0 && e < 0.05,
            "tool barely overlapping: expected <0.05, got {e}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detailed_tool_at_straight_wall() {
        // Tool center on the boundary edge x=0 of a rectangle.
        // Circle centered at (0, 50), radius 5. Material is [0,100]×[0,100].
        // Half the circle is inside → engagement ~0.5.
        let material = big_rect();
        let e = compute_engagement((0.0, 50.0), 5.0, &material);
        assert!(
            (e - 0.5).abs() < 0.05,
            "tool at straight wall: expected ~0.5, got {e}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detailed_tool_at_outside_corner() {
        // Tool center at corner (0, 0) of the rectangle.
        // Quarter of the circle is inside → engagement ~0.25.
        let material = big_rect();
        let e = compute_engagement((0.0, 0.0), 5.0, &material);
        assert!(
            (e - 0.25).abs() < 0.05,
            "tool at outside corner: expected ~0.25, got {e}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detailed_tool_half_in_half_out() {
        // Tool center at (2, 50), radius 5. Material is [0,100]×[0,100].
        // The circle extends 3 units outside x=0. Engagement should be > 0.5
        // but < 1.0.
        let material = big_rect();
        let e = compute_engagement((2.0, 50.0), 5.0, &material);
        assert!(
            e > 0.5 && e < 1.0,
            "tool partially inside: expected (0.5, 1.0), got {e}"
        );
    }
}
