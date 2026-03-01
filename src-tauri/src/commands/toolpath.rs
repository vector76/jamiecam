//! Toolpath and post-processor IPC command handlers.
//!
//! All handlers follow the pattern of an `_inner` function (testable without
//! Tauri) wrapped by the `#[tauri::command]` entry point that extracts the
//! managed state.

use std::sync::RwLock;

use crate::error::AppError;
use crate::postprocessor::{program::GenerateOptions, PostProcessor, PostProcessorMeta};
use crate::state::{AppState, Project};
use crate::toolpath::types::PassKind;
use crate::toolpath::{LineGeometryData, ToolpathStats};

use super::{build_tool_infos, parse_entity_id, read_project, write_project};

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
/// 3. Calls [`crate::toolpath::planner::plan`] to generate the toolpath.
/// 4. Stores the result in `project.toolpaths`.
/// 5. Returns statistics about the generated toolpath.
pub(crate) fn calculate_toolpath_inner(
    operation_id: &str,
    project_lock: &RwLock<Project>,
) -> Result<ToolpathStats, AppError> {
    let op_uuid = parse_entity_id(operation_id, "operation")?;

    let (operation, tool, stock) = {
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

        (operation, tool, stock)
    }; // read lock released here

    let (toolpath, stats) = crate::toolpath::planner::plan(&operation, &tool, &stock)?;

    {
        let mut project = write_project(project_lock)?;
        project.toolpaths.insert(op_uuid, toolpath);
    } // write lock released here

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
) -> Result<ToolpathStats, AppError> {
    calculate_toolpath_inner(&operation_id, &state.project)
}

/// Get the flat-array line geometry for the toolpath of the given operation.
#[tauri::command]
pub async fn get_toolpath_geometry(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<LineGeometryData, AppError> {
    get_toolpath_geometry_inner(&operation_id, &state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::models::{
        operation::{OperationParams, PocketParams},
        tool::ToolType,
        Operation, Tool, Vec3,
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
        let result = calculate_toolpath_inner(&valid_uuid, &state.project);
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
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
            }),
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.operations.push(operation);
            // No stock set, no tool needed — should fail at stock lookup.
        }

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project);
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
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
            }),
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

        let result = calculate_toolpath_inner(&op_id.to_string(), &state.project);
        let stats = result.expect("calculate_toolpath_inner should succeed for pocket");
        assert!(stats.total_pass_count > 0, "expected non-zero pass count");

        let project = state.project.read().expect("read lock");
        assert!(
            project.toolpaths.contains_key(&op_id),
            "toolpath should be stored in project"
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
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
            }),
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
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 10.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
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
}
