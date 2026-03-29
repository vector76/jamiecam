//! Line interpreter: semantic layer that processes tokenized G-code words,
//! updates modal state, and emits motion segments.

#![allow(dead_code)]

use crate::models::Vec3;

use super::cycles::expand_cycle;
use super::modal::{classify_gcode, classify_mcode, MCodeAction, ModalGroup};
use super::state::{CycleMode, DistanceMode, ModalState, MotionMode, RetractMode};
use super::tokenizer::GcodeWord;
use super::types::{
    FeedMode, MotionSegment, ParseWarning, Plane, SegmentMetadata, SpindleDir, ToolChange, Units,
};

/// Result of interpreting a single G-code line.
#[derive(Debug, Clone)]
pub(crate) struct LineResult {
    pub segments: Vec<MotionSegment>,
    pub warnings: Vec<ParseWarning>,
    pub tool_change: Option<ToolChange>,
    pub program_end: bool,
}

/// Interpret a tokenized line's words: update modal state, emit motion segments.
pub(crate) fn interpret_line(
    words: &[GcodeWord],
    state: &mut ModalState,
    source_line: usize,
) -> LineResult {
    let mut segments = Vec::new();
    let mut warnings = Vec::new();
    let mut tool_change = None;
    let mut program_end = false;

    // === First pass: classify and collect ===

    let mut motion_code: Option<f64> = None;
    let mut plane_code: Option<f64> = None;
    let mut distance_code: Option<f64> = None;
    let mut feed_mode_code: Option<f64> = None;
    let mut units_code: Option<f64> = None;
    let mut canned_cycle_code: Option<f64> = None;
    let mut cycle_retract_code: Option<f64> = None;
    let mut non_modal_code: Option<f64> = None;
    let mut cutter_comp_code: Option<f64> = None;
    let mut unrecognized_g: Vec<f64> = Vec::new();

    let mut m_actions: Vec<(MCodeAction, f64)> = Vec::new();
    let mut unrecognized_m: Vec<f64> = Vec::new();

    let mut x_word: Option<f64> = None;
    let mut y_word: Option<f64> = None;
    let mut z_word: Option<f64> = None;
    let mut i_word: Option<f64> = None;
    let mut j_word: Option<f64> = None;
    let mut k_word: Option<f64> = None;
    let mut r_word: Option<f64> = None;
    let mut f_word: Option<f64> = None;
    let mut s_word: Option<f64> = None;
    let mut t_word: Option<f64> = None;
    let mut p_word: Option<f64> = None;
    let mut q_word: Option<f64> = None;
    let mut l_word: Option<f64> = None;

    for word in words {
        match word.letter {
            'G' => match classify_gcode(word.value) {
                Some(ModalGroup::Motion) => motion_code = Some(word.value),
                Some(ModalGroup::PlaneSelection) => plane_code = Some(word.value),
                Some(ModalGroup::DistanceMode) => distance_code = Some(word.value),
                Some(ModalGroup::FeedMode) => feed_mode_code = Some(word.value),
                Some(ModalGroup::Units) => units_code = Some(word.value),
                Some(ModalGroup::CannedCycle) => canned_cycle_code = Some(word.value),
                Some(ModalGroup::CycleRetract) => cycle_retract_code = Some(word.value),
                Some(ModalGroup::NonModal) => non_modal_code = Some(word.value),
                Some(ModalGroup::ToolLengthComp) => {} // noted silently
                Some(ModalGroup::WorkOffset) => {}     // noted silently
                Some(ModalGroup::CutterComp) => cutter_comp_code = Some(word.value),
                None => unrecognized_g.push(word.value),
            },
            'M' => match classify_mcode(word.value) {
                Some(action) => m_actions.push((action, word.value)),
                None => unrecognized_m.push(word.value),
            },
            'X' => x_word = Some(word.value),
            'Y' => y_word = Some(word.value),
            'Z' => z_word = Some(word.value),
            'I' => i_word = Some(word.value),
            'J' => j_word = Some(word.value),
            'K' => k_word = Some(word.value),
            'R' => r_word = Some(word.value),
            'F' => f_word = Some(word.value),
            'S' => s_word = Some(word.value),
            'T' => t_word = Some(word.value),
            'P' => p_word = Some(word.value),
            'Q' => q_word = Some(word.value),
            'L' => l_word = Some(word.value),
            _ => {} // N-words, O-words, etc. — ignored
        }
    }

    // === Second pass: apply state changes (in specified order) ===

    if let Some(code) = units_code {
        match code.round() as i32 {
            20 => state.set_units(Units::Imperial),
            21 => state.set_units(Units::Metric),
            _ => {}
        }
    }

    if let Some(code) = distance_code {
        match code.round() as i32 {
            90 => state.set_distance_mode(DistanceMode::Absolute),
            91 => state.set_distance_mode(DistanceMode::Incremental),
            _ => {}
        }
    }

    if let Some(code) = plane_code {
        match code.round() as i32 {
            17 => state.set_plane(Plane::Xy),
            18 => state.set_plane(Plane::Xz),
            19 => state.set_plane(Plane::Yz),
            _ => {}
        }
    }

    if let Some(code) = feed_mode_code {
        match code.round() as i32 {
            93 => state.set_feed_mode(FeedMode::InverseTime),
            94 => state.set_feed_mode(FeedMode::PerMinute),
            95 => state.set_feed_mode(FeedMode::PerRevolution),
            _ => {}
        }
    }

    if let Some(code) = cycle_retract_code {
        match code.round() as i32 {
            98 => state.retract_mode = RetractMode::InitialPoint,
            99 => state.retract_mode = RetractMode::RPlane,
            _ => {}
        }
    }

    for (action, value) in &m_actions {
        if *action == MCodeAction::Spindle {
            match value.round() as i32 {
                3 => state.set_spindle(SpindleDir::Cw),
                4 => state.set_spindle(SpindleDir::Ccw),
                5 => state.set_spindle(SpindleDir::Off),
                _ => {}
            }
        }
    }

    if let Some(f) = f_word {
        state.feed_rate = state.normalize_feed_rate(f);
    }

    if let Some(s) = s_word {
        state.set_spindle_speed(s);
    }

    if let Some(t) = t_word {
        state.stage_tool(t as u32);
    }

    if let Some(code) = cutter_comp_code {
        warnings.push(ParseWarning {
            line: source_line,
            message: format!("cutter compensation G{} not supported", code.round() as i32),
        });
    }

    for code in &unrecognized_g {
        warnings.push(ParseWarning {
            line: source_line,
            message: format!("unrecognized G-code G{}", code),
        });
    }

    for code in &unrecognized_m {
        warnings.push(ParseWarning {
            line: source_line,
            message: format!("unrecognized M-code M{}", code),
        });
    }

    for (action, value) in &m_actions {
        if *action == MCodeAction::Subprogram {
            warnings.push(ParseWarning {
                line: source_line,
                message: format!("subprogram call M{} not supported", value.round() as i32),
            });
        }
    }

    // === Third pass: determine action ===

    // M2/M30 → program end
    for (action, _) in &m_actions {
        if *action == MCodeAction::ProgramEnd {
            program_end = true;
        }
    }

    // M6 → tool change
    for (action, _) in &m_actions {
        if *action == MCodeAction::ToolChange {
            let tool_num = state.activate_tool();
            tool_change = Some(ToolChange {
                segment_index: segments.len(),
                tool_number: tool_num,
            });
        }
    }

    // Non-modal actions
    if let Some(code) = non_modal_code {
        match code.round() as i32 {
            4 => {
                let seconds = p_word.unwrap_or(0.0);
                let metadata = state.build_segment_metadata(source_line);
                segments.push(MotionSegment::Dwell {
                    position: state.position.clone(),
                    seconds,
                    metadata,
                });
            }
            28 => {
                let intermediate = state.resolve_position(x_word, y_word, z_word);
                let metadata = state.build_segment_metadata(source_line);
                segments.push(MotionSegment::Rapid {
                    start: state.position.clone(),
                    end: intermediate.clone(),
                    metadata,
                });
                state.position = intermediate;
                warnings.push(ParseWarning {
                    line: source_line,
                    message:
                        "G28: machine home position unknown, intermediate point used as endpoint"
                            .to_string(),
                });
                return LineResult {
                    segments,
                    warnings,
                    tool_change,
                    program_end,
                };
            }
            _ => {}
        }
    }

    // Canned cycles
    if let Some(code) = canned_cycle_code {
        let int_code = code.round() as i32;
        if int_code == 80 {
            state.cycle_active = None;
        } else {
            let cycle_mode = match int_code {
                73 => CycleMode::G73,
                81 => CycleMode::G81,
                82 => CycleMode::G82,
                83 => CycleMode::G83,
                _ => unreachable!(),
            };
            state.cycle_active = Some(cycle_mode);

            let scale = match state.units {
                Units::Imperial => 25.4,
                Units::Metric => 1.0,
            };
            if let Some(r) = r_word {
                state.cycle_r = r * scale;
            }
            if let Some(z) = z_word {
                state.cycle_z = z * scale;
            }
            if let Some(q) = q_word {
                state.cycle_q = q * scale;
            }
            if let Some(p) = p_word {
                state.cycle_p = p;
            }
            state.initial_z = state.position.z;

            // X/Y update position for hole location; Z is routed to cycle state
            let hole_pos = state.resolve_position(x_word, y_word, None);
            state.position = hole_pos;

            let l_count = l_word.map(|l| l as u32).unwrap_or(1);
            let (cycle_segments, cycle_warnings) =
                expand_cycle(state, x_word, y_word, l_count, source_line);
            segments.extend(cycle_segments);
            warnings.extend(cycle_warnings);

            return LineResult {
                segments,
                warnings,
                tool_change,
                program_end,
            };
        }
    }

    // Modal cycle execution: cycle is active, X/Y present, no cycle or motion G-code
    if canned_cycle_code.is_none()
        && state.cycle_active.is_some()
        && (x_word.is_some() || y_word.is_some())
        && motion_code.is_none()
    {
        let hole_pos = state.resolve_position(x_word, y_word, None);
        state.position = hole_pos;

        let l_count = l_word.map(|l| l as u32).unwrap_or(1);
        let (cycle_segments, cycle_warnings) =
            expand_cycle(state, x_word, y_word, l_count, source_line);
        segments.extend(cycle_segments);
        warnings.extend(cycle_warnings);

        return LineResult {
            segments,
            warnings,
            tool_change,
            program_end,
        };
    }

    // Motion mode update from this line's G-code
    if let Some(code) = motion_code {
        let mode = match code.round() as i32 {
            0 => MotionMode::Rapid,
            1 => MotionMode::Linear,
            2 => MotionMode::CwArc,
            3 => MotionMode::CcwArc,
            _ => unreachable!(),
        };
        state.set_motion_mode(mode);
    }

    // Emit motion if axis words or arc params are present
    let has_axis_words = x_word.is_some() || y_word.is_some() || z_word.is_some();
    let has_arc_params =
        i_word.is_some() || j_word.is_some() || k_word.is_some() || r_word.is_some();

    if has_axis_words || has_arc_params {
        if state.motion_mode.is_none() {
            if !has_axis_words {
                // Only arc params with no motion mode — nothing actionable
                return LineResult {
                    segments,
                    warnings,
                    tool_change,
                    program_end,
                };
            }
            warnings.push(ParseWarning {
                line: source_line,
                message: "axis words without prior motion mode, defaulting to G1".to_string(),
            });
            state.set_motion_mode(MotionMode::Linear);
        }

        let mode = state.motion_mode.clone().unwrap();
        let resolved = state.resolve_position(x_word, y_word, z_word);
        let metadata = state.build_segment_metadata(source_line);

        match mode {
            MotionMode::Rapid => {
                segments.push(MotionSegment::Rapid {
                    start: state.position.clone(),
                    end: resolved.clone(),
                    metadata,
                });
            }
            MotionMode::Linear => {
                if state.feed_rate == 0.0 && f_word.is_none() {
                    warnings.push(ParseWarning {
                        line: source_line,
                        message: "feed rate is zero for linear move".to_string(),
                    });
                }
                segments.push(MotionSegment::Linear {
                    start: state.position.clone(),
                    end: resolved.clone(),
                    feed_rate: state.feed_rate,
                    metadata,
                });
            }
            MotionMode::CwArc | MotionMode::CcwArc => {
                let clockwise = mode == MotionMode::CwArc;
                let (arc_seg, arc_warnings) = resolve_arc(
                    &state.position,
                    &resolved,
                    i_word,
                    j_word,
                    k_word,
                    r_word,
                    clockwise,
                    &state.plane,
                    state.feed_rate,
                    metadata,
                );
                if let Some(seg) = arc_seg {
                    segments.push(seg);
                }
                warnings.extend(arc_warnings);
            }
        }

        state.position = resolved;
    }

    LineResult {
        segments,
        warnings,
        tool_change,
        program_end,
    }
}

