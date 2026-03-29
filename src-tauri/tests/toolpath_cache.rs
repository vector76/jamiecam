use jamiecam_lib::commands::project::get_project_snapshot_inner;
use jamiecam_lib::commands::toolpath::calculate_toolpath_inner;
use jamiecam_lib::models::operation::{CacheState, DrillParams, DrillPoint, OperationParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::project::serialization::{load, save};
use jamiecam_lib::state::Project;
use std::sync::RwLock;
use uuid::Uuid;

fn make_drill_project() -> (RwLock<Project>, Uuid) {
    let mut project = Project::default();

    let tool_id = Uuid::new_v4();
    let mut tool = Tool {
        id: tool_id,
        name: "5mm Drill".to_string(),
        tool_type: ToolType::Drill,
        material: "hss".to_string(),
        diameter: 5.0,
        flute_count: 2,
        default_spindle_speed: Some(1200),
        default_feed_rate: Some(150.0),
        cutting_length: 15.0,
        shank_diameter: 5.0,
        overall_length: 45.0,
        corner_radius: None,
        included_angle: None,
        point_angle: None,
        pilot_diameter: None,
        pilot_length: None,
        thread_pitch: None,
        min_bore_diameter: None,
        taper_half_angle: None,
    };
    // Pre-resolve so that the in-memory tool matches what load() would
    // produce after deserializing and calling resolve_defaults().
    tool.resolve_defaults();
    project.tools.push(tool);

    let op_id = Uuid::new_v4();
    project.operations.push(Operation {
        id: op_id,
        name: "Cache Test Drill".to_string(),
        enabled: true,
        tool_id,
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Drill(DrillParams {
            depth: 10.0,
            peck_depth: Some(3.0),
            points: vec![
                DrillPoint { x: 10.0, y: 10.0 },
                DrillPoint { x: 30.0, y: 10.0 },
            ],
        }),
        cache: CacheState::default(),
    });

    project.stock = Some(StockDefinition::Box(BoxDimensions {
        origin: Vec3::zero(),
        width: 50.0,
        depth: 50.0,
        height: 10.0,
    }));

    (RwLock::new(project), op_id)
}

#[test]
fn round_trip_preserves_toolpath_and_not_stale() {
    let (project_lock, op_id) = make_drill_project();

    calculate_toolpath_inner(&op_id.to_string(), &project_lock, None).expect("calculate ok");

    let original_toolpath = {
        let p = project_lock.read().unwrap();
        let op = p.operations.iter().find(|o| o.id == op_id).unwrap();
        assert!(op.cache.valid, "cache should be valid after calculate");
        assert!(
            op.cache.key.is_some(),
            "cache key should be set after calculate"
        );
        p.toolpaths[&op_id].clone()
    };

    let tmp = std::env::temp_dir().join("jcam_test_toolpath_cache.jcam");
    {
        let p = project_lock.read().unwrap();
        save(&p, &tmp).expect("save ok");
    }

    let loaded = load(&tmp).expect("load ok");
    let _ = std::fs::remove_file(&tmp);

    assert!(
        loaded.toolpaths.contains_key(&op_id),
        "toolpath should survive save/load round-trip"
    );
    assert_eq!(
        loaded.toolpaths[&op_id], original_toolpath,
        "loaded toolpath should match original"
    );

    let loaded_lock = RwLock::new(loaded);
    let snap = get_project_snapshot_inner(&loaded_lock).expect("snapshot ok");
    let op_snap = snap.operations.iter().find(|o| o.id == op_id).unwrap();
    assert!(
        !op_snap.needs_recalculate,
        "round-tripped toolpath should not be stale"
    );
}

#[test]
fn stale_after_param_change() {
    // Repeat full setup through round-trip to obtain a loaded_lock
    let (project_lock, op_id) = make_drill_project();
    calculate_toolpath_inner(&op_id.to_string(), &project_lock, None).expect("calculate ok");

    let tmp = std::env::temp_dir().join("jcam_test_toolpath_cache_stale.jcam");
    {
        let p = project_lock.read().unwrap();
        save(&p, &tmp).expect("save ok");
    }
    let loaded = load(&tmp).expect("load ok");
    let _ = std::fs::remove_file(&tmp);

    let loaded_lock = RwLock::new(loaded);

    // Verify not stale before mutation
    let snap = get_project_snapshot_inner(&loaded_lock).expect("snapshot ok");
    assert!(
        !snap
            .operations
            .iter()
            .find(|o| o.id == op_id)
            .unwrap()
            .needs_recalculate,
        "should not be stale before param change"
    );

    // Mutate depth — changes the cache key
    {
        let mut p = loaded_lock.write().unwrap();
        let op = p.operations.iter_mut().find(|o| o.id == op_id).unwrap();
        if let OperationParams::Drill(ref mut d) = op.params {
            d.depth = 20.0;
        }
    }

    let snap2 = get_project_snapshot_inner(&loaded_lock).expect("snapshot ok");
    assert!(
        snap2
            .operations
            .iter()
            .find(|o| o.id == op_id)
            .unwrap()
            .needs_recalculate,
        "should be stale after depth param change"
    );
}
