use jamiecam_lib::models::operation::{DrillParams, DrillPoint};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
use jamiecam_lib::toolpath::operations::drill::drill_passes;
use jamiecam_lib::toolpath::Toolpath;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(cam_geometry_bindings)]
use jamiecam_lib::geometry::OcctShape;
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::models::operation::{
    CacheState, OperationParams, PocketParams, ZLevelFinishingParams,
};
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::models::tool::ToolType;
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::models::{Operation, Tool};
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::toolpath::arc_fitting;
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::toolpath::linking;
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::toolpath::planner;
#[cfg(cam_geometry_bindings)]
use jamiecam_lib::toolpath::types::{LinkingParams, DEFAULT_CLEARANCE_OFFSET};

fn golden_dir(controller: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/integration/golden_gcode")
        .join(controller)
}

#[cfg(cam_geometry_bindings)]
fn pocket_toolpath() -> Toolpath {
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
        cutting_length: 30.0,
        shank_diameter: 10.0,
        overall_length: 90.0,
    };
    let arc_lead_in_radius = Some(5.0);
    let arc_lead_out_radius = Some(5.0);
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
            arc_lead_in_radius,
            arc_lead_out_radius,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        }),
        cache: CacheState::default(),
    };
    let (raw_passes, _stats) =
        planner::plan(&operation, &tool, &stock, None, None).expect("plan should succeed");

    // Apply linking (adds lead-in/lead-out with arc moves) + arc fitting,
    // matching the full calculate_toolpath pipeline.
    let stock_top_z = 10.0;
    let linked_passes = linking::link_passes(
        raw_passes,
        &LinkingParams {
            tool_diameter: tool.diameter,
            clearance_z: stock_top_z + DEFAULT_CLEARANCE_OFFSET,
            lead_ratio: linking::DEFAULT_LEAD_RATIO,
            arc_lead_in_radius,
            arc_lead_out_radius,
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

    Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 8000.0,
        feed_rate: 500.0,
        passes,
    }
}

#[cfg(cam_geometry_bindings)]
fn simple_pocket_golden(controller: &str) {
    let toolpath = pocket_toolpath();
    let dir = golden_dir(controller);
    std::fs::create_dir_all(&dir).expect("create golden dir");

    let toolpath_fixture = dir.join("simple_pocket.toolpath.json");
    let nc_fixture = dir.join("simple_pocket.nc");

    let pp = PostProcessor::builtin(controller).expect("load postprocessor");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 10.0,
        description: "10mm Flat Endmill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath.clone()],
            &[tool_info],
            GenerateOptions {
                program_number: Some(1000),
                include_comments: false,
            },
        )
        .expect("generate");

    if !nc_fixture.exists() {
        let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");
        std::fs::write(&toolpath_fixture, &json).expect("write toolpath fixture");
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "Fixtures written. Inspect {:?} — verify G02/G03 arcs from arc fitting. Re-run to lock.",
            nc_fixture
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "{controller} simple_pocket golden file mismatch"
    );
}

#[test]
#[cfg(cam_geometry_bindings)]
fn fanuc_0i_golden_matches() {
    simple_pocket_golden("fanuc-0i");
}

#[test]
#[cfg(cam_geometry_bindings)]
fn fanuc_0i_pocket_contains_arcs() {
    let nc = std::fs::read_to_string(golden_dir("fanuc-0i").join("simple_pocket.nc"))
        .expect("read fanuc-0i simple_pocket.nc");
    assert!(
        nc.contains("G02") || nc.contains("G03"),
        "expected arc commands (G02/G03) in pocket G-code"
    );
}

fn two_hole_drill_toolpath(peck_depth: Option<f64>) -> Toolpath {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        width: 100.0,
        depth: 100.0,
        height: 10.0,
    });
    let params = DrillParams {
        depth: 8.0,
        peck_depth,
        points: vec![
            DrillPoint { x: 10.0, y: 0.0 },
            DrillPoint { x: 30.0, y: 0.0 },
        ],
    };
    let passes = drill_passes(&stock, &params).expect("drill_passes must succeed");
    Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 3000.0,
        feed_rate: 150.0,
        passes,
    }
}

