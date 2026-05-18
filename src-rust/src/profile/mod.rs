//! Profile-cut operation: input descriptor, planner, and toolpath output.
//!
//! Per `docs/phase-4-design.md` §5 the first Mode 2 ship is **profile cuts
//! only** — a single operation per project. This module defines the two
//! types that bracket the planner plus the planner itself:
//!
//! - [`ProfileOperationInput`] — what the user (or the operation editor)
//!   hands to the planner: boundaries, tool, side, depth schedule, feeds.
//! - [`ToolpathOutput`] — the planner's output: an ordered list of
//!   [`ToolpathMotion`]s in machine coordinates, ready for the GRBL
//!   emitter (§7) or a direct dexel preview.
//! - [`generate_profile`] — the planner: offsets each boundary by tool
//!   radius and emits step-down passes around the resulting rings.
//!
//! # Single operation per project (MVP)
//!
//! Mode 2 MVP is one profile operation per project. We do not model an
//! operation list; the `.jcam` mode shell holds exactly one
//! [`ProfileOperationInput`]. When multi-operation support lands, it will
//! wrap this type rather than replace it.
//!
//! # Type-coupling boundary
//!
//! §10 of the design doc calls out that the parser output (§3), planner
//! input/output (§5), tool model (§6), and emitter input (§7) must agree
//! on shared Rust types. This module is the planner-facing half of that
//! agreement:
//!
//! - Boundaries are [`crate::geometry2d::Polyline`]s (the canonical 2D
//!   path type, landed first). **Phase 3 (SVG/DXF parsers) conforms** —
//!   the parsers produce `Polyline` values that flow directly into
//!   `ProfileOperationInput::boundaries` with no intermediate adapter.
//!   Closed and open polylines are both legal (a closed contour cuts a
//!   perimeter; an open path engraves along the line).
//! - The tool is a [`crate::working_env::Tool`] (§6, already landed),
//!   passed by value so the planner does not need a live
//!   `WorkingEnvironment` reference and so serialized inputs are
//!   self-contained.
//! - [`ToolpathMotion`] is the input the §7 GRBL emitter will consume.
//!
//! Changes to either shape are a coordinated change with §3 (parsers)
//! and §7 (emitter).
//!
//! # No arcs in first ship
//!
//! [`ToolpathMotion`] models only rapids and linear moves. Arc output
//! (`G2`/`G3`) is a planner-side choice deferred until profile-only is
//! stable; emitting only linear segments keeps the simulator and the
//! GRBL emitter symmetric for the first slice.

use serde::{Deserialize, Serialize};

use crate::clipper::offset_region;
use crate::error::AppError;
use crate::geometry2d::{Point2, Polyline, Region};
use crate::working_env::Tool;

/// Which side of each boundary the cutter travels on.
///
/// `Outside` and `Inside` offset the boundary by the tool radius before
/// generating the toolpath (so the cut edge lies on the boundary line).
/// `OnLine` runs the tool centre along the boundary itself — used for
/// engraving and V-carving, where the boundary is the desired tool path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CutSide {
    Outside,
    Inside,
    OnLine,
}

/// Everything the profile planner needs to produce a [`ToolpathOutput`].
///
/// Units are millimetres for distances and mm/min for feeds, matching the
/// rest of the Mode 2 pipeline (see [`crate::geometry2d`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOperationInput {
    /// Boundaries to cut along. Each entry is processed independently;
    /// closed polylines yield perimeter cuts, open ones yield path cuts.
    pub boundaries: Vec<Polyline>,
    /// The cutter (geometry, material, recommended feeds). The planner
    /// may override the recommended values with the explicit fields below.
    pub tool: Tool,
    /// Which side of each boundary to cut on.
    pub cut_side: CutSide,
    /// Total depth of cut, in mm (positive). The planner steps down to
    /// `-depth_total` from the stock top in increments of `depth_per_pass`.
    pub depth_total: f64,
    /// Maximum Z step per pass, in mm (positive).
    pub depth_per_pass: f64,
    /// Clearance height above the stock for rapid moves, in mm.
    pub safe_z: f64,
    /// Plunge (Z-down) feed rate, in mm/min.
    pub plunge_feed: f64,
    /// Lateral cutting feed rate, in mm/min.
    pub cut_feed: f64,
    /// Spindle speed for the operation, in RPM.
    pub spindle_rpm: f64,
}

