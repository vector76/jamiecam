use crate::dexel::MotionSegment;
use crate::gcode_parser;
use crate::toolpath::types::MoveKind;
use crate::toolpath::Toolpath;

/// Convert a [`Toolpath`] into a flat list of [`MotionSegment`] values
/// suitable for the dexel material-removal engine.
///
/// Position is carried across passes: if one pass ends at position A and
/// the next pass starts at position B, a connecting segment is generated
/// (matching real machine behavior).
///
/// When `initial_position` is `Some`, the very first [`CutPoint`] produces
/// a connecting segment from that position (used to chain across operations).
/// When `None`, the first point establishes position without a segment.
///
/// [`MoveKind::Dwell`] points are skipped (no geometry).
pub fn toolpath_to_segments(
    toolpath: &Toolpath,
    initial_position: Option<&crate::models::Vec3>,
) -> Vec<MotionSegment> {
    let mut segments = Vec::new();
    let mut prev_position: Option<crate::models::Vec3> = initial_position.cloned();

    for pass in &toolpath.passes {
        for cp in &pass.cuts {
            if let Some(ref prev) = prev_position {
                let seg = match &cp.move_kind {
                    MoveKind::Rapid | MoveKind::Feed => Some(MotionSegment::Linear {
                        start: prev.clone(),
                        end: cp.position.clone(),
                    }),
                    MoveKind::Arc {
                        center, clockwise, ..
                    } => Some(MotionSegment::Arc {
                        start: prev.clone(),
                        end: cp.position.clone(),
                        center: center.clone(),
                        clockwise: *clockwise,
                    }),
                    MoveKind::Dwell { .. } => None,
                };

                if let Some(s) = seg {
                    segments.push(s);
                }
            }
            prev_position = Some(cp.position.clone());
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;
    use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind, Toolpath};
    use uuid::Uuid;

    fn pt(x: f64, y: f64, z: f64, kind: MoveKind) -> CutPoint {
        CutPoint {
            position: Vec3 { x, y, z },
            move_kind: kind,
            tool_orientation: None,
            feed_rate_override: None,
        }
    }

    fn simple_toolpath(passes: Vec<Pass>) -> Toolpath {
        Toolpath {
            operation_id: Uuid::nil(),
            tool_number: 1,
            spindle_speed: 10000.0,
            feed_rate: 1000.0,
            passes,
        }
    }

    #[test]
    fn three_feed_points_produce_two_linear_segments() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                pt(0.0, 0.0, 0.0, MoveKind::Feed),
                pt(10.0, 0.0, 0.0, MoveKind::Feed),
                pt(10.0, 10.0, 0.0, MoveKind::Feed),
            ],
        }]);

        let segs = toolpath_to_segments(&tp, None);
        assert_eq!(segs.len(), 2);
        assert_eq!(
            segs[0],
            MotionSegment::Linear {
                start: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                },
                end: Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0
                },
            }
        );
        assert_eq!(
            segs[1],
            MotionSegment::Linear {
                start: Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0
                },
                end: Vec3 {
                    x: 10.0,
                    y: 10.0,
                    z: 0.0
                },
            }
        );
    }

    #[test]
    fn arc_move_produces_arc_segment() {
        let center = Vec3 {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        };
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                pt(10.0, 0.0, 0.0, MoveKind::Feed),
                CutPoint {
                    position: Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    move_kind: MoveKind::Arc {
                        center: center.clone(),
                        end: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        clockwise: true,
                    },
                    tool_orientation: None,
                    feed_rate_override: None,
                },
            ],
        }]);

        let segs = toolpath_to_segments(&tp, None);
        assert_eq!(segs.len(), 1);
        assert_eq!(
            segs[0],
            MotionSegment::Arc {
                start: Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0
                },
                end: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                },
                center,
                clockwise: true,
            }
        );
    }

    #[test]
    fn dwell_points_are_skipped() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                pt(0.0, 0.0, 0.0, MoveKind::Feed),
                pt(0.0, 0.0, 0.0, MoveKind::Dwell { seconds: 1.5 }),
                pt(10.0, 0.0, 0.0, MoveKind::Feed),
            ],
        }]);

        let segs = toolpath_to_segments(&tp, None);
        assert_eq!(segs.len(), 1);
        assert_eq!(
            segs[0],
            MotionSegment::Linear {
                start: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                },
                end: Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0
                },
            }
        );
    }

    #[test]
    fn rapid_moves_produce_linear_segments() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Linking,
            cuts: vec![
                pt(0.0, 0.0, 10.0, MoveKind::Rapid),
                pt(50.0, 50.0, 10.0, MoveKind::Rapid),
            ],
        }]);

        let segs = toolpath_to_segments(&tp, None);
        assert_eq!(segs.len(), 1);
        assert_eq!(
            segs[0],
            MotionSegment::Linear {
                start: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 10.0
                },
                end: Vec3 {
                    x: 50.0,
                    y: 50.0,
                    z: 10.0
                },
            }
        );
    }

    #[test]
    fn empty_toolpath_produces_no_segments() {
        let tp = simple_toolpath(vec![]);
        let segs = toolpath_to_segments(&tp, None);
        assert!(segs.is_empty());
    }

    #[test]
    fn single_cutpoint_produces_no_segments() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![pt(0.0, 0.0, 0.0, MoveKind::Feed)],
        }]);

        let segs = toolpath_to_segments(&tp, None);
        assert!(segs.is_empty());
    }

    #[test]
    fn position_carries_across_passes() {
        // Pass 1 ends at (10,0,0). Pass 2 starts at (20,0,0).
        // The implicit move from (10,0,0) to (20,0,0) must be simulated.
        let tp = simple_toolpath(vec![
            Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    pt(0.0, 0.0, 0.0, MoveKind::Feed),
                    pt(10.0, 0.0, 0.0, MoveKind::Feed),
                ],
            },
            Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    pt(20.0, 0.0, 0.0, MoveKind::Feed),
                    pt(30.0, 0.0, 0.0, MoveKind::Feed),
                ],
            },
        ]);

        let segs = toolpath_to_segments(&tp, None);
        assert_eq!(segs.len(), 3, "should include connecting segment between passes");
        // Segment 0: (0,0,0) → (10,0,0)  — pass 1
        assert_eq!(segs[0], MotionSegment::Linear {
            start: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            end: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
        });
        // Segment 1: (10,0,0) → (20,0,0)  — implicit connecting move
        assert_eq!(segs[1], MotionSegment::Linear {
            start: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            end: Vec3 { x: 20.0, y: 0.0, z: 0.0 },
        });
        // Segment 2: (20,0,0) → (30,0,0)  — pass 2
        assert_eq!(segs[2], MotionSegment::Linear {
            start: Vec3 { x: 20.0, y: 0.0, z: 0.0 },
            end: Vec3 { x: 30.0, y: 0.0, z: 0.0 },
        });
    }

    #[test]
    fn initial_position_generates_connecting_segment() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                pt(10.0, 0.0, 0.0, MoveKind::Rapid),
                pt(20.0, 0.0, 0.0, MoveKind::Feed),
            ],
        }]);

        let start = Vec3 { x: 0.0, y: 0.0, z: 5.0 };
        let segs = toolpath_to_segments(&tp, Some(&start));
        assert_eq!(segs.len(), 2);
        // Segment 0: initial position (0,0,5) → first point (10,0,0)
        assert_eq!(segs[0], MotionSegment::Linear {
            start: Vec3 { x: 0.0, y: 0.0, z: 5.0 },
            end: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
        });
        // Segment 1: (10,0,0) → (20,0,0)
        assert_eq!(segs[1], MotionSegment::Linear {
            start: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            end: Vec3 { x: 20.0, y: 0.0, z: 0.0 },
        });
    }
}