#[test]
fn test_assemble_nonpeck_cycle_g81() {
    let toolpath = two_hole_drill_toolpath(None);
    let pp = PostProcessor::builtin("fanuc-0i").expect("load fanuc-0i");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 8.0,
        description: "8mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(1000),
                include_comments: false,
            },
        )
        .expect("generate");
    assert!(output.contains("G81"), "expected G81 in output:\n{output}");
    assert!(output.contains("G80"), "expected G80 in output:\n{output}");
    assert!(
        !output.contains("G01"),
        "did not expect G01 in output:\n{output}"
    );
}

#[test]
fn test_assemble_peck_cycle_g83() {
    let toolpath = two_hole_drill_toolpath(Some(3.0));
    let pp = PostProcessor::builtin("fanuc-0i").expect("load fanuc-0i");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 8.0,
        description: "8mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(1000),
                include_comments: false,
            },
        )
        .expect("generate");
    assert!(output.contains("G83"), "expected G83 in output:\n{output}");
    assert!(output.contains('Q'), "expected Q in output:\n{output}");
    assert!(output.contains("G80"), "expected G80 in output:\n{output}");
    assert!(
        !output.contains("G01"),
        "did not expect G01 in output:\n{output}"
    );
}

#[test]
fn test_assemble_cycles_not_supported_uses_linear() {
    let toolpath = two_hole_drill_toolpath(Some(3.0));
    let pp = PostProcessor::builtin("grbl").expect("load grbl");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 8.0,
        description: "8mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath],
            &[tool_info],
            GenerateOptions {
                program_number: Some(1000),
                include_comments: false,
            },
        )
        .expect("generate");
    assert!(output.contains("G00"), "expected G00 in output:\n{output}");
    assert!(output.contains("G01"), "expected G01 in output:\n{output}");
    assert!(
        !output.contains("G81"),
        "did not expect G81 in output:\n{output}"
    );
    assert!(
        !output.contains("G83"),
        "did not expect G83 in output:\n{output}"
    );
    assert!(
        !output.contains("G80"),
        "did not expect G80 in output:\n{output}"
    );
}

fn five_hole_peck_toolpath() -> Toolpath {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        width: 50.0,
        depth: 50.0,
        height: 10.0,
    });
    let params = DrillParams {
        depth: 10.0,
        peck_depth: Some(3.0),
        points: vec![
            DrillPoint { x: 10.0, y: 10.0 },
            DrillPoint { x: 30.0, y: 10.0 },
            DrillPoint { x: 10.0, y: 30.0 },
            DrillPoint { x: 30.0, y: 30.0 },
            DrillPoint { x: 20.0, y: 20.0 },
        ],
    };
    let passes = drill_passes(&stock, &params).expect("drill_passes must succeed");
    Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 1200.0,
        feed_rate: 150.0,
        passes,
    }
}

#[test]
fn grbl_drill_expansion_golden_matches() {
    let toolpath = five_hole_peck_toolpath();
    let dir = golden_dir("grbl");
    std::fs::create_dir_all(&dir).expect("create grbl golden dir");

    let toolpath_fixture = dir.join("drill_expansion.toolpath.json");
    let nc_fixture = dir.join("drill_expansion.nc");

    let pp = PostProcessor::builtin("grbl").expect("load grbl");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 5.0,
        description: "5mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath.clone()],
            &[tool_info],
            GenerateOptions {
                program_number: None,
                include_comments: false,
            },
        )
        .expect("generate");

    if !nc_fixture.exists() {
        let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");
        std::fs::write(&toolpath_fixture, &json).expect("write toolpath fixture");
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "Fixtures written. Inspect {:?} — verify: no G81/G83/G80, correct peck Z steps, correct XY order. Re-run to lock.",
            nc_fixture
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(output, golden, "grbl drill_expansion golden file mismatch");
}

#[test]
fn linuxcnc_drill_cycle_golden_matches() {
    let toolpath = five_hole_peck_toolpath();
    let dir = golden_dir("linuxcnc");
    std::fs::create_dir_all(&dir).expect("create linuxcnc golden dir");

    let toolpath_fixture = dir.join("drill_cycle.toolpath.json");
    let nc_fixture = dir.join("drill_cycle.nc");

    let pp = PostProcessor::builtin("linuxcnc").expect("load linuxcnc");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 5.0,
        description: "5mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath.clone()],
            &[tool_info],
            GenerateOptions {
                program_number: None,
                include_comments: false,
            },
        )
        .expect("generate");

    if !nc_fixture.exists() {
        let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");
        std::fs::write(&toolpath_fixture, &json).expect("write toolpath fixture");
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "Fixtures written. Inspect {:?} — verify: G83 with correct Z/R/Q words, G80 cancel, sorted hole order matching Phase 1. Re-run to lock.",
            nc_fixture
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(output, golden, "linuxcnc drill_cycle golden file mismatch");
}

