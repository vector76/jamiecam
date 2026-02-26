//! Operation CRUD IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.
//!
//! Operations validate that the referenced `tool_id` exists in `project.tools`
//! before accepting an add or edit. Both the tool list and operation list live
//! behind the same `RwLock<Project>`, so validation and mutation happen in one
//! write-lock scope with no ordering issues.

use std::sync::RwLock;

use uuid::Uuid;

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::Operation;
use crate::state::{AppState, Project};

// ── Input type ────────────────────────────────────────────────────────────────

/// Fields required to create or replace an operation (ID is excluded; it is
/// either generated on add or provided separately on edit).
///
/// The `type` discriminant and `params` object are flattened from
/// [`OperationParams`] so the JSON shape matches the on-disk operation format.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationInput {
    /// Human-readable operation name.
    pub name: String,
    /// Whether the operation is active; defaults to `true` when absent.
    pub enabled: Option<bool>,
    /// UUID string of the tool assigned to this operation.
    pub tool_id: String,
    /// Type-discriminated parameters (`"type"` + `"params"` at the same level).
    #[serde(flatten)]
    pub params: OperationParams,
}

// ── add_operation ─────────────────────────────────────────────────────────────

/// Testable inner logic for [`add_operation`].
///
/// Validates that `input.tool_id` exists in `project.tools`, generates a new
/// UUID for the operation, appends it to `project.operations`, and returns the
/// created [`Operation`]. Returns [`AppError::NotFound`] if the tool is absent.
pub(crate) fn add_operation_inner(
    input: OperationInput,
    project_lock: &RwLock<Project>,
) -> Result<Operation, AppError> {
    let tool_uuid = Uuid::parse_str(&input.tool_id).map_err(|_| {
        AppError::NotFound(format!("tool id '{}' is not a valid UUID", input.tool_id))
    })?;

    let mut project = project_lock
        .write()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;

    if !project.tools.iter().any(|t| t.id == tool_uuid) {
        return Err(AppError::NotFound(format!(
            "tool {} not found",
            input.tool_id
        )));
    }

    let op = Operation {
        id: Uuid::new_v4(),
        name: input.name,
        enabled: input.enabled.unwrap_or(true),
        tool_id: tool_uuid,
        params: input.params,
    };
    project.operations.push(op.clone());
    Ok(op)
}

// ── edit_operation ────────────────────────────────────────────────────────────

/// Testable inner logic for [`edit_operation`].
///
/// Finds the operation with the given `id`, validates the new `tool_id`,
/// replaces all fields, and returns the updated [`Operation`].
/// Returns [`AppError::NotFound`] if the operation or tool is missing.
pub(crate) fn edit_operation_inner(
    id: &str,
    input: OperationInput,
    project_lock: &RwLock<Project>,
) -> Result<Operation, AppError> {
    let op_uuid = Uuid::parse_str(id)
        .map_err(|_| AppError::NotFound(format!("operation id '{id}' is not a valid UUID")))?;

    let tool_uuid = Uuid::parse_str(&input.tool_id).map_err(|_| {
        AppError::NotFound(format!("tool id '{}' is not a valid UUID", input.tool_id))
    })?;

    let mut project = project_lock
        .write()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;

    if !project.tools.iter().any(|t| t.id == tool_uuid) {
        return Err(AppError::NotFound(format!(
            "tool {} not found",
            input.tool_id
        )));
    }

    let entry = project
        .operations
        .iter_mut()
        .find(|op| op.id == op_uuid)
        .ok_or_else(|| AppError::NotFound(format!("operation {id} not found")))?;

    entry.name = input.name;
    entry.enabled = input.enabled.unwrap_or(true);
    entry.tool_id = tool_uuid;
    entry.params = input.params;

    Ok(entry.clone())
}

// ── delete_operation ──────────────────────────────────────────────────────────

