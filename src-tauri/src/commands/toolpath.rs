//! Toolpath and post-processor IPC command handlers.
//!
//! All handlers follow the pattern of an `_inner` function (testable without
//! Tauri) wrapped by the `#[tauri::command]` entry point that extracts the
//! managed state.

use std::sync::RwLock;

use tauri::Emitter;

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::tool::ToolType;
use crate::models::StockDefinition;
use crate::postprocessor::{program::GenerateOptions, PostProcessor, PostProcessorMeta};
use crate::state::{AppState, Project};
use crate::toolpath::gouge::GougeCheckResult;
use crate::toolpath::types::{LinkingParams, PassKind, Toolpath, DEFAULT_CLEARANCE_OFFSET};
use crate::toolpath::{arc_fitting, linking, LineGeometryData, ToolpathStats};

use super::{build_tool_infos, parse_entity_id, read_project, write_project};

// ── ToolpathProgressEvent ─────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolpathProgressEvent {
    operation_id: String,
    percent: u32,
    message: String,
}

// ── list_post_processors ──────────────────────────────────────────────────────

/// Testable inner logic for [`list_post_processors`].
///
/// Returns the metadata for all builtin post-processors.
pub(crate) fn list_post_processors_inner() -> Result<Vec<PostProcessorMeta>, AppError> {
    Ok(PostProcessor::list_builtins())
}

// ── get_gcode_preview ─────────────────────────────────────────────────────────

/// Testable inner logic for [`get_gcode_preview`].
///
/// 1. Parses `operation_id` as a UUID.
/// 2. Looks up the toolpath for that operation in `project.toolpaths`.
/// 3. Builds [`ToolInfo`] from the matching operation and tool in the project.
/// 4. Loads the named builtin post-processor.
/// 5. Generates and returns the G-code string.
pub(crate) fn get_gcode_preview_inner(
    operation_id: &str,
    post_processor_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<String, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    // Extract only the data we need, then release the lock before the
    // CPU-intensive TOML parse and G-code generation below.
    let (toolpath, tool_infos) = {
        let project = read_project(project_lock)?;

        let toolpath = project
            .toolpaths
            .get(&op_uuid)
            .ok_or_else(|| AppError::NotFound(format!("no toolpath for operation {op_uuid}")))?
            .clone();

        let tool_infos = build_tool_infos(std::slice::from_ref(&toolpath), &project);

        (toolpath, tool_infos)
    }; // read lock released here

    let pp = PostProcessor::builtin(post_processor_id)
        .map_err(|e| AppError::PostProcessor(e.to_string()))?;

    pp.generate(
        &[toolpath],
        &tool_infos,
        GenerateOptions {
            program_number: None,
            include_comments: true,
        },
    )
    .map_err(|e| AppError::PostProcessor(e.to_string()))
}

// ── calculate_toolpath ────────────────────────────────────────────────────────