/// Arc resolution stub — replaced by the arc resolution bead.
#[allow(clippy::too_many_arguments)]
fn resolve_arc(
    _start: &Vec3,
    _end: &Vec3,
    _i: Option<f64>,
    _j: Option<f64>,
    _k: Option<f64>,
    _r: Option<f64>,
    _clockwise: bool,
    _plane: &Plane,
    _feed_rate: f64,
    metadata: SegmentMetadata,
) -> (Option<MotionSegment>, Vec<ParseWarning>) {
    let line = metadata.source_line;
    (
        None,
        vec![ParseWarning {
            line,
            message: "arc interpolation not yet implemented".to_string(),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::super::tokenizer::tokenize_line;
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    fn interp(line: &str, state: &mut ModalState, source_line: usize) -> LineResult {
        let tokens = tokenize_line(line);
        interpret_line(&tokens.words, state, source_line)
    }

    // --- Single G1 line ---

    #[test]
    fn single_g1_line() {
        let mut state = ModalState::default();
        let result = interp("G1 X10 Y20 F500", &mut state, 1);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear {
                start,
                end,
                feed_rate,
                ..
            } => {
                assert_eq!(*start, v(0.0, 0.0, 0.0));
                assert_eq!(*end, v(10.0, 20.0, 0.0));
                assert_eq!(*feed_rate, 500.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
        assert_eq!(state.position, v(10.0, 20.0, 0.0));
    }

    // --- Modal persistence ---

    #[test]
    fn modal_persistence() {
        let mut state = ModalState::default();
        interp("G1 X10 F500", &mut state, 1);
        let result = interp("Y20", &mut state, 2);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear {
                start,
                end,
                feed_rate,
                ..
            } => {
                assert_eq!(*start, v(10.0, 0.0, 0.0));
                assert_eq!(*end, v(10.0, 20.0, 0.0));
                assert_eq!(*feed_rate, 500.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- G90/G91/G90 switching ---

    #[test]
    fn distance_mode_switching() {
        let mut state = ModalState::default();

        // G90 absolute: move to (10, 0, 0)
        interp("G1 X10 F100", &mut state, 1);
        assert_eq!(state.position, v(10.0, 0.0, 0.0));

        // G91 incremental: +5 on X → (15, 0, 0)
        interp("G91", &mut state, 2);
        interp("X5", &mut state, 3);
        assert_eq!(state.position, v(15.0, 0.0, 0.0));

        // Back to G90 absolute: X20 → (20, 0, 0)
        interp("G90", &mut state, 4);
        interp("X20", &mut state, 5);
        assert_eq!(state.position, v(20.0, 0.0, 0.0));
    }

    // --- Unit conversion ---

    #[test]
    fn unit_conversion_imperial() {
        let mut state = ModalState::default();
        interp("G20", &mut state, 1);
        interp("G1 X1 F10", &mut state, 2);
        assert!((state.position.x - 25.4).abs() < 1e-10);
    }

    // --- Last wins ---

    #[test]
    fn last_wins_same_modal_group() {
        let mut state = ModalState::default();
        let result = interp("G0 G1 X5 F100", &mut state, 1);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear { .. } => {}
            other => panic!("expected Linear (G1 wins), got {:?}", other),
        }
    }

    // --- Missing feed rate warning ---

    #[test]
    fn missing_feed_rate_warning() {
        let mut state = ModalState::default();
        let result = interp("G1 X10", &mut state, 5);

        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("feed rate is zero") && w.line == 5));
    }

    // --- Tool staging and activation ---

    #[test]
    fn tool_staging_and_activation() {
        let mut state = ModalState::default();

        // T2 stages the tool but doesn't activate
        interp("T2", &mut state, 1);
        assert_eq!(state.tool_number, 0);
        assert_eq!(state.staged_tool, 2);

        // M6 activates the staged tool
        let result = interp("M6", &mut state, 2);
        assert_eq!(state.tool_number, 2);
        assert!(result.tool_change.is_some());
        let tc = result.tool_change.unwrap();
        assert_eq!(tc.tool_number, 2);
        assert_eq!(tc.segment_index, 0);
    }

    // --- Unrecognized code warning ---

    #[test]
    fn unrecognized_gcode_warning() {
        let mut state = ModalState::default();
        let result = interp("G999", &mut state, 7);

        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("unrecognized G-code"));
        assert_eq!(result.warnings[0].line, 7);
    }

    // --- Program end ---

    #[test]
    fn m2_program_end() {
        let mut state = ModalState::default();
        let result = interp("M2", &mut state, 10);
        assert!(result.program_end);
    }

    // --- Dwell ---

    #[test]
    fn g4_dwell() {
        let mut state = ModalState::default();
        state.position = v(1.0, 2.0, 3.0);
        let result = interp("G4 P2.5", &mut state, 4);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Dwell {
                position, seconds, ..
            } => {
                assert_eq!(*position, v(1.0, 2.0, 3.0));
                assert_eq!(*seconds, 2.5);
            }
            other => panic!("expected Dwell, got {:?}", other),
        }
    }

    // --- G28 ---

    #[test]
    fn g28_rapid_and_warning() {
        let mut state = ModalState::default();
        state.position = v(10.0, 20.0, 5.0);
        let result = interp("G28 X0 Y0 Z0", &mut state, 3);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(10.0, 20.0, 5.0));
                assert_eq!(*end, v(0.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
        assert!(result.warnings.iter().any(|w| w.message.contains("G28")));
        assert_eq!(state.position, v(0.0, 0.0, 0.0));
    }

    // --- Multi-word line ---

    #[test]
    fn multi_word_line() {
        let mut state = ModalState::default();
        let result = interp("G90 G01 G17 X5 F200", &mut state, 1);

        // State updates applied
        assert_eq!(state.plane, Plane::Xy);

        // One linear segment
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear {
                start,
                end,
                feed_rate,
                ..
            } => {
                assert_eq!(*start, v(0.0, 0.0, 0.0));
                assert_eq!(*end, v(5.0, 0.0, 0.0));
                assert_eq!(*feed_rate, 200.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- M30 also sets program_end ---

    #[test]
    fn m30_program_end() {
        let mut state = ModalState::default();
        let result = interp("M30", &mut state, 1);
        assert!(result.program_end);
    }

    // --- G0 rapid ---

    #[test]
    fn g0_rapid_segment() {
        let mut state = ModalState::default();
        let result = interp("G0 X50 Y25 Z10", &mut state, 1);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(0.0, 0.0, 0.0));
                assert_eq!(*end, v(50.0, 25.0, 10.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
        assert!(result.warnings.is_empty());
    }

    // --- No motion mode defaults to G1 with warning ---

    #[test]
    fn no_motion_mode_defaults_to_g1() {
        let mut state = ModalState::default();
        let result = interp("X10 Y5", &mut state, 1);

        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("defaulting to G1")));
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Linear { .. } => {}
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- Subprogram warning ---

    #[test]
    fn subprogram_warning() {
        let mut state = ModalState::default();
        let result = interp("M98", &mut state, 1);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("subprogram")));
    }

    // --- Cutter comp warning ---

    #[test]
    fn cutter_comp_warning() {
        let mut state = ModalState::default();
        let result = interp("G41", &mut state, 1);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("cutter compensation")));
    }

    // --- Spindle state update ---

    #[test]
    fn spindle_state_update() {
        let mut state = ModalState::default();
        interp("M3 S12000", &mut state, 1);
        assert_eq!(state.spindle_dir, SpindleDir::Cw);
        assert_eq!(state.spindle_speed, 12000.0);
    }

    // --- Segment metadata ---

    #[test]
    fn segment_metadata_attached() {
        let mut state = ModalState::default();
        state.tool_number = 3;
        state.spindle_speed = 8000.0;
        state.spindle_dir = SpindleDir::Cw;
        let result = interp("G1 X10 F500", &mut state, 42);

        match &result.segments[0] {
            MotionSegment::Linear { metadata, .. } => {
                assert_eq!(metadata.source_line, 42);
                assert_eq!(metadata.tool_number, 3);
                assert_eq!(metadata.spindle_speed, 8000.0);
                assert_eq!(metadata.spindle_dir, SpindleDir::Cw);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- G80 cancels canned cycle ---

    #[test]
    fn g80_cancels_cycle() {
        let mut state = ModalState::default();
        state.cycle_active = Some(CycleMode::G81);
        interp("G80", &mut state, 1);
        assert!(state.cycle_active.is_none());
    }

    // --- Canned cycle activation stores parameters ---

    #[test]
    fn canned_cycle_activation() {
        let mut state = ModalState::default();
        state.position = v(0.0, 0.0, 5.0);
        interp("G81 X10 Y20 R2 Z-5 F100", &mut state, 1);

        assert_eq!(state.cycle_active, Some(CycleMode::G81));
        assert_eq!(state.cycle_r, 2.0);
        assert_eq!(state.cycle_z, -5.0);
        assert_eq!(state.initial_z, 5.0);
        // X/Y update position; Z routed to cycle state (z stays at 5.0)
        assert_eq!(state.position.x, 10.0);
        assert_eq!(state.position.y, 20.0);
        assert_eq!(state.position.z, 5.0);
    }

    // --- Arc stub returns warning ---

    #[test]
    fn arc_stub_warning() {
        let mut state = ModalState::default();
        let result = interp("G2 X10 Y0 I5 J0 F300", &mut state, 1);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("arc interpolation not yet implemented")));
        assert!(result.segments.is_empty());
    }

    // --- Empty line produces no segments ---

    #[test]
    fn empty_words_no_output() {
        let mut state = ModalState::default();
        let result = interpret_line(&[], &mut state, 1);
        assert!(result.segments.is_empty());
        assert!(result.warnings.is_empty());
        assert!(!result.program_end);
        assert!(result.tool_change.is_none());
    }

    // --- G28 with no axis words uses current position ---

    #[test]
    fn g28_no_axis_words() {
        let mut state = ModalState::default();
        state.position = v(10.0, 20.0, 5.0);
        let result = interp("G28", &mut state, 1);

        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(10.0, 20.0, 5.0));
                assert_eq!(*end, v(10.0, 20.0, 5.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- M0/M1 no effect, no warning ---

    #[test]
    fn m0_m1_no_effect() {
        let mut state = ModalState::default();
        let result = interp("M0", &mut state, 1);
        assert!(result.warnings.is_empty());
        assert!(!result.program_end);
    }

    // --- Coolant no effect, no warning ---

    #[test]
    fn coolant_no_effect() {
        let mut state = ModalState::default();
        let result = interp("M8", &mut state, 1);
        assert!(result.warnings.is_empty());
    }

    // --- Unrecognized M-code warning ---

    #[test]
    fn unrecognized_mcode_warning() {
        let mut state = ModalState::default();
        let result = interp("M50", &mut state, 3);
        assert!(result.warnings[0].message.contains("unrecognized M-code"));
        assert_eq!(result.warnings[0].line, 3);
    }

    // --- Arc params alone with no motion mode → no segment, no crash ---

    #[test]
    fn arc_params_only_no_motion_mode() {
        let mut state = ModalState::default();
        let result = interp("I5 J5", &mut state, 1);
        assert!(result.segments.is_empty());
        // Should NOT produce a "defaulting to G1" warning
        assert!(!result
            .warnings
            .iter()
            .any(|w| w.message.contains("defaulting to G1")));
    }
}
