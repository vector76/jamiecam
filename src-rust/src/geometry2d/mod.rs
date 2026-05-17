//! Shared 2D path representation used across the Mode 2 pipeline.
//!
//! Per `docs/phase-4-design.md` §10, the parser output, planner input/output,
//! and G-code emitter input must agree on a single set of 2D types. This
//! module is that agreement. Keep it minimal — new fields belong here only
//! when at least two of those consumers genuinely need them.
//!
//! # Unit contract
//!
//! **All coordinates and lengths in this module are millimetres.** Importers
//! (SVG, DXF) are responsible for converting source units to mm before
//! constructing values here. The planner and emitter consume mm directly
//! with no further scaling. Coordinates are `f64` throughout — this matches
//! `clipper2-rust`'s `f64` API and avoids a precision conversion at the
//! planner boundary.
//!
//! # Closed vs. open polylines
//!
//! A [`Polyline`] carries an explicit `closed` flag. For closed polylines
//! the first and last points are *not* duplicated in `points` — closure is
//! implicit (an edge from the last point back to the first). Open polylines
//! are simple chains with no implicit closing edge.
//!
//! # Regions and holes
//!
//! A [`Region`] is an exterior ring with zero or more interior holes. Both
//! the exterior and each hole are stored as point lists with implicit
//! closure (matching [`Polyline::closed`] convention). Orientation is not
//! enforced by this module; the planner normalises it as needed.

use serde::{Deserialize, Serialize};

/// A 2D point in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An ordered chain of [`Point2`]s, either open or closed.
///
/// When `closed` is `true`, the closing edge from the last point back to the
/// first is implicit — do not repeat the first point at the end of `points`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Polyline {
    pub points: Vec<Point2>,
    pub closed: bool,
}

impl Polyline {
    /// Constructs an open polyline (no implicit closing edge).
    pub fn open(points: Vec<Point2>) -> Self {
        Self {
            points,
            closed: false,
        }
    }

    /// Constructs a closed polyline. The first point is *not* repeated at
    /// the end of `points` — closure is implicit.
    pub fn closed(points: Vec<Point2>) -> Self {
        Self {
            points,
            closed: true,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// A filled 2D region: one exterior ring and zero or more interior holes.
///
/// Both the exterior and each hole are closed rings stored without a
/// duplicated final point (same convention as [`Polyline`] with `closed =
/// true`). Holes must lie inside the exterior; this module does not verify
/// that — it is the caller's responsibility (parser or planner).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub exterior: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
}

impl Region {
    /// Constructs a region with no holes.
    pub fn new(exterior: Vec<Point2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
        }
    }

    /// Constructs a region with the given exterior and holes.
    pub fn with_holes(exterior: Vec<Point2>, holes: Vec<Vec<Point2>>) -> Self {
        Self { exterior, holes }
    }

    pub fn hole_count(&self) -> usize {
        self.holes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point2_constructs_with_mm_coordinates() {
        let p = Point2::new(1.5, -2.25);
        assert_eq!(p.x, 1.5);
        assert_eq!(p.y, -2.25);
    }

    #[test]
    fn open_polyline_reports_open_and_preserves_points() {
        let pl = Polyline::open(vec![Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)]);
        assert!(!pl.is_closed());
        assert_eq!(pl.len(), 2);
        assert!(!pl.is_empty());
    }

    #[test]
    fn closed_polyline_reports_closed() {
        let pl = Polyline::closed(vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
        ]);
        assert!(pl.is_closed());
        assert_eq!(pl.len(), 3);
    }

    #[test]
    fn empty_polyline_is_empty() {
        let pl = Polyline::open(Vec::new());
        assert!(pl.is_empty());
        assert_eq!(pl.len(), 0);
    }

    #[test]
    fn region_new_has_no_holes() {
        let r = Region::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ]);
        assert_eq!(r.hole_count(), 0);
        assert_eq!(r.exterior.len(), 4);
    }

    #[test]
    fn region_with_holes_records_each_hole() {
        let exterior = vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let hole_a = vec![
            Point2::new(2.0, 2.0),
            Point2::new(4.0, 2.0),
            Point2::new(4.0, 4.0),
            Point2::new(2.0, 4.0),
        ];
        let hole_b = vec![
            Point2::new(6.0, 6.0),
            Point2::new(8.0, 6.0),
            Point2::new(8.0, 8.0),
            Point2::new(6.0, 8.0),
        ];
        let r = Region::with_holes(exterior, vec![hole_a, hole_b]);
        assert_eq!(r.hole_count(), 2);
        assert_eq!(r.holes[0].len(), 4);
        assert_eq!(r.holes[1].len(), 4);
    }

    #[test]
    fn polyline_serializes_with_camelcase_closed_flag() {
        let pl = Polyline::closed(vec![Point2::new(1.0, 2.0)]);
        let json = serde_json::to_string(&pl).unwrap();
        assert!(json.contains("\"closed\":true"));
        assert!(json.contains("\"points\""));
        assert!(json.contains("\"x\":1.0"));
        assert!(json.contains("\"y\":2.0"));
    }
}
