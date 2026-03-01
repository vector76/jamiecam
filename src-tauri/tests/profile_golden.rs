#![cfg(cam_geometry_bindings)]

use jamiecam_lib::models::operation::{CompensationSide, OperationParams, ProfileParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::toolpath::planner;
use std::path::PathBuf;
use uuid::Uuid;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/integration/profile/toolpath.json")
}

#[test]
fn profile_algorithm_golden_matches() {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3::zero(),
        width: 50.0,
        depth: 50.0,
        height: 10.0,
    });

    let tool = Tool {
        id: Uuid::nil(),
        name: "6mm Flat Endmill".to_string(),
        tool_type: ToolType::FlatEndmill,
        material: "carbide".to_string(),
        diameter: 6.0,
        flute_count: 4,
        default_spindle_speed: Some(8000),
        default_feed_rate: Some(500.0),
    };

    let operation = Operation {
        id: Uuid::nil(),
        name: "Profile Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        params: OperationParams::Profile(ProfileParams {
            depth: 10.0,
            stepdown: 2.5,
            compensation_side: CompensationSide::Left,
        }),
    };

    let (toolpath, _stats) = planner::plan(&operation, &tool, &stock).expect("plan should succeed");
    let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");

    let fixture = fixture_path();
    if !fixture.exists() {
        std::fs::create_dir_all(fixture.parent().unwrap()).expect("create fixture dir");
        std::fs::write(&fixture, &json).expect("write fixture");
        panic!(
            "Fixture written. Inspect geometry (expected: 4 Z levels at stepdown 2.5 over depth 10, one rectangular contour per level, offset inward by 3 mm). Then re-run to compare."
        );
    }

    let golden = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {fixture:?}: {e}"));
    assert_eq!(
        json, golden,
        "profile algorithm output does not match golden file"
    );
}
