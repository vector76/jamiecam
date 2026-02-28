use jamiecam_lib::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
use jamiecam_lib::toolpath::Toolpath;
use std::path::PathBuf;

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
