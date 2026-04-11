//! G-code viewer IPC command handlers.
//!
//! Provides three commands that drive the Toolpath Viewer mode:
//!
//! - [`load_gcode_for_viewer`] — parse a G-code file, returning metadata and
//!   viewport geometry.
//! - [`simulate_gcode_viewer`] — run a dexel material-removal simulation on a
//!   G-code file with user-supplied stock and tool parameters.
//! - [`get_sample_gcode_path`] — return the resolved on-disk path to the
//!   bundled sample G-code file.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern.
//! The `_inner` functions take plain Rust arguments and are directly testable
//! without Tauri.

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::dexel::{flat_endmill_clearance, gcode_segments_to_dexel, DexelGrid};
use crate::error::AppError;
use crate::gcode_parser::{
    self, gcode_segments_to_line_geometry, parse_metadata, GcodeStockMetadata, GcodeToolMetadata,
    ParseWarning,
};
use crate::geometry::MeshData;
use crate::models::stock::{BoxDimensions, Vec3};
use crate::models::StockDefinition;
use crate::toolpath::types::LineGeometryData;

// ── Result type for load_gcode_for_viewer ────────────────────────────────────

/// Return type of [`load_gcode_for_viewer`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcodeViewerLoadResult {
    /// Parsed stock metadata, if a valid `; @STOCK` comment was found.
    pub stock: Option<GcodeStockMetadata>,
    /// Parsed tool metadata entries (one per valid `; @TOOL` comment).
    pub tools: Vec<GcodeToolMetadata>,
    /// Toolpath centerline geometry for immediate 3D viewport display.
    pub line_geometry: LineGeometryData,
    /// Non-fatal warnings from G-code parsing and metadata parsing combined.
    pub warnings: Vec<ParseWarning>,
}

// ── load_gcode_for_viewer ─────────────────────────────────────────────────────

/// Testable inner logic for [`load_gcode_for_viewer`].
///
/// Reads a G-code file, parses it with the G-code parser, extracts `@STOCK`
/// and `@TOOL` metadata from the header, and converts the motion segments to
/// `LineGeometryData` for the 3D viewport.
///
/// Warnings from both the G-code parser and the metadata parser are merged
/// into a single list.
pub fn load_gcode_for_viewer_inner(path: &str) -> Result<GcodeViewerLoadResult, AppError> {
    // Read the file from disk.
    if !std::path::Path::new(path).exists() {
        return Err(AppError::FileNotFound);
    }
    let content = std::fs::read_to_string(path)?;

    // Parse the G-code text.
    let parsed = gcode_parser::parse_gcode(&content);

    // Parse structured metadata from the header comments.
    let meta = parse_metadata(&parsed.metadata.header_comments);

    // Merge warnings from both sources.
    let mut warnings: Vec<ParseWarning> = parsed.warnings;
    warnings.extend(meta.warnings);

    // Build viewport line geometry from the motion segments.
    let line_geometry = gcode_segments_to_line_geometry(&parsed.segments);

    Ok(GcodeViewerLoadResult {
        stock: meta.stock,
        tools: meta.tools,
        line_geometry,
        warnings,
    })
}

/// Parse a G-code file, extracting metadata and generating viewport geometry.
///
/// Returns a composite result containing optional stock metadata, tool
/// metadata entries, toolpath centerline geometry, and any non-fatal warnings.
#[tauri::command]
pub async fn load_gcode_for_viewer(path: String) -> Result<GcodeViewerLoadResult, AppError> {
    load_gcode_for_viewer_inner(&path)
}

// ── simulate_gcode_viewer ────────────────────────────────────────────────────