#[test]
fn fanuc_0i_drill_cycle_golden_matches() {
    let toolpath = five_hole_peck_toolpath();
    let dir = golden_dir("fanuc-0i");
    std::fs::create_dir_all(&dir).expect("create fanuc-0i golden dir");

    let toolpath_fixture = dir.join("drill_cycle.toolpath.json");
    let nc_fixture = dir.join("drill_cycle.nc");

    let pp = PostProcessor::builtin("fanuc-0i").expect("load fanuc-0i");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 5.0,
        description: "5mm Drill".to_string(),
    };
    let output = pp
        .generate(
            &[toolpath.clone()],
            &[tool_info],
            GenerateOptions {
                program_number: None,
                include_comments: false,
            },
        )
        .expect("generate");

    if !nc_fixture.exists() {
        let json = serde_json::to_string_pretty(&toolpath).expect("serialize toolpath");
        std::fs::write(&toolpath_fixture, &json).expect("write toolpath fixture");
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "Fixtures written. Inspect {:?} — verify: G83 with correct Z/R/Q words, G80 cancel, sorted hole order. Re-run to lock.",
            nc_fixture
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(output, golden, "fanuc-0i drill_cycle golden file mismatch");
}

#[test]
#[cfg(cam_geometry_bindings)]
fn linuxcnc_golden_matches() {
    simple_pocket_golden("linuxcnc");
}

#[cfg(cam_geometry_bindings)]
fn finishing_toolpath() -> Toolpath {
    let step_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/box.step");
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
    let tool = Tool {
        id: Uuid::nil(),
        name: "6mm Flat Endmill".to_string(),
        tool_type: ToolType::FlatEndmill,
        material: "carbide".to_string(),
        diameter: 6.0,
        flute_count: 4,
        default_spindle_speed: Some(10000),
        default_feed_rate: Some(400.0),
        cutting_length: 18.0,
        shank_diameter: 6.0,
        overall_length: 54.0,
    };
    let arc_lead_in_radius = Some(3.0);
    let arc_lead_out_radius = Some(3.0);
    let operation = Operation {
        id: Uuid::nil(),
        name: "Finishing Op".to_string(),
        enabled: true,
        tool_id: Uuid::nil(),
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::ZLevelFinishing(ZLevelFinishingParams {
            depth: 5.0,
            stepdown: 1.0,
            finishing_allowance: 0.1,
            spring_pass: false,
            geometry: None,
            arc_lead_in_radius,
            arc_lead_out_radius,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
            rest_machining: false,
            rest_machining_reference_id: None,
        }),
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
            arc_lead_in_radius,
            arc_lead_out_radius,
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

    Toolpath {
        operation_id: Uuid::nil(),
        tool_number: 1,
        spindle_speed: 10000.0,
        feed_rate: 400.0,
        passes,
    }
}

#[test]
#[cfg(cam_geometry_bindings)]
fn fanuc_0i_zlevel_finishing_golden_matches() {
    let toolpath = finishing_toolpath();
    let dir = golden_dir("fanuc-0i");
    std::fs::create_dir_all(&dir).expect("create golden dir");

    let nc_fixture = dir.join("zlevel_finishing.nc");

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
                program_number: Some(1000),
                include_comments: false,
            },
        )
        .expect("generate");

    if !nc_fixture.exists() {
        std::fs::write(&nc_fixture, &output).expect("write nc fixture");
        panic!(
            "Fixture written. Inspect {:?} — verify finishing G-code. Re-run to lock.",
            nc_fixture
        );
    }

    let golden = std::fs::read_to_string(&nc_fixture)
        .unwrap_or_else(|e| panic!("read golden {nc_fixture:?}: {e}"));
    assert_eq!(
        output, golden,
        "fanuc-0i zlevel_finishing golden file mismatch"
    );
}