/// Testable inner logic for [`calculate_toolpath`].
///
/// 1. Parses `operation_id` as a UUID.
/// 2. Reads the operation, stock, and tool from the project (all must exist).
/// 3. Calls [`crate::toolpath::planner::plan`] to generate unlinked passes.
/// 4. Calls [`linking::link_passes`] for Pocket/Profile/ZLevelRoughing operations.
/// 5. Assembles the final [`Toolpath`].
/// 6. Computes a deterministic SHA-256 cache key from the operation, tool, stock,
///    optional model checksum, and engine version.
/// 7. Stores the toolpath in `project.toolpaths` and populates `operation.cache`
///    with the key, validity flag, UTC timestamp, and summary stats.
/// 8. Returns statistics about the generated toolpath.
///
/// Progress events are emitted via `emit` at five milestones (0%, 50%, 80%, 95%, 100%).
pub fn calculate_toolpath_inner(
    operation_id: &str,
    project_lock: &RwLock<Project>,
    emit: Option<&dyn Fn(ToolpathProgressEvent)>,
) -> Result<ToolpathStats, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    let progress = |pct: u32, msg: &str| {
        if let Some(f) = emit {
            f(ToolpathProgressEvent {
                operation_id: operation_id.to_string(),
                percent: pct,
                message: msg.into(),
            });
        }
    };

    let (raw_passes, stats, model_sha, operation, tool, stock) = {
        let project = read_project(project_lock)?;

        let operation = project
            .operations
            .iter()
            .find(|op| op.id == op_uuid)
            .ok_or_else(|| AppError::NotFound(format!("operation {op_uuid} not found")))?
            .clone();

        let stock = project
            .stock
            .clone()
            .ok_or_else(|| AppError::NotFound("project has no stock defined".to_string()))?;

        let tool = project
            .tools
            .iter()
            .find(|t| t.id == operation.tool_id)
            .ok_or_else(|| AppError::NotFound(format!("tool {} not found", operation.tool_id)))?
            .clone();

        // ── Rest machining: look up and validate roughing data ────────────
        let roughing_data = if let OperationParams::ZLevelFinishing(ref p) = operation.params {
            if p.rest_machining {
                let ref_id_str = p.rest_machining_reference_id.as_deref().ok_or_else(|| {
                    AppError::InvalidInput(
                        "Rest machining is enabled but no reference operation ID was provided"
                            .to_string(),
                    )
                })?;
                let ref_op_uuid = parse_entity_id(ref_id_str, "rest machining reference")?;

                let ref_op = project
                    .operations
                    .iter()
                    .find(|op| op.id == ref_op_uuid)
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "referenced roughing operation {ref_op_uuid} not found"
                        ))
                    })?;

                if !matches!(ref_op.params, OperationParams::ZLevelRoughing(_)) {
                    return Err(AppError::InvalidInput(format!(
                        "referenced operation {ref_op_uuid} is not a ZLevelRoughing operation"
                    )));
                }

                let ref_toolpath =
                    project.toolpaths.get(&ref_op_uuid).ok_or_else(|| {
                        AppError::InvalidInput(
                            "Referenced roughing operation has not been calculated yet. Calculate it first."
                                .to_string(),
                        )
                    })?;

                let cutting_passes: Vec<_> = ref_toolpath
                    .passes
                    .iter()
                    .filter(|pass| pass.kind == PassKind::Cutting)
                    .cloned()
                    .collect();

                let ref_tool = project
                    .tools
                    .iter()
                    .find(|t| t.id == ref_op.tool_id)
                    .ok_or_else(|| {
                        AppError::NotFound(format!(
                            "tool {} for referenced roughing operation not found",
                            ref_op.tool_id
                        ))
                    })?;

                Some(
                    crate::toolpath::operations::zlevel_finishing::RoughingData {
                        passes: cutting_passes,
                        tool_diameter: ref_tool.diameter,
                    },
                )
            } else {
                None
            }
        } else {
            None
        };

        let model_sha = project.source_model.as_ref().map(|m| m.checksum.clone());
        let shape = project.source_model.as_ref().and_then(|m| m.shape.as_ref());

        progress(0, "Starting toolpath calculation");
        let (raw_passes, stats) = crate::toolpath::planner::plan(
            &operation,
            &tool,
            &stock,
            shape,
            roughing_data.as_ref(),
        )?;
        progress(50, "Passes generated");

        (raw_passes, stats, model_sha, operation, tool, stock)
        // read lock releases here at end of block
    };

    // Apply linking for operations that return unlinked cutting passes.
    // Drill operations handle their own linking internally in drill_passes.
    let StockDefinition::Box(b) = &stock;
    let stock_top_z = b.origin.z + b.height;
    let (
        arc_lead_in_radius,
        arc_lead_out_radius,
        helical_entry_radius,
        helical_entry_pitch,
        ramp_entry_angle_deg,
    ) = match &operation.params {
        OperationParams::Profile(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::Pocket(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::ZLevelRoughing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::ZLevelFinishing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::Drill(_) => (None, None, None, None, None),
        OperationParams::AdaptiveClearing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::ParallelFinishing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::ScallopFinishing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::FlowlineFinishing(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
        OperationParams::PencilMilling(p) => (
            p.arc_lead_in_radius,
            p.arc_lead_out_radius,
            p.helical_entry_radius,
            p.helical_entry_pitch,
            p.ramp_entry_angle_deg,
        ),
    };
    let linked_passes = match &operation.params {
        OperationParams::Pocket(_)
        | OperationParams::Profile(_)
        | OperationParams::ZLevelRoughing(_)
        | OperationParams::ZLevelFinishing(_)
        | OperationParams::AdaptiveClearing(_)
        | OperationParams::ParallelFinishing(_)
        | OperationParams::ScallopFinishing(_)
        | OperationParams::FlowlineFinishing(_)
        | OperationParams::PencilMilling(_) => linking::link_passes(
            raw_passes,
            &LinkingParams {
                tool_diameter: tool.diameter,
                clearance_z: stock_top_z + DEFAULT_CLEARANCE_OFFSET,
                lead_ratio: linking::DEFAULT_LEAD_RATIO,
                arc_lead_in_radius,
                arc_lead_out_radius,
                helical_entry_radius,
                helical_entry_pitch,
                ramp_entry_angle_deg,
            },
        ),
        OperationParams::Drill(_) => raw_passes,
    };
    progress(80, "Passes linked");

    // Fit arcs: replace sequences of collinear feed moves that approximate
    // circular arcs with proper MoveKind::Arc moves.
    let linked_passes: Vec<_> = linked_passes
        .into_iter()
        .map(|mut pass| {
            pass.cuts = arc_fitting::fit_arcs(pass.cuts, 0.01);
            pass
        })
        .collect();

    // Assemble Toolpath.
    let spindle_speed = operation
        .spindle_speed_override
        .map(|v| v as f64)
        .or_else(|| tool.default_spindle_speed.map(|v| v as f64))
        .unwrap_or(8000.0);
    let feed_rate = operation
        .feed_rate_override
        .or(tool.default_feed_rate)
        .unwrap_or(500.0);
    let toolpath = Toolpath {
        operation_id: op_uuid,
        tool_number: 1,
        spindle_speed,
        feed_rate,
        passes: linked_passes,
    };

    let key = crate::toolpath::cache::compute_cache_key(
        &operation,
        &tool,
        &stock,
        model_sha.as_deref(),
        env!("CARGO_PKG_VERSION"),
    );

    {
        let mut project = write_project(project_lock)?;
        project.toolpaths.insert(op_uuid, toolpath);
        if let Some(op) = project.operations.iter_mut().find(|o| o.id == op_uuid) {
            op.cache = crate::models::operation::CacheState {
                key: Some(key),
                valid: true,
                computed_at: Some(
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
                stats: Some(crate::models::operation::CachedStats {
                    total_pass_count: stats.total_pass_count as u32,
                    total_point_count: stats.total_point_count as u32,
                    total_path_length_mm: stats.total_path_length_mm,
                }),
                binary_file: None,
            };
        }
    } // write lock released here

    progress(95, "Cache key stored");
    progress(100, "Complete");
    Ok(stats)
}

// ── get_toolpath_geometry ─────────────────────────────────────────────────────

/// Testable inner logic for [`get_toolpath_geometry`].
///
/// Converts the stored [`crate::toolpath::Toolpath`] for the given operation
/// into flat-array line geometry suitable for Three.js rendering.
pub(crate) fn get_toolpath_geometry_inner(
    operation_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<LineGeometryData, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    let (toolpath, op_index) = {
        let project = read_project(project_lock)?;

        let toolpath = project
            .toolpaths
            .get(&op_uuid)
            .ok_or_else(|| AppError::NotFound(format!("no toolpath for operation {op_uuid}")))?
            .clone();

        let op_index = project
            .operations
            .iter()
            .position(|op| op.id == op_uuid)
            .ok_or_else(|| AppError::NotFound(format!("operation {op_uuid} not found")))?;

        (toolpath, op_index)
    }; // read lock released here

    const PALETTE: [(f32, f32, f32); 6] = [
        (1.0, 0.0, 0.0),
        (0.0, 0.8, 0.0),
        (0.0, 0.0, 1.0),
        (0.0, 0.8, 0.8),
        (0.8, 0.0, 0.8),
        (0.8, 0.8, 0.0),
    ];
    let op_colour = PALETTE[op_index % 6];
    let linking_colour: (f32, f32, f32) = (0.5, 0.5, 0.5);
    let lead_colour = (op_colour.0 * 0.6, op_colour.1 * 0.6, op_colour.2 * 0.6);

    let segment_count: usize = toolpath
        .passes
        .iter()
        .map(|p| p.cuts.len().saturating_sub(1))
        .sum();
    let mut positions: Vec<f32> = Vec::with_capacity(segment_count * 6);
    let mut colours: Vec<f32> = Vec::with_capacity(segment_count * 6);
    let mut types: Vec<u8> = Vec::with_capacity(segment_count);

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
            colours
                .extend_from_slice(&[colour.0, colour.1, colour.2, colour.0, colour.1, colour.2]);
            types.push(type_byte);
        }
    }

    Ok(LineGeometryData {
        positions,
        colours,
        types,
    })
}

// ── check_gouge ──────────────────────────────────────────────────────────────

/// Map a [`ToolType`] to the string expected by the gouge detection module.
fn tool_type_str(tool_type: &ToolType) -> &'static str {
    match tool_type {
        ToolType::BallNose => "ball",
        _ => "flat",
    }
}