/// Testable inner logic for [`delete_operation`].
///
/// Removes the operation with the given `id`. Returns [`AppError::NotFound`]
/// if no operation with that ID exists.
pub(crate) fn delete_operation_inner(
    id: &str,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let uuid = Uuid::parse_str(id)
        .map_err(|_| AppError::NotFound(format!("operation id '{id}' is not a valid UUID")))?;

    let mut project = project_lock
        .write()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;

    let before = project.operations.len();
    project.operations.retain(|op| op.id != uuid);
    if project.operations.len() == before {
        return Err(AppError::NotFound(format!("operation {id} not found")));
    }

    Ok(())
}

// ── reorder_operations ────────────────────────────────────────────────────────

/// Testable inner logic for [`reorder_operations`].
///
/// Accepts a complete ordered list of operation UUIDs. Validates that the
/// submitted list contains exactly the same IDs as `project.operations` (no
/// additions, deletions, or unknown IDs). On success, `project.operations` is
/// replaced with the new ordering.
///
/// Returns [`AppError::Io`] if the ID count does not match, or
/// [`AppError::NotFound`] if any submitted ID is not present in the project.
pub(crate) fn reorder_operations_inner(
    ids: Vec<String>,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let uuids: Vec<Uuid> = ids
        .iter()
        .map(|s| {
            Uuid::parse_str(s)
                .map_err(|_| AppError::NotFound(format!("operation id '{s}' is not a valid UUID")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Detect duplicates before acquiring the lock.
    {
        let unique: std::collections::HashSet<&Uuid> = uuids.iter().collect();
        if unique.len() != uuids.len() {
            return Err(AppError::Io(
                "reorder list contains duplicate operation IDs".to_string(),
            ));
        }
    }

    let mut project = project_lock
        .write()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;

    if uuids.len() != project.operations.len() {
        return Err(AppError::Io(format!(
            "reorder list has {} IDs but project has {} operations",
            uuids.len(),
            project.operations.len()
        )));
    }

    for uuid in &uuids {
        if !project.operations.iter().any(|op| &op.id == uuid) {
            return Err(AppError::NotFound(format!("operation {uuid} not found")));
        }
    }

    let mut reordered = Vec::with_capacity(project.operations.len());
    for uuid in &uuids {
        let pos = project
            .operations
            .iter()
            .position(|op| &op.id == uuid)
            .unwrap();
        reordered.push(project.operations[pos].clone());
    }
    project.operations = reordered;

    Ok(())
}

// ── list_operations ───────────────────────────────────────────────────────────

/// Testable inner logic for [`list_operations`].
///
/// Returns a snapshot of the current operation list (cloned to release the lock).
pub(crate) fn list_operations_inner(
    project_lock: &RwLock<Project>,
) -> Result<Vec<Operation>, AppError> {
    let project = project_lock
        .read()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))?;
    Ok(project.operations.clone())
}

// ── Tauri command wrappers ────────────────────────────────────────────────────

/// Add a new operation to the project.
///
/// Validates that `tool_id` references an existing tool. The operation ID is
/// generated server-side. Returns the created [`Operation`].
#[tauri::command]
pub async fn add_operation(
    input: OperationInput,
    state: tauri::State<'_, AppState>,
) -> Result<Operation, AppError> {
    add_operation_inner(input, &state.project)
}

/// Replace all fields of an existing operation.
///
/// Returns the updated [`Operation`], or [`AppError::NotFound`] if `id` does
/// not match any operation or the new `tool_id` does not match any tool.
#[tauri::command]
pub async fn edit_operation(
    id: String,
    input: OperationInput,
    state: tauri::State<'_, AppState>,
) -> Result<Operation, AppError> {
    edit_operation_inner(&id, input, &state.project)
}

/// Remove an operation from the project.
///
/// Returns [`AppError::NotFound`] if `id` does not match any operation.
#[tauri::command]
pub async fn delete_operation(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    delete_operation_inner(&id, &state.project)
}

/// Reorder the project's operation list.
///
/// `ids` must contain exactly the same set of UUIDs as the current operation
/// list. Returns an error if the count or any ID does not match.
#[tauri::command]
pub async fn reorder_operations(
    ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    reorder_operations_inner(ids, &state.project)
}

/// Return all operations in the project in their current order.
#[tauri::command]
pub async fn list_operations(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Operation>, AppError> {
    list_operations_inner(&state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{CompensationSide, DrillParams, PocketParams, ProfileParams};
    use crate::models::{Tool, ToolType};
    use crate::state::AppState;

    /// Add a tool to the project and return its UUID string.
    fn add_test_tool(state: &AppState) -> String {
        let tool = Tool {
            id: Uuid::new_v4(),
            name: "Test Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        };
        let id = tool.id.to_string();
        state.project.write().expect("write lock").tools.push(tool);
        id
    }

    fn profile_input(name: &str, tool_id: &str) -> OperationInput {
        OperationInput {
            name: name.to_string(),
            enabled: None,
            tool_id: tool_id.to_string(),
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: 2.5,
                compensation_side: CompensationSide::Left,
            }),
        }
    }

    fn pocket_input(name: &str, tool_id: &str) -> OperationInput {
        OperationInput {
            name: name.to_string(),
            enabled: None,
            tool_id: tool_id.to_string(),
            params: OperationParams::Pocket(PocketParams {
                depth: 15.0,
                stepdown: 3.0,
                stepover_percent: 45.0,
            }),
        }
    }

    fn drill_input(name: &str, tool_id: &str) -> OperationInput {
        OperationInput {
            name: name.to_string(),
            enabled: None,
            tool_id: tool_id.to_string(),
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                peck_depth: Some(5.0),
            }),
        }
    }

    // ── CRUD lifecycle ────────────────────────────────────────────────────────

    #[test]
    fn add_operation_appears_in_list() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op = add_operation_inner(profile_input("Outer Profile", &tid), &state.project)
            .expect("add should succeed");

        let ops = list_operations_inner(&state.project).expect("list should succeed");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].id, op.id);
        assert_eq!(ops[0].name, "Outer Profile");
        assert!(ops[0].enabled, "enabled should default to true");
    }

    #[test]
    fn edit_operation_updates_fields() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op = add_operation_inner(profile_input("Original", &tid), &state.project)
            .expect("add should succeed");

        let updated = edit_operation_inner(
            &op.id.to_string(),
            OperationInput {
                name: "Renamed".to_string(),
                enabled: Some(false),
                tool_id: tid.clone(),
                params: OperationParams::Pocket(PocketParams {
                    depth: 8.0,
                    stepdown: 2.0,
                    stepover_percent: 50.0,
                }),
            },
            &state.project,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, op.id);
        assert_eq!(updated.name, "Renamed");
        assert!(!updated.enabled);
        assert!(matches!(updated.params, OperationParams::Pocket(_)));

        let ops = list_operations_inner(&state.project).expect("list should succeed");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Renamed");
    }

    #[test]
    fn delete_operation_removes_it() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op = add_operation_inner(profile_input("To Delete", &tid), &state.project)
            .expect("add should succeed");

        delete_operation_inner(&op.id.to_string(), &state.project).expect("delete should succeed");

        let ops = list_operations_inner(&state.project).expect("list should succeed");
        assert!(ops.is_empty());
    }

    #[test]
    fn delete_only_operation_succeeds() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op = add_operation_inner(drill_input("Solo Drill", &tid), &state.project)
            .expect("add should succeed");

        let result = delete_operation_inner(&op.id.to_string(), &state.project);
        assert!(result.is_ok(), "deleting the only operation should succeed");

        let ops = list_operations_inner(&state.project).expect("list");
        assert!(ops.is_empty());
    }

    // ── Reorder ───────────────────────────────────────────────────────────────

    #[test]
    fn reorder_operations_changes_order() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op1 =
            add_operation_inner(profile_input("First", &tid), &state.project).expect("add op1");
        let op2 =
            add_operation_inner(pocket_input("Second", &tid), &state.project).expect("add op2");
        let op3 = add_operation_inner(drill_input("Third", &tid), &state.project).expect("add op3");

        // Reverse the order.
        reorder_operations_inner(
            vec![op3.id.to_string(), op2.id.to_string(), op1.id.to_string()],
            &state.project,
        )
        .expect("reorder should succeed");

        let ops = list_operations_inner(&state.project).expect("list");
        assert_eq!(ops[0].id, op3.id);
        assert_eq!(ops[1].id, op2.id);
        assert_eq!(ops[2].id, op1.id);
    }

    #[test]
    fn reorder_round_trip_persists_order() {
        use crate::project::serialization::{load, save};

        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op1 =
            add_operation_inner(profile_input("Alpha", &tid), &state.project).expect("add op1");
        let op2 = add_operation_inner(pocket_input("Beta", &tid), &state.project).expect("add op2");
        let op3 = add_operation_inner(drill_input("Gamma", &tid), &state.project).expect("add op3");

        // Reorder: Gamma, Alpha, Beta
        reorder_operations_inner(
            vec![op3.id.to_string(), op1.id.to_string(), op2.id.to_string()],
            &state.project,
        )
        .expect("reorder");

        let tmp = std::env::temp_dir().join("jcam_test_reorder_round_trip.jcam");
        {
            let project = state.project.read().expect("read");
            save(&*project, &tmp).expect("save");
        }
        let loaded = load(&tmp).expect("load");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.operations.len(), 3);
        assert_eq!(loaded.operations[0].id, op3.id, "Gamma first");
        assert_eq!(loaded.operations[1].id, op1.id, "Alpha second");
        assert_eq!(loaded.operations[2].id, op2.id, "Beta third");
    }

    // ── Tool ID validation ────────────────────────────────────────────────────

    #[test]
    fn add_with_nonexistent_tool_id_fails() {
        let state = AppState::default();
        let fake_tid = Uuid::new_v4().to_string();
        let result = add_operation_inner(profile_input("Bad Op", &fake_tid), &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn edit_with_invalid_new_tool_id_fails() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op = add_operation_inner(profile_input("Good Op", &tid), &state.project)
            .expect("add should succeed");

        let fake_tid = Uuid::new_v4().to_string();
        let result = edit_operation_inner(
            &op.id.to_string(),
            profile_input("Bad Edit", &fake_tid),
            &state.project,
        );
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn edit_nonexistent_operation_returns_not_found() {
        let state = AppState::default();
        let tid = add_test_tool(&state);
        let fake_id = Uuid::new_v4().to_string();
        let result = edit_operation_inner(&fake_id, profile_input("X", &tid), &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn delete_nonexistent_operation_returns_not_found() {
        let state = AppState::default();
        let fake_id = Uuid::new_v4().to_string();
        let result = delete_operation_inner(&fake_id, &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    // ── Reorder error cases ───────────────────────────────────────────────────

    #[test]
    fn reorder_with_wrong_count_fails() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op1 = add_operation_inner(profile_input("A", &tid), &state.project).expect("add");
        add_operation_inner(pocket_input("B", &tid), &state.project).expect("add");

        // Submit only one ID for a two-operation list.
        let result = reorder_operations_inner(vec![op1.id.to_string()], &state.project);
        assert!(matches!(result, Err(AppError::Io(_))));
    }

    #[test]
    fn reorder_with_duplicate_ids_fails() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op1 = add_operation_inner(profile_input("A", &tid), &state.project).expect("add");
        add_operation_inner(pocket_input("B", &tid), &state.project).expect("add");

        // Submit op1 twice — count matches but set is wrong.
        let result =
            reorder_operations_inner(vec![op1.id.to_string(), op1.id.to_string()], &state.project);
        assert!(matches!(result, Err(AppError::Io(_))));
    }

    #[test]
    fn reorder_with_unknown_id_fails() {
        let state = AppState::default();
        let tid = add_test_tool(&state);

        let op1 = add_operation_inner(profile_input("A", &tid), &state.project).expect("add");
        add_operation_inner(pocket_input("B", &tid), &state.project).expect("add");

        let fake_id = Uuid::new_v4().to_string();
        let result = reorder_operations_inner(vec![op1.id.to_string(), fake_id], &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }
}
