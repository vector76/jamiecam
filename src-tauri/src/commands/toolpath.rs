//! Toolpath and post-processor IPC command handlers.
//!
//! All handlers follow the pattern of an `_inner` function (testable without
//! Tauri) wrapped by the `#[tauri::command]` entry point that extracts the
//! managed state.

use std::sync::RwLock;

use crate::error::AppError;
use crate::postprocessor::{program::GenerateOptions, PostProcessor, PostProcessorMeta, ToolInfo};
use crate::state::{AppState, Project};

use super::{parse_entity_id, read_project};

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

        let tool_infos: Vec<ToolInfo> = project
            .operations
            .iter()
            .find(|op| op.id == op_uuid)
            .and_then(|op| {
                project
                    .tools
                    .iter()
                    .find(|t| t.id == op.tool_id)
                    .map(|tool| ToolInfo {
                        number: toolpath.tool_number,
                        diameter: tool.diameter,
                        description: tool.name.clone(),
                    })
            })
            .into_iter()
            .collect();

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
    fn get_gcode_preview_inner_returns_not_found_when_no_toolpath() {
        let state = AppState::default();
        let valid_uuid = Uuid::new_v4().to_string();
        let result = get_gcode_preview_inner(&valid_uuid, "fanuc-0i", &state.project);
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
