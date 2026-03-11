#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::ZLevelRoughingParams;
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::zlevel_roughing::zlevel_roughing_passes;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/zlevel_roughing_golden.json")
}

#[test]
fn zlevel_roughing_golden_matches() {
    let step_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
    let shape = OcctShape::load_step(&step_path).expect("load box.step");
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

    let params = ZLevelRoughingParams {
        depth: 5.0,
        stepdown: 2.0,
        stepover: 0.4,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    };

    let passes = zlevel_roughing_passes(&stock, &params, 6.0, Some(&shape))
        .expect("zlevel_roughing_passes should succeed");
    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path();
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("read fixture {fixture:?}: {e}"));
    assert_eq!(
        json, golden,
        "zlevel_roughing output does not match golden file"
    );
}
