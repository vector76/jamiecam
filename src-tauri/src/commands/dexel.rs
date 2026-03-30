//! Dexel simulation IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::sync::RwLock;

use uuid::Uuid;

use crate::dexel::{clearance_for_tool, toolpath_to_segments, DexelGrid};
use crate::error::AppError;
use crate::geometry::MeshData;
use crate::state::{AppState, Project};

use super::read_project;

// ── get_simulation_mesh ──────────────────────────────────────────────────────

/// Testable inner logic for [`get_simulation_mesh`].
///
/// Builds a dexel grid from the project stock, applies toolpath segments for
/// the requested operations, and returns the resulting triangle mesh.
pub(crate) fn get_simulation_mesh_inner(
    resolution: f64,
    operation_ids: Option<Vec<Uuid>>,
    up_to_segment: Option<usize>,
    project_lock: &RwLock<Project>,
) -> Result<MeshData, AppError> {
    // (a) Validate resolution range.
    if !(0.01..=5.0).contains(&resolution) {
        return Err(AppError::InvalidInput(
            "resolution must be between 0.01 and 5.0".into(),
        ));
    }

    // (b) Acquire read lock and clone the data we need, then release.
    let (stock, operations, tools, toolpaths) = {
        let project = read_project(project_lock)?;

        let stock = project.stock.clone();
        let operations = project.operations.clone();
        let tools = project.tools.clone();
        let toolpaths = project.toolpaths.clone();

        (stock, operations, tools, toolpaths)
    };

    // (c) Require stock.
    let stock = stock.ok_or_else(|| AppError::InvalidInput("no stock defined".into()))?;

    // (d) Resolve which operations to simulate.
    let ops = match operation_ids {
        Some(ids) => {
            let mut resolved = Vec::with_capacity(ids.len());
            for id in &ids {
                let op = operations
                    .iter()
                    .find(|op| op.id == *id)
                    .ok_or_else(|| AppError::NotFound(format!("operation {id} not found")))?;
                resolved.push(op.clone());
            }
            resolved
        }
        None => operations.into_iter().filter(|op| op.enabled).collect(),
    };

    // (e) Initialize dexel grid from stock.
    let mut grid = DexelGrid::from_stock(&stock, resolution);

    // (f) Apply segments for each operation.
    let mut segments_applied: usize = 0;

    for op in &ops {
        // Find the tool for this operation.
        let tool = tools
            .iter()
            .find(|t| t.id == op.tool_id)
            .ok_or_else(|| AppError::NotFound(format!("tool {} not found", op.tool_id)))?;

        let (tool_radius, z_clearance) = clearance_for_tool(tool);

        // Find the toolpath for this operation.
        let toolpath = toolpaths.get(&op.id).ok_or_else(|| {
            AppError::NotFound(format!("toolpath for operation {} not found", op.id))
        })?;

        let segments = toolpath_to_segments(toolpath);

        // Apply segments, respecting the budget if set.
        let to_apply = if let Some(budget) = up_to_segment {
            let remaining = budget.saturating_sub(segments_applied);
            &segments[..remaining.min(segments.len())]
        } else {
            &segments[..]
        };

        grid.apply_segments(to_apply, tool_radius, &z_clearance);
        segments_applied += to_apply.len();

        // Break if budget exhausted.
        if let Some(budget) = up_to_segment {
            if segments_applied >= budget {
                break;
            }
        }
    }

    // (g) Extract and return mesh.
    Ok(grid.extract_mesh())
}

// ── Tauri command wrapper ────────────────────────────────────────────────────

/// Compute a simulation mesh by applying toolpath segments to the stock via the
/// dexel engine.
#[tauri::command]
pub async fn get_simulation_mesh(
    resolution: f64,
    operation_ids: Option<Vec<Uuid>>,
    up_to_segment: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<MeshData, AppError> {
    get_simulation_mesh_inner(resolution, operation_ids, up_to_segment, &state.project)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{CacheState, CompensationSide, OperationParams, ProfileParams};
    use crate::models::stock::{BoxDimensions, Vec3};
    use crate::models::{Operation, StockDefinition, Tool, ToolType};
    use crate::state::AppState;
    use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind, Toolpath};

    fn make_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 20.0,
            depth: 20.0,
            height: 10.0,
        })
    }

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: "Test Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: 90.0,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        }
    }

    fn make_operation(tool_id: Uuid) -> Operation {
        Operation {
            id: Uuid::new_v4(),
            name: "Test Op".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 5.0,
                stepdown: Some(2.5),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_toolpath(operation_id: Uuid) -> Toolpath {
        Toolpath {
            operation_id,
            tool_number: 1,
            spindle_speed: 10000.0,
            feed_rate: 1000.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: 5.0,
                            y: 5.0,
                            z: 10.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 15.0,
                            y: 5.0,
                            z: 5.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                ],
            }],
        }
    }

    #[test]
    fn error_invalid_resolution_too_low() {
        let state = AppState::default();
        let result = get_simulation_mesh_inner(0.001, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_invalid_resolution_too_high() {
        let state = AppState::default();
        let result = get_simulation_mesh_inner(10.0, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_missing_stock() {
        let state = AppState::default();
        // No stock set — should fail.
        let result = get_simulation_mesh_inner(1.0, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_unknown_operation_id() {
        let state = AppState::default();
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
        }
        let random_id = Uuid::new_v4();
        let result = get_simulation_mesh_inner(1.0, Some(vec![random_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn error_missing_tool() {
        let state = AppState::default();
        let nonexistent_tool_id = Uuid::new_v4();
        let op = make_operation(nonexistent_tool_id);
        let op_id = op.id;
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
            project.operations.push(op);
            project.toolpaths.insert(op_id, make_toolpath(op_id));
            // No tool added — should fail.
        }
        let result = get_simulation_mesh_inner(1.0, Some(vec![op_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn error_missing_toolpath() {
        let state = AppState::default();
        let tool = make_tool();
        let op = make_operation(tool.id);
        let op_id = op.id;
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
            project.tools.push(tool);
            project.operations.push(op);
            // No toolpath inserted — should fail.
        }
        let result = get_simulation_mesh_inner(1.0, Some(vec![op_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }
}
