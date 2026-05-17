//! WebAssembly entry points for the Mode 1 (G-code Viewer) frontend.
//!
//! The browser passes G-code file contents (already read from a `<input
//! type="file">` element) as a JavaScript string. Each wasm-bindgen wrapper
//! delegates to a plain Rust `_inner` function that takes `&str` so it can
//! be tested without the wasm runtime.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::error::AppError;
use crate::gcode_parser::{
    self, gcode_segments_to_line_geometry, parse_metadata, GcodeStockMetadata, GcodeToolMetadata,
    ParseWarning,
};
use crate::types::LineGeometryData;

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
}
