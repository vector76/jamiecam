use jamiecam_lib::gcode_parser::{self, MotionSegment};
use jamiecam_lib::models::Vec3;
use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
use jamiecam_lib::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};
use jamiecam_lib::toolpath::Toolpath;
use uuid::Uuid;

/// Build a simple linear toolpath: rapid to clearance, feed down, cut a square.
fn simple_linear_toolpath() -> (Toolpath, Vec<Vec3>) {
    let clearance_z = 15.0;
    let cut_z = -5.0;
    let feed_rate = 500.0;

    // The positions we expect to recover after the round trip.
    // Rapid to start XY at clearance Z, then feed down, cut a square, retract.
    let positions = vec![
        Vec3 {
            x: 10.0,
            y: 10.0,
            z: clearance_z,
        }, // rapid position
        Vec3 {
            x: 10.0,
            y: 10.0,
            z: cut_z,
        }, // feed down
        Vec3 {
            x: 50.0,
            y: 10.0,
            z: cut_z,
        }, // cut +X
        Vec3 {
            x: 50.0,
            y: 50.0,
            z: cut_z,
        }, // cut +Y
        Vec3 {
            x: 10.0,
            y: 50.0,
            z: cut_z,
        }, // cut -X
        Vec3 {
            x: 10.0,
            y: 10.0,
            z: cut_z,
        }, // close square
    ];

    let cuts: Vec<CutPoint> = positions
        .iter()
        .enumerate()
        .map(|(i, pos)| CutPoint {
            position: pos.clone(),
            move_kind: if i == 0 {
                MoveKind::Rapid
            } else {
                MoveKind::Feed
            },
            tool_orientation: None,
            feed_rate_override: None,
        })
        .collect();

    let pass = Pass {
        kind: PassKind::Cutting,
        cuts,
    };

    let toolpath = Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 8000.0,
        feed_rate,
        passes: vec![pass],
    };

    (toolpath, positions)
}

#[test]
fn round_trip_linear_positions() {
    let (toolpath, expected_positions) = simple_linear_toolpath();

    // Post-process to G-code text
    let pp = PostProcessor::builtin("fanuc-0i").expect("load fanuc-0i");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 10.0,
        description: "10mm Flat Endmill".to_string(),
    };
    let gcode = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(9000),
                include_comments: false,
            },
        )
        .expect("generate G-code");

    // Parse the G-code back
    let parsed = gcode_parser::parse_gcode(&gcode);

    // Collect all endpoint positions from parsed segments (skip dwells)
    let parsed_positions: Vec<Vec3> = parsed
        .segments
        .iter()
        .filter_map(|seg| match seg {
            MotionSegment::Rapid { end, .. } => Some(end.clone()),
            MotionSegment::Linear { end, .. } => Some(end.clone()),
            MotionSegment::Arc { end, .. } => Some(end.clone()),
            MotionSegment::Dwell { .. } => None,
        })
        .collect();

    // The postprocessor may add setup moves (home, tool change positioning) before
    // and after our toolpath points. We need to find our expected positions within
    // the parsed output as a subsequence.
    let tolerance = 0.001;
    let mut match_idx = 0;
    for parsed_pos in &parsed_positions {
        if match_idx >= expected_positions.len() {
            break;
        }
        let expected = &expected_positions[match_idx];
        let dx = (parsed_pos.x - expected.x).abs();
        let dy = (parsed_pos.y - expected.y).abs();
        let dz = (parsed_pos.z - expected.z).abs();
        if dx < tolerance && dy < tolerance && dz < tolerance {
            match_idx += 1;
        }
    }

    assert_eq!(
        match_idx,
        expected_positions.len(),
        "round-trip failed: only matched {match_idx}/{} expected positions.\n\
         Expected: {expected_positions:?}\n\
         Parsed endpoints: {parsed_positions:?}",
        expected_positions.len()
    );
}