/// A single move in the generated toolpath.
///
/// `to` is an absolute machine-coordinate triple `[x, y, z]` in mm. Feeds
/// are mm/min. Rapids have no associated feed (the post-processor /
/// emitter applies the configured rapid rate).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ToolpathMotion {
    /// Non-cutting traverse to `to` (G0).
    Rapid { to: [f64; 3] },
    /// Cutting linear move to `to` at `feed` mm/min (G1).
    Linear { to: [f64; 3], feed: f64 },
}

/// Ordered list of [`ToolpathMotion`]s that, executed in sequence,
/// realise the operation. The first motion is normally a rapid to the
/// approach point at `safe_z`; the last is normally a rapid retract.
///
/// Modeled as a bare `Vec` (not a struct) so consumers can append,
/// concatenate, and iterate without ceremony; richer metadata (bounding
/// box, estimated runtime, etc.) can be added as a wrapper type when a
/// caller needs it.
pub type ToolpathOutput = Vec<ToolpathMotion>;

/// Plan a profile-cut toolpath for the supplied operation input.
///
/// # Algorithm
///
/// For each closed boundary, the planner offsets the polygon by
/// `±tool.diameter / 2` using [`crate::clipper::offset_region`]:
/// `+radius` for [`CutSide::Outside`], `-radius` for [`CutSide::Inside`],
/// and `0` for [`CutSide::OnLine`] (the cutter centre tracks the
/// boundary directly, no offset). Open polylines are always traced as
/// engraving along the line — offset semantics are not defined for an
/// open chain, so `cut_side` is ignored for them.
///
/// Each offset ring is then walked at every step-down level from
/// `depth_per_pass` down to `depth_total` (the final level is clamped to
/// `depth_total` even when `depth_per_pass` does not divide evenly). The
/// per-pass motion shape is:
///
/// ```text
///   Rapid  → (start.x, start.y, safe_z)
///   Linear → (start.x, start.y, -depth)       at plunge_feed
///   Linear → (vertex_i.x, vertex_i.y, -depth) at cut_feed   (around the ring)
///   Linear → (start.x, start.y, -depth)       at cut_feed   (close, closed rings only)
///   Rapid  → (start.x, start.y, safe_z)       (lift)
/// ```
///
/// # Multi-boundary ordering
///
/// Boundaries are processed in ascending lexicographic order of their
/// **first vertex** `(x, y)` (using the original polyline's first
/// vertex, not any offset-derived point). This keeps the emitted G-code
/// deterministic across runs and independent of input order.
///
/// # Holes inside a boundary
///
/// Per `docs/phase-4-design.md` §5 MVP, each interior loop is cut as its
/// own profile pass at the same parameters. Two sources contribute:
///
/// 1. An offset of a closed boundary may produce [`Region`]s with holes
///    (e.g. self-touching inputs). Each hole becomes an additional
///    contour cut after that region's exterior.
/// 2. A polyline that lies inside another in `input.boundaries` is *not*
///    promoted to a hole — it is simply another boundary in the flat
///    list and is processed independently with the same parameters.
///
/// # Errors
///
/// Returns [`AppError::InvalidInput`] when `depth_total`,
/// `depth_per_pass`, or `tool.diameter` is not a positive, finite value
/// (zero, negative, NaN, and ±∞ are all rejected). Empty or degenerate
/// boundaries (<2 points open, <3 points closed) are silently skipped.
pub fn generate_profile(input: &ProfileOperationInput) -> Result<ToolpathOutput, AppError> {
    // is_finite() rejects NaN and ±∞ together; the > 0.0 check then catches
    // zero and negatives. An infinite depth_total would loop forever in
    // compute_levels, so finiteness matters as much as positivity here.
    if !input.depth_total.is_finite() || input.depth_total <= 0.0 {
        return Err(AppError::InvalidInput(
            "depth_total must be a positive, finite value".into(),
        ));
    }
    if !input.depth_per_pass.is_finite() || input.depth_per_pass <= 0.0 {
        return Err(AppError::InvalidInput(
            "depth_per_pass must be a positive, finite value".into(),
        ));
    }
    if !input.tool.diameter.is_finite() || input.tool.diameter <= 0.0 {
        return Err(AppError::InvalidInput(
            "tool diameter must be a positive, finite value".into(),
        ));
    }

    let radius = input.tool.diameter / 2.0;
    let delta = match input.cut_side {
        CutSide::Outside => radius,
        CutSide::Inside => -radius,
        CutSide::OnLine => 0.0,
    };

    let mut prepared: Vec<PreparedBoundary> = Vec::new();
    for boundary in &input.boundaries {
        if boundary.is_empty() {
            continue;
        }
        let first = boundary.points[0];
        let sort_key = (first.x, first.y);
        let paths = if !boundary.closed {
            if boundary.len() < 2 {
                continue;
            }
            vec![CutPath::open(boundary.points.clone())]
        } else if boundary.len() < 3 {
            continue;
        } else if input.cut_side == CutSide::OnLine {
            vec![CutPath::closed(boundary.points.clone())]
        } else {
            let region = Region::new(boundary.points.clone());
            offset_region(&region, delta)
                .into_iter()
                .flat_map(|r| {
                    let mut all = Vec::with_capacity(1 + r.holes.len());
                    all.push(CutPath::closed(r.exterior));
                    all.extend(r.holes.into_iter().map(CutPath::closed));
                    all
                })
                .collect()
        };
        if paths.is_empty() {
            continue;
        }
        prepared.push(PreparedBoundary { sort_key, paths });
    }

    // total_cmp gives a total order over all f64 (including NaN), which
    // makes the sort fully deterministic even with weird inputs — and
    // reads as one expression instead of a partial_cmp + unwrap dance.
    prepared.sort_by(|a, b| {
        a.sort_key
            .0
            .total_cmp(&b.sort_key.0)
            .then(a.sort_key.1.total_cmp(&b.sort_key.1))
    });

    let levels = compute_levels(input.depth_total, input.depth_per_pass);

    let mut motions = ToolpathOutput::new();
    for prep in prepared {
        for path in prep.paths {
            for &depth in &levels {
                emit_pass(&mut motions, &path, depth, input);
            }
        }
    }
    Ok(motions)
}

