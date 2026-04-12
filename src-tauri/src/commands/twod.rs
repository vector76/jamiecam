//! 2D Profiling mode IPC command handlers.
//!
//! - [`load_2d_file`] — parse an SVG or DXF file and store the result as the
//!   project's active 2D artwork.
//! - [`get_2d_curves`] — return curve summaries and point data for the
//!   currently loaded 2D artwork, or `null` if none is loaded.
//! - [`set_safe_height`] / [`get_safe_height`] — manage the Z rapid-retract height.
//! - [`set_artwork_origin`] / [`get_artwork_origin`] — manage the XY artwork offset.
//! - [`generate_2d_gcode`] — run the full G-code generation pipeline for all
//!   enabled 2D Profile operations.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::twod::{
    parse_dxf, parse_svg, BoundingBox2d, CurveSummary, LoadedArtwork, UnitSystem,
};
use crate::models::Tool;
use crate::postprocessor::{program::GenerateOptions, PostProcessor, ToolInfo};
use crate::state::{AppState, Project};
use crate::toolpath::operations::twod_profile::plan_2d_profile;
use crate::toolpath::types::PassKind;
use crate::toolpath::{LineGeometryData, Toolpath, ToolpathStats};

use super::{read_project, write_project};

// ── Result types ──────────────────────────────────────────────────────────────

/// Return type of [`load_2d_file`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Load2dFileResult {
    /// Lightweight summaries (id, is_closed, bbox) for each curve.
    pub curves: Vec<CurveSummary>,
    /// Full point arrays keyed by curve UUID string.
    pub curve_points: HashMap<String, Vec<[f64; 2]>>,
    /// Unit system detected or supplied for this file.
    pub unit_system: UnitSystem,
    /// Non-fatal import warnings from the parser.
    pub warnings: Vec<String>,
}

/// Return type of [`generate_2d_gcode`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generate2dResult {
    /// Generated G-code text.
    pub gcode: String,
    /// Merged line geometry for viewport rendering.
    pub line_geometry: LineGeometryData,
    /// Non-fatal validation warnings (e.g. top_of_cut at/below stock top).
    pub warnings: Vec<String>,
    /// Aggregated toolpath statistics across all operations.
    pub stats: ToolpathStats,
}

/// Return type of [`get_2d_curves`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Get2dCurvesResult {
    /// Lightweight summaries (id, is_closed, bbox) for each curve.
    pub curves: Vec<CurveSummary>,
    /// Full point arrays keyed by curve UUID string.
    pub curve_points: HashMap<String, Vec<[f64; 2]>>,
    /// Unit system of the loaded artwork.
    pub unit_system: UnitSystem,
}

// ── load_2d_file ──────────────────────────────────────────────────────────────

