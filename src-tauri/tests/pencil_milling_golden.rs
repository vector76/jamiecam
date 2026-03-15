#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::PencilMillingParams;
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::pencil_milling::pencil_milling_passes;
use std::path::{Path, PathBuf};

fn plate_step_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/plate_with_holes.step")
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

fn plate_params() -> PencilMillingParams {
    PencilMillingParams {
        allowance: 0.0,
        tool_diameter: 6.0,
        curvature_threshold: None,
        min_pass_length: 1.0,
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
fn pencil_milling_golden_matches() {
    let (shape, stock) = load_shape(&plate_step_path());
    let params = plate_params();

    let passes = pencil_milling_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");
    assert!(passes.len() >= 1, "expected at least 1 pass, got 0");

    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("pencil_milling_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "pencil_milling output does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 2: G-code golden — full pipeline (linking + arc fitting + postprocessor)
// ---------------------------------------------------------------------------

#[test]
fn pencil_milling_gcode_golden() {
    use jamiecam_lib::models::operation::{CacheState, OperationParams};
    use jamiecam_lib::models::tool::ToolType;
    use jamiecam_lib::models::{Operation, Tool};
    use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
    use jamiecam_lib::toolpath::types::{LinkingParams, Toolpath, DEFAULT_CLEARANCE_OFFSET};
    use jamiecam_lib::toolpath::{arc_fitting, linking, planner};
    use uuid::Uuid;

    let (shape, stock) = load_shape(&plate_step_path());
    let (_, _, _, _, _, zmax) = shape.bounding_box();

    let params = plate_params();

    let tool = Tool {
        id: Uuid::nil(),
        name: "6mm Flat Endmill".to_string(),
        tool_type: ToolType::FlatEndmill,
        material: "carbide".to_string(),
        diameter: 6.0,
        flute_count: 4,
        default_spindle_speed: Some(10000),
        default_feed_rate: Some(500.0),
    };
    let operation = Operation {
        id: Uuid::nil(),
        name: "Pencil Milling Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        params: OperationParams::PencilMilling(params),
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

    let nc_fixture = fixture_path("pencil_milling_golden.nc");
    if !nc_fixture.exists() {
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "G-code fixture written to {nc_fixture:?}. Inspect output — verify pencil milling passes. Re-run to lock."
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "pencil_milling G-code does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Structural/threshold — curvature threshold reduces passes
// ---------------------------------------------------------------------------

#[test]
fn pencil_milling_threshold_reduces_passes() {
    // plate_with_holes has inside corners and fillets with high curvature,
    // making it suitable for verifying threshold-based filtering.
    let (shape, stock) = load_shape(&plate_step_path());

    let params_default = plate_params();

    let passes_default =
        pencil_milling_passes(&stock, &params_default, 6.0, Some(&shape)).expect("should succeed");
    assert!(
        !passes_default.is_empty(),
        "expected non-empty passes with default threshold"
    );

    let params_high_threshold = PencilMillingParams {
        curvature_threshold: Some(1000.0),
        ..plate_params()
    };

    let passes_high = pencil_milling_passes(&stock, &params_high_threshold, 6.0, Some(&shape))
        .expect("should succeed");

    assert!(
        passes_high.len() < passes_default.len(),
        "expected fewer passes with high curvature threshold ({}) than with default ({})",
        passes_high.len(),
        passes_default.len()
    );
}
