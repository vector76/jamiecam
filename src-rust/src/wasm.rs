//! WebAssembly entry points for the browser frontend.
//!
//! Covers both modes: Mode 1 (G-code Viewer) takes G-code as a `&str`, Mode 2
//! (2D Profile Cuts) takes SVG/DXF file contents as `&[u8]` and the planner /
//! emitter inputs as `JsValue` deserialised via `serde_wasm_bindgen`.
//!
//! Each `#[wasm_bindgen]` wrapper delegates to a plain Rust `_inner` function
//! that takes ordinary Rust types, so the core logic can be tested without
//! the wasm runtime. `AppError` is serialised to a `JsValue` on the error
//! path so the TS frontend can pattern-match on the `kind` discriminant.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::dexel::{flat_endmill_clearance, gcode_segments_to_dexel, DexelGrid};
use crate::error::AppError;
use crate::gcode_parser::{
    self, gcode_segments_to_line_geometry, parse_metadata, GcodeStockMetadata, GcodeToolMetadata,
    ParseWarning,
};
use crate::geometry2d::Polyline;
use crate::grbl::emit_grbl;
use crate::parsers::{dxf as dxf_parser, svg as svg_parser};
use crate::profile::{generate_profile, ProfileOperationInput, ToolpathOutput};
use crate::types::{BoxDimensions, LineGeometryData, MeshData, StockDefinition};
use crate::working_env::Tool;

// ── load_gcode_for_viewer ─────────────────────────────────────────────────────

/// Composite return of [`load_gcode_for_viewer_inner`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcodeViewerLoadResult {
    pub stock: Option<GcodeStockMetadata>,
    pub tools: Vec<GcodeToolMetadata>,
    pub line_geometry: LineGeometryData,
    pub warnings: Vec<ParseWarning>,
}

/// Parse G-code text and produce viewport geometry + header metadata.
///
/// Pure logic, no I/O — accepts the file contents as a `&str` because
/// the browser cannot expose a real filesystem to the wasm module.
pub fn load_gcode_for_viewer_inner(content: &str) -> Result<GcodeViewerLoadResult, AppError> {
    let parsed = gcode_parser::parse_gcode(content);
    let meta = parse_metadata(&parsed.metadata.header_comments);

    let mut warnings = parsed.warnings;
    warnings.extend(meta.warnings);

    let line_geometry = gcode_segments_to_line_geometry(&parsed.segments);

    Ok(GcodeViewerLoadResult {
        stock: meta.stock,
        tools: meta.tools,
        line_geometry,
        warnings,
    })
}

#[wasm_bindgen(js_name = loadGcodeForViewer)]
pub fn load_gcode_for_viewer(content: &str) -> Result<JsValue, JsValue> {
    let result = load_gcode_for_viewer_inner(content).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── simulate_gcode_viewer ─────────────────────────────────────────────────────

/// Parameters for the dexel material-removal simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateGcodeViewerParams {
    pub stock: BoxDimensions,
    pub tool_diameter: f64,
    pub resolution: f64,
}

/// Run a dexel simulation of the supplied G-code against the given stock and
/// tool, returning the resulting workpiece mesh.
///
/// Currently assumes a flat endmill — tool type selection is deferred until
/// the UI exposes it.
pub fn simulate_gcode_viewer_inner(
    content: &str,
    params: &SimulateGcodeViewerParams,
) -> Result<MeshData, AppError> {
    fn positive(x: f64) -> bool {
        x.is_finite() && x > 0.0
    }

    if !positive(params.tool_diameter) {
        return Err(AppError::InvalidInput(
            "tool diameter must be positive".into(),
        ));
    }
    if !positive(params.resolution) {
        return Err(AppError::InvalidInput("resolution must be positive".into()));
    }
    if !(positive(params.stock.width)
        && positive(params.stock.depth)
        && positive(params.stock.height))
    {
        return Err(AppError::InvalidInput(
            "stock dimensions must be positive".into(),
        ));
    }

    let parsed = gcode_parser::parse_gcode(content);
    let segments = gcode_segments_to_dexel(&parsed.segments);

    let stock = StockDefinition::Box(params.stock.clone());
    let mut grid = DexelGrid::from_stock(&stock, params.resolution);
    let tool_radius = params.tool_diameter / 2.0;
    let clearance = flat_endmill_clearance(tool_radius);
    grid.apply_segments(&segments, tool_radius, &clearance);

    Ok(grid.extract_mesh())
}

