#![cfg(cam_geometry_bindings)]

use jamiecam_lib::models::operation::{CacheState, OperationParams, PocketParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::toolpath::planner;
use std::path::PathBuf;
use uuid::Uuid;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/integration/pocket/toolpath.json")
}

#[test]
fn pocket_algorithm_golden_matches() {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3::zero(),
        width: 50.0,
        depth: 50.0,
        height: 10.0,
    });

    let tool = Tool {
        id: Uuid::nil(),
        name: "10mm Flat Endmill".to_string(),
        tool_type: ToolType::FlatEndmill,
        material: "carbide".to_string(),
        diameter: 10.0,
        flute_count: 4,
        default_spindle_speed: Some(8000),
        default_feed_rate: Some(500.0),
    };

    let operation = Operation {
        id: Uuid::nil(),
        name: "Pocket Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Pocket(PocketParams {
            depth: 10.0,
            stepdown: 2.0,
            stepover_percent: 50.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }),
        cache: CacheState::default(),
    };

    let (toolpath, _stats) =
        planner::plan(&operation, &tool, &stock, None, None).expect("plan should succeed");
    let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");

    let fixture = fixture_path();
    if !fixture.exists() {
        std::fs::create_dir_all(fixture.parent().unwrap()).expect("create fixture dir");
        std::fs::write(&fixture, &json).expect("write fixture");
        panic!(
            "Fixture written. Inspect geometry (5 Z levels, shrinking XY contours), then re-run to compare."
        );
    }

    let golden = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {fixture:?}: {e}"));
    assert_eq!(
        json, golden,
        "pocket algorithm output does not match golden file"
    );
}