/// Testable inner logic for [`load_2d_file`].
///
/// Reads a file, detects its type from the extension (`.svg` or `.dxf`,
/// case-insensitive), parses it, and stores the result as
/// `project.source_2d_artwork`.
///
/// For SVG files `unit_system_hint` is required; for DXF files it is ignored
/// (the unit system is read from `$INSUNITS`).
pub fn load_2d_file_inner(
    path: &str,
    unit_system_hint: Option<UnitSystem>,
    project_lock: &RwLock<Project>,
) -> Result<Load2dFileResult, AppError> {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Validate extension and SVG hint before touching disk so we fail fast
    // without unnecessary I/O.
    match ext.as_str() {
        "svg" if unit_system_hint.is_none() => {
            return Err(AppError::InvalidInput(
                "unit_system_hint is required for SVG files".to_string(),
            ));
        }
        "svg" | "dxf" => {}
        other => {
            return Err(AppError::InvalidInput(format!(
                "unsupported file extension '.{other}'; expected .svg or .dxf"
            )));
        }
    }

    let bytes = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::FileNotFound
        } else {
            AppError::Io(e.to_string())
        }
    })?;

    let (curves, unit_system, warnings) = match ext.as_str() {
        "svg" => {
            let unit_system = unit_system_hint.expect("validated above");
            let curves = parse_svg(&bytes, unit_system.clone())
                .map_err(|e| AppError::InvalidInput(format!("SVG parse error: {e}")))?;
            (curves, unit_system, Vec::<String>::new())
        }
        "dxf" => {
            let (curves, unit_system) = parse_dxf(&bytes)
                .map_err(|e| AppError::InvalidInput(format!("DXF parse error: {e}")))?;
            (curves, unit_system, Vec::<String>::new())
        }
        _ => unreachable!("extension validated above"),
    };

    // Build the return data from the original curves before moving them into
    // the artwork.  This avoids cloning the full Vec<Curve2d>.
    let summaries: Vec<CurveSummary> = curves
        .iter()
        .map(|c| CurveSummary {
            id: c.id,
            is_closed: c.is_closed,
            bbox: BoundingBox2d::from_points(&c.points),
        })
        .collect();

    let curve_points: HashMap<String, Vec<[f64; 2]>> = curves
        .iter()
        .map(|c| (c.id.to_string(), c.points.clone()))
        .collect();

    let artwork = LoadedArtwork {
        file_path: path.to_string(),
        unit_system: unit_system.clone(),
        curves, // moved, not cloned
        import_warnings: warnings.clone(),
    };

    {
        let mut project = write_project(project_lock)?;
        project.source_2d_artwork = Some(artwork);
    } // write lock released here

    Ok(Load2dFileResult {
        curves: summaries,
        curve_points,
        unit_system,
        warnings,
    })
}

