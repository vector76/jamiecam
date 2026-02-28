//! Toolpath data types produced by the post-processor engine.
//!
//! A [`Toolpath`] is the ordered sequence of moves that a CNC controller
//! will execute for a single machining operation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::Vec3;

/// A complete toolpath for one machining operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Toolpath {
    /// UUID of the [`Operation`] this toolpath was generated from.
    pub operation_id: Uuid,
    /// Tool number (T-word) to be output in the G-code header.
    pub tool_number: u32,
    /// Spindle speed in RPM.
    pub spindle_speed: f64,
    /// Default feed rate in mm/min (or in/min depending on project units).
    pub feed_rate: f64,
    /// Ordered list of passes that make up this toolpath.
    pub passes: Vec<Pass>,
}

/// A single pass within a toolpath (e.g. one depth step, one linking move).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pass {
    /// Classification of this pass.
    pub kind: PassKind,
    /// Ordered list of cut points that define the path geometry.
    pub cuts: Vec<CutPoint>,
}

/// Classification of a toolpath pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassKind {
    Cutting,
    Linking,
    LeadIn,
    LeadOut,
    SpringPass,
}

/// A single point along a toolpath pass, including move semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutPoint {
    /// XYZ position in the work coordinate system.
    pub position: Vec3,
    /// The type of machine move to reach this point.
    pub move_kind: MoveKind,
    /// Optional tool orientation (required for 5-axis moves).
    pub tool_orientation: Option<ToolOrientation>,
}

/// The machine move type used to reach a [`CutPoint`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MoveKind {
    /// Rapid positioning move (G0).
    Rapid,
    /// Linear feed move (G1).
    Feed,
    /// Circular arc move (G2/G3).
    Arc {
        /// Arc center in the work coordinate system.
        center: Vec3,
        /// Arc end point (same as the next [`CutPoint`] position).
        end: Vec3,
        /// `true` → clockwise (G2); `false` → counter-clockwise (G3).
        clockwise: bool,
    },
    /// Dwell (G4).
    Dwell {
        /// Duration in seconds.
        seconds: f64,
    },
}

/// Tool orientation for multi-axis moves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolOrientation {
    /// Standard 3-axis — tool axis is always +Z; no tilt data needed.
    ThreeAxis,
    /// 5-axis — explicit tool axis vector in the work coordinate system.
    FiveAxis {
        /// Unit vector pointing along the tool axis (away from spindle).
        tool_axis: Vec3,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_feed_toolpath() -> Toolpath {
        Toolpath {
            operation_id: Uuid::nil(),
            tool_number: 1,
            spindle_speed: 12000.0,
            feed_rate: 1500.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: -5.0,
                        },
                        move_kind: MoveKind::Rapid,
                        tool_orientation: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 50.0,
                            y: 0.0,
                            z: -5.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: Some(ToolOrientation::ThreeAxis),
                    },
                ],
            }],
        }
    }

    fn sample_arc_toolpath() -> Toolpath {
        Toolpath {
            operation_id: Uuid::nil(),
            tool_number: 2,
            spindle_speed: 8000.0,
            feed_rate: 800.0,
            passes: vec![Pass {
                kind: PassKind::LeadIn,
                cuts: vec![CutPoint {
                    position: Vec3 {
                        x: 10.0,
                        y: 0.0,
                        z: -3.0,
                    },
                    move_kind: MoveKind::Arc {
                        center: Vec3 {
                            x: 5.0,
                            y: 0.0,
                            z: -3.0,
                        },
                        end: Vec3 {
                            x: 5.0,
                            y: 5.0,
                            z: -3.0,
                        },
                        clockwise: false,
                    },
                    tool_orientation: Some(ToolOrientation::FiveAxis {
                        tool_axis: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 1.0,
                        },
                    }),
                }],
            }],
        }
    }

    #[test]
    fn feed_toolpath_serde_round_trip() {
        let original = sample_feed_toolpath();
        let json = serde_json::to_string(&original).expect("serialize feed toolpath");
        let recovered: Toolpath = serde_json::from_str(&json).expect("deserialize feed toolpath");
        assert_eq!(original, recovered);
    }

    #[test]
    fn arc_toolpath_serde_round_trip() {
        let original = sample_arc_toolpath();
        let json = serde_json::to_string(&original).expect("serialize arc toolpath");
        let recovered: Toolpath = serde_json::from_str(&json).expect("deserialize arc toolpath");
        assert_eq!(original, recovered);
    }

    #[test]
    fn move_kind_rapid_serializes_with_type_tag() {
        let mk = MoveKind::Rapid;
        let value = serde_json::to_value(&mk).expect("serialize Rapid");
        assert_eq!(value["type"], "rapid");
    }

    #[test]
    fn move_kind_arc_serializes_with_type_tag_and_fields() {
        let mk = MoveKind::Arc {
            center: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            },
            end: Vec3 {
                x: 3.0,
                y: 4.0,
                z: 0.0,
            },
            clockwise: true,
        };
        let value = serde_json::to_value(&mk).expect("serialize Arc");
        assert_eq!(value["type"], "arc");
        assert_eq!(value["clockwise"], true);
        assert_eq!(value["center"]["x"], 1.0);
    }

    #[test]
    fn pass_kind_variants_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_value(PassKind::SpringPass)
                .expect("serialize SpringPass")
                .as_str()
                .unwrap(),
            "spring_pass"
        );
        assert_eq!(
            serde_json::to_value(PassKind::LeadIn)
                .expect("serialize LeadIn")
                .as_str()
                .unwrap(),
            "lead_in"
        );
    }
}
