use jamiecam_lib::models::operation::{DrillParams, DrillPoint};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
use jamiecam_lib::toolpath::operations::drill::drill_passes;
use jamiecam_lib::toolpath::Toolpath;
use std::path::PathBuf;
use uuid::Uuid;

fn golden_dir(controller: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/integration/golden_gcode")
        .join(controller)
}

fn load_toolpath(controller: &str) -> Toolpath {
    let path = golden_dir(controller).join("simple_pocket.toolpath.json");
    let json =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {path:?}: {e}"));
    serde_json::from_str(&json).expect("deserialize toolpath")
}

fn load_golden(controller: &str) -> String {
    let path = golden_dir(controller).join("simple_pocket.nc");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read golden {path:?}: {e}"))
}

#[test]
fn fanuc_0i_golden_matches() {
    let toolpath = load_toolpath("fanuc-0i");
    let pp = PostProcessor::builtin("fanuc-0i").expect("load fanuc-0i");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 10.0,
        description: "10mm Flat Endmill".to_string(),
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
    assert_eq!(
        output,
        load_golden("fanuc-0i"),
        "fanuc-0i golden file mismatch"
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

#[test]
fn linuxcnc_golden_matches() {
    let toolpath = load_toolpath("linuxcnc");
    let pp = PostProcessor::builtin("linuxcnc").expect("load linuxcnc");
    let tool_info = ToolInfo {
        number: 1,
        diameter: 10.0,
        description: "10mm Flat Endmill".to_string(),
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
    assert_eq!(
        output,
        load_golden("linuxcnc"),
        "linuxcnc golden file mismatch"
    );
}