/// Testable inner logic for [`simulate_gcode_viewer`].
///
/// Re-parses the G-code file, converts segments to dexel segments, and runs
/// the dexel material-removal simulation with the provided stock and tool.
///
/// # Validation
/// - `resolution` must be in `[0.01, 5.0]`.
/// - `width`, `depth`, and `height` must all be positive (> 0). `origin_x`,
///   `origin_y`, and `origin_z` are unconstrained.
/// - `tool_type` must be `"flat_endmill"` (the only type supported in this
///   initial implementation).
#[allow(clippy::too_many_arguments)]
pub fn simulate_gcode_viewer_inner(
    path: &str,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
    width: f64,
    depth: f64,
    height: f64,
    tool_type: &str,
    tool_diameter: f64,
    resolution: f64,
) -> Result<MeshData, AppError> {
    // (a) Validate resolution.
    if !(0.01..=5.0).contains(&resolution) {
        return Err(AppError::InvalidInput(
            "resolution must be between 0.01 and 5.0".into(),
        ));
    }

    // (b) Validate stock dimensions (size only; origin is unconstrained).
    if width <= 0.0 {
        return Err(AppError::InvalidInput(
            "stock width must be positive".into(),
        ));
    }
    if depth <= 0.0 {
        return Err(AppError::InvalidInput(
            "stock depth must be positive".into(),
        ));
    }
    if height <= 0.0 {
        return Err(AppError::InvalidInput(
            "stock height must be positive".into(),
        ));
    }

    // (c) Validate tool type.
    let tool_type_lower = tool_type.to_lowercase();
    if tool_type_lower != "flat_endmill" {
        return Err(AppError::InvalidInput(format!(
            "tool type '{tool_type}' is not supported; only 'flat_endmill' is supported in \
             the initial implementation"
        )));
    }

    // (d) Validate tool diameter.
    if tool_diameter <= 0.0 {
        return Err(AppError::InvalidInput(
            "tool diameter must be positive".into(),
        ));
    }

    // (e) Check file exists and parse.
    if !std::path::Path::new(path).exists() {
        return Err(AppError::FileNotFound);
    }
    let content = std::fs::read_to_string(path)?;
    let parsed = gcode_parser::parse_gcode(&content);

    // (f) Convert G-code segments to dexel segments.
    let dexel_segments = gcode_segments_to_dexel(&parsed.segments);

    // (g) Build stock definition.
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3 {
            x: origin_x,
            y: origin_y,
            z: origin_z,
        },
        width,
        depth,
        height,
    });

    // (h) Build tool clearance profile (flat endmill).
    let tool_radius = tool_diameter / 2.0;
    let z_clearance = flat_endmill_clearance(tool_radius);

    // (i) Run dexel simulation.
    let mut grid = DexelGrid::from_stock(&stock, resolution);
    grid.apply_segments(&dexel_segments, tool_radius, &z_clearance);

    Ok(grid.extract_mesh())
}

/// Run a dexel material-removal simulation on a G-code file.
///
/// The file is re-parsed on every invocation (stateless design). Stock and
/// tool are supplied by the caller rather than read from the project state.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn simulate_gcode_viewer(
    path: String,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
    width: f64,
    depth: f64,
    height: f64,
    tool_type: String,
    tool_diameter: f64,
    resolution: f64,
) -> Result<MeshData, AppError> {
    simulate_gcode_viewer_inner(
        &path,
        origin_x,
        origin_y,
        origin_z,
        width,
        depth,
        height,
        &tool_type,
        tool_diameter,
        resolution,
    )
}

// ── get_sample_gcode_path ─────────────────────────────────────────────────────