#[wasm_bindgen(js_name = simulateGcodeViewer)]
pub fn simulate_gcode_viewer(content: &str, params: JsValue) -> Result<JsValue, JsValue> {
    let params: SimulateGcodeViewerParams =
        serde_wasm_bindgen::from_value(params).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mesh = simulate_gcode_viewer_inner(content, &params).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&mesh).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── parse_svg ─────────────────────────────────────────────────────────────────

/// Serializable result of [`parse_svg_inner`]. Mirrors
/// [`crate::parsers::svg::ParsedSvg`] but derives `Serialize`/`Deserialize` so
/// it can cross the wasm boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseSvgResult {
    pub paths: Vec<Polyline>,
    pub warnings: Vec<ParseWarning>,
}

/// Parse an SVG document into 2D polylines in millimetres.
pub fn parse_svg_inner(bytes: &[u8]) -> Result<ParseSvgResult, AppError> {
    let parsed = svg_parser::parse_svg(bytes)?;
    Ok(ParseSvgResult {
        paths: parsed.paths,
        warnings: parsed.warnings,
    })
}

#[wasm_bindgen(js_name = parseSvg)]
pub fn parse_svg(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let result = parse_svg_inner(bytes).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── parse_dxf ─────────────────────────────────────────────────────────────────

/// Serializable result of [`parse_dxf_inner`]. Mirrors
/// [`crate::parsers::dxf::ParsedDxf`] but derives `Serialize`/`Deserialize` so
/// it can cross the wasm boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseDxfResult {
    pub paths: Vec<Polyline>,
    pub warnings: Vec<ParseWarning>,
}

/// Parse a DXF document into 2D polylines in millimetres.
pub fn parse_dxf_inner(bytes: &[u8]) -> Result<ParseDxfResult, AppError> {
    let parsed = dxf_parser::parse_dxf(bytes)?;
    Ok(ParseDxfResult {
        paths: parsed.paths,
        warnings: parsed.warnings,
    })
}