struct PreparedBoundary {
    sort_key: (f64, f64),
    paths: Vec<CutPath>,
}

struct CutPath {
    points: Vec<Point2>,
    closed: bool,
}

impl CutPath {
    fn closed(points: Vec<Point2>) -> Self {
        Self {
            points,
            closed: true,
        }
    }
    fn open(points: Vec<Point2>) -> Self {
        Self {
            points,
            closed: false,
        }
    }
}

fn emit_pass(
    motions: &mut ToolpathOutput,
    path: &CutPath,
    depth: f64,
    input: &ProfileOperationInput,
) {
    if path.points.is_empty() {
        return;
    }
    let start = path.points[0];
    let z = -depth;
    motions.push(ToolpathMotion::Rapid {
        to: [start.x, start.y, input.safe_z],
    });
    motions.push(ToolpathMotion::Linear {
        to: [start.x, start.y, z],
        feed: input.plunge_feed,
    });
    for p in &path.points[1..] {
        motions.push(ToolpathMotion::Linear {
            to: [p.x, p.y, z],
            feed: input.cut_feed,
        });
    }
    if path.closed {
        motions.push(ToolpathMotion::Linear {
            to: [start.x, start.y, z],
            feed: input.cut_feed,
        });
    }
    motions.push(ToolpathMotion::Rapid {
        to: [start.x, start.y, input.safe_z],
    });
}

