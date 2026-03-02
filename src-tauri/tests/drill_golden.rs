use jamiecam_lib::models::operation::{CacheState, DrillParams, DrillPoint, OperationParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::toolpath::planner;
use std::path::PathBuf;
use uuid::Uuid;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/integration/drill/toolpath.json")
}

#[test]
fn drill_algorithm_golden_matches() {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3::zero(),
        width: 50.0,
        depth: 50.0,
        height: 10.0,
    });

    let tool = Tool {
        id: Uuid::nil(),
        name: "5mm Drill".to_string(),
        tool_type: ToolType::Drill,
        material: "hss".to_string(),
        diameter: 5.0,
        flute_count: 2,
        default_spindle_speed: Some(1200),
        default_feed_rate: Some(150.0),
    };

    let operation = Operation {
        id: Uuid::nil(),
        name: "Drill Golden".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        params: OperationParams::Drill(DrillParams {
            depth: 10.0,
            peck_depth: Some(3.0),
            points: vec![
                DrillPoint { x: 10.0, y: 10.0 },
                DrillPoint { x: 30.0, y: 10.0 },
                DrillPoint { x: 10.0, y: 30.0 },
                DrillPoint { x: 30.0, y: 30.0 },
                DrillPoint { x: 20.0, y: 20.0 },
            ],
        }),
        cache: CacheState::default(),
    };

    let (toolpath, _stats) = planner::plan(&operation, &tool, &stock).expect("plan should succeed");
    let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");

    let fixture = fixture_path();
    if !fixture.exists() {
        std::fs::create_dir_all(fixture.parent().unwrap()).expect("create fixture dir");
        std::fs::write(&fixture, &json).expect("write fixture");
        panic!(
            "Fixture written. Inspect geometry (expected: 5 holes × peck drilling with 3mm peck depth over 10mm total depth; multiple peck steps per hole; correct Z values). Then re-run to compare."
        );
    }

    let golden = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {fixture:?}: {e}"));
    assert_eq!(
        json, golden,
        "drill algorithm output does not match golden file"
    );
}
