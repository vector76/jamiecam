use crate::dexel::MotionSegment;
use crate::toolpath::types::MoveKind;
use crate::toolpath::Toolpath;

/// Convert a [`Toolpath`] into a flat list of [`MotionSegment`] values
/// suitable for the dexel material-removal engine.
///
/// The first [`CutPoint`] in each pass establishes position only; subsequent
/// points are paired with the previous position to produce segments.
/// [`MoveKind::Dwell`] points are skipped (no geometry).
pub fn toolpath_to_segments(toolpath: &Toolpath) -> Vec<MotionSegment> {
    let mut segments = Vec::new();

    for pass in &toolpath.passes {
        let mut prev_position = match pass.cuts.first() {
            Some(cp) => cp.position.clone(),
            None => continue,
        };

        for cp in pass.cuts.iter().skip(1) {
            let seg = match &cp.move_kind {
                MoveKind::Rapid | MoveKind::Feed => Some(MotionSegment::Linear {
                    start: prev_position.clone(),
                    end: cp.position.clone(),
                }),
                MoveKind::Arc {
                    center, clockwise, ..
                } => Some(MotionSegment::Arc {
                    start: prev_position.clone(),
                    end: cp.position.clone(),
                    center: center.clone(),
                    clockwise: *clockwise,
                }),
                MoveKind::Dwell { .. } => None,
            };

            if let Some(s) = seg {
                segments.push(s);
            }
            prev_position = cp.position.clone();
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

        let segs = toolpath_to_segments(&tp);
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

        let segs = toolpath_to_segments(&tp);
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

        let segs = toolpath_to_segments(&tp);
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

        let segs = toolpath_to_segments(&tp);
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
        let segs = toolpath_to_segments(&tp);
        assert!(segs.is_empty());
    }

    #[test]
    fn single_cutpoint_produces_no_segments() {
        let tp = simple_toolpath(vec![Pass {
            kind: PassKind::Cutting,
            cuts: vec![pt(0.0, 0.0, 0.0, MoveKind::Feed)],
        }]);

        let segs = toolpath_to_segments(&tp);
        assert!(segs.is_empty());
    }
}
