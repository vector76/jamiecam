//! WebAssembly entry points for the Mode 1 (G-code Viewer) frontend.
//!
//! The browser passes G-code file contents (already read from a `<input
//! type="file">` element) as a JavaScript string. Each wasm-bindgen wrapper
//! delegates to a plain Rust `_inner` function that takes `&str` so it can
//! be tested without the wasm runtime.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::dexel::{flat_endmill_clearance, gcode_segments_to_dexel, DexelGrid};
use crate::error::AppError;
use crate::gcode_parser::{
    self, gcode_segments_to_line_geometry, parse_metadata, GcodeStockMetadata, GcodeToolMetadata,
    ParseWarning,
};
use crate::types::{BoxDimensions, LineGeometryData, MeshData, StockDefinition};

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
}
