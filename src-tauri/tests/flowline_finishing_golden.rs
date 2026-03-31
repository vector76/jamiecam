#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::{FlowlineDirection, FlowlineFinishingParams};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::operations::flowline_finishing::flowline_finishing_passes;
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

fn make_tool() -> jamiecam_lib::models::Tool {
    use jamiecam_lib::models::tool::ToolType;
    use jamiecam_lib::models::Tool;
    use uuid::Uuid;
    Tool {
        id: Uuid::nil(),
        name: "6mm Ball Endmill".to_string(),
        tool_type: ToolType::BallNose,
        material: Some("carbide".to_string()),
        diameter: 6.0,
        flute_count: Some(4),
        default_spindle_speed: Some(10000),
        default_feed_rate: Some(500.0),
        cutting_length: 18.0,
        shank_diameter: 6.0,
        corner_radius: None,
        included_angle: None,
        point_angle: None,
        pilot_diameter: None,
        pilot_length: None,
        thread_pitch: None,
        min_bore_diameter: None,
        taper_half_angle: None,
        overall_length: Some(54.0),
    }
}

/// Compute the elevation angle (degrees above the XY plane) of the dominant
/// cross-pass direction from a set of passes.
///
/// U-direction passes (latitude circles on a sphere) step over latitude, so
/// consecutive pass centroids move mainly in Z → high elevation angle.
/// V-direction passes (meridians) step over longitude, so consecutive centroids
/// move mainly in XY → low elevation angle.
fn cross_pass_elevation_angle(passes: &[jamiecam_lib::toolpath::types::Pass]) -> f64 {
    let centroids: Vec<[f64; 3]> = passes
        .iter()
        .filter(|p| !p.cuts.is_empty())
        .map(|p| {
            let n = p.cuts.len() as f64;
            let sx: f64 = p.cuts.iter().map(|c| c.position.x).sum();
            let sy: f64 = p.cuts.iter().map(|c| c.position.y).sum();
            let sz: f64 = p.cuts.iter().map(|c| c.position.z).sum();
            [sx / n, sy / n, sz / n]
        })
        .collect();
    let mut sum_dz_sq = 0.0_f64;
    let mut sum_dxy_sq = 0.0_f64;
    for window in centroids.windows(2) {
        let dx = window[1][0] - window[0][0];
        let dy = window[1][1] - window[0][1];
        let dz = window[1][2] - window[0][2];
        sum_dz_sq += dz * dz;
        sum_dxy_sq += dx * dx + dy * dy;
    }
    sum_dz_sq.sqrt().atan2(sum_dxy_sq.sqrt()).to_degrees()
}

// ---------------------------------------------------------------------------
// Test 1: JSON golden snapshot
// ---------------------------------------------------------------------------

#[test]
fn flowline_finishing_golden_matches() {
    let (shape, stock) = load_shape(&sphere_step_path());
    let params = FlowlineFinishingParams {
        stepover: 0.1,
        direction: FlowlineDirection::U,
        allowance: 0.0,
        tool_diameter: 6.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    };

    let passes =
        flowline_finishing_passes(&stock, &params, 6.0, Some(&shape)).expect("should succeed");
    let json = serde_json::to_string_pretty(&passes).expect("serialize passes");

    let fixture = fixture_path("flowline_finishing_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "flowline_finishing output does not match golden file"
    );

    // Verify Z span covers the sphere's curvature.
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
        z_max - z_min >= 1.0,
        "Z span too small ({:.3}mm): algorithm should follow sphere curvature",
        z_max - z_min
    );
}

// ---------------------------------------------------------------------------
// Test 2: G-code golden — full pipeline (linking + arc fitting + postprocessor)
// ---------------------------------------------------------------------------

#[test]
fn flowline_finishing_gcode_golden() {
    use jamiecam_lib::models::operation::{CacheState, OperationParams};
    use jamiecam_lib::models::Operation;
    use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
    use jamiecam_lib::toolpath::types::{LinkingParams, Toolpath, DEFAULT_CLEARANCE_OFFSET};
    use jamiecam_lib::toolpath::{arc_fitting, linking, planner};
    use uuid::Uuid;

    let (shape, stock) = load_shape(&box_step_path());
    let (_, _, _, _, _, zmax) = shape.bounding_box();

    let box_params = FlowlineFinishingParams {
        stepover: 2.0,
        direction: FlowlineDirection::U,
        allowance: 0.0,
        tool_diameter: 6.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    };

    let tool = make_tool();
    let operation = Operation {
        id: Uuid::nil(),
        name: "Flowline Finishing Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::FlowlineFinishing(box_params),
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
        diameter: tool.diameter,
        description: tool.name.clone(),
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

    let nc_fixture = fixture_path("flowline_finishing_golden.nc");
    if !nc_fixture.exists() {
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!("G-code fixture written to {nc_fixture:?}. Inspect output, then re-run to lock.");
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "flowline_finishing G-code does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Structural/perpendicularity — U and V directions differ by >= 45 deg
// ---------------------------------------------------------------------------

#[test]
fn flowline_finishing_u_v_perpendicular() {
    use jamiecam_lib::models::operation::{CacheState, OperationParams};
    use jamiecam_lib::models::Operation;
    use jamiecam_lib::toolpath::planner;
    use uuid::Uuid;

    let (shape, stock) = load_shape(&sphere_step_path());

    let tool = make_tool();

    let make_op = |direction: FlowlineDirection| Operation {
        id: Uuid::nil(),
        name: "Flowline Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::FlowlineFinishing(FlowlineFinishingParams {
            stepover: 0.5,
            direction,
            allowance: 0.0,
            tool_diameter: 6.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }),
        cache: CacheState::default(),
    };

    let (passes_u, _) = planner::plan(
        &make_op(FlowlineDirection::U),
        &tool,
        &stock,
        Some(&shape),
        None,
    )
    .expect("U plan should succeed");
    let (passes_v, _) = planner::plan(
        &make_op(FlowlineDirection::V),
        &tool,
        &stock,
        Some(&shape),
        None,
    )
    .expect("V plan should succeed");

    assert!(!passes_u.is_empty(), "U direction produced no passes");
    assert!(!passes_v.is_empty(), "V direction produced no passes");

    // All passes should be Cutting (no linking passes in raw planner output).
    for pass in passes_u.iter().chain(passes_v.iter()) {
        assert_eq!(pass.kind, PassKind::Cutting, "expected all Cutting passes");
    }

    let elev_u = cross_pass_elevation_angle(&passes_u);
    let elev_v = cross_pass_elevation_angle(&passes_v);
    let diff = (elev_u - elev_v).abs();

    assert!(
        diff >= 45.0,
        "U and V cross-pass elevation angles should differ by at least 45 degrees, got {diff:.1}° (U={elev_u:.1}°, V={elev_v:.1}°)"
    );
}