#[wasm_bindgen(js_name = parseDxf)]
pub fn parse_dxf(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let result = parse_dxf_inner(bytes).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── generate_profile_toolpath ─────────────────────────────────────────────────

/// Run the Mode 2 profile planner. Inner thin wrapper around
/// [`crate::profile::generate_profile`] so the wasm shim can stay symmetrical
/// with the other exports.
pub fn generate_profile_toolpath_inner(
    input: &ProfileOperationInput,
) -> Result<ToolpathOutput, AppError> {
    generate_profile(input)
}

#[wasm_bindgen(js_name = generateProfileToolpath)]
pub fn generate_profile_toolpath(input: JsValue) -> Result<JsValue, JsValue> {
    let input: ProfileOperationInput =
        serde_wasm_bindgen::from_value(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let toolpath = generate_profile_toolpath_inner(&input).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&toolpath).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── emit_grbl_gcode ───────────────────────────────────────────────────────────

/// Render a planner-generated toolpath as a GRBL-flavoured G-code program.
pub fn emit_grbl_gcode_inner(
    toolpath: &ToolpathOutput,
    tool: &Tool,
    stock: &BoxDimensions,
) -> Result<String, AppError> {
    emit_grbl(toolpath, tool, stock)
}

#[wasm_bindgen(js_name = emitGrblGcode)]
pub fn emit_grbl_gcode(
    toolpath: JsValue,
    tool: JsValue,
    stock: JsValue,
) -> Result<JsValue, JsValue> {
    let toolpath: ToolpathOutput =
        serde_wasm_bindgen::from_value(toolpath).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let tool: Tool =
        serde_wasm_bindgen::from_value(tool).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let stock: BoxDimensions =
        serde_wasm_bindgen::from_value(stock).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let gcode = emit_grbl_gcode_inner(&toolpath, &tool, &stock).map_err(app_error_to_js)?;
    serde_wasm_bindgen::to_value(&gcode).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ── init ──────────────────────────────────────────────────────────────────────

/// Install panic hook so Rust panics surface as readable messages in the
/// browser console. Call once at app startup.
#[wasm_bindgen(start)]
pub fn wasm_init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn app_error_to_js(err: AppError) -> JsValue {
    serde_wasm_bindgen::to_value(&err).unwrap_or_else(|_| JsValue::from_str(&err.to_string()))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal G-code program with `@STOCK` and `@TOOL` headers and a few
    /// motion commands. Enough to exercise the parser + metadata + line
    /// geometry without depending on an external fixture.
    const TINY_PROGRAM: &str = r#"; @STOCK type=box width=100 depth=50 height=10
; @TOOL number=1 type=flat_endmill diameter=6
G21
G90
T1 M6
S12000 M3
G0 X0 Y0 Z5
G1 Z-1 F200
G1 X20 Y0 F800
G1 X20 Y20
G1 X0 Y20
G1 X0 Y0
G0 Z5
M30
"#;

    #[test]
    fn loads_stock_metadata() {
        let result = load_gcode_for_viewer_inner(TINY_PROGRAM).unwrap();
        let stock = result.stock.expect("stock metadata present");
        assert_eq!(stock.width, 100.0);
        assert_eq!(stock.depth, 50.0);
        assert_eq!(stock.height, 10.0);
    }

    #[test]
    fn loads_tool_metadata() {
        let result = load_gcode_for_viewer_inner(TINY_PROGRAM).unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].tool_type, "flat_endmill");
        assert_eq!(result.tools[0].diameter, 6.0);
    }

    #[test]
    fn produces_non_empty_line_geometry() {
        let result = load_gcode_for_viewer_inner(TINY_PROGRAM).unwrap();
        assert!(!result.line_geometry.positions.is_empty());
        assert!(!result.line_geometry.types.is_empty());
        // Each segment contributes 6 floats to positions and 1 byte to types.
        assert_eq!(
            result.line_geometry.types.len() * 6,
            result.line_geometry.positions.len()
        );
        assert_eq!(
            result.line_geometry.positions.len(),
            result.line_geometry.colours.len()
        );
    }

    #[test]
    fn produces_no_warnings_for_valid_program() {
        let result = load_gcode_for_viewer_inner(TINY_PROGRAM).unwrap();
        assert!(
            result.warnings.is_empty(),
            "got warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn empty_input_yields_empty_geometry() {
        let result = load_gcode_for_viewer_inner("").unwrap();
        assert!(result.stock.is_none());
        assert!(result.tools.is_empty());
        assert!(result.line_geometry.positions.is_empty());
    }

    // ── simulate_gcode_viewer_inner ──────────────────────────────────────

    fn box_params(tool_diameter: f64, resolution: f64) -> SimulateGcodeViewerParams {
        SimulateGcodeViewerParams {
            stock: BoxDimensions {
                origin: crate::types::Vec3::zero(),
                width: 100.0,
                depth: 50.0,
                height: 10.0,
            },
            tool_diameter,
            resolution,
        }
    }

    #[test]
    fn simulate_rejects_zero_tool_diameter() {
        let err = simulate_gcode_viewer_inner(TINY_PROGRAM, &box_params(0.0, 1.0)).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_rejects_zero_resolution() {
        let err = simulate_gcode_viewer_inner(TINY_PROGRAM, &box_params(3.0, 0.0)).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_rejects_zero_stock_height() {
        let mut params = box_params(3.0, 1.0);
        params.stock.height = 0.0;
        let err = simulate_gcode_viewer_inner(TINY_PROGRAM, &params).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_empty_program_returns_full_stock_mesh() {
        // No motion segments → mesh should be the untouched stock box: 6 faces ×
        // 2 triangles per cell, scaled by grid cells.
        let result =
            simulate_gcode_viewer_inner("", &box_params(3.0, 5.0)).expect("simulation succeeds");
        assert!(!result.vertices.is_empty());
        assert!(!result.indices.is_empty());
        assert_eq!(result.indices.len() % 3, 0);
        assert_eq!(result.vertices.len(), result.normals.len());
    }

    #[test]
    fn simulate_cutting_program_removes_top_surface() {
        // A program that cuts into the stock must reduce the number of
        // top-surface vertices remaining at the original stock height: cells
        // touched by the tool either disappear (their column empties) or get
        // lowered below the original top.
        let params = box_params(6.0, 2.0);
        let uncut = simulate_gcode_viewer_inner("", &params).unwrap();
        let cut = simulate_gcode_viewer_inner(TINY_PROGRAM, &params).unwrap();

        let stock_top = params.stock.height as f32;
        let uncut_top_verts = count_top_surface_at_z(&uncut, stock_top);
        let cut_top_verts = count_top_surface_at_z(&cut, stock_top);

        assert!(
            uncut_top_verts > 0,
            "untouched stock must have a top surface"
        );
        assert!(
            cut_top_verts < uncut_top_verts,
            "cut mesh should have fewer top-surface vertices at the stock top \
             (uncut={uncut_top_verts}, cut={cut_top_verts})"
        );
    }

    fn count_top_surface_at_z(mesh: &MeshData, z: f32) -> usize {
        mesh.vertices
            .chunks_exact(3)
            .zip(mesh.normals.chunks_exact(3))
            .filter(|(v, n)| (n[2] - 1.0).abs() < 1e-3 && (v[2] - z).abs() < 1e-3)
            .count()
    }

    // ── parse_svg_inner ─────────────────────────────────────────────────

    const RECT_SVG: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="10mm" viewBox="0 0 20 10">
  <rect x="0" y="0" width="20" height="10" fill="black"/>
</svg>"#;

    #[test]
    fn parse_svg_inner_returns_paths_for_rect() {
        let result = parse_svg_inner(RECT_SVG).expect("rect SVG parses");
        assert!(!result.paths.is_empty(), "expected at least one polyline");
    }

    // ── parse_dxf_inner ─────────────────────────────────────────────────

    const LINE_DXF: &[u8] = include_bytes!("parsers/dxf_fixtures/line_mm.dxf");

    #[test]
    fn parse_dxf_inner_returns_paths_for_line() {
        let result = parse_dxf_inner(LINE_DXF).expect("LINE DXF parses");
        assert!(!result.paths.is_empty(), "expected at least one polyline");
    }

    // ── generate_profile_toolpath_inner ─────────────────────────────────

    fn profile_sample_tool() -> crate::working_env::Tool {
        crate::working_env::Tool {
            id: crate::working_env::ToolId::new("t1"),
            name: "1/8\" flat".into(),
            diameter: 3.175,
            flute_count: 2,
            length: 38.0,
            material: "carbide".into(),
            recommended: crate::working_env::FeedsAndSpeeds {
                spindle_rpm: 18000.0,
                feed_rate: 800.0,
                plunge_rate: 200.0,
            },
        }
    }

    fn profile_sample_input() -> ProfileOperationInput {
        use crate::geometry2d::Point2;
        use crate::profile::CutSide;
        ProfileOperationInput {
            boundaries: vec![Polyline::closed(vec![
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
                Point2::new(0.0, 10.0),
            ])],
            tool: profile_sample_tool(),
            cut_side: CutSide::Outside,
            depth_total: 3.0,
            depth_per_pass: 1.5,
            safe_z: 5.0,
            plunge_feed: 200.0,
            cut_feed: 800.0,
            spindle_rpm: 18000.0,
        }
    }

    #[test]
    fn generate_profile_toolpath_inner_emits_motions_for_square() {
        let toolpath =
            generate_profile_toolpath_inner(&profile_sample_input()).expect("planner succeeds");
        assert!(!toolpath.is_empty(), "expected at least one motion");
    }

    // ── emit_grbl_gcode_inner ───────────────────────────────────────────

    #[test]
    fn emit_grbl_gcode_inner_renders_program_for_simple_toolpath() {
        use crate::profile::ToolpathMotion;
        let toolpath = vec![
            ToolpathMotion::Rapid {
                to: [0.0, 0.0, 5.0],
            },
            ToolpathMotion::Linear {
                to: [10.0, 0.0, -1.0],
                feed: 800.0,
            },
        ];
        let tool = profile_sample_tool();
        let stock = BoxDimensions {
            origin: crate::types::Vec3::zero(),
            width: 100.0,
            depth: 50.0,
            height: 10.0,
        };
        let gcode = emit_grbl_gcode_inner(&toolpath, &tool, &stock).expect("emitter succeeds");
        assert!(gcode.contains("G21"), "expected mm mode in:\n{gcode}");
        assert!(
            gcode.contains("G1 X10 Y0 Z-1 F800"),
            "expected linear cut line in:\n{gcode}"
        );
    }
}
