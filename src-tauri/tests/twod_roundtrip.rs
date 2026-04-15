// Test fixtures deliberately use `let mut x = T::default(); x.field = ...;`
// for readability when only a couple of fields differ from defaults.
#![allow(clippy::field_reassign_with_default)]

//! Serialization roundtrip test for Mode 2 (2D Profiling) project fields.
//!
//! Does NOT require the `cam_geometry_bindings` feature and must always run.
//! Verifies that `source_2d_artwork`, `safe_height`, `artwork_origin`, and a
//! `Profile2d` operation survive a save → load cycle without data loss.

use jamiecam_lib::models::operation::{
    CacheState, CutType, MillingDirection, OperationParams, Profile2dParams,
};
use jamiecam_lib::models::twod::{Curve2d, LoadedArtwork, UnitSystem};
use jamiecam_lib::models::Operation;
use jamiecam_lib::project::serialization;
use jamiecam_lib::state::{Mode, Project};
use uuid::Uuid;

fn make_project() -> (Project, Uuid) {
    let curve_id = Uuid::parse_str("aaaabbbb-cccc-dddd-eeee-000000000001").unwrap();
    let op_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
    let tool_id = Uuid::parse_str("ffffffff-eeee-dddd-cccc-bbbbbbbbbbbb").unwrap();

    let curve = Curve2d {
        id: curve_id,
        is_closed: true,
        points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        layer: Some("outline".to_string()),
    };

    let artwork = LoadedArtwork {
        file_path: "/tmp/test.svg".to_string(),
        unit_system: UnitSystem::Mm,
        curves: vec![curve],
        import_warnings: vec![],
    };

    let operation = Operation {
        id: op_id,
        name: "2D Profile Op".to_string(),
        enabled: true,
        tool_id,
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Profile2d(Profile2dParams {
            curve_id,
            cut_type: CutType::Outside,
            direction: MillingDirection::Climb,
            top_of_cut: 0.0,
            depth_of_cut: 5.0,
            step_down: 2.5,
            feed_rate: 1000.0,
        }),
        cache: CacheState::default(),
    };

    let mut project = Project::default();
    project.mode = Mode::TwoD;
    project.source_2d_artwork = Some(artwork);
    project.safe_height = Some(5.0);
    project.artwork_origin = [3.0, 2.0];
    project.operations = vec![operation];

    (project, curve_id)
}

#[test]
fn twod_fields_survive_save_load_roundtrip() {
    let (project, curve_id) = make_project();

    let tmp = std::env::temp_dir().join("jcam_twod_roundtrip_test.jcam");
    serialization::save(&project, &tmp).expect("save should succeed");

    let loaded = serialization::load(&tmp).expect("load should succeed");
    let _ = std::fs::remove_file(&tmp);

    // Mode is preserved
    assert_eq!(
        loaded.mode,
        Mode::TwoD,
        "mode should be TwoD after roundtrip"
    );

    // safe_height is preserved
    assert_eq!(
        loaded.safe_height,
        Some(5.0),
        "safe_height should be Some(5.0) after roundtrip"
    );

    // artwork_origin is preserved
    assert_eq!(
        loaded.artwork_origin,
        [3.0, 2.0],
        "artwork_origin should be [3.0, 2.0] after roundtrip"
    );

    // source_2d_artwork is preserved
    let artwork = loaded
        .source_2d_artwork
        .as_ref()
        .expect("source_2d_artwork should be Some after roundtrip");
    assert_eq!(
        artwork.curves[0].id, curve_id,
        "curve id should survive roundtrip"
    );

    // Profile2d operation params are preserved
    assert_eq!(
        loaded.operations.len(),
        1,
        "one operation should survive roundtrip"
    );
    let op = &loaded.operations[0];
    match &op.params {
        OperationParams::Profile2d(p) => {
            assert_eq!(
                p.curve_id, curve_id,
                "curve_id in operation should survive roundtrip"
            );
            assert_eq!(
                p.cut_type,
                CutType::Outside,
                "cut_type should survive roundtrip"
            );
        }
        other => panic!("expected Profile2d operation, got: {other:?}"),
    }
}
