//! Canned cycle expansion for G81/G82/G83/G73.

use crate::models::Vec3;

use super::state::{CycleMode, ModalState, RetractMode};
use super::types::{MotionSegment, ParseWarning};

/// Small clearance distance (mm) for rapid approach in peck cycles.
const PECK_CLEARANCE: f64 = 1.0;

/// Expand a canned cycle at the current hole position.
///
/// The caller must have already resolved X/Y into `state.position` and stored
/// cycle parameters (`cycle_r`, `cycle_z`, `cycle_q`, `cycle_p`) before calling.
pub(crate) fn expand_cycle(
    state: &mut ModalState,
    _x: Option<f64>,
    _y: Option<f64>,
    l_count: u32,
    source_line: usize,
) -> (Vec<MotionSegment>, Vec<ParseWarning>) {
    let cycle_mode = match &state.cycle_active {
        Some(mode) => mode.clone(),
        None => return (Vec::new(), Vec::new()),
    };

    let mut segments = Vec::new();
    let mut warnings = Vec::new();

    let hole_x = state.position.x;
    let hole_y = state.position.y;
    let r = state.cycle_r;
    let z = state.cycle_z;
    let feed_rate = state.feed_rate;
    let retract_z = match state.retract_mode {
        RetractMode::InitialPoint => state.initial_z,
        RetractMode::RPlane => r,
    };
    let metadata = state.build_segment_metadata(source_line);

    for _ in 0..l_count {
        // Rapid to R-plane at hole position
        let r_pos = Vec3 {
            x: hole_x,
            y: hole_y,
            z: r,
        };
        segments.push(MotionSegment::Rapid {
            start: state.position.clone(),
            end: r_pos.clone(),
            metadata: metadata.clone(),
        });
        let mut pos = r_pos;

        match cycle_mode {
            CycleMode::G81 => {
                expand_simple_drill(
                    &mut segments,
                    &mut pos,
                    hole_x,
                    hole_y,
                    z,
                    retract_z,
                    feed_rate,
                    &metadata,
                );
            }
            CycleMode::G82 => {
                // Feed to depth
                let z_pos = Vec3 {
                    x: hole_x,
                    y: hole_y,
                    z,
                };
                segments.push(MotionSegment::Linear {
                    start: pos.clone(),
                    end: z_pos.clone(),
                    feed_rate,
                    metadata: metadata.clone(),
                });
                pos = z_pos;

                // Dwell at bottom
                segments.push(MotionSegment::Dwell {
                    position: pos.clone(),
                    seconds: state.cycle_p,
                    metadata: metadata.clone(),
                });

                // Rapid retract
                let ret_pos = Vec3 {
                    x: hole_x,
                    y: hole_y,
                    z: retract_z,
                };
                segments.push(MotionSegment::Rapid {
                    start: pos,
                    end: ret_pos.clone(),
                    metadata: metadata.clone(),
                });
                pos = ret_pos;
            }
            CycleMode::G83 => {
                let q = state.cycle_q;
                if q <= 0.0 {
                    warnings.push(ParseWarning {
                        line: source_line,
                        message: "G83 peck cycle Q <= 0, using single plunge".to_string(),
                    });
                    expand_simple_drill(
                        &mut segments,
                        &mut pos,
                        hole_x,
                        hole_y,
                        z,
                        retract_z,
                        feed_rate,
                        &metadata,
                    );
                } else {
                    expand_peck_full_retract(
                        &mut segments,
                        &mut pos,
                        hole_x,
                        hole_y,
                        r,
                        z,
                        q,
                        retract_z,
                        feed_rate,
                        &metadata,
                    );
                }
            }
            CycleMode::G73 => {
                let q = state.cycle_q;
                if q <= 0.0 {
                    warnings.push(ParseWarning {
                        line: source_line,
                        message: "G73 chip break cycle Q <= 0, using single plunge".to_string(),
                    });
                    expand_simple_drill(
                        &mut segments,
                        &mut pos,
                        hole_x,
                        hole_y,
                        z,
                        retract_z,
                        feed_rate,
                        &metadata,
                    );
                } else {
                    expand_peck_chip_break(
                        &mut segments,
                        &mut pos,
                        hole_x,
                        hole_y,
                        r,
                        z,
                        q,
                        retract_z,
                        feed_rate,
                        &metadata,
                    );
                }
            }
        }

        state.position = pos;
    }

    (segments, warnings)
}