/// Extract the surface finishing allowance from the operation params.
///
/// Only `ParallelFinishing`, `ScallopFinishing`, `FlowlineFinishing`, and
/// `PencilMilling` are supported; all other operation types return an error.
fn extract_finishing_allowance(params: &OperationParams) -> Result<f64, AppError> {
    match params {
        OperationParams::ParallelFinishing(p) => Ok(p.allowance),
        OperationParams::ScallopFinishing(p) => Ok(p.allowance),
        OperationParams::FlowlineFinishing(p) => Ok(p.allowance),
        OperationParams::PencilMilling(p) => Ok(p.allowance),
        _ => Err(AppError::InvalidInput(
            "gouge checking is only supported for surface finishing operations".to_string(),
        )),
    }
}

/// Testable inner logic for [`check_gouge`].
///
/// Looks up the operation, tool, shape, and cached toolpath, then delegates
/// to [`crate::toolpath::gouge::check_gouges`].
pub(crate) fn check_gouge_inner(
    operation_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<GougeCheckResult, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    let project = read_project(project_lock)?;

    let operation = project
        .operations
        .iter()
        .find(|op| op.id == op_uuid)
        .ok_or_else(|| AppError::NotFound(format!("operation {op_uuid} not found")))?;

    let tool = project
        .tools
        .iter()
        .find(|t| t.id == operation.tool_id)
        .ok_or_else(|| AppError::NotFound(format!("tool {} not found", operation.tool_id)))?;

    let allowance = extract_finishing_allowance(&operation.params)?;

    let shape = project
        .source_model
        .as_ref()
        .and_then(|m| m.shape.as_ref())
        .ok_or_else(|| AppError::NotFound("no model shape loaded".to_string()))?;

    let toolpath = project
        .toolpaths
        .get(&op_uuid)
        .ok_or_else(|| AppError::NotFound(format!("no toolpath for operation {op_uuid}")))?;

    crate::toolpath::gouge::check_gouges(
        &toolpath.passes,
        shape,
        tool_type_str(&tool.tool_type),
        tool.diameter,
        allowance,
    )
}

