//! End-to-end test: hole detection → drill operation → toolpath generation.
//!
//! Exercises the full stack: C++ detection → Rust FFI → IPC layer → drill
//! toolpath generation, verifying that detected holes can be fed directly
//! into the drill toolpath pipeline.

#![cfg(cam_geometry_bindings)]

use jamiecam_lib::commands::file::open_model_inner;
use jamiecam_lib::commands::geometry::detect_holes_inner;
use jamiecam_lib::commands::toolpath::calculate_toolpath_inner;
use jamiecam_lib::models::operation::{CacheState, DrillParams, DrillPoint, OperationParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::state::Project;
use std::sync::RwLock;
use uuid::Uuid;

/// Load plate_with_holes.step and verify hole detection returns expected results.
#[test]
fn detect_holes_returns_expected_geometry() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/plate_with_holes.step"
    );
    let project_lock = RwLock::new(Project::default());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(open_model_inner(fixture, &project_lock))
        .expect("open_model_inner should succeed");

    let holes = detect_holes_inner(&project_lock).expect("detect_holes_inner should succeed");

    // plate_with_holes.step has 3 Z-parallel holes (tilted hole is filtered out).
    assert_eq!(holes.len(), 3, "expected 3 holes (tilted hole filtered)");

    // Sort by center_x then center_y for deterministic ordering.
    let mut holes = holes;
    holes.sort_by(|a, b| {
        a.center_x
            .partial_cmp(&b.center_x)
            .unwrap()
            .then(a.center_y.partial_cmp(&b.center_y).unwrap())
    });

    // Hole 1: center=(25,25), diameter=10, depth=20, through=true
    assert!((holes[0].center_x - 25.0).abs() < 0.5, "hole 0 center_x");
    assert!((holes[0].center_y - 25.0).abs() < 0.5, "hole 0 center_y");
    assert!(
        (holes[0].radius - 5.0).abs() < 0.5,
        "hole 0 radius (diameter 10)"
    );
    assert!((holes[0].depth - 20.0).abs() < 0.5, "hole 0 depth");
    assert!(holes[0].is_through, "hole 0 should be through");

    // Hole 2: center=(50,75), diameter=8, depth=12, through=false (blind)
    assert!((holes[1].center_x - 50.0).abs() < 0.5, "hole 1 center_x");
    assert!((holes[1].center_y - 75.0).abs() < 0.5, "hole 1 center_y");
    assert!(
        (holes[1].radius - 4.0).abs() < 0.5,
        "hole 1 radius (diameter 8)"
    );
    assert!((holes[1].depth - 12.0).abs() < 0.5, "hole 1 depth");
    assert!(!holes[1].is_through, "hole 1 should be blind");

    // Hole 3: center=(75,25), diameter=6, depth=20, through=true
    assert!((holes[2].center_x - 75.0).abs() < 0.5, "hole 2 center_x");
    assert!((holes[2].center_y - 25.0).abs() < 0.5, "hole 2 center_y");
    assert!(
        (holes[2].radius - 3.0).abs() < 0.5,
        "hole 2 radius (diameter 6)"
    );
    assert!((holes[2].depth - 20.0).abs() < 0.5, "hole 2 depth");
    assert!(holes[2].is_through, "hole 2 should be through");
}

/// Full E2E: detect holes → build drill operation → calculate toolpath.
///
/// Verifies that the drill toolpath contains moves at the expected XY positions
/// from the detected holes.
#[test]
fn detected_holes_produce_valid_drill_toolpath() {
    // Step 1: Load the STEP file.
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/plate_with_holes.step"
    );
    let project_lock = RwLock::new(Project::default());
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(open_model_inner(fixture, &project_lock))
        .expect("open_model_inner should succeed");

    // Step 2: Detect holes.
    let holes = detect_holes_inner(&project_lock).expect("detect_holes_inner should succeed");
    assert_eq!(holes.len(), 3, "expected 3 holes");

    // Step 3: Build drill points from detected holes.
    let drill_points: Vec<DrillPoint> = holes
        .iter()
        .map(|h| DrillPoint {
            x: h.center_x as f64,
            y: h.center_y as f64,
        })
        .collect();

    // Use the shallowest hole depth as the drill depth (conservative).
    let min_depth = holes.iter().map(|h| h.depth).fold(f32::INFINITY, f32::min);

    // Step 4: Set up project with tool, stock, and drill operation.
    let tool_id = Uuid::new_v4();
    let op_id = Uuid::new_v4();
    {
        let mut project = project_lock.write().unwrap();
        project.tools.push(Tool {
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
        });
        project.stock = Some(StockDefinition::Box(BoxDimensions {
            origin: Vec3::zero(),
            width: 100.0,
            depth: 100.0,
            height: 20.0,
        }));
        project.operations.push(Operation {
            id: op_id,
            name: "Hole Detection Drill".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: min_depth as f64,
                peck_depth: Some(3.0),
                points: drill_points.clone(),
            }),
            cache: CacheState::default(),
        });
    }

    // Step 5: Calculate toolpath.
    let stats = calculate_toolpath_inner(&op_id.to_string(), &project_lock, None)
        .expect("calculate_toolpath_inner should succeed");

    assert!(stats.total_pass_count > 0, "should have at least one pass");
    assert!(
        stats.total_point_count > 0,
        "should have at least one point"
    );

    // Step 6: Verify the toolpath contains drill moves at the expected XY positions.
    let project = project_lock.read().unwrap();
    let toolpath = project
        .toolpaths
        .get(&op_id)
        .expect("toolpath should exist");

    // Drill toolpaths include linking (rapid) passes between holes,
    // so we expect more passes than just the number of holes.
    assert!(
        toolpath.passes.len() >= 3,
        "should have at least one pass per detected hole, got {}",
        toolpath.passes.len()
    );

    // Collect the XY positions from the cutting passes and verify each detected
    // hole center appears in the toolpath.
    let expected_xys: Vec<(f64, f64)> = drill_points.iter().map(|p| (p.x, p.y)).collect();

    for (x, y) in &expected_xys {
        let found = toolpath.passes.iter().any(|pass| {
            pass.cuts
                .iter()
                .any(|cp| (cp.position.x - x).abs() < 0.5 && (cp.position.y - y).abs() < 0.5)
        });
        assert!(found, "toolpath should contain drill move near ({x}, {y})");
    }
}