/// G81-style simple drill: feed to depth, rapid retract.
#[allow(clippy::too_many_arguments)]
fn expand_simple_drill(
    segments: &mut Vec<MotionSegment>,
    pos: &mut Vec3,
    hole_x: f64,
    hole_y: f64,
    z: f64,
    retract_z: f64,
    feed_rate: f64,
    metadata: &super::types::SegmentMetadata,
) {
    let z_pos = Vec3 {
        x: hole_x,
        y: hole_y,
        z,
    };
    segments.push(MotionSegment::Linear {
        start: pos.clone(),
        end: z_pos.clone(),
        feed_rate,
        metadata: metadata.clone(),
    });
    *pos = z_pos;

    let ret_pos = Vec3 {
        x: hole_x,
        y: hole_y,
        z: retract_z,
    };
    segments.push(MotionSegment::Rapid {
        start: pos.clone(),
        end: ret_pos.clone(),
        metadata: metadata.clone(),
    });
    *pos = ret_pos;
}

/// G83 peck drill with full retract to R-plane between pecks.
#[allow(clippy::too_many_arguments)]
fn expand_peck_full_retract(
    segments: &mut Vec<MotionSegment>,
    pos: &mut Vec3,
    hole_x: f64,
    hole_y: f64,
    r: f64,
    z: f64,
    q: f64,
    retract_z: f64,
    feed_rate: f64,
    metadata: &super::types::SegmentMetadata,
) {
    let mut prev_depth = r;
    let mut peck_num = 0u32;

    loop {
        peck_num += 1;
        let target_depth = (r - (peck_num as f64) * q).max(z);

        if peck_num > 1 {
            // Retract to R-plane
            let r_pos = Vec3 {
                x: hole_x,
                y: hole_y,
                z: r,
            };
            segments.push(MotionSegment::Rapid {
                start: pos.clone(),
                end: r_pos.clone(),
                metadata: metadata.clone(),
            });
            *pos = r_pos;

            // Rapid approach to just above previous depth
            let approach_z = prev_depth + PECK_CLEARANCE;
            if approach_z < r {
                let approach_pos = Vec3 {
                    x: hole_x,
                    y: hole_y,
                    z: approach_z,
                };
                segments.push(MotionSegment::Rapid {
                    start: pos.clone(),
                    end: approach_pos.clone(),
                    metadata: metadata.clone(),
                });
                *pos = approach_pos;
            }
        }

        // Feed to target depth
        let depth_pos = Vec3 {
            x: hole_x,
            y: hole_y,
            z: target_depth,
        };
        segments.push(MotionSegment::Linear {
            start: pos.clone(),
            end: depth_pos.clone(),
            feed_rate,
            metadata: metadata.clone(),
        });
        *pos = depth_pos;
        prev_depth = target_depth;

        if target_depth <= z {
            break;
        }
    }

    // Final retract
    let ret_pos = Vec3 {
        x: hole_x,
        y: hole_y,
        z: retract_z,
    };
    segments.push(MotionSegment::Rapid {
        start: pos.clone(),
        end: ret_pos.clone(),
        metadata: metadata.clone(),
    });
    *pos = ret_pos;
}

/// G73 chip-break peck drill with partial retract (1mm) between pecks.
#[allow(clippy::too_many_arguments)]
fn expand_peck_chip_break(
    segments: &mut Vec<MotionSegment>,
    pos: &mut Vec3,
    hole_x: f64,
    hole_y: f64,
    r: f64,
    z: f64,
    q: f64,
    retract_z: f64,
    feed_rate: f64,
    metadata: &super::types::SegmentMetadata,
) {
    let mut peck_num = 0u32;

    loop {
        peck_num += 1;
        let target_depth = (r - (peck_num as f64) * q).max(z);

        if peck_num > 1 {
            // Partial retract: 1mm above current depth
            let retract_pos = Vec3 {
                x: hole_x,
                y: hole_y,
                z: pos.z + PECK_CLEARANCE,
            };
            segments.push(MotionSegment::Rapid {
                start: pos.clone(),
                end: retract_pos.clone(),
                metadata: metadata.clone(),
            });
            *pos = retract_pos;
        }

        // Feed to target depth
        let depth_pos = Vec3 {
            x: hole_x,
            y: hole_y,
            z: target_depth,
        };
        segments.push(MotionSegment::Linear {
            start: pos.clone(),
            end: depth_pos.clone(),
            feed_rate,
            metadata: metadata.clone(),
        });
        *pos = depth_pos;

        if target_depth <= z {
            break;
        }
    }

    // Final retract
    let ret_pos = Vec3 {
        x: hole_x,
        y: hole_y,
        z: retract_z,
    };
    segments.push(MotionSegment::Rapid {
        start: pos.clone(),
        end: ret_pos.clone(),
        metadata: metadata.clone(),
    });
    *pos = ret_pos;
}

#[cfg(test)]
mod tests {
    use super::super::interpreter::interpret_line;
    use super::super::state::{ModalState, RetractMode};
    use super::super::tokenizer::tokenize_line;
    use super::super::types::MotionSegment;
    use crate::models::Vec3;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    fn interp(
        line: &str,
        state: &mut ModalState,
        source_line: usize,
    ) -> super::super::interpreter::LineResult {
        let tokens = tokenize_line(line);
        interpret_line(&tokens.words, state, source_line)
    }