// ── auto_lift ────────────────────────────────────────────────────────────────

/// Testable inner logic for [`auto_lift`].
///
/// Same lookups as [`check_gouge_inner`], but obtains a mutable reference to
/// the cached passes and calls [`crate::toolpath::gouge::auto_lift_gouges`].
/// Returns the number of corrected points.
pub(crate) fn auto_lift_inner(
    operation_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<usize, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    let mut project = write_project(project_lock)?;

    // Extract scalar values before taking the toolpath out.
    let (allowance, tt, diameter) = {
        let operation = project
            .operations
            .iter()
            .find(|op| op.id == op_uuid)
            .ok_or_else(|| AppError::NotFound(format!("operation {op_uuid} not found")))?;

        let tool = project
            .tools
            .iter()
            .find(|t| t.id == operation.tool_id)
            .ok_or_else(|| AppError::NotFound(format!("tool {} not found", operation.tool_id)))?;

        (
            extract_finishing_allowance(&operation.params)?,
            tool_type_str(&tool.tool_type),
            tool.diameter,
        )
    };

    // Check shape exists before removing the toolpath.
    if project
        .source_model
        .as_ref()
        .and_then(|m| m.shape.as_ref())
        .is_none()
    {
        return Err(AppError::NotFound("no model shape loaded".to_string()));
    }

    // Temporarily remove the toolpath so we can mutate its passes while
    // holding an immutable reference to the shape (a sibling field).
    let mut toolpath = project
        .toolpaths
        .remove(&op_uuid)
        .ok_or_else(|| AppError::NotFound(format!("no toolpath for operation {op_uuid}")))?;

    let shape = project
        .source_model
        .as_ref()
        .and_then(|m| m.shape.as_ref())
        .expect("shape existence checked above");

    let result = crate::toolpath::gouge::auto_lift_gouges(
        &mut toolpath.passes,
        shape,
        tt,
        diameter,
        allowance,
    );

    // Re-insert regardless of success/failure.
    project.toolpaths.insert(op_uuid, toolpath);

    result
}

