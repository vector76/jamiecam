#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::{ZLevelFinishingParams, ZLevelRoughingParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::zlevel_finishing::{zlevel_finishing_passes, RoughingData};
use jamiecam_lib::toolpath::operations::zlevel_roughing::zlevel_roughing_passes;
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

fn finishing_params(spring_pass: bool) -> ZLevelFinishingParams {
    ZLevelFinishingParams {
        depth: 5.0,
        stepdown: 1.0,
        finishing_allowance: 0.1,
        spring_pass,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
        rest_machining: false,
        rest_machining_reference_id: None,
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

// ---------------------------------------------------------------------------
// Test 1: Basic finishing (no rest machining, no spring pass)
// ---------------------------------------------------------------------------

#[test]
fn zlevel_finishing_golden_matches() {
    let (shape, stock) = load_box();
    let params = finishing_params(false);

    let passes =
        zlevel_finishing_passes(&stock, &params, 6.0, Some(&shape), None).expect("should succeed");
    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("zlevel_finishing_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "zlevel_finishing output does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Finishing with spring pass
// ---------------------------------------------------------------------------

#[test]
fn zlevel_finishing_spring_pass_golden_matches() {
    let (shape, stock) = load_box();
    let params = finishing_params(true);

    let passes =
        zlevel_finishing_passes(&stock, &params, 6.0, Some(&shape), None).expect("should succeed");

    // Spring pass doubles the number of passes (Cutting + SpringPass per Z level)
    let no_spring_params = finishing_params(false);
    let no_spring_passes =
        zlevel_finishing_passes(&stock, &no_spring_params, 6.0, Some(&shape), None)
            .expect("should succeed");
    assert_eq!(
        passes.len(),
        no_spring_passes.len() * 2,
        "spring pass should double the pass count"
    );

    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("zlevel_finishing_spring_pass_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "zlevel_finishing spring pass output does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Rest machining behavioral test (NOT golden)
// ---------------------------------------------------------------------------

#[test]
fn rest_machining_reduces_or_equals_unconstrained() {
    let (shape, stock) = load_box();

    // Step 1: Compute roughing toolpath (same params as roughing golden test)
    let roughing_params = ZLevelRoughingParams {
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
    let roughing_tool_diameter = 6.0;
    let roughing_passes = zlevel_roughing_passes(
        &stock,
        &roughing_params,
        roughing_tool_diameter,
        Some(&shape),
    )
    .expect("roughing should succeed");

    let rd = RoughingData {
        passes: roughing_passes,
        tool_diameter: roughing_tool_diameter,
    };

    // Step 2: Finishing WITH rest machining
    let finishing_tool_diameter = 6.0;
    let params = finishing_params(false);
    let with_rest = zlevel_finishing_passes(
        &stock,
        &params,
        finishing_tool_diameter,
        Some(&shape),
        Some(&rd),
    )
    .expect("finishing with rest should succeed");

    // Step 3: Finishing WITHOUT rest machining
    let without_rest =
        zlevel_finishing_passes(&stock, &params, finishing_tool_diameter, Some(&shape), None)
            .expect("finishing without rest should succeed");

    // Rest machining should produce fewer or equal passes
    assert!(
        with_rest.len() <= without_rest.len(),
        "rest machining should produce fewer or equal passes: with_rest={}, without_rest={}",
        with_rest.len(),
        without_rest.len()
    );
}