    // --- G81 basic: rapid to R, feed to Z, rapid retract ---

    #[test]
    fn g81_basic() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        let result = interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);

        assert_eq!(result.segments.len(), 3);

        // Rapid to R-plane
        match &result.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(10.0, 20.0, 5.0));
                assert_eq!(*end, v(10.0, 20.0, 2.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        // Feed to depth
        match &result.segments[1] {
            MotionSegment::Linear {
                start,
                end,
                feed_rate,
                ..
            } => {
                assert_eq!(*start, v(10.0, 20.0, 2.0));
                assert_eq!(*end, v(10.0, 20.0, -5.0));
                assert_eq!(*feed_rate, 100.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        // Rapid retract to initial_z (G98 default)
        match &result.segments[2] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(10.0, 20.0, -5.0));
                assert_eq!(*end, v(10.0, 20.0, 5.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- G81 multiple positions: 3 holes × 3 segments = 9 ---

    #[test]
    fn g81_multiple_positions() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);

        let r1 = interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);
        assert_eq!(r1.segments.len(), 3);

        let r2 = interp("X30 Y40", &mut state, 2);
        assert_eq!(r2.segments.len(), 3);

        let r3 = interp("X50 Y60", &mut state, 3);
        assert_eq!(r3.segments.len(), 3);

        // Verify hole 2 positions
        match &r2.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(30.0, 40.0, 5.0));
                assert_eq!(*end, v(30.0, 40.0, 2.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
        match &r2.segments[1] {
            MotionSegment::Linear { end, .. } => {
                assert_eq!(*end, v(30.0, 40.0, -5.0));
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        // Total across all calls
        assert_eq!(r1.segments.len() + r2.segments.len() + r3.segments.len(), 9);
    }

    // --- G82: dwell at bottom ---

    #[test]
    fn g82_dwell_at_bottom() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        let result = interp("G82 X10 Y20 R2 Z-5 P1.5 F100", &mut state, 1);

        assert_eq!(result.segments.len(), 4);

        // Rapid to R
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => assert_eq!(*end, v(10.0, 20.0, 2.0)),
            other => panic!("expected Rapid, got {:?}", other),
        }
        // Feed to depth
        match &result.segments[1] {
            MotionSegment::Linear { end, .. } => assert_eq!(*end, v(10.0, 20.0, -5.0)),
            other => panic!("expected Linear, got {:?}", other),
        }
        // Dwell
        match &result.segments[2] {
            MotionSegment::Dwell {
                position, seconds, ..
            } => {
                assert_eq!(*position, v(10.0, 20.0, -5.0));
                assert_eq!(*seconds, 1.5);
            }
            other => panic!("expected Dwell, got {:?}", other),
        }
        // Retract
        match &result.segments[3] {
            MotionSegment::Rapid { end, .. } => assert_eq!(*end, v(10.0, 20.0, 5.0)),
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- G83 peck drill ---

    #[test]
    fn g83_peck_drill() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 10.0);
        let result = interp("G83 X0 Y0 R2 Z-30 Q10 F100", &mut state, 1);

        // 4 pecks: R=2, Q=10 → depths -8, -18, -28, -30
        // Peck 1: rapid-to-R + feed = 2 segments
        // Peck 2: retract-to-R + approach + feed = 3 segments
        // Peck 3: retract-to-R + approach + feed = 3 segments
        // Peck 4: retract-to-R + approach + feed = 3 segments
        // Final retract: 1 segment
        // Total: 1 + 1 + 3 + 3 + 3 + 1 = 12
        assert_eq!(result.segments.len(), 12);

        // First segment: rapid to R
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, 2.0),
            other => panic!("expected Rapid, got {:?}", other),
        }

        // Peck 1 feed to -8
        match &result.segments[1] {
            MotionSegment::Linear { start, end, .. } => {
                assert_eq!(start.z, 2.0);
                assert_eq!(end.z, -8.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        // Peck 2: retract to R
        match &result.segments[2] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(start.z, -8.0);
                assert_eq!(end.z, 2.0);
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        // Peck 2: approach to -7
        match &result.segments[3] {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, -7.0),
            other => panic!("expected Rapid, got {:?}", other),
        }

        // Peck 2: feed to -18
        match &result.segments[4] {
            MotionSegment::Linear { start, end, .. } => {
                assert_eq!(start.z, -7.0);
                assert_eq!(end.z, -18.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        // Final retract (last segment)
        match result.segments.last().unwrap() {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(start.z, -30.0);
                assert_eq!(end.z, 10.0); // G98 retract to initial_z
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- G73 chip break ---

    #[test]
    fn g73_chip_break() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 10.0);
        let result = interp("G73 X0 Y0 R2 Z-30 Q10 F100", &mut state, 1);

        // 4 pecks: depths -8, -18, -28, -30
        // Peck 1: rapid-to-R + feed = 2 segments
        // Peck 2: partial-retract + feed = 2 segments
        // Peck 3: partial-retract + feed = 2 segments
        // Peck 4: partial-retract + feed = 2 segments
        // Final retract: 1 segment
        // Total: 2 + 2 + 2 + 2 + 1 = 9
        assert_eq!(result.segments.len(), 9);

        // Peck 2: partial retract (1mm above peck 1 depth of -8)
        match &result.segments[2] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(start.z, -8.0);
                assert_eq!(end.z, -7.0); // only 1mm retract
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        // Peck 2: feed from -7 to -18
        match &result.segments[3] {
            MotionSegment::Linear { start, end, .. } => {
                assert_eq!(start.z, -7.0);
                assert_eq!(end.z, -18.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        // Final retract
        match result.segments.last().unwrap() {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, 10.0),
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- Q <= 0 produces warning + single plunge ---

    #[test]
    fn g83_q_zero_warning_single_plunge() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        let result = interp("G83 X10 Y20 R2 Z-5 Q0 F100", &mut state, 1);

        // 3 segments like G81: rapid to R, feed to Z, rapid retract
        assert_eq!(result.segments.len(), 3);
        assert!(result.warnings.iter().any(|w| w.message.contains("Q <= 0")));
    }

    #[test]
    fn g73_q_zero_warning_single_plunge() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        let result = interp("G73 X10 Y20 R2 Z-5 Q0 F100", &mut state, 1);

        assert_eq!(result.segments.len(), 3);
        assert!(result.warnings.iter().any(|w| w.message.contains("Q <= 0")));
    }

    // --- L > 1 repeats cycle ---

    #[test]
    fn l_count_repeats_cycle() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        let result = interp("G81 X10 Y20 R2 Z-5 L3 F100", &mut state, 1);

        // 3 segments per cycle × 3 repetitions = 9
        assert_eq!(result.segments.len(), 9);
    }

    // --- G98 vs G99 retract mode ---

    #[test]
    fn g98_retracts_to_initial_z() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 10.0);
        // G98 is the default retract mode
        let result = interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);

        // Retract goes to initial_z = 10.0
        match result.segments.last().unwrap() {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, 10.0),
            other => panic!("expected Rapid, got {:?}", other),
        }
        assert_eq!(state.position.z, 10.0);
    }

    #[test]
    fn g99_retracts_to_r_plane() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 10.0);
        state.retract_mode = RetractMode::RPlane; // G99
        let result = interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);

        // Retract goes to R = 2.0
        match result.segments.last().unwrap() {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, 2.0),
            other => panic!("expected Rapid, got {:?}", other),
        }
        assert_eq!(state.position.z, 2.0);
    }

    // --- G80 cancels cycle, subsequent XY uses normal motion ---

    #[test]
    fn g80_cancel_then_normal_motion() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);

        // Activate G81
        interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);
        assert!(state.cycle_active.is_some());

        // Cancel with G80
        interp("G80", &mut state, 2);
        assert!(state.cycle_active.is_none());

        // Now X/Y should use normal motion (G1 default with warning)
        let result = interp("X30 Y40", &mut state, 3);
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear { .. } => {} // normal motion, not cycle
            other => panic!("expected Linear (normal motion), got {:?}", other),
        }
    }

    // --- Cycle triggered by XY-only line ---

    #[test]
    fn cycle_triggered_by_xy_only_line() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);

        // Activate G81
        let r1 = interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);
        assert_eq!(r1.segments.len(), 3);

        // XY-only line triggers cycle again
        let r2 = interp("X30 Y40", &mut state, 2);
        assert_eq!(r2.segments.len(), 3);

        // Verify it's a cycle expansion, not normal motion
        match &r2.segments[0] {
            MotionSegment::Rapid { end, .. } => assert_eq!(end.z, 2.0), // rapid to R-plane
            other => panic!("expected Rapid to R-plane, got {:?}", other),
        }
        match &r2.segments[1] {
            MotionSegment::Linear { end, .. } => assert_eq!(end.z, -5.0), // feed to depth
            other => panic!("expected Linear to depth, got {:?}", other),
        }
    }

    // --- Position tracking after cycle ---

    #[test]
    fn position_after_cycle_is_retract_height() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 10.0);

        interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);
        assert_eq!(state.position, v(10.0, 20.0, 10.0)); // G98: retract to initial_z

        // G99 mode
        state.position = v(0.0, 0.0, 10.0);
        state.retract_mode = RetractMode::RPlane;
        state.cycle_active = None;
        interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 2);
        assert_eq!(state.position, v(10.0, 20.0, 2.0)); // G99: retract to R
    }
}