// ── Tauri command wrappers ────────────────────────────────────────────────────

/// List all builtin post-processors, returning their metadata.
#[tauri::command]
pub async fn list_post_processors(
    _state: tauri::State<'_, AppState>,
) -> Result<Vec<PostProcessorMeta>, AppError> {
    list_post_processors_inner()
}

/// Generate a G-code preview for the given operation using the named builtin
/// post-processor.
#[tauri::command]
pub async fn get_gcode_preview(
    operation_id: String,
    post_processor_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, AppError> {
    get_gcode_preview_inner(&operation_id, &post_processor_id, &state.project)
}

/// Calculate and store the toolpath for the given operation, returning statistics.
#[tauri::command]
pub async fn calculate_toolpath(
    operation_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ToolpathStats, AppError> {
    let emit = |event: ToolpathProgressEvent| {
        let _ = app.emit("toolpath:progress", &event);
    };
    calculate_toolpath_inner(&operation_id, &state.project, Some(&emit))
}

/// Get the flat-array line geometry for the toolpath of the given operation.
#[tauri::command]
pub async fn get_toolpath_geometry(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<LineGeometryData, AppError> {
    get_toolpath_geometry_inner(&operation_id, &state.project)
}

/// Check the cached toolpath for gouge violations against the model surface.
#[tauri::command]
pub async fn check_gouge(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<GougeCheckResult, AppError> {
    check_gouge_inner(&operation_id, &state.project)
}

/// Auto-lift gouging points in the cached toolpath so they no longer violate.
#[tauri::command]
pub async fn auto_lift(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, AppError> {
    auto_lift_inner(&operation_id, &state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::models::{
        operation::{
            CacheState, OperationParams, PocketParams, ZLevelFinishingParams, ZLevelRoughingParams,
        },
        stock::BoxDimensions,
        tool::ToolType,
        Operation, StockDefinition, Tool, Vec3,
    };
    use crate::state::AppState;
    use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};
    use crate::toolpath::Toolpath;

    use super::*;

    #[test]
    fn list_post_processors_inner_returns_four_entries() {
        let result = list_post_processors_inner().expect("should succeed");
        assert_eq!(result.len(), 4);
        let ids: Vec<&str> = result.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"fanuc-0i"));
    }

    #[test]
    fn calculate_toolpath_inner_returns_not_found_with_no_operation() {
        let state = AppState::default();
        let valid_uuid = Uuid::new_v4().to_string();
        let result = calculate_toolpath_inner(&valid_uuid, &state.project, None);
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "expected NotFound, got: {result:?}"
        );
    }

    #[test]
    fn calculate_toolpath_inner_returns_not_found_with_no_stock() {
        let state = AppState::default();

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let operation = Operation {
            id: op_id,
            name: "Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.operations.push(operation);
            // No stock set, no tool needed — should fail at stock lookup.
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "expected NotFound (no stock), got: {result:?}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn calculate_toolpath_inner_stores_toolpath_for_pocket() {
        use crate::models::stock::BoxDimensions;

        let state = AppState::default();

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let tool = Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };

        let operation = Operation {
            id: op_id,
            name: "Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        let stock = crate::models::StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        });

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(tool);
            project.operations.push(operation);
            project.stock = Some(stock);
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        let stats = result.expect("calculate_toolpath_inner should succeed for pocket");
        assert!(stats.total_pass_count > 0, "expected non-zero pass count");

        let project = state.project.read().expect("read lock");
        assert!(
            project.toolpaths.contains_key(&op_id),
            "toolpath should be stored in project"
        );

        let op = project
            .operations
            .iter()
            .find(|o| o.id == op_id)
            .expect("operation must still exist");
        assert!(op.cache.valid, "cache.valid should be true after calculate");
        assert!(
            op.cache
                .key
                .as_deref()
                .map(|k| k.starts_with("sha256:"))
                .unwrap_or(false),
            "cache.key should start with 'sha256:'"
        );
        assert!(
            op.cache.computed_at.is_some(),
            "cache.computed_at should be set"
        );
        let cached_stats = op.cache.stats.as_ref().expect("cache.stats should be set");
        assert_eq!(
            cached_stats.total_pass_count, stats.total_pass_count as u32,
            "cached pass count must match returned stats"
        );
        assert_eq!(
            cached_stats.total_point_count, stats.total_point_count as u32,
            "cached point count must match returned stats"
        );
        assert!(
            op.cache.binary_file.is_none(),
            "binary_file should remain None"
        );
    }

    #[test]
    fn get_toolpath_geometry_inner_returns_not_found_when_no_toolpath() {
        let state = AppState::default();
        let valid_uuid = Uuid::new_v4().to_string();
        let result = get_toolpath_geometry_inner(&valid_uuid, &state.project);
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "expected NotFound, got: {result:?}"
        );
    }

    #[test]
    fn get_gcode_preview_inner_returns_gcode_when_toolpath_exists() {
        let state = AppState::default();

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let tool = Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };

        let operation = Operation {
            id: op_id,
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        let toolpath = Toolpath {
            operation_id: op_id,
            tool_number: 1,
            spindle_speed: 8000.0,
            feed_rate: 500.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 5.0,
                        },
                        move_kind: MoveKind::Rapid,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 10.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                ],
            }],
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(tool);
            project.operations.push(operation);
            project.toolpaths.insert(op_id, toolpath);
        }

        let gcode = get_gcode_preview_inner(&op_id.to_string(), "fanuc-0i", &state.project)
            .expect("expected Ok G-code output");
        assert!(
            gcode.contains("G00") || gcode.contains("G0 "),
            "expected rapid move (G00/G0) in output, got:\n{}",
            gcode
        );
        assert!(
            gcode.contains("G01") || gcode.contains("G1 "),
            "expected feed move (G01/G1) in output, got:\n{}",
            gcode
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn calculate_toolpath_inner_with_geometry_selection_bounds_passes_within_face() {
        use crate::models::operation::PocketParams;
        use crate::models::stock::BoxDimensions;
        use crate::models::StockDefinition;

        let project_lock = std::sync::RwLock::new(crate::state::Project::default());

        // Load the box.step fixture.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/box.step");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(crate::commands::file::open_model_inner(
            fixture,
            &project_lock,
        ))
        .expect("open_model_inner should succeed");

        // Get the faces and take the first fingerprint.
        let faces = crate::commands::geometry::get_model_faces_inner(&project_lock)
            .expect("get_model_faces_inner should succeed");
        assert!(!faces.is_empty(), "box.step must have at least one face");
        let first_fingerprint = faces[0].fingerprint.clone();

        // Set up stock clearly larger than any single face of the test box.
        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: -200.0,
                y: -200.0,
                z: 0.0,
            },
            width: 400.0,
            depth: 400.0,
            height: 50.0,
        });

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let tool = Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };

        let operation = Operation {
            id: op_id,
            name: "Geo-Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 5.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: Some(vec![first_fingerprint]),
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        {
            let mut project = project_lock.write().expect("write lock");
            project.tools.push(tool);
            project.operations.push(operation);
            project.stock = Some(stock);
        }

        let stats = calculate_toolpath_inner(&op_id.to_string(), &project_lock, None)
            .expect("calculate_toolpath_inner with geometry selection should succeed");
        assert!(stats.total_pass_count > 0, "expected non-zero pass count");

        // Verify that the passes' XY extents are strictly smaller than stock extents.
        let project = project_lock.read().expect("read lock");
        let toolpath = project
            .toolpaths
            .get(&op_id)
            .expect("toolpath must be stored");
        let all_cuts: Vec<_> = toolpath.passes.iter().flat_map(|p| p.cuts.iter()).collect();
        assert!(!all_cuts.is_empty(), "toolpath must have cut points");

        let x_min = all_cuts
            .iter()
            .map(|c| c.position.x)
            .fold(f64::INFINITY, f64::min);
        let x_max = all_cuts
            .iter()
            .map(|c| c.position.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_min = all_cuts
            .iter()
            .map(|c| c.position.y)
            .fold(f64::INFINITY, f64::min);
        let y_max = all_cuts
            .iter()
            .map(|c| c.position.y)
            .fold(f64::NEG_INFINITY, f64::max);

        let pass_width = x_max - x_min;
        let pass_depth = y_max - y_min;

        // The face boundary must be smaller than the 400×400 stock.
        assert!(
            pass_width < 400.0,
            "passes X extent ({pass_width}) must be smaller than stock width (400)"
        );
        assert!(
            pass_depth < 400.0,
            "passes Y extent ({pass_depth}) must be smaller than stock depth (400)"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn calculate_toolpath_inner_with_invalid_fingerprint_returns_geometry_import_error() {
        use crate::models::operation::PocketParams;
        use crate::models::stock::BoxDimensions;
        use crate::models::StockDefinition;

        let project_lock = std::sync::RwLock::new(crate::state::Project::default());

        // Load the box.step fixture.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/box.step");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(crate::commands::file::open_model_inner(
            fixture,
            &project_lock,
        ))
        .expect("open_model_inner should succeed");

        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 100.0,
            depth: 100.0,
            height: 20.0,
        });

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let tool = Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };

        // "deadbeef" repeated 8 times = 64 hex chars (not a valid face fingerprint).
        let bogus_fingerprint = "deadbeef".repeat(8);
        assert_eq!(bogus_fingerprint.len(), 64);

        let operation = Operation {
            id: op_id,
            name: "Invalid-Geo-Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 5.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: Some(vec![bogus_fingerprint]),
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        {
            let mut project = project_lock.write().expect("write lock");
            project.tools.push(tool);
            project.operations.push(operation);
            project.stock = Some(stock);
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &project_lock, None);
        assert!(
            matches!(result, Err(AppError::GeometryImport(_))),
            "expected GeometryImport error for invalid fingerprint, got: {result:?}"
        );
    }

    #[test]
    fn calculate_toolpath_inner_emits_progress_events() {
        use crate::models::operation::{DrillParams, DrillPoint};
        use crate::models::stock::BoxDimensions;
        use crate::models::StockDefinition;

        let state = AppState::default();

        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        let tool = Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };

        let operation = Operation {
            id: op_id,
            name: "Drill".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Drill(DrillParams {
                depth: 5.0,
                peck_depth: None,
                points: vec![DrillPoint { x: 10.0, y: 10.0 }],
            }),
            cache: CacheState::default(),
        };

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

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(tool);
            project.operations.push(operation);
            project.stock = Some(stock);
        }

        let events = std::cell::RefCell::new(Vec::<super::ToolpathProgressEvent>::new());
        let emit = |event: super::ToolpathProgressEvent| {
            events.borrow_mut().push(event);
        };

        calculate_toolpath_inner(&op_id.to_string(), &state.project, Some(&emit))
            .expect("calculate_toolpath_inner should succeed for drill");

        let events = events.into_inner();
        assert!(
            events.iter().any(|e| e.percent == 0),
            "expected a percent=0 event"
        );
        assert!(
            events.iter().any(|e| e.percent == 100),
            "expected a percent=100 event"
        );
        for pair in events.windows(2) {
            assert!(
                pair[0].percent <= pair[1].percent,
                "percentages must be non-decreasing: {} > {}",
                pair[0].percent,
                pair[1].percent
            );
        }
    }

    // ── Helper: build a finishing operation with rest machining fields ────

    fn make_finishing_op(
        op_id: Uuid,
        tool_id: Uuid,
        rest_machining: bool,
        rest_machining_reference_id: Option<String>,
    ) -> Operation {
        Operation {
            id: op_id,
            name: "Z-Level Finish".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ZLevelFinishing(ZLevelFinishingParams {
                depth: 10.0,
                stepdown: 0.5,
                finishing_allowance: 0.1,
                spring_pass: false,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
                rest_machining,
                rest_machining_reference_id,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        })
    }

    fn make_tool(tool_id: Uuid) -> Tool {
        Tool {
            id: tool_id,
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        }
    }

    #[test]
    fn rest_machining_enabled_no_reference_id() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(make_tool(tool_id));
            project
                .operations
                .push(make_finishing_op(op_id, tool_id, true, None));
            project.stock = Some(make_stock());
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "expected InvalidInput when rest_machining=true but no reference ID, got: {result:?}"
        );
    }

    #[test]
    fn rest_machining_reference_not_found() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();
        let bogus_ref_id = Uuid::new_v4();

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(make_tool(tool_id));
            project.operations.push(make_finishing_op(
                op_id,
                tool_id,
                true,
                Some(bogus_ref_id.to_string()),
            ));
            project.stock = Some(make_stock());
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "expected NotFound when reference operation doesn't exist, got: {result:?}"
        );
    }

    #[test]
    fn rest_machining_reference_wrong_type() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();
        let ref_op_id = Uuid::new_v4();

        // Reference operation is a Pocket, not ZLevelRoughing.
        let ref_op = Operation {
            id: ref_op_id,
            name: "Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(make_tool(tool_id));
            project.operations.push(ref_op);
            project.operations.push(make_finishing_op(
                op_id,
                tool_id,
                true,
                Some(ref_op_id.to_string()),
            ));
            project.stock = Some(make_stock());
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "expected InvalidInput when reference is not ZLevelRoughing, got: {result:?}"
        );
    }

    #[test]
    fn rest_machining_reference_not_calculated() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();
        let ref_op_id = Uuid::new_v4();

        // Reference operation is ZLevelRoughing but has no toolpath stored.
        let ref_op = Operation {
            id: ref_op_id,
            name: "Z-Level Rough".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ZLevelRoughing(ZLevelRoughingParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover: 0.5,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.tools.push(make_tool(tool_id));
            project.operations.push(ref_op);
            project.operations.push(make_finishing_op(
                op_id,
                tool_id,
                true,
                Some(ref_op_id.to_string()),
            ));
            project.stock = Some(make_stock());
            // No toolpath inserted for ref_op_id.
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project, None);
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "expected InvalidInput when roughing toolpath not calculated, got: {result:?}"
        );
        if let Err(AppError::InvalidInput(msg)) = &result {
            assert!(
                msg.contains("not been calculated yet"),
                "error message should mention 'not been calculated yet', got: {msg}"
            );
        }
    }
}
