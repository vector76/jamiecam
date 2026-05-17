//! Profile-cut operation: input descriptor and toolpath output.
//!
//! Per `docs/phase-4-design.md` §5 the first Mode 2 ship is **profile cuts
//! only** — a single operation per project. This module defines the two
//! types that bracket the planner:
//!
//! - [`ProfileOperationInput`] — what the user (or the operation editor)
//!   hands to the planner: boundaries, tool, side, depth schedule, feeds.
//! - [`ToolpathOutput`] — the planner's output: an ordered list of
//!   [`ToolpathMotion`]s in machine coordinates, ready for the GRBL
//!   emitter (§7) or a direct dexel preview.
//!
//! No generator logic lives here yet — only the type-coupling boundary.
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

use crate::geometry2d::Polyline;
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
}