// ── gcode_segments_to_dexel ──────────────────────────────────────────────────

/// Convert a slice of G-code parser [`gcode_parser::MotionSegment`]s into
/// dexel [`MotionSegment`]s suitable for the material-removal engine.
///
/// Mapping rules:
/// - `Rapid { start, end, .. }` → `Linear { start, end }` (rapid traversals still
///   sweep the tool through the model in the dexel simulation).
/// - `Linear { start, end, .. }` → `Linear { start, end }`.
/// - `Arc { start, end, center, clockwise, .. }` → `Arc { start, end, center, clockwise }`.
/// - `Dwell { .. }` → skipped (no positional change).
pub fn gcode_segments_to_dexel(segments: &[gcode_parser::MotionSegment]) -> Vec<MotionSegment> {
    segments
        .iter()
        .filter_map(|seg| match seg {
            gcode_parser::MotionSegment::Rapid { start, end, .. } => Some(MotionSegment::Linear {
                start: start.clone(),
                end: end.clone(),
            }),
            gcode_parser::MotionSegment::Linear { start, end, .. } => Some(MotionSegment::Linear {
                start: start.clone(),
                end: end.clone(),
            }),
            gcode_parser::MotionSegment::Arc {
                start,
                end,
                center,
                clockwise,
                ..
            } => Some(MotionSegment::Arc {
                start: start.clone(),
                end: end.clone(),
                center: center.clone(),
                clockwise: *clockwise,
            }),
            gcode_parser::MotionSegment::Dwell { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod gcode_to_dexel_tests {
    use super::*;
    use crate::gcode_parser::types::{FeedMode, Plane, SegmentMetadata, SpindleDir};
    use crate::models::Vec3;

    fn meta() -> SegmentMetadata {
        SegmentMetadata {
            source_line: 1,
            tool_number: 1,
            spindle_speed: 0.0,
            spindle_dir: SpindleDir::Off,
            feed_mode: FeedMode::PerMinute,
        }
    }

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    #[test]
    fn rapid_maps_to_linear() {
        let segs = vec![gcode_parser::MotionSegment::Rapid {
            start: v(0.0, 0.0, 0.0),
            end: v(10.0, 0.0, 0.0),
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
            }
        );
    }

    #[test]
    fn linear_maps_to_linear() {
        let segs = vec![gcode_parser::MotionSegment::Linear {
            start: v(0.0, 0.0, 0.0),
            end: v(5.0, 5.0, 5.0),
            feed_rate: 1000.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(5.0, 5.0, 5.0),
            }
        );
    }

    #[test]
    fn arc_maps_to_arc_with_same_geometry() {
        let segs = vec![gcode_parser::MotionSegment::Arc {
            start: v(10.0, 0.0, 0.0),
            end: v(0.0, 10.0, 0.0),
            center: v(0.0, 0.0, 0.0),
            clockwise: false,
            plane: Plane::Xy,
            feed_rate: 500.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Arc {
                start: v(10.0, 0.0, 0.0),
                end: v(0.0, 10.0, 0.0),
                center: v(0.0, 0.0, 0.0),
                clockwise: false,
            }
        );
    }

    #[test]
    fn dwell_produces_no_segment() {
        let segs = vec![gcode_parser::MotionSegment::Dwell {
            position: v(0.0, 0.0, 0.0),
            seconds: 1.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_input_produces_empty_output() {
        let result = gcode_segments_to_dexel(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn mixed_types_correct_count_and_order() {
        let segs = vec![
            gcode_parser::MotionSegment::Rapid {
                start: v(0.0, 0.0, 0.0),
                end: v(5.0, 0.0, 0.0),
                metadata: meta(),
            },
            gcode_parser::MotionSegment::Dwell {
                position: v(5.0, 0.0, 0.0),
                seconds: 0.5,
                metadata: meta(),
            },
            gcode_parser::MotionSegment::Linear {
                start: v(5.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
                feed_rate: 500.0,
                metadata: meta(),
            },
            gcode_parser::MotionSegment::Arc {
                start: v(10.0, 0.0, 0.0),
                end: v(0.0, 10.0, 0.0),
                center: v(0.0, 0.0, 0.0),
                clockwise: true,
                plane: Plane::Xy,
                feed_rate: 500.0,
                metadata: meta(),
            },
        ];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 3); // Dwell skipped; Rapid+Linear+Arc each produce one segment
        assert!(matches!(result[0], MotionSegment::Linear { .. }));
        assert!(matches!(result[1], MotionSegment::Linear { .. }));
        assert!(matches!(result[2], MotionSegment::Arc { .. }));
    }
}