/// Step-down depth schedule from `depth_per_pass` to `depth_total` (mm,
/// positive). The final entry is always exactly `depth_total`, even when
/// `depth_per_pass` does not divide evenly — the trailing pass becomes a
/// thinner finish cut rather than overshooting.
fn compute_levels(depth_total: f64, depth_per_pass: f64) -> Vec<f64> {
    let mut levels = Vec::new();
    let mut depth = depth_per_pass;
    // 1e-9 mm tolerance avoids a redundant final pass when the schedule
    // happens to land on depth_total within f64 rounding.
    while depth + 1e-9 < depth_total {
        levels.push(depth);
        depth += depth_per_pass;
    }
    levels.push(depth_total);
    levels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry2d::Point2;
    use crate::working_env::{FeedsAndSpeeds, ToolId};

    fn sample_tool() -> Tool {
        Tool {
            id: ToolId::new("t1"),
            name: "1/8\" flat".into(),
            diameter: 3.175,
            flute_count: 2,
            length: 38.0,
            material: "carbide".into(),
            recommended: FeedsAndSpeeds {
                spindle_rpm: 18000.0,
                feed_rate: 800.0,
                plunge_rate: 200.0,
            },
        }
    }

    fn sample_input() -> ProfileOperationInput {
        ProfileOperationInput {
            boundaries: vec![Polyline::closed(vec![
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
                Point2::new(0.0, 10.0),
            ])],
            tool: sample_tool(),
            cut_side: CutSide::Outside,
            depth_total: 6.0,
            depth_per_pass: 1.5,
            safe_z: 5.0,
            plunge_feed: 200.0,
            cut_feed: 800.0,
            spindle_rpm: 18000.0,
        }
    }

    #[test]
    fn cut_side_serializes_camel_case() {
        assert_eq!(
            serde_json::to_value(CutSide::Outside).unwrap(),
            serde_json::json!("outside")
        );
        assert_eq!(
            serde_json::to_value(CutSide::Inside).unwrap(),
            serde_json::json!("inside")
        );
        assert_eq!(
            serde_json::to_value(CutSide::OnLine).unwrap(),
            serde_json::json!("onLine")
        );
    }

    #[test]
    fn cut_side_round_trips_via_json() {
        for side in [CutSide::Outside, CutSide::Inside, CutSide::OnLine] {
            let json = serde_json::to_string(&side).unwrap();
            let back: CutSide = serde_json::from_str(&json).unwrap();
            assert_eq!(back, side);
        }
    }

    #[test]
    fn profile_input_serializes_camel_case_fields() {
        let v = serde_json::to_value(sample_input()).unwrap();
        assert_eq!(v["cutSide"], "outside");
        assert_eq!(v["depthTotal"], 6.0);
        assert_eq!(v["depthPerPass"], 1.5);
        assert_eq!(v["safeZ"], 5.0);
        assert_eq!(v["plungeFeed"], 200.0);
        assert_eq!(v["cutFeed"], 800.0);
        assert_eq!(v["spindleRpm"], 18000.0);
        assert_eq!(v["tool"]["id"], "t1");
        assert_eq!(v["boundaries"][0]["closed"], true);
    }

    #[test]
    fn profile_input_round_trips_via_json() {
        let input = sample_input();
        let json = serde_json::to_string(&input).unwrap();
        let back: ProfileOperationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back, input);
    }

    #[test]
    fn rapid_motion_serializes_with_kind_tag() {
        let m = ToolpathMotion::Rapid {
            to: [1.0, 2.0, 5.0],
        };
        let v = serde_json::to_value(m).unwrap();
        assert_eq!(v["kind"], "rapid");
        assert_eq!(v["to"], serde_json::json!([1.0, 2.0, 5.0]));
        assert!(v.get("feed").is_none());
    }

    #[test]
    fn linear_motion_serializes_with_kind_tag_and_feed() {
        let m = ToolpathMotion::Linear {
            to: [1.0, 2.0, -1.5],
            feed: 800.0,
        };
        let v = serde_json::to_value(m).unwrap();
        assert_eq!(v["kind"], "linear");
        assert_eq!(v["to"], serde_json::json!([1.0, 2.0, -1.5]));
        assert_eq!(v["feed"], 800.0);
    }

    #[test]
    fn toolpath_motion_round_trips_via_json() {
        let moves = vec![
            ToolpathMotion::Rapid {
                to: [0.0, 0.0, 5.0],
            },
            ToolpathMotion::Linear {
                to: [0.0, 0.0, -1.5],
                feed: 200.0,
            },
            ToolpathMotion::Linear {
                to: [10.0, 0.0, -1.5],
                feed: 800.0,
            },
        ];
        let json = serde_json::to_string(&moves).unwrap();
        let back: ToolpathOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back, moves);
    }

    #[test]
    fn toolpath_output_is_vec_of_motions() {
        let out: ToolpathOutput = Vec::new();
        assert!(out.is_empty());
    }

    // ── generate_profile ────────────────────────────────────────────────────

    fn square_boundary(x0: f64, y0: f64, side: f64) -> Polyline {
        Polyline::closed(vec![
            Point2::new(x0, y0),
            Point2::new(x0 + side, y0),
            Point2::new(x0 + side, y0 + side),
            Point2::new(x0, y0 + side),
        ])
    }

    fn base_input(boundaries: Vec<Polyline>, side: CutSide) -> ProfileOperationInput {
        ProfileOperationInput {
            boundaries,
            tool: sample_tool(),
            cut_side: side,
            depth_total: 6.0,
            depth_per_pass: 1.5,
            safe_z: 5.0,
            plunge_feed: 200.0,
            cut_feed: 800.0,
            spindle_rpm: 18000.0,
        }
    }

    /// Group emitted motions into per-pass blocks delimited by lift rapids
    /// at `safe_z`. Each block starts with the approach rapid and ends with
    /// the lift rapid (so blocks share a closing rapid only in sequence —
    /// each block is independent here).
    fn split_passes(motions: &[ToolpathMotion], safe_z: f64) -> Vec<Vec<ToolpathMotion>> {
        let mut passes: Vec<Vec<ToolpathMotion>> = Vec::new();
        let mut current: Vec<ToolpathMotion> = Vec::new();
        for m in motions {
            current.push(*m);
            if let ToolpathMotion::Rapid { to } = m {
                if (to[2] - safe_z).abs() < 1e-9 && current.len() > 1 {
                    passes.push(std::mem::take(&mut current));
                }
            }
        }
        if !current.is_empty() {
            passes.push(current);
        }
        passes
    }

    #[test]
    fn generate_profile_rejects_non_positive_depth_total() {
        let mut input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::OnLine);
        input.depth_total = 0.0;
        let err = generate_profile(&input).expect_err("depth_total=0 must error");
        assert!(matches!(err, AppError::InvalidInput(ref s) if s.contains("depth_total")));
    }

    #[test]
    fn generate_profile_rejects_non_positive_depth_per_pass() {
        let mut input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::OnLine);
        input.depth_per_pass = -1.0;
        let err = generate_profile(&input).expect_err("negative depth_per_pass must error");
        assert!(matches!(err, AppError::InvalidInput(ref s) if s.contains("depth_per_pass")));
    }

    #[test]
    fn generate_profile_rejects_non_finite_depth_total() {
        // Guards against an infinite loop in compute_levels — an infinite
        // depth_total would balloon memory before any output is produced.
        let mut input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::OnLine);
        input.depth_total = f64::INFINITY;
        let err = generate_profile(&input).expect_err("infinite depth_total must error");
        assert!(matches!(err, AppError::InvalidInput(ref s) if s.contains("depth_total")));

        input.depth_total = f64::NAN;
        let err = generate_profile(&input).expect_err("NaN depth_total must error");
        assert!(matches!(err, AppError::InvalidInput(ref s) if s.contains("depth_total")));
    }

    #[test]
    fn generate_profile_rejects_non_positive_tool_diameter() {
        let mut input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::Outside);
        input.tool.diameter = 0.0;
        let err = generate_profile(&input).expect_err("zero diameter must error");
        assert!(matches!(err, AppError::InvalidInput(ref s) if s.contains("diameter")));
    }

    #[test]
    fn generate_profile_with_no_boundaries_is_empty() {
        let input = base_input(Vec::new(), CutSide::Outside);
        let out = generate_profile(&input).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn square_outside_emits_one_pass_per_step_down_level() {
        // depth_total=6, depth_per_pass=1.5 ⇒ four passes at 1.5/3.0/4.5/6.0.
        let input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::Outside);
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        assert_eq!(passes.len(), 4, "expected 4 step-down passes");

        // First motion of the operation must be a rapid up to safe_z.
        match motions.first().unwrap() {
            ToolpathMotion::Rapid { to } => assert!((to[2] - 5.0).abs() < 1e-9),
            other => panic!("expected leading rapid, got {other:?}"),
        }
        // Last motion must be a lift rapid back to safe_z.
        match motions.last().unwrap() {
            ToolpathMotion::Rapid { to } => assert!((to[2] - 5.0).abs() < 1e-9),
            other => panic!("expected trailing rapid, got {other:?}"),
        }

        // Verify each pass cuts at the expected Z and uses the right feeds.
        let expected_depths = [1.5_f64, 3.0, 4.5, 6.0];
        for (pass, expected) in passes.iter().zip(expected_depths) {
            let z = -expected;
            // Pass shape: Rapid(safe_z), Linear(plunge), Linear×N(cut), Linear(close), Rapid(safe_z).
            assert!(
                matches!(pass.first().unwrap(), ToolpathMotion::Rapid { to } if (to[2] - 5.0).abs() < 1e-9)
            );
            match pass[1] {
                ToolpathMotion::Linear { to, feed } => {
                    assert!(
                        (to[2] - z).abs() < 1e-9,
                        "plunge Z mismatch at depth {expected}"
                    );
                    assert!((feed - 200.0).abs() < 1e-9, "plunge feed mismatch");
                }
                ref other => panic!("expected plunge linear, got {other:?}"),
            }
            // All interior moves cut at the same Z and at cut_feed.
            for m in &pass[2..pass.len() - 1] {
                match m {
                    ToolpathMotion::Linear { to, feed } => {
                        assert!(
                            (to[2] - z).abs() < 1e-9,
                            "cut Z mismatch at depth {expected}"
                        );
                        assert!((feed - 800.0).abs() < 1e-9, "cut feed mismatch");
                    }
                    other => panic!("expected linear cut, got {other:?}"),
                }
            }
            assert!(
                matches!(pass.last().unwrap(), ToolpathMotion::Rapid { to } if (to[2] - 5.0).abs() < 1e-9)
            );
        }
    }

    #[test]
    fn outside_offset_traces_path_at_tool_radius_outside_boundary() {
        // Tool dia 3.175 ⇒ radius 1.5875. Outside cut on a 10×10 square at
        // origin should put the tool centre on the bounding rectangle
        // (−r, −r)..(10+r, 10+r). Check by sampling X/Y extents of the
        // emitted linear motions in the first pass.
        let input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::Outside);
        let motions = generate_profile(&input).unwrap();
        let pass = &split_passes(&motions, input.safe_z)[0];

        let radius = input.tool.diameter / 2.0;
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        for m in pass {
            if let ToolpathMotion::Linear { to, .. } = m {
                xs.push(to[0]);
                ys.push(to[1]);
            }
        }
        let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((xmin - (-radius)).abs() < 1e-3, "xmin {xmin} ≠ {}", -radius);
        assert!((xmax - (10.0 + radius)).abs() < 1e-3, "xmax {xmax}");
        assert!((ymin - (-radius)).abs() < 1e-3, "ymin {ymin}");
        assert!((ymax - (10.0 + radius)).abs() < 1e-3, "ymax {ymax}");
    }

    #[test]
    fn inside_offset_traces_path_at_tool_radius_inside_boundary() {
        let input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::Inside);
        let motions = generate_profile(&input).unwrap();
        let pass = &split_passes(&motions, input.safe_z)[0];

        let radius = input.tool.diameter / 2.0;
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        for m in pass {
            if let ToolpathMotion::Linear { to, .. } = m {
                xs.push(to[0]);
                ys.push(to[1]);
            }
        }
        let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // Tool centre stays inside the 10×10 boundary by `radius`.
        assert!((xmin - radius).abs() < 1e-3, "xmin {xmin}");
        assert!((xmax - (10.0 - radius)).abs() < 1e-3, "xmax {xmax}");
        assert!((ymin - radius).abs() < 1e-3, "ymin {ymin}");
        assert!((ymax - (10.0 - radius)).abs() < 1e-3, "ymax {ymax}");
    }

    #[test]
    fn on_line_uses_boundary_vertices_directly() {
        let boundary = square_boundary(0.0, 0.0, 10.0);
        let input = base_input(vec![boundary.clone()], CutSide::OnLine);
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        let pass = &passes[0];
        // First plunge XY equals the original first vertex.
        match pass[1] {
            ToolpathMotion::Linear { to, .. } => {
                assert!((to[0] - 0.0).abs() < 1e-9);
                assert!((to[1] - 0.0).abs() < 1e-9);
            }
            ref other => panic!("expected plunge linear, got {other:?}"),
        }
        // Then three cut vertices matching the boundary's remaining points,
        // plus one closing cut back to the start.
        let expected_xy: &[(f64, f64)] = &[(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)];
        for (m, (ex, ey)) in pass[2..pass.len() - 1].iter().zip(expected_xy) {
            match m {
                ToolpathMotion::Linear { to, .. } => {
                    assert!((to[0] - ex).abs() < 1e-9, "cut x {} ≠ {ex}", to[0]);
                    assert!((to[1] - ey).abs() < 1e-9, "cut y {} ≠ {ey}", to[1]);
                }
                other => panic!("expected linear cut, got {other:?}"),
            }
        }
    }

    #[test]
    fn open_polyline_is_traced_without_closing_edge() {
        // Open chain (engraving): three points, traced as-is regardless of cut_side.
        let open = Polyline::open(vec![
            Point2::new(0.0, 0.0),
            Point2::new(5.0, 0.0),
            Point2::new(10.0, 5.0),
        ]);
        let mut input = base_input(vec![open], CutSide::Outside);
        // One pass per level keeps the structural check simple.
        input.depth_per_pass = 6.0;
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        assert_eq!(passes.len(), 1, "single depth ⇒ single pass");
        let pass = &passes[0];
        // Approach + plunge + 2 cuts + lift = 5 motions (no closing edge).
        assert_eq!(
            pass.len(),
            5,
            "open path should not add a closing cut, got {pass:?}"
        );
    }

    #[test]
    fn square_with_separate_hole_polyline_emits_passes_for_each() {
        // Outer 10×10 boundary and an interior 4×4 polyline listed as its
        // own entry — each is processed as a profile cut at the same params.
        let outer = square_boundary(0.0, 0.0, 10.0);
        let hole = square_boundary(3.0, 3.0, 4.0);
        let input = base_input(vec![outer, hole], CutSide::Inside);
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        // 2 rings × 4 step-down levels = 8 passes.
        assert_eq!(passes.len(), 8, "expected 8 passes (2 rings × 4 levels)");

        // Outer boundary first (lex order on first vertex: (0,0) < (3,3)).
        // Its first plunge XY should lie inside the 10×10 envelope, the
        // hole's inside the 4×4 at (3..7).
        let first_plunge = match passes[0][1] {
            ToolpathMotion::Linear { to, .. } => (to[0], to[1]),
            ref other => panic!("expected plunge linear, got {other:?}"),
        };
        assert!(first_plunge.0 >= 0.0 && first_plunge.0 <= 10.0);
        assert!(first_plunge.1 >= 0.0 && first_plunge.1 <= 10.0);

        let hole_pass_idx = 4; // first pass of the second ring
        let hole_plunge = match passes[hole_pass_idx][1] {
            ToolpathMotion::Linear { to, .. } => (to[0], to[1]),
            ref other => panic!("expected plunge linear, got {other:?}"),
        };
        assert!(hole_plunge.0 >= 3.0 && hole_plunge.0 <= 7.0);
        assert!(hole_plunge.1 >= 3.0 && hole_plunge.1 <= 7.0);
    }

    #[test]
    fn multi_boundary_order_is_first_vertex_lexicographic() {
        // Three squares at very different first-vertex positions, supplied
        // in a non-sorted order. The output must process them in (x, y)
        // ascending order of first vertex regardless of input order.
        let a = square_boundary(50.0, 50.0, 5.0); // first vertex (50, 50)
        let b = square_boundary(0.0, 0.0, 5.0); // first vertex (0, 0)
        let c = square_boundary(0.0, 20.0, 5.0); // first vertex (0, 20)
        let input = base_input(vec![a, b, c], CutSide::OnLine);
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        // 3 rings × 4 levels = 12 passes.
        assert_eq!(passes.len(), 12);

        // Pull the plunge XY of the first pass of each ring (passes 0, 4, 8).
        let first_plunge_of = |pass_idx: usize| match passes[pass_idx][1] {
            ToolpathMotion::Linear { to, .. } => (to[0], to[1]),
            ref other => panic!("expected plunge linear, got {other:?}"),
        };
        let p0 = first_plunge_of(0);
        let p1 = first_plunge_of(4);
        let p2 = first_plunge_of(8);
        // Ordering: (0,0), then (0,20), then (50,50).
        assert!(
            (p0.0 - 0.0).abs() < 1e-9 && (p0.1 - 0.0).abs() < 1e-9,
            "got {p0:?}"
        );
        assert!(
            (p1.0 - 0.0).abs() < 1e-9 && (p1.1 - 20.0).abs() < 1e-9,
            "got {p1:?}"
        );
        assert!(
            (p2.0 - 50.0).abs() < 1e-9 && (p2.1 - 50.0).abs() < 1e-9,
            "got {p2:?}"
        );
    }

    #[test]
    fn step_down_schedule_includes_partial_final_pass() {
        // depth_total=5, depth_per_pass=2 ⇒ levels at 2, 4, 5 (3 passes).
        let mut input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::OnLine);
        input.depth_total = 5.0;
        input.depth_per_pass = 2.0;
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        assert_eq!(passes.len(), 3);
        let depths: Vec<f64> = passes
            .iter()
            .map(|p| match p[1] {
                ToolpathMotion::Linear { to, .. } => -to[2],
                _ => panic!("expected plunge linear"),
            })
            .collect();
        assert!((depths[0] - 2.0).abs() < 1e-9, "{depths:?}");
        assert!((depths[1] - 4.0).abs() < 1e-9, "{depths:?}");
        assert!((depths[2] - 5.0).abs() < 1e-9, "{depths:?}");
    }

    #[test]
    fn step_down_schedule_with_exact_division_has_no_duplicate_final_pass() {
        // depth_total=6, depth_per_pass=1.5 ⇒ exactly 4 levels, not 5.
        let input = base_input(vec![square_boundary(0.0, 0.0, 10.0)], CutSide::OnLine);
        let motions = generate_profile(&input).unwrap();
        let passes = split_passes(&motions, input.safe_z);
        assert_eq!(passes.len(), 4);
    }

    #[test]
    fn closed_polyline_under_three_vertices_is_skipped() {
        let degenerate = Polyline::closed(vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)]);
        let input = base_input(vec![degenerate], CutSide::OnLine);
        let motions = generate_profile(&input).unwrap();
        assert!(motions.is_empty());
    }

    #[test]
    fn deflating_past_inradius_produces_no_motions_for_that_boundary() {
        // 1×1 square deflated by tool radius 1.5875 ⇒ empty offset result.
        let small = square_boundary(0.0, 0.0, 1.0);
        let input = base_input(vec![small], CutSide::Inside);
        let motions = generate_profile(&input).unwrap();
        assert!(
            motions.is_empty(),
            "expected empty toolpath, got {motions:?}"
        );
    }
}
