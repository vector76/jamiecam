//! Project state query commands.
//!
//! [`get_project_snapshot`] returns a lightweight view of the current project
//! for display in the frontend toolbar / title bar and operation list panel.
//! It acquires only a read lock and is safe to call concurrently with other
//! read commands.

use std::sync::RwLock;

use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::{StockDefinition, WorkCoordinateSystem};
use crate::state::{AppState, Project};

// ── Summary types ─────────────────────────────────────────────────────────────

/// A compact summary of a tool for display in dropdowns and the operation panel.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSummary {
    /// Tool UUID.
    pub id: Uuid,
    /// Human-readable tool name.
    pub name: String,
    /// Snake_case tool type string (e.g. `"flat_endmill"`, `"ball_nose"`).
    pub tool_type: String,
}

/// A compact summary of a machining operation for the operation list panel.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationSummary {
    /// Operation UUID.
    pub id: Uuid,
    /// Human-readable operation name.
    pub name: String,
    /// Operation type discriminant (e.g. `"profile"`, `"pocket"`, `"drill"`).
    pub operation_type: String,
    /// Whether the operation is active in the toolpath.
    pub enabled: bool,
    /// Placeholder for Phase 1 cache invalidation; always `true` in Phase 0.
    pub needs_recalculate: bool,
}

// ── ProjectSnapshot ───────────────────────────────────────────────────────────

/// Serializable snapshot of the current project sent to the frontend.
///
/// Contains only the fields the UI needs immediately; large geometry data
/// is not included (it is streamed separately when the viewport renders).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    /// Absolute path to the loaded model file, if any.
    pub model_path: Option<String>,
    /// SHA-256 hex digest of the loaded model file, if any.
    pub model_checksum: Option<String>,
    /// Human-readable project name.
    pub project_name: String,
    /// ISO-8601 last-modified timestamp (empty string when not yet saved).
    pub modified_at: String,
    /// Tool library summaries.
    pub tools: Vec<ToolSummary>,
    /// Stock solid definition, if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock: Option<StockDefinition>,
    /// Work coordinate systems.
    pub wcs: Vec<WorkCoordinateSystem>,
    /// Machining operation summaries, in program order.
    pub operations: Vec<OperationSummary>,
}

impl From<&Project> for ProjectSnapshot {
    fn from(p: &Project) -> Self {
        let tools = p
            .tools
            .iter()
            .map(|t| ToolSummary {
                id: t.id,
                name: t.name.clone(),
                tool_type: serde_json::to_value(&t.tool_type)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default(),
            })
            .collect();

        let operations = p
            .operations
            .iter()
            .map(|op| OperationSummary {
                id: op.id,
                name: op.name.clone(),
                operation_type: match &op.params {
                    OperationParams::Profile(_) => "profile".to_string(),
                    OperationParams::Pocket(_) => "pocket".to_string(),
                    OperationParams::Drill(_) => "drill".to_string(),
                },
                enabled: op.enabled,
                needs_recalculate: true,
            })
            .collect();

        Self {
            model_path: p
                .source_model
                .as_ref()
                .map(|m| m.path.to_string_lossy().into_owned()),
            model_checksum: p.source_model.as_ref().map(|m| m.checksum.clone()),
            project_name: p.name.clone(),
            modified_at: p.modified_at.clone(),
            tools,
            stock: p.stock.clone(),
            wcs: p.wcs.clone(),
            operations,
        }
    }
}

/// Testable inner logic for [`get_project_snapshot`].
///
/// Acquires a READ lock on `project_lock` and returns a [`ProjectSnapshot`].
pub(crate) fn get_project_snapshot_inner(
    project_lock: &RwLock<Project>,
) -> Result<ProjectSnapshot, AppError> {
    let project = project_lock
        .read()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;
    Ok(ProjectSnapshot::from(&*project))
}