/// Return the absolute path to the bundled sample G-code file.
///
/// Uses Tauri's resource directory API to resolve the platform-correct path.
/// The sample file is bundled at `samples/demo-pocket.nc` relative to the
/// resource directory (see `tauri.conf.json` → `bundle.resources`).
#[tauri::command]
pub fn get_sample_gcode_path(app: tauri::AppHandle) -> Result<String, AppError> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::Io(format!("could not resolve resource directory: {e}")))?;
    let sample_path = resource_dir.join("samples").join("demo-pocket.nc");
    sample_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Io("sample path contains invalid UTF-8".into()))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Path to the demo-pocket.nc file used in integration tests.
    fn demo_nc_path() -> String {
        format!(
            "{}/../resources/samples/demo-pocket.nc",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    // ── load_gcode_for_viewer tests ───────────────────────────────────────

    #[test]
    fn load_returns_metadata_from_demo_file() {
        let result = load_gcode_for_viewer_inner(&demo_nc_path()).unwrap();
        let stock = result.stock.unwrap();
        assert_eq!(stock.width, 100.0);
        assert_eq!(stock.depth, 100.0);
        assert_eq!(stock.height, 20.0);
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].tool_type, "flat_endmill");
        assert_eq!(result.tools[0].diameter, 10.0);
    }

    #[test]
    fn load_returns_non_empty_line_geometry_from_demo_file() {
        let result = load_gcode_for_viewer_inner(&demo_nc_path()).unwrap();
        assert!(!result.line_geometry.positions.is_empty());
        assert!(!result.line_geometry.types.is_empty());
        assert_eq!(
            result.line_geometry.positions.len(),
            result.line_geometry.colours.len()
        );
        assert_eq!(
            result.line_geometry.types.len() * 6,
            result.line_geometry.positions.len()
        );
    }

    #[test]
    fn load_warnings_list_present_and_empty_for_valid_file() {
        let result = load_gcode_for_viewer_inner(&demo_nc_path()).unwrap();
        // demo-pocket.nc is valid; no warnings expected.
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn load_file_not_found_returns_error() {
        let result = load_gcode_for_viewer_inner("/nonexistent/path/file.nc");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::FileNotFound));
    }

    // ── simulate_gcode_viewer tests ───────────────────────────────────────

    /// Assert that a `MeshData` is structurally valid (matches the helper in dexel tests).
    fn assert_mesh_valid(mesh: &MeshData) {
        let vertex_count = mesh.vertices.len() / 3;
        assert!(vertex_count > 0, "mesh has no vertices");
        assert_eq!(mesh.vertices.len() % 3, 0);
        assert_eq!(mesh.normals.len(), mesh.vertices.len());
        assert!(mesh.indices.len() >= 3);
        assert_eq!(mesh.indices.len() % 3, 0);
        for &idx in &mesh.indices {
            assert!((idx as usize) < vertex_count);
        }
    }

    #[test]
    fn simulate_demo_file_produces_valid_mesh() {
        let mesh = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0, // origin
            100.0,
            100.0,
            20.0, // stock
            "flat_endmill",
            10.0, // diameter
            2.0,  // resolution (coarse for speed)
        )
        .unwrap();
        assert_mesh_valid(&mesh);
    }

    #[test]
    fn simulate_negative_origin_is_valid() {
        // Negative origin coordinates must not be rejected.
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            -5.0,
            -5.0,
            0.0,
            100.0,
            100.0,
            20.0,
            "flat_endmill",
            10.0,
            2.0,
        );
        assert!(
            result.is_ok(),
            "negative origin should be accepted: {result:?}"
        );
    }

    #[test]
    fn simulate_error_resolution_too_low() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            100.0,
            100.0,
            20.0,
            "flat_endmill",
            10.0,
            0.001,
        );
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_error_resolution_too_high() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            100.0,
            100.0,
            20.0,
            "flat_endmill",
            10.0,
            10.0,
        );
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_error_zero_width() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            0.0,
            100.0,
            20.0,
            "flat_endmill",
            10.0,
            1.0,
        );
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_error_negative_depth() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            100.0,
            -1.0,
            20.0,
            "flat_endmill",
            10.0,
            1.0,
        );
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_error_zero_height() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            100.0,
            100.0,
            0.0,
            "flat_endmill",
            10.0,
            1.0,
        );
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn simulate_error_unsupported_tool_type() {
        let result = simulate_gcode_viewer_inner(
            &demo_nc_path(),
            0.0,
            0.0,
            0.0,
            100.0,
            100.0,
            20.0,
            "ball_nose",
            10.0,
            1.0,
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput(_)),
            "expected InvalidInput, got {err:?}"
        );
        if let AppError::InvalidInput(msg) = err {
            assert!(
                msg.contains("ball_nose") || msg.contains("flat_endmill"),
                "error should mention the tool type: {msg}"
            );
        }
    }

    #[test]
    fn simulate_error_file_not_found() {
        let result = simulate_gcode_viewer_inner(
            "/nonexistent/path/file.nc",
            0.0,
            0.0,
            0.0,
            100.0,
            100.0,
            20.0,
            "flat_endmill",
            10.0,
            1.0,
        );
        assert!(matches!(result.unwrap_err(), AppError::FileNotFound));
    }
}