/// Parse a 2D artwork file (SVG or DXF), store it as the project's active 2D
/// artwork, and return curve summaries with full point data.
#[tauri::command]
pub async fn load_2d_file(
    path: String,
    unit_system_hint: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Load2dFileResult, AppError> {
    let hint = unit_system_hint
        .as_deref()
        .map(|s| match s {
            "mm" => Ok(UnitSystem::Mm),
            "inches" => Ok(UnitSystem::Inches),
            other => Err(AppError::InvalidInput(format!(
                "unknown unit_system_hint '{other}'; expected 'mm' or 'inches'"
            ))),
        })
        .transpose()?;
    load_2d_file_inner(&path, hint, &state.project)
}

// ── get_2d_curves ─────────────────────────────────────────────────────────────

/// Testable inner logic for [`get_2d_curves`].
///
/// Returns `None` if no 2D artwork is currently loaded in the project;
/// otherwise returns curve summaries and full point data.
pub fn get_2d_curves_inner(
    project_lock: &RwLock<Project>,
) -> Result<Option<Get2dCurvesResult>, AppError> {
    let project = read_project(project_lock)?;

    let artwork = match &project.source_2d_artwork {
        None => return Ok(None),
        Some(a) => a,
    };

    let summaries: Vec<CurveSummary> = artwork
        .curves
        .iter()
        .map(|c| CurveSummary {
            id: c.id,
            is_closed: c.is_closed,
            bbox: BoundingBox2d::from_points(&c.points),
        })
        .collect();

    let curve_points: HashMap<String, Vec<[f64; 2]>> = artwork
        .curves
        .iter()
        .map(|c| (c.id.to_string(), c.points.clone()))
        .collect();

    Ok(Some(Get2dCurvesResult {
        curves: summaries,
        curve_points,
        unit_system: artwork.unit_system.clone(),
    }))
}

/// Return curve summaries and point data for the currently loaded 2D artwork.
///
/// Returns `null` (serialised as JSON `null`) when no artwork is loaded.
#[tauri::command]
pub async fn get_2d_curves(
    state: tauri::State<'_, AppState>,
) -> Result<Option<Get2dCurvesResult>, AppError> {
    get_2d_curves_inner(&state.project)
}

// ── set_safe_height / get_safe_height ────────────────────────────────────────

/// Testable inner logic for [`set_safe_height`].
///
/// Sets (or clears, when `None`) the project's safe height for 2D Profiling mode.
pub fn set_safe_height_inner(
    height: Option<f64>,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let mut project = write_project(project_lock)?;
    project.safe_height = height;
    Ok(())
}

/// Testable inner logic for [`get_safe_height`].
///
/// Returns the current safe height, or `None` if unset.
pub fn get_safe_height_inner(project_lock: &RwLock<Project>) -> Result<Option<f64>, AppError> {
    let project = read_project(project_lock)?;
    Ok(project.safe_height)
}

/// Set (or clear) the safe height for 2D Profiling mode rapid moves.
///
/// Pass `null` from the frontend to clear the safe height.
#[tauri::command]
pub async fn set_safe_height(
    height: Option<f64>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    set_safe_height_inner(height, &state.project)?;

    {
        let mut flag = state
            .dirty
            .write()
            .map_err(|e| AppError::Io(format!("dirty lock poisoned: {e}")))?;
        *flag = true;
    }

    crate::menu::update_save_enabled(&app, true);

    let is_open = *state
        .project_is_open
        .read()
        .map_err(|e| AppError::Io(format!("project_is_open lock poisoned: {e}")))?;
    let snapshot = super::project::get_project_snapshot_inner(&state.project, is_open, true)?;
    let _ = app.emit("project:modified", &snapshot);

    Ok(())
}

/// Return the current safe height for 2D Profiling mode, or `null` if unset.
#[tauri::command]
pub async fn get_safe_height(state: tauri::State<'_, AppState>) -> Result<Option<f64>, AppError> {
    get_safe_height_inner(&state.project)
}

// ── set_artwork_origin / get_artwork_origin ───────────────────────────────────

/// Testable inner logic for [`set_artwork_origin`].
///
/// Sets the artwork origin offset for 2D Profiling mode.
pub fn set_artwork_origin_inner(
    x: f64,
    y: f64,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let mut project = write_project(project_lock)?;
    project.artwork_origin = [x, y];
    Ok(())
}

/// Testable inner logic for [`get_artwork_origin`].
///
/// Returns the current artwork origin as `[x, y]`.
pub fn get_artwork_origin_inner(project_lock: &RwLock<Project>) -> Result<[f64; 2], AppError> {
    let project = read_project(project_lock)?;
    Ok(project.artwork_origin)
}

/// Set the artwork origin offset for 2D Profiling mode geometry.
#[tauri::command]
pub async fn set_artwork_origin(
    x: f64,
    y: f64,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    set_artwork_origin_inner(x, y, &state.project)?;

    {
        let mut flag = state
            .dirty
            .write()
            .map_err(|e| AppError::Io(format!("dirty lock poisoned: {e}")))?;
        *flag = true;
    }

    crate::menu::update_save_enabled(&app, true);

    let is_open = *state
        .project_is_open
        .read()
        .map_err(|e| AppError::Io(format!("project_is_open lock poisoned: {e}")))?;
    let snapshot = super::project::get_project_snapshot_inner(&state.project, is_open, true)?;
    let _ = app.emit("project:modified", &snapshot);

    Ok(())
}

/// Return the artwork origin offset for 2D Profiling mode as `[x, y]`.
#[tauri::command]
pub async fn get_artwork_origin(state: tauri::State<'_, AppState>) -> Result<[f64; 2], AppError> {
    get_artwork_origin_inner(&state.project)
}

// ── generate_2d_gcode ─────────────────────────────────────────────────────────

/// Testable inner logic for [`generate_2d_gcode`].
///
/// Full pipeline:
/// 1. Validate that `source_2d_artwork` is loaded.
/// 2. Validate that all enabled `Profile2d` operations reference closed curves
///    that exist in the loaded artwork.
/// 3. Enforce the single-tool constraint across enabled operations.
/// 4. Emit validation warnings when `top_of_cut ≤ stock.top_z()`.
/// 5. Call [`plan_2d_profile`] for each enabled operation in order.
/// 6. Post-process the combined toolpaths through the named post-processor.
/// 7. Derive merged [`LineGeometryData`] and aggregate [`ToolpathStats`].
pub fn generate_2d_gcode_inner(
    post_processor_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<Generate2dResult, AppError> {
    // ── Collect data under a single read lock ────────────────────────────────

    struct OpRecord {
        id: Uuid,
        name: String,
        tool_id: Uuid,
        params: crate::models::operation::Profile2dParams,
    }

    let (op_records, curves_by_id, tools_by_id, stock_def, stock_top_z, safe_height, artwork_origin) = {
        let project = read_project(project_lock)?;

        let artwork = project.source_2d_artwork.as_ref().ok_or_else(|| {
            AppError::InvalidInput(
                "no 2D artwork loaded; load a file before generating G-code".to_string(),
            )
        })?;

        // Collect enabled Profile2d operations.
        let op_records: Vec<OpRecord> = project
            .operations
            .iter()
            .filter(|op| op.enabled)
            .filter_map(|op| {
                if let OperationParams::Profile2d(ref p) = op.params {
                    Some(OpRecord {
                        id: op.id,
                        name: op.name.clone(),
                        tool_id: op.tool_id,
                        params: p.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Validate each op's curve reference.
        for rec in &op_records {
            let curve = artwork
                .curves
                .iter()
                .find(|c| c.id == rec.params.curve_id)
                .ok_or_else(|| {
                    AppError::InvalidInput(format!(
                        "operation '{}': curve {} not found in loaded artwork",
                        rec.name, rec.params.curve_id
                    ))
                })?;
            if !curve.is_closed {
                return Err(AppError::InvalidInput(format!(
                    "operation '{}': curve {} is not closed; \
                     only closed curves are supported for 2D profile operations",
                    rec.name, rec.params.curve_id
                )));
            }
        }

        // Single-tool constraint.
        let distinct_tools: HashSet<Uuid> = op_records.iter().map(|r| r.tool_id).collect();
        if distinct_tools.len() > 1 {
            return Err(AppError::InvalidInput(
                "multiple tools used across operations; \
                 only one tool is allowed until tool-change support is added"
                    .to_string(),
            ));
        }

        // Clone curve points keyed by id.
        let curves_by_id: HashMap<Uuid, Vec<[f64; 2]>> = artwork
            .curves
            .iter()
            .map(|c| (c.id, c.points.clone()))
            .collect();

        // Clone tools keyed by id.
        let tools_by_id: HashMap<Uuid, Tool> =
            project.tools.iter().map(|t| (t.id, t.clone())).collect();

        let stock_def = project.stock.clone();
        let stock_top_z: Option<f64> = stock_def.as_ref().map(|s| {
            let crate::models::StockDefinition::Box(b) = s;
            b.origin.z + b.height
        });

        let safe_height = project.safe_height;
        let artwork_origin = project.artwork_origin;

        (
            op_records,
            curves_by_id,
            tools_by_id,
            stock_def,
            stock_top_z,
            safe_height,
            artwork_origin,
        )
    }; // read lock released here

    // ── Validation warnings ──────────────────────────────────────────────────

    let mut warnings: Vec<String> = Vec::new();
    if let Some(top_z) = stock_top_z {
        for rec in &op_records {
            if rec.params.top_of_cut <= top_z {
                warnings.push(format!(
                    "operation '{}': top_of_cut ({:.3}) is at or below \
                     stock top ({:.3}); verify Z origin setup",
                    rec.name, rec.params.top_of_cut, top_z
                ));
            }
        }
    }

    // ── Safe height resolution ───────────────────────────────────────────────

    let safe_height_value = safe_height
        .or_else(|| stock_top_z.map(|z| z + 5.0))
        .unwrap_or(5.0);

    // ── Plan each operation ──────────────────────────────────────────────────

    let mut toolpaths: Vec<Toolpath> = Vec::with_capacity(op_records.len());
    let mut tool_infos: Vec<ToolInfo> = Vec::with_capacity(op_records.len());

    for rec in &op_records {
        let curve_points = curves_by_id.get(&rec.params.curve_id).ok_or_else(|| {
            AppError::InvalidInput(format!("curve {} not found", rec.params.curve_id))
        })?;

        let tool = tools_by_id.get(&rec.tool_id).ok_or_else(|| {
            AppError::NotFound(format!(
                "tool {} not found for operation '{}'",
                rec.tool_id, rec.name
            ))
        })?;

        let tool_radius = tool.diameter / 2.0;
        let passes = plan_2d_profile(
            &rec.params,
            tool_radius,
            artwork_origin,
            safe_height_value,
            curve_points,
        )?;

        toolpaths.push(Toolpath {
            operation_id: rec.id,
            tool_number: 1,
            spindle_speed: 0.0,
            feed_rate: rec.params.feed_rate,
            passes,
        });

        tool_infos.push(ToolInfo {
            number: 1,
            diameter: tool.diameter,
            description: tool.name.clone(),
        });
    }

    // ── Post-processor ───────────────────────────────────────────────────────

    let pp = PostProcessor::builtin(post_processor_id)
        .map_err(|e| AppError::PostProcessor(e.to_string()))?;

    let gcode = pp
        .generate(
            &toolpaths,
            &tool_infos,
            GenerateOptions {
                program_number: None,
                include_comments: true,
            },
        )
        .map_err(|e| AppError::PostProcessor(e.to_string()))?;

    // ── Prepend @STOCK / @TOOL metadata for G-code viewer round-trip ────────

    let mut metadata_header = String::new();
    if let Some(ref sd) = stock_def {
        let crate::models::StockDefinition::Box(b) = sd;
        metadata_header.push_str(&format!(
            "; @STOCK type=box width={} depth={} height={} origin={},{},{}\n",
            b.width, b.depth, b.height, b.origin.x, b.origin.y, b.origin.z
        ));
    }
    for ti in &tool_infos {
        let tool = tools_by_id.values().find(|t| t.name == ti.description);
        let tool_type_str = tool
            .and_then(|t| serde_json::to_string(&t.tool_type).ok())
            .unwrap_or_else(|| "\"flat_endmill\"".to_string());
        // serde produces a quoted string like "flat_endmill"; strip the quotes.
        let tool_type_str = tool_type_str.trim_matches('"');
        metadata_header.push_str(&format!(
            "; @TOOL number={} type={} diameter={}\n",
            ti.number, tool_type_str, ti.diameter
        ));
    }
    let gcode = if metadata_header.is_empty() {
        gcode
    } else {
        format!("{metadata_header}{gcode}")
    };

    // ── Line geometry ────────────────────────────────────────────────────────

    const PALETTE: [(f32, f32, f32); 6] = [
        (1.0, 0.0, 0.0),
        (0.0, 0.8, 0.0),
        (0.0, 0.0, 1.0),
        (0.0, 0.8, 0.8),
        (0.8, 0.0, 0.8),
        (0.8, 0.8, 0.0),
    ];
    let linking_colour: (f32, f32, f32) = (0.5, 0.5, 0.5);

    let segment_count: usize = toolpaths
        .iter()
        .flat_map(|tp| tp.passes.iter())
        .map(|p| p.cuts.len().saturating_sub(1))
        .sum();
    let mut positions: Vec<f32> = Vec::with_capacity(segment_count * 6);
    let mut colours: Vec<f32> = Vec::with_capacity(segment_count * 6);
    let mut types: Vec<u8> = Vec::with_capacity(segment_count);

    for (op_index, toolpath) in toolpaths.iter().enumerate() {
        let op_colour = PALETTE[op_index % 6];
        let lead_colour = (op_colour.0 * 0.6, op_colour.1 * 0.6, op_colour.2 * 0.6);

        for pass in &toolpath.passes {
            let (colour, type_byte): ((f32, f32, f32), u8) = match pass.kind {
                PassKind::Linking => (linking_colour, 0),
                PassKind::Cutting | PassKind::SpringPass => (op_colour, 1),
                PassKind::LeadIn => (lead_colour, 2),
                PassKind::LeadOut => (lead_colour, 3),
            };

            for pair in pass.cuts.windows(2) {
                let a = &pair[0].position;
                let b = &pair[1].position;
                positions.extend_from_slice(&[
                    a.x as f32, a.y as f32, a.z as f32, b.x as f32, b.y as f32, b.z as f32,
                ]);
                colours.extend_from_slice(&[
                    colour.0, colour.1, colour.2, colour.0, colour.1, colour.2,
                ]);
                types.push(type_byte);
            }
        }
    }

    let line_geometry = LineGeometryData {
        positions,
        colours,
        types,
    };

    // ── Store toolpaths in project for dexel simulation ──────────────────────
    {
        let mut project = write_project(project_lock)?;
        project.toolpaths.clear();
        for tp in &toolpaths {
            project.toolpaths.insert(tp.operation_id, tp.clone());
        }
    }

    // ── Stats ────────────────────────────────────────────────────────────────

    let total_pass_count: usize = toolpaths.iter().map(|tp| tp.passes.len()).sum();
    let total_point_count: usize = toolpaths
        .iter()
        .flat_map(|tp| tp.passes.iter())
        .map(|p| p.cuts.len())
        .sum();
    let total_path_length_mm: f64 = toolpaths
        .iter()
        .flat_map(|tp| tp.passes.iter())
        .flat_map(|p| p.cuts.windows(2))
        .map(|pair| {
            let a = &pair[0].position;
            let b = &pair[1].position;
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let dz = b.z - a.z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .sum();

    let stats = ToolpathStats {
        total_pass_count,
        total_point_count,
        total_path_length_mm,
    };

    Ok(Generate2dResult {
        gcode,
        line_geometry,
        warnings,
        stats,
    })
}

/// Generate G-code for all enabled 2D Profile operations in the current project.
#[tauri::command]
pub async fn generate_2d_gcode(
    post_processor_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Generate2dResult, AppError> {
    generate_2d_gcode_inner(&post_processor_id, &state.project)
}

/// Save G-code text to a file.
#[tauri::command]
pub async fn save_2d_gcode(path: String, gcode: String) -> Result<(), AppError> {
    std::fs::write(&path, &gcode)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::serialization;
    use crate::state::Project;

    fn svg_path() -> String {
        format!(
            "{}/../tests/integration/twod/rect.svg",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn dxf_path() -> String {
        format!(
            "{}/../tests/integration/twod/rect.dxf",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn fresh_project_lock() -> RwLock<Project> {
        RwLock::new(Project::default())
    }

    // ── load_2d_file_inner (SVG) ──────────────────────────────────────────

    #[test]
    fn load_svg_returns_correct_curve_count() {
        let lock = fresh_project_lock();
        let result = load_2d_file_inner(&svg_path(), Some(UnitSystem::Mm), &lock).unwrap();
        assert_eq!(result.curves.len(), 2, "expected 2 curves from rect.svg");
    }

    #[test]
    fn load_svg_closed_and_open_curves() {
        let lock = fresh_project_lock();
        let result = load_2d_file_inner(&svg_path(), Some(UnitSystem::Mm), &lock).unwrap();
        assert!(result.curves.iter().any(|c| c.is_closed), "no closed curve");
        assert!(result.curves.iter().any(|c| !c.is_closed), "no open curve");
    }

    #[test]
    fn load_svg_requires_unit_system_hint() {
        let lock = fresh_project_lock();
        let result = load_2d_file_inner(&svg_path(), None, &lock);
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "expected InvalidInput when hint is absent"
        );
    }

    #[test]
    fn load_svg_stores_artwork_in_project() {
        let lock = fresh_project_lock();
        load_2d_file_inner(&svg_path(), Some(UnitSystem::Mm), &lock).unwrap();
        let project = lock.read().unwrap();
        assert!(
            project.source_2d_artwork.is_some(),
            "artwork should be stored in project"
        );
    }

    #[test]
    fn load_svg_curve_points_map_populated() {
        let lock = fresh_project_lock();
        let result = load_2d_file_inner(&svg_path(), Some(UnitSystem::Mm), &lock).unwrap();
        for summary in &result.curves {
            let key = summary.id.to_string();
            assert!(
                result.curve_points.contains_key(&key),
                "curve_points missing entry for {key}"
            );
        }
    }

    // ── load_2d_file_inner (DXF) ──────────────────────────────────────────

    #[test]
    fn load_dxf_returns_correct_result() {
        let lock = fresh_project_lock();
        let result = load_2d_file_inner(&dxf_path(), None, &lock).unwrap();
        assert_eq!(result.curves.len(), 2, "expected 2 curves from rect.dxf");
        assert!(result.curves.iter().any(|c| c.is_closed), "no closed curve");
        assert!(result.curves.iter().any(|c| !c.is_closed), "no open curve");
    }

    #[test]
    fn load_dxf_stores_artwork_in_project() {
        let lock = fresh_project_lock();
        load_2d_file_inner(&dxf_path(), None, &lock).unwrap();
        let project = lock.read().unwrap();
        assert!(project.source_2d_artwork.is_some());
    }

    // ── get_2d_curves_inner ───────────────────────────────────────────────

    #[test]
    fn get_2d_curves_returns_none_on_fresh_project() {
        let lock = fresh_project_lock();
        let result = get_2d_curves_inner(&lock).unwrap();
        assert!(result.is_none(), "expected None on fresh project");
    }

    #[test]
    fn get_2d_curves_returns_some_after_load() {
        let lock = fresh_project_lock();
        load_2d_file_inner(&svg_path(), Some(UnitSystem::Mm), &lock).unwrap();
        let result = get_2d_curves_inner(&lock).unwrap();
        assert!(result.is_some(), "expected Some after loading artwork");
        let r = result.unwrap();
        assert_eq!(r.curves.len(), 2);
    }

    // ── Project roundtrip (save → load preserves artwork) ────────────────

    #[test]
    fn project_roundtrip_preserves_source_2d_artwork() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let jcam_path = dir.path().join("test.jcam");

        // Build a project with artwork loaded from the SVG fixture.
        let mut project = Project::default();
        project.name = "roundtrip-test".to_string();
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-01T00:00:00Z".to_string();

        let svg_bytes = std::fs::read(svg_path()).unwrap();
        let curves = parse_svg(&svg_bytes, UnitSystem::Mm).unwrap();
        let curve_ids: Vec<_> = curves.iter().map(|c| c.id).collect();
        let closed_flags: Vec<_> = curves.iter().map(|c| c.is_closed).collect();

        project.source_2d_artwork = Some(LoadedArtwork {
            file_path: svg_path(),
            unit_system: UnitSystem::Mm,
            curves,
            import_warnings: Vec::new(),
        });

        // Save then reload.
        serialization::save(&project, &jcam_path).expect("save should succeed");
        let loaded = serialization::load(&jcam_path).expect("load should succeed");

        let artwork = loaded
            .source_2d_artwork
            .expect("source_2d_artwork should be present after roundtrip");

        assert_eq!(artwork.curves.len(), curve_ids.len());
        for (i, id) in curve_ids.iter().enumerate() {
            assert_eq!(artwork.curves[i].id, *id, "curve id mismatch at index {i}");
            assert_eq!(
                artwork.curves[i].is_closed, closed_flags[i],
                "is_closed mismatch at index {i}"
            );
        }
    }

    // ── set_safe_height_inner / get_safe_height_inner ─────────────────────

    #[test]
    fn set_safe_height_some_then_get_returns_some() {
        let lock = fresh_project_lock();
        set_safe_height_inner(Some(5.0), &lock).expect("set should succeed");
        let result = get_safe_height_inner(&lock).expect("get should succeed");
        assert_eq!(result, Some(5.0));
    }

    #[test]
    fn set_safe_height_none_then_get_returns_none() {
        let lock = fresh_project_lock();
        set_safe_height_inner(Some(10.0), &lock).expect("set Some");
        set_safe_height_inner(None, &lock).expect("set None");
        let result = get_safe_height_inner(&lock).expect("get should succeed");
        assert!(result.is_none());
    }

    // ── set_artwork_origin_inner / get_artwork_origin_inner ───────────────

    #[test]
    fn set_artwork_origin_then_get_returns_same() {
        let lock = fresh_project_lock();
        set_artwork_origin_inner(10.0, -5.0, &lock).expect("set should succeed");
        let result = get_artwork_origin_inner(&lock).expect("get should succeed");
        assert_eq!(result, [10.0, -5.0]);
    }

    #[test]
    fn default_artwork_origin_is_zero_zero() {
        let lock = fresh_project_lock();
        let result = get_artwork_origin_inner(&lock).expect("get should succeed");
        assert_eq!(result, [0.0, 0.0]);
    }

    // ── ProjectSnapshot includes safe_height and artwork_origin ───────────

    #[test]
    fn snapshot_includes_safe_height_and_artwork_origin_after_setting() {
        use crate::state::AppState;

        let state = AppState::default();
        set_safe_height_inner(Some(8.0), &state.project).expect("set safe height");
        set_artwork_origin_inner(3.0, 4.0, &state.project).expect("set artwork origin");

        let snapshot =
            super::super::project::get_project_snapshot_inner(&state.project, false, false)
                .expect("snapshot should succeed");

        assert_eq!(snapshot.safe_height, Some(8.0));
        assert_eq!(snapshot.artwork_origin, [3.0, 4.0]);
    }

    // ── Serialization roundtrip: safe_height and artwork_origin ──────────

    #[test]
    fn roundtrip_preserves_safe_height_and_artwork_origin() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let jcam_path = dir.path().join("test.jcam");

        let mut project = Project::default();
        project.name = "roundtrip-settings".to_string();
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-01T00:00:00Z".to_string();
        project.safe_height = Some(8.0);
        project.artwork_origin = [3.0, 4.0];

        serialization::save(&project, &jcam_path).expect("save should succeed");
        let loaded = serialization::load(&jcam_path).expect("load should succeed");

        assert_eq!(loaded.safe_height, Some(8.0));
        assert_eq!(loaded.artwork_origin, [3.0, 4.0]);
    }

    #[test]
    fn load_old_project_defaults_safe_height_none_and_origin_zero() {
        use crate::project::types::ProjectFile;

        // Simulate loading a project.json that has no safe_height or artwork_origin.
        let json = r#"{
            "schema_version": 1,
            "app_version": "0.0.1",
            "created_at": "2024-01-01T00:00:00Z",
            "modified_at": "2024-01-01T00:00:00Z",
            "project": {
                "name": "old",
                "description": "",
                "units": "mm"
            }
        }"#;
        let pf: ProjectFile = serde_json::from_str(json).expect("should deserialize");
        assert!(pf.safe_height.is_none());
        assert_eq!(pf.artwork_origin, [0.0, 0.0]);
    }
}
