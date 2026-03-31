#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::AdaptiveClearingParams;
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::adaptive_clearing::adaptive_clearing_passes;
use jamiecam_lib::toolpath::types::{MoveKind, PassKind};
use std::path::PathBuf;

fn box_step_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step")
}

fn load_box() -> (OcctShape, StockDefinition) {
    let shape = OcctShape::load_step(&box_step_path()).expect("load box.step");
    let (xmin, ymin, zmin, xmax, ymax, zmax) = shape.bounding_box();
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3 {
            x: xmin,
            y: ymin,
            z: zmin,
        },
        width: xmax - xmin,
        depth: ymax - ymin,
        height: zmax - zmin,
    });
    (shape, stock)
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

fn representative_params() -> AdaptiveClearingParams {
    AdaptiveClearingParams {
        depth: 5.0,
        stepdown: 1.0,
        optimal_load: 0.25,
        stepover_percent: 50.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Golden JSON snapshot
// ---------------------------------------------------------------------------

#[test]
fn adaptive_clearing_golden_matches() {
    let (shape, stock) = load_box();
    let params = representative_params();
    let tool_diameter = 6.0;
    let base_feed = 500.0;

    let passes = adaptive_clearing_passes(&stock, &params, tool_diameter, Some(&shape), base_feed)
        .expect("should succeed");
    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("adaptive_clearing_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "adaptive_clearing output does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Structural assertions — trochoidal geometry
// ---------------------------------------------------------------------------

#[test]
fn output_contains_trochoidal_geometry() {
    let (shape, stock) = load_box();
    let params = representative_params();

    let passes = adaptive_clearing_passes(&stock, &params, 6.0, Some(&shape), 500.0)
        .expect("should succeed");

    // Trochoidal loops are linearized Feed segments that form arc-like patterns.
    // We detect them by looking for sequences of closely-spaced Feed points
    // (>= 8 consecutive points with short inter-point distance, characteristic
    // of the 16-segment trochoidal circle).
    let mut found_arc_like_sequence = false;

    for pass in &passes {
        let feed_points: Vec<&Vec3> = pass
            .cuts
            .iter()
            .filter(|c| matches!(c.move_kind, MoveKind::Feed))
            .map(|c| &c.position)
            .collect();

        if feed_points.len() < 8 {
            continue;
        }

        // Look for a run of >=8 consecutive points with short distances
        // (trochoidal loops produce many closely-spaced points).
        let mut run_length = 0usize;
        for w in feed_points.windows(2) {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 5.0 {
                run_length += 1;
                if run_length >= 8 {
                    found_arc_like_sequence = true;
                    break;
                }
            } else {
                run_length = 0;
            }
        }
        if found_arc_like_sequence {
            break;
        }
    }

    assert!(
        found_arc_like_sequence,
        "expected trochoidal arc-like geometry (runs of closely-spaced Feed points)"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Per-point feed_rate_override values vary
// ---------------------------------------------------------------------------

#[test]
fn feed_rate_overrides_vary_across_points() {
    let (shape, stock) = load_box();
    let params = representative_params();

    let passes = adaptive_clearing_passes(&stock, &params, 6.0, Some(&shape), 500.0)
        .expect("should succeed");

    let feeds: Vec<f64> = passes
        .iter()
        .flat_map(|p| p.cuts.iter())
        .filter_map(|c| c.feed_rate_override)
        .collect();

    assert!(feeds.len() >= 2, "expected multiple feed-rate points");

    let min = feeds.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = feeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (max - min) > 1e-6,
        "expected varying feed rates, but all are ~{min}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Multiple Z levels are represented
// ---------------------------------------------------------------------------

#[test]
fn multiple_z_levels_represented() {
    let (shape, stock) = load_box();
    let params = representative_params();

    let passes = adaptive_clearing_passes(&stock, &params, 6.0, Some(&shape), 500.0)
        .expect("should succeed");

    let mut z_set = std::collections::HashSet::new();
    for pass in &passes {
        for cut in &pass.cuts {
            z_set.insert((cut.position.z * 1000.0) as i64);
        }
    }

    // depth=5.0, stepdown=1.0 → up to 6 Z levels
    assert!(
        z_set.len() >= 2,
        "expected multiple Z levels, got {} ({z_set:?})",
        z_set.len()
    );
}

// ---------------------------------------------------------------------------
// Test 5: Pass count within expected bounds
// ---------------------------------------------------------------------------

#[test]
fn pass_count_within_expected_bounds() {
    let (shape, stock) = load_box();
    let params = representative_params();

    let passes = adaptive_clearing_passes(&stock, &params, 6.0, Some(&shape), 500.0)
        .expect("should succeed");

    // With depth=5, stepdown=1 we have up to 6 Z levels.
    // Each level produces at least 1 pass.
    assert!(
        passes.len() >= 2,
        "expected at least 2 passes, got {}",
        passes.len()
    );
    assert!(
        passes.len() <= 500,
        "unexpectedly many passes: {}",
        passes.len()
    );

    // All passes should be Cutting kind.
    for pass in &passes {
        assert_eq!(pass.kind, PassKind::Cutting, "expected all Cutting passes");
    }
}

// ---------------------------------------------------------------------------
// Test 6: G-code golden — full pipeline (linking + arc fitting + postprocessor)
// ---------------------------------------------------------------------------

#[test]
fn adaptive_clearing_gcode_golden() {
    use jamiecam_lib::models::operation::{CacheState, OperationParams};
    use jamiecam_lib::models::tool::ToolType;
    use jamiecam_lib::models::{Operation, Tool};
    use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
    use jamiecam_lib::toolpath::types::{LinkingParams, Toolpath, DEFAULT_CLEARANCE_OFFSET};
    use jamiecam_lib::toolpath::{arc_fitting, linking, planner};
    use uuid::Uuid;

    let (shape, stock) = load_box();
    let (_, _, _, _, _, zmax) = shape.bounding_box();

    let tool = Tool {
        id: Uuid::nil(),
        name: "6mm Flat Endmill".to_string(),
        tool_type: ToolType::FlatEndmill,
        material: Some("carbide".to_string()),
        diameter: 6.0,
        flute_count: Some(4),
        default_spindle_speed: Some(10000),
        default_feed_rate: Some(500.0),
        cutting_length: 18.0,
        shank_diameter: 6.0,
        corner_radius: None,
        included_angle: None,
        point_angle: None,
        pilot_diameter: None,
        pilot_length: None,
        thread_pitch: None,
        min_bore_diameter: None,
        taper_half_angle: None,
        overall_length: Some(54.0),
    };
    let operation = Operation {
        id: Uuid::nil(),
        name: "Adaptive Clearing Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::AdaptiveClearing(representative_params()),
        cache: CacheState::default(),
    };
    let (raw_passes, _stats) =
        planner::plan(&operation, &tool, &stock, Some(&shape), None).expect("plan should succeed");

    let stock_top_z = zmax;
    let linked_passes = linking::link_passes(
        raw_passes,
        &LinkingParams {
            tool_diameter: tool.diameter,
            clearance_z: stock_top_z + DEFAULT_CLEARANCE_OFFSET,
            lead_ratio: linking::DEFAULT_LEAD_RATIO,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        },
    );
    let passes: Vec<_> = linked_passes
        .into_iter()
        .map(|mut pass| {
            pass.cuts = arc_fitting::fit_arcs(pass.cuts, 0.01);
            pass
        })
        .collect();

    let toolpath = Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 10000.0,
        feed_rate: 500.0,
        passes,
    };

    let pp = PostProcessor::builtin("fanuc-0i").expect("load postprocessor");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 6.0,
        description: "6mm Flat Endmill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(2000),
                include_comments: false,
            },
        )
        .expect("generate");

    let nc_fixture = fixture_path("adaptive_clearing_golden.nc");
    if !nc_fixture.exists() {
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "G-code fixture written to {nc_fixture:?}. Inspect output — verify varying F-words and G02/G03 arcs. Re-run to lock."
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "adaptive_clearing G-code does not match golden file"
    );

    // Structural checks on the G-code output.
    assert!(
        output.contains("G02") || output.contains("G03"),
        "expected G02/G03 arc moves in adaptive clearing G-code"
    );

    // Verify varying F-words: extract all F values and check they're not all identical.
    let f_values: Vec<f64> = output
        .lines()
        .filter_map(|line| {
            line.split_whitespace()
                .find(|w| w.starts_with('F'))
                .and_then(|w| w[1..].parse::<f64>().ok())
        })
        .collect();
    assert!(
        f_values.len() >= 2,
        "expected at least 2 F-words in G-code, got {}",
        f_values.len()
    );
    let f_min = f_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let f_max = f_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (f_max - f_min) > 0.01,
        "expected varying F-words in G-code, but all are ~{f_min}"
    );
}
