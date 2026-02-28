//! Tool CRUD IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::sync::RwLock;

use uuid::Uuid;

use crate::error::AppError;
use crate::models::{Tool, ToolType};
use crate::state::{AppState, Project};

use super::{parse_entity_id, read_project, write_project};

// ── Input type ────────────────────────────────────────────────────────────────

/// Fields required to create or replace a tool (ID is excluded; it is either
/// generated on add or provided separately on edit).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInput {
    pub name: String,
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub material: String,
    pub diameter: f64,
    pub flute_count: u32,
    pub default_spindle_speed: Option<u32>,
    pub default_feed_rate: Option<f64>,
}

// ── add_tool ──────────────────────────────────────────────────────────────────

/// Testable inner logic for [`add_tool`].
///
/// Generates a new UUID for the tool, inserts it into `project.tools`, and
/// returns the created [`Tool`].
pub(crate) fn add_tool_inner(
    input: ToolInput,
    project_lock: &RwLock<Project>,
) -> Result<Tool, AppError> {
    let tool = Tool {
        id: Uuid::new_v4(),
        name: input.name,
        tool_type: input.tool_type,
        material: input.material,
        diameter: input.diameter,
        flute_count: input.flute_count,
        default_spindle_speed: input.default_spindle_speed,
        default_feed_rate: input.default_feed_rate,
    };
    let mut project = write_project(project_lock)?;
    project.tools.push(tool.clone());
    Ok(tool)
}

// ── edit_tool ─────────────────────────────────────────────────────────────────

/// Testable inner logic for [`edit_tool`].
///
/// Finds the tool with the given `id`, replaces all its fields with `input`,
/// and returns the updated [`Tool`]. Returns [`AppError::NotFound`] if no tool
/// with that ID exists.
pub(crate) fn edit_tool_inner(
    id: &str,
    input: ToolInput,
    project_lock: &RwLock<Project>,
) -> Result<Tool, AppError> {
    let uuid = parse_entity_id(id, "tool")?;

    let mut project = write_project(project_lock)?;

    let entry = project
        .tools
        .iter_mut()
        .find(|t| t.id == uuid)
        .ok_or_else(|| AppError::NotFound(format!("tool {id} not found")))?;

    entry.name = input.name;
    entry.tool_type = input.tool_type;
    entry.material = input.material;
    entry.diameter = input.diameter;
    entry.flute_count = input.flute_count;
    entry.default_spindle_speed = input.default_spindle_speed;
    entry.default_feed_rate = input.default_feed_rate;

    Ok(entry.clone())
}

// ── delete_tool ───────────────────────────────────────────────────────────────

/// Testable inner logic for [`delete_tool`].
///
/// Removes the tool with the given `id`. Returns [`AppError::NotFound`] if no
/// tool with that ID exists.
pub(crate) fn delete_tool_inner(id: &str, project_lock: &RwLock<Project>) -> Result<(), AppError> {
    let uuid = parse_entity_id(id, "tool")?;

    let mut project = write_project(project_lock)?;

    let before = project.tools.len();
    project.tools.retain(|t| t.id != uuid);
    if project.tools.len() == before {
        return Err(AppError::NotFound(format!("tool {id} not found")));
    }

    Ok(())
}

// ── list_tools ────────────────────────────────────────────────────────────────

/// Testable inner logic for [`list_tools`].
///
/// Returns a snapshot of the current tool library (cloned to release the lock).
pub(crate) fn list_tools_inner(project_lock: &RwLock<Project>) -> Result<Vec<Tool>, AppError> {
    let project = read_project(project_lock)?;
    Ok(project.tools.clone())
}

// ── Tauri command wrappers ────────────────────────────────────────────────────

/// Add a new tool to the project tool library.
///
/// The tool ID is generated server-side. Returns the created [`Tool`] so the
/// frontend can immediately display it with its assigned ID.
#[tauri::command]
pub async fn add_tool(
    input: ToolInput,
    state: tauri::State<'_, AppState>,
) -> Result<Tool, AppError> {
    add_tool_inner(input, &state.project)
}

/// Replace all fields of an existing tool.
///
/// Returns the updated [`Tool`], or [`AppError::NotFound`] if `id` does not
/// match any tool in the project library.
#[tauri::command]
pub async fn edit_tool(
    id: String,
    input: ToolInput,
    state: tauri::State<'_, AppState>,
) -> Result<Tool, AppError> {
    edit_tool_inner(&id, input, &state.project)
}

/// Remove a tool from the project tool library.
///
/// Returns [`AppError::NotFound`] if `id` does not match any tool.
#[tauri::command]
pub async fn delete_tool(id: String, state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    delete_tool_inner(&id, &state.project)
}

/// Return all tools in the project tool library.
#[tauri::command]
pub async fn list_tools(state: tauri::State<'_, AppState>) -> Result<Vec<Tool>, AppError> {
    list_tools_inner(&state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn make_input(name: &str) -> ToolInput {
        ToolInput {
            name: name.to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
        }
    }

    #[test]
    fn add_tool_appears_in_list() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("My Endmill"), &state.project).expect("add should succeed");

        let tools = list_tools_inner(&state.project).expect("list should succeed");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, tool.id);
        assert_eq!(tools[0].name, "My Endmill");
    }

    #[test]
    fn edit_tool_updates_fields() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("Original"), &state.project).expect("add should succeed");

        let updated = edit_tool_inner(
            &tool.id.to_string(),
            ToolInput {
                name: "Renamed".to_string(),
                tool_type: ToolType::BallNose,
                material: "hss".to_string(),
                diameter: 6.0,
                flute_count: 2,
                default_spindle_speed: None,
                default_feed_rate: None,
            },
            &state.project,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, tool.id);
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.tool_type, ToolType::BallNose);
        assert_eq!(updated.material, "hss");
        assert_eq!(updated.diameter, 6.0);
        assert_eq!(updated.flute_count, 2);
        assert!(updated.default_spindle_speed.is_none());

        let tools = list_tools_inner(&state.project).expect("list should succeed");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Renamed");
    }

    #[test]
    fn delete_tool_removes_it() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("To Delete"), &state.project).expect("add should succeed");

        delete_tool_inner(&tool.id.to_string(), &state.project).expect("delete should succeed");

        let tools = list_tools_inner(&state.project).expect("list should succeed");
        assert!(tools.is_empty());
    }

    #[test]
    fn add_multiple_tools_have_distinct_ids() {
        let state = AppState::default();
        let t1 = add_tool_inner(make_input("Tool A"), &state.project).expect("add t1");
        let t2 = add_tool_inner(make_input("Tool B"), &state.project).expect("add t2");
        let t3 = add_tool_inner(make_input("Tool C"), &state.project).expect("add t3");

        assert_ne!(t1.id, t2.id);
        assert_ne!(t2.id, t3.id);
        assert_ne!(t1.id, t3.id);

        let tools = list_tools_inner(&state.project).expect("list should succeed");
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn edit_nonexistent_id_returns_not_found() {
        let state = AppState::default();
        let fake_id = Uuid::new_v4().to_string();
        let result = edit_tool_inner(&fake_id, make_input("X"), &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn delete_nonexistent_id_returns_not_found() {
        let state = AppState::default();
        let fake_id = Uuid::new_v4().to_string();
        let result = delete_tool_inner(&fake_id, &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn edit_invalid_uuid_string_returns_not_found() {
        let state = AppState::default();
        let result = edit_tool_inner("not-a-valid-uuid", make_input("X"), &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn delete_invalid_uuid_string_returns_not_found() {
        let state = AppState::default();
        let result = delete_tool_inner("not-a-valid-uuid", &state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }
}
