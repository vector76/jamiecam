#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::ScallopFinishingParams;
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::scallop_finishing::scallop_finishing_passes;
use jamiecam_lib::toolpath::types::PassKind;
use std::path::{Path, PathBuf};

fn sphere_step_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/sphere.step")
}

fn box_step_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step")
}

fn load_shape(path: &Path) -> (OcctShape, StockDefinition) {
    let shape = OcctShape::load_step(path).expect("load shape");
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

fn sphere_params() -> ScallopFinishingParams {
    ScallopFinishingParams {
        target_scallop_height: 0.01,
        tool_radius: 3.0,
        min_stepover: 0.1,
        max_stepover: 2.0,
        direction_angle_deg: 0.0,
        allowance: 0.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: JSON golden snapshot
// ---------------------------------------------------------------------------

#[test]
fn scallop_finishing_golden_matches() {
    let (shape, stock) = load_shape(&sphere_step_path());
    let params = sphere_params();

    let passes =
        scallop_finishing_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");
    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("scallop_finishing_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "scallop_finishing output does not match golden file"
    );

    // Multiple passes generated.
    assert!(
        passes.len() >= 2,
        "expected at least 2 passes, got {}",
        passes.len()
    );

    // Structural: stepovers should vary (adaptive stepover).
    // Measure Y-offset between consecutive pass first points as a proxy for stepover.
    if passes.len() >= 3 {
        let first_ys: Vec<f64> = passes
            .iter()
            .filter_map(|p| p.cuts.first().map(|c| c.position.y))
            .collect();
        let stepovers: Vec<f64> = first_ys.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let all_equal = stepovers.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9);
        assert!(
            !all_equal,
            "stepovers should vary across passes (adaptive), but all were equal"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: G-code golden — full pipeline (linking + arc fitting + postprocessor)
// ---------------------------------------------------------------------------

#[test]
fn scallop_finishing_gcode_golden() {
    use jamiecam_lib::models::operation::{CacheState, OperationParams};
    use jamiecam_lib::models::tool::ToolType;
    use jamiecam_lib::models::{Operation, Tool};
    use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
    use jamiecam_lib::toolpath::types::{LinkingParams, Toolpath, DEFAULT_CLEARANCE_OFFSET};
    use jamiecam_lib::toolpath::{arc_fitting, linking, planner};
    use uuid::Uuid;

    let (shape, stock) = load_shape(&box_step_path());
    let (_, _, _, _, _, zmax) = shape.bounding_box();

    let box_params = ScallopFinishingParams {
        target_scallop_height: 0.01,
        tool_radius: 3.0,
        min_stepover: 0.1,
        max_stepover: 2.0,
        direction_angle_deg: 0.0,
        allowance: 0.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    };

    let tool = Tool {
        id: Uuid::nil(),
        name: "6mm Ball Endmill".to_string(),
        tool_type: ToolType::BallNose,
        material: "carbide".to_string(),
        diameter: 6.0,
        flute_count: 4,
        default_spindle_speed: Some(10000),
        default_feed_rate: Some(500.0),
        cutting_length: 18.0,
        shank_diameter: 6.0,
        overall_length: 54.0,
    };
    let operation = Operation {
        id: Uuid::nil(),
        name: "Scallop Finishing Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::ScallopFinishing(box_params),
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
        description: "6mm Ball Endmill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(2001),
                include_comments: false,
            },
        )
        .expect("generate");

    let nc_fixture = fixture_path("scallop_finishing_golden.nc");
    if !nc_fixture.exists() {
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "G-code fixture written to {nc_fixture:?}. Inspect output — verify G01 linear moves. Re-run to lock."
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "scallop_finishing G-code does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Structural — all Cutting, Z span, variable stepover
// ---------------------------------------------------------------------------

#[test]
fn scallop_finishing_structural_properties() {
    let (shape, stock) = load_shape(&sphere_step_path());
    let params = sphere_params();

    let passes =
        scallop_finishing_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");

    // All passes should be Cutting.
    for pass in &passes {
        assert_eq!(pass.kind, PassKind::Cutting, "expected all Cutting passes");
    }

    // Z span should be substantial on sphere surface.
    let z_min = passes
        .iter()
        .flat_map(|p| p.cuts.iter())
        .map(|c| c.position.z)
        .fold(f64::INFINITY, f64::min);
    let z_max = passes
        .iter()
        .flat_map(|p| p.cuts.iter())
        .map(|c| c.position.z)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        z_max - z_min > 1.0,
        "Z values should span more than 1mm on sphere surface (span={:.3}mm)",
        z_max - z_min
    );

    // Stepover should vary between passes (adaptive based on curvature).
    assert!(
        passes.len() >= 3,
        "expected at least 3 passes for stepover variation test, got {}",
        passes.len()
    );
    let first_ys: Vec<f64> = passes
        .iter()
        .filter_map(|p| p.cuts.first().map(|c| c.position.y))
        .collect();
    let stepovers: Vec<f64> = first_ys.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let all_equal = stepovers.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9);
    assert!(
        !all_equal,
        "stepovers should vary across passes (narrower where curvature is higher)"
    );
}
