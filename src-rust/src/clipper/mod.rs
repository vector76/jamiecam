//! Polygon offset facade over `clipper2-rust`.
//!
//! This module exposes a single narrow entry point — [`offset_region`] — for
//! inflating or deflating a closed polygon (with optional holes) by a signed
//! distance. Every other knob is pinned to a project-wide default so the
//! Mode 2 pipeline (parser → planner → emitter) cannot drift into inconsistent
//! offset semantics.
//!
//! # Project-wide defaults
//!
//! - **Units**: millimetres. Inputs and outputs use the [`crate::geometry2d`]
//!   types, whose unit contract is mm. Clipper2's internal integer scaling is
//!   driven by [`PRECISION`] and is not exposed.
//! - **Fill rule**: [`FillRule::EvenOdd`]. Applied to a pre-offset union that
//!   cleans up self-touching / self-intersecting inputs (e.g. a figure-8),
//!   and to a post-offset union that organises the result into a
//!   hole-aware [`Region`] hierarchy.
//! - **Join style**: [`JoinType::Miter`] with miter limit [`MITER_LIMIT`].
//!   Sharp acute corners whose miter would extend beyond the limit fall back
//!   to a bevel.
//! - **End style**: [`EndType::Polygon`] — every input ring is treated as a
//!   closed polygon. This facade does not offset open polylines.
//!
//! Anything outside that envelope (round joins, non-zero fill, open-path
//! offsetting, polytree access, integer paths) should reach for
//! `clipper2-rust` directly rather than grow this module.

use crate::geometry2d::{Point2, Region};
use clipper2_rust::{
    boolean_op_tree_d, inflate_paths_d, union_subjects_d, ClipType, EndType, FillRule, JoinType,
    PathD, PathsD, Point, PolyTreeD,
};

/// Decimal-digit precision used when converting our mm `f64` coordinates to
/// clipper2's internal `i64` representation. 6 digits = 1 nanometre resolution,
/// which is well below any meaningful machining tolerance and leaves headroom
/// in the `i64` range for parts up to several kilometres across.
const PRECISION: i32 = 6;

/// Miter limit ratio passed to clipper2. Matches the upstream default.
const MITER_LIMIT: f64 = 2.0;

/// Arc tolerance is unused under [`JoinType::Miter`] — kept here so the call
/// site reads with named arguments rather than a bare `0.0`.
const ARC_TOLERANCE: f64 = 0.0;

/// Inflate (`delta_mm > 0`) or deflate (`delta_mm < 0`) `region` by the given
/// signed distance in millimetres. Returns a list of regions, since an offset
/// can split one input region into several (deflating a thin neck) or merge
/// disjoint loops in a self-touching input (inflating a figure-8 by enough
/// to close the pinch).
///
/// A `delta_mm` of `0.0` returns the input *as interpreted under even-odd*:
/// self-intersections are resolved into disjoint regions and orientation is
/// normalised. For a well-formed input (non-self-intersecting exterior, holes
/// strictly inside) this is geometrically equivalent to the input.
pub fn offset_region(region: &Region, delta_mm: f64) -> Vec<Region> {
    let subject = region_to_paths_d(region);
    if subject.is_empty() {
        return Vec::new();
    }

    // Pre-clean: union with even-odd resolves self-intersections (figure-8)
    // and normalises orientation so the offset op sees a well-formed polygon
    // set regardless of how the caller wound the input rings.
    let cleaned = union_subjects_d(&subject, FillRule::EvenOdd, PRECISION);
    if cleaned.is_empty() {
        return Vec::new();
    }

    let inflated = inflate_paths_d(
        &cleaned,
        delta_mm,
        JoinType::Miter,
        EndType::Polygon,
        MITER_LIMIT,
        PRECISION,
        ARC_TOLERANCE,
    );
    if inflated.is_empty() {
        return Vec::new();
    }

    paths_d_to_regions(&inflated)
}

fn region_to_paths_d(region: &Region) -> PathsD {
    // A region with no meaningful exterior is empty — feeding only holes to the
    // offsetter would (mis-)treat them as outer rings under even-odd union.
    if region.exterior.len() < 3 {
        return PathsD::new();
    }
    let mut paths = PathsD::with_capacity(1 + region.holes.len());
    paths.push(points_to_path_d(&region.exterior));
    for hole in &region.holes {
        if hole.len() >= 3 {
            paths.push(points_to_path_d(hole));
        }
    }
    paths
}