/// Return a lightweight snapshot of the current project.
///
/// Acquires a read lock — does not block concurrent read-only commands.
#[tauri::command]
pub async fn get_project_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<ProjectSnapshot, AppError> {
    get_project_snapshot_inner(&state.project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{
        CompensationSide, OperationParams, PocketParams, ProfileParams,
    };
    use crate::models::stock::{BoxDimensions, Vec3};
    use crate::models::wcs::WorkCoordinateSystem;
    use crate::models::{Operation, StockDefinition, Tool, ToolType};
    use crate::state::AppState;

    #[test]
    fn snapshot_of_default_project_has_no_model() {
        let state = AppState::default();
        let snap = get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
        assert!(snap.model_path.is_none());
        assert!(snap.model_checksum.is_none());
        assert_eq!(snap.project_name, "");
        assert_eq!(snap.modified_at, "");
        assert!(snap.tools.is_empty());
        assert!(snap.stock.is_none());
        assert!(snap.wcs.is_empty());
        assert!(snap.operations.is_empty());
    }

    #[test]
    fn snapshot_reflects_project_name_and_modified_at() {
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.name = "My Project".to_string();
            p.modified_at = "2026-01-01T00:00:00Z".to_string();
        }
        let snap = get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
        assert_eq!(snap.project_name, "My Project");
        assert_eq!(snap.modified_at, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn snapshot_with_model_populates_path_and_checksum() {
        use crate::geometry::MeshData;
        use crate::state::LoadedModel;
        use std::path::PathBuf;

        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.source_model = Some(LoadedModel {
                path: PathBuf::from("/home/user/part.step"),
                checksum: "deadbeef".to_string(),
                mesh_data: MeshData {
                    vertices: vec![],
                    normals: vec![],
                    indices: vec![],
                },
            });
        }
        let snap = get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
        assert_eq!(snap.model_path.as_deref(), Some("/home/user/part.step"));
        assert_eq!(snap.model_checksum.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn project_snapshot_serializes_camel_case() {
        let snap = ProjectSnapshot {
            model_path: Some("/path/to/model.step".to_string()),
            model_checksum: Some("abc123".to_string()),
            project_name: "Test".to_string(),
            modified_at: "2026-01-01T00:00:00Z".to_string(),
            tools: vec![],
            stock: None,
            wcs: vec![],
            operations: vec![],
        };
        let value = serde_json::to_value(&snap).expect("serialize");
        assert!(
            value.get("modelPath").is_some(),
            "expected camelCase modelPath"
        );
        assert!(
            value.get("modelChecksum").is_some(),
            "expected camelCase modelChecksum"
        );
        assert!(
            value.get("projectName").is_some(),
            "expected camelCase projectName"
        );
        assert!(
            value.get("modifiedAt").is_some(),
            "expected camelCase modifiedAt"
        );
        assert!(value.get("tools").is_some(), "expected tools field");
        assert!(value.get("wcs").is_some(), "expected wcs field");
        assert!(
            value.get("operations").is_some(),
            "expected operations field"
        );
    }

    #[test]
    fn snapshot_includes_tool_summaries() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        {
            let mut p = state.project.write().expect("write lock");
            p.tools.push(Tool {
                id: tool_id,
                name: "10mm Flat Endmill".to_string(),
                tool_type: ToolType::FlatEndmill,
                material: "carbide".to_string(),
                diameter: 10.0,
                flute_count: 4,
                default_spindle_speed: None,
                default_feed_rate: None,
            });
        }

        let snap = get_project_snapshot_inner(&state.project).expect("snapshot");
        assert_eq!(snap.tools.len(), 1);
        assert_eq!(snap.tools[0].id, tool_id);
        assert_eq!(snap.tools[0].name, "10mm Flat Endmill");
        assert_eq!(snap.tools[0].tool_type, "flat_endmill");
    }

    #[test]
    fn tool_summary_serializes_camel_case_tool_type() {
        let summary = ToolSummary {
            id: Uuid::new_v4(),
            name: "Ball Nose".to_string(),
            tool_type: "ball_nose".to_string(),
        };
        let value = serde_json::to_value(&summary).expect("serialize");
        assert!(
            value.get("toolType").is_some(),
            "toolType must be camelCase"
        );
        assert!(value.get("tool_type").is_none());
    }

    #[test]
    fn snapshot_includes_stock() {
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.stock = Some(StockDefinition::Box(BoxDimensions {
                origin: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                width: 100.0,
                depth: 80.0,
                height: 20.0,
            }));
        }

        let snap = get_project_snapshot_inner(&state.project).expect("snapshot");
        assert!(snap.stock.is_some());
        let StockDefinition::Box(b) = snap.stock.unwrap();
        assert_eq!(b.width, 100.0);
    }

    #[test]
    fn snapshot_includes_wcs() {
        let state = AppState::default();
        let wcs_id = Uuid::new_v4();
        {
            let mut p = state.project.write().expect("write lock");
            p.wcs.push(WorkCoordinateSystem {
                id: wcs_id,
                name: "G54".to_string(),
                origin: crate::models::Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                x_axis: crate::models::Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                z_axis: crate::models::Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
            });
        }

        let snap = get_project_snapshot_inner(&state.project).expect("snapshot");
        assert_eq!(snap.wcs.len(), 1);
        assert_eq!(snap.wcs[0].id, wcs_id);
        assert_eq!(snap.wcs[0].name, "G54");
    }

    #[test]
    fn snapshot_includes_operation_summaries() {
        let state = AppState::default();
        let tool_id = Uuid::new_v4();
        let op_id = Uuid::new_v4();
        {
            let mut p = state.project.write().expect("write lock");
            p.operations.push(Operation {
                id: op_id,
                name: "Rough Pocket".to_string(),
                enabled: true,
                tool_id,
                params: OperationParams::Pocket(PocketParams {
                    depth: 15.0,
                    stepdown: 3.0,
                    stepover_percent: 45.0,
                }),
            });
            p.operations.push(Operation {
                id: Uuid::new_v4(),
                name: "Disabled Profile".to_string(),
                enabled: false,
                tool_id,
                params: OperationParams::Profile(ProfileParams {
                    depth: 10.0,
                    stepdown: 2.5,
                    compensation_side: CompensationSide::Left,
                }),
            });
        }

        let snap = get_project_snapshot_inner(&state.project).expect("snapshot");
        assert_eq!(snap.operations.len(), 2);

        assert_eq!(snap.operations[0].id, op_id);
        assert_eq!(snap.operations[0].name, "Rough Pocket");
        assert_eq!(snap.operations[0].operation_type, "pocket");
        assert!(snap.operations[0].enabled);
        assert!(snap.operations[0].needs_recalculate);

        assert_eq!(snap.operations[1].operation_type, "profile");
        assert!(!snap.operations[1].enabled);
        assert!(snap.operations[1].needs_recalculate);
    }

    #[test]
    fn operation_summary_serializes_camel_case() {
        let summary = OperationSummary {
            id: Uuid::new_v4(),
            name: "Test Op".to_string(),
            operation_type: "drill".to_string(),
            enabled: true,
            needs_recalculate: true,
        };
        let value = serde_json::to_value(&summary).expect("serialize");
        assert!(
            value.get("operationType").is_some(),
            "operationType must be camelCase"
        );
        assert!(value.get("operation_type").is_none());
        assert!(
            value.get("needsRecalculate").is_some(),
            "needsRecalculate must be camelCase"
        );
    }
}