fn points_to_path_d(points: &[Point2]) -> PathD {
    points.iter().map(|p| Point::new(p.x, p.y)).collect()
}

fn path_d_to_points(path: &PathD) -> Vec<Point2> {
    path.iter().map(|p| Point2::new(p.x, p.y)).collect()
}

/// Walk a clipper PolyTree (even-odd union of the offset result) to recover a
/// flat list of [`Region`]s. Top-level children of the root are outer rings;
/// their direct children are holes of that region; any grandchildren begin
/// new regions nested inside those holes.
fn paths_d_to_regions(paths: &PathsD) -> Vec<Region> {
    let mut tree = PolyTreeD::new();
    boolean_op_tree_d(
        ClipType::Union,
        FillRule::EvenOdd,
        paths,
        &PathsD::new(),
        &mut tree,
        PRECISION,
    );

    let mut regions = Vec::new();
    let root_children: Vec<usize> = tree.root().children().to_vec();
    for outer_idx in root_children {
        collect_region(&tree, outer_idx, &mut regions);
    }
    regions
}

fn collect_region(tree: &PolyTreeD, outer_idx: usize, regions: &mut Vec<Region>) {
    let outer = &tree.nodes[outer_idx];
    let exterior = path_d_to_points(outer.polygon());
    let mut holes: Vec<Vec<Point2>> = Vec::new();
    let mut nested_outers: Vec<usize> = Vec::new();
    for &hole_idx in outer.children() {
        let hole = &tree.nodes[hole_idx];
        holes.push(path_d_to_points(hole.polygon()));
        for &grandchild in hole.children() {
            nested_outers.push(grandchild);
        }
    }
    regions.push(Region::with_holes(exterior, holes));
    for nested in nested_outers {
        collect_region(tree, nested, regions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed shoelace area of a closed ring (CCW positive).
    fn ring_area(points: &[Point2]) -> f64 {
        let n = points.len();
        if n < 3 {
            return 0.0;
        }
        let mut acc = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            acc += points[i].x * points[j].y - points[j].x * points[i].y;
        }
        acc * 0.5
    }

    /// Net signed area of a region (|exterior| − Σ|holes|), orientation-agnostic.
    fn region_area(region: &Region) -> f64 {
        let mut a = ring_area(&region.exterior).abs();
        for hole in &region.holes {
            a -= ring_area(hole).abs();
        }
        a
    }

    fn total_area(regions: &[Region]) -> f64 {
        regions.iter().map(region_area).sum()
    }

    fn unit_square() -> Region {
        Region::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.0, 1.0),
        ])
    }

    #[test]
    fn unit_square_inflated_by_one_mm_is_three_by_three() {
        let regions = offset_region(&unit_square(), 1.0);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].hole_count(), 0);
        // Original 1×1 grows by 1 mm on every side under miter joins at 90°
        // ⇒ 3×3 = 9 mm². Tolerance covers integer-scaling rounding.
        assert!(
            (region_area(&regions[0]) - 9.0).abs() < 1e-3,
            "expected ~9 mm², got {}",
            region_area(&regions[0]),
        );
    }

    #[test]
    fn unit_square_deflated_by_point_three_mm_is_point_four_squared() {
        let regions = offset_region(&unit_square(), -0.3);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].hole_count(), 0);
        // 1×1 shrunk by 0.3 mm per side ⇒ 0.4×0.4 = 0.16 mm².
        assert!(
            (region_area(&regions[0]) - 0.16).abs() < 1e-3,
            "expected ~0.16 mm², got {}",
            region_area(&regions[0]),
        );
    }

    #[test]
    fn unit_square_deflated_past_inradius_vanishes() {
        // A 1×1 square has an inradius of 0.5; offsetting by −0.6 must erase it.
        let regions = offset_region(&unit_square(), -0.6);
        assert!(regions.is_empty(), "expected empty result, got {regions:?}");
    }

    #[test]
    fn l_shape_inflated_grows_and_stays_one_piece() {
        // L-shape: a 3×3 square with a 2×2 bite out of the upper-right corner.
        // Area = 9 − 4 = 5.
        let l = Region::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(3.0, 0.0),
            Point2::new(3.0, 1.0),
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 3.0),
            Point2::new(0.0, 3.0),
        ]);
        let baseline = region_area(&l);
        assert!((baseline - 5.0).abs() < 1e-9);

        let regions = offset_region(&l, 1.0);
        assert_eq!(regions.len(), 1, "inflating an L should keep it connected");
        assert_eq!(regions[0].hole_count(), 0);
        assert!(
            region_area(&regions[0]) > baseline,
            "inflated area {} should exceed baseline {baseline}",
            region_area(&regions[0]),
        );
        // Upper bound: bounding box of the inflated L is the 5×5 square covering
        // (−1,−1)..(4,4), so the result cannot exceed 25 mm².
        assert!(region_area(&regions[0]) < 25.0);
    }

    #[test]
    fn square_with_square_hole_inflated_expands_outer_and_shrinks_hole() {
        // 10×10 outer with a 4×4 hole at (3,3)–(7,7). Net area = 100 − 16 = 84.
        let region = Region::with_holes(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
                Point2::new(0.0, 10.0),
            ],
            vec![vec![
                Point2::new(3.0, 3.0),
                Point2::new(7.0, 3.0),
                Point2::new(7.0, 7.0),
                Point2::new(3.0, 7.0),
            ]],
        );
        assert!((region_area(&region) - 84.0).abs() < 1e-9);

        let regions = offset_region(&region, 1.0);
        assert_eq!(regions.len(), 1);
        assert_eq!(
            regions[0].hole_count(),
            1,
            "+1 mm offset must keep one hole (4×4 → 2×2)",
        );
        // Outer 12×12 = 144, hole 2×2 = 4 ⇒ net 140 mm².
        assert!(
            (region_area(&regions[0]) - 140.0).abs() < 1e-3,
            "expected ~140 mm², got {}",
            region_area(&regions[0]),
        );
    }

    #[test]
    fn square_with_square_hole_inflated_enough_closes_hole() {
        let region = Region::with_holes(
            vec![
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
                Point2::new(0.0, 10.0),
            ],
            vec![vec![
                Point2::new(3.0, 3.0),
                Point2::new(7.0, 3.0),
                Point2::new(7.0, 7.0),
                Point2::new(3.0, 7.0),
            ]],
        );
        // Hole is 4×4; inflating by 2.5 mm shrinks it to 0 (4 − 2·2.5 < 0).
        let regions = offset_region(&region, 2.5);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].hole_count(), 0, "hole should be erased");
    }

    #[test]
    fn region_without_meaningful_exterior_returns_empty_even_with_holes() {
        // Degenerate exterior (< 3 points) means the region is not a polygon;
        // its holes must not be promoted to outer rings by the even-odd union.
        let bad = Region::with_holes(
            vec![Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)],
            vec![vec![
                Point2::new(2.0, 2.0),
                Point2::new(3.0, 2.0),
                Point2::new(3.0, 3.0),
                Point2::new(2.0, 3.0),
            ]],
        );
        assert!(offset_region(&bad, 1.0).is_empty());
        assert!(offset_region(&bad, 0.0).is_empty());
        assert!(offset_region(&bad, -1.0).is_empty());
    }

    #[test]
    fn figure_eight_under_even_odd_resolves_to_two_lobes() {
        // Bowtie: a self-intersecting quadrilateral whose two diagonals cross
        // at (1,1). Under even-odd, this is two triangles each of area 1
        // meeting at the single pinch point (1,1).
        let bowtie = Region::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 2.0),
        ]);

        // Zero offset: only the even-odd cleanup runs. The result must be two
        // disjoint triangles, not a single self-intersecting ring.
        let cleaned = offset_region(&bowtie, 0.0);
        assert_eq!(
            cleaned.len(),
            2,
            "even-odd cleanup should split the bowtie into two lobes, got {cleaned:?}",
        );
        assert!(
            (total_area(&cleaned) - 2.0).abs() < 1e-3,
            "two unit-area triangles ⇒ total ~2 mm², got {}",
            total_area(&cleaned),
        );
        for r in &cleaned {
            assert_eq!(r.hole_count(), 0);
        }

        // Deflating shrinks each lobe but keeps them disjoint; each triangle's
        // inradius is 1/(1+√2) ≈ 0.414, so −0.1 leaves both intact.
        let deflated = offset_region(&bowtie, -0.1);
        assert_eq!(
            deflated.len(),
            2,
            "deflating a bowtie within the inradius keeps two lobes, got {deflated:?}",
        );
        assert!(total_area(&deflated) < total_area(&cleaned));

        // The lobes touch at a single point, so any positive offset bridges
        // the pinch and merges them back into a single region.
        let inflated = offset_region(&bowtie, 0.2);
        assert_eq!(
            inflated.len(),
            1,
            "+0.2 mm bridges the pinch, expected one merged region, got {inflated:?}",
        );
    }
}
