//! Tool CRUD IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::sync::RwLock;

use tauri::Emitter;
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
    pub material: Option<String>,
    pub diameter: f64,
    pub flute_count: Option<u32>,
    pub default_spindle_speed: Option<u32>,
    pub default_feed_rate: Option<f64>,
    // Universal geometry
    pub cutting_length: Option<f64>,
    pub shank_diameter: Option<f64>,
    pub overall_length: Option<f64>,
    // Type-specific geometry
    pub corner_radius: Option<f64>,
    pub included_angle: Option<f64>,
    pub point_angle: Option<f64>,
    pub pilot_diameter: Option<f64>,
    pub pilot_length: Option<f64>,
    pub thread_pitch: Option<f64>,
    pub min_bore_diameter: Option<f64>,
    pub taper_half_angle: Option<f64>,
}

// ── Validation ───────────────────────────────────────────────────────────────

/// Basic sanity checks on geometry values before inserting/updating a tool.
pub(crate) fn validate_tool_geometry(input: &ToolInput) -> Result<(), AppError> {
    if input.diameter < 0.0 {
        return Err(AppError::InvalidInput(
            "diameter must not be negative".into(),
        ));
    }
    if let Some(cl) = input.cutting_length {
        if cl < 0.0 {
            return Err(AppError::InvalidInput(
                "cutting_length must not be negative".into(),
            ));
        }
    }
    if let Some(sd) = input.shank_diameter {
        if sd < 0.0 {
            return Err(AppError::InvalidInput(
                "shank_diameter must not be negative".into(),
            ));
        }
    }
    if let Some(ol) = input.overall_length {
        if ol < 0.0 {
            return Err(AppError::InvalidInput(
                "overall_length must not be negative".into(),
            ));
        }
        if let Some(cl) = input.cutting_length {
            if ol < cl {
                return Err(AppError::InvalidInput(
                    "overall_length must not be less than cutting_length".into(),
                ));
            }
        }
    }
    if let Some(cr) = input.corner_radius {
        if cr < 0.0 {
            return Err(AppError::InvalidInput(
                "corner_radius must not be negative".into(),
            ));
        }
        if cr > input.diameter / 2.0 {
            return Err(AppError::InvalidInput(
                "corner_radius must not exceed diameter / 2".into(),
            ));
        }
    }
    if let Some(a) = input.included_angle {
        if a < 0.0 {
            return Err(AppError::InvalidInput(
                "included_angle must not be negative".into(),
            ));
        }
    }
    if let Some(a) = input.point_angle {
        if a < 0.0 {
            return Err(AppError::InvalidInput(
                "point_angle must not be negative".into(),
            ));
        }
    }
    if let Some(a) = input.taper_half_angle {
        if a < 0.0 {
            return Err(AppError::InvalidInput(
                "taper_half_angle must not be negative".into(),
            ));
        }
    }
    Ok(())
}

// ── Helper: build Tool from ToolInput ────────────────────────────────────────

/// Map [`ToolInput`] fields onto a [`Tool`], using `0.0` for absent
/// `cutting_length` / `shank_diameter` and passing optional fields through
/// directly, then apply heuristic defaults.
pub(crate) fn tool_from_input(id: Uuid, input: ToolInput) -> Tool {
    let mut tool = Tool {
        id,
        name: input.name,
        tool_type: input.tool_type,
        material: input
            .material
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        diameter: input.diameter,
        flute_count: input.flute_count,
        default_spindle_speed: input.default_spindle_speed,
        default_feed_rate: input.default_feed_rate,
        cutting_length: input.cutting_length.unwrap_or(0.0),
        shank_diameter: input.shank_diameter.unwrap_or(0.0),
        overall_length: input.overall_length,
        corner_radius: input.corner_radius,
        included_angle: input.included_angle,
        point_angle: input.point_angle,
        pilot_diameter: input.pilot_diameter,
        pilot_length: input.pilot_length,
        thread_pitch: input.thread_pitch,
        min_bore_diameter: input.min_bore_diameter,
        taper_half_angle: input.taper_half_angle,
    };
    tool.resolve_defaults();
    tool
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
    validate_tool_geometry(&input)?;
    let tool = tool_from_input(Uuid::new_v4(), input);
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
    validate_tool_geometry(&input)?;

    let mut project = write_project(project_lock)?;

    let entry = project
        .tools
        .iter_mut()
        .find(|t| t.id == uuid)
        .ok_or_else(|| AppError::NotFound(format!("tool {id} not found")))?;

    // Replace with a fresh Tool built from the input, preserving the ID.
    *entry = tool_from_input(uuid, input);

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
    app: tauri::AppHandle,
) -> Result<Tool, AppError> {
    let tool = add_tool_inner(input, &state.project)?;

    let is_open = *state
        .project_is_open
        .read()
        .map_err(|e| AppError::Io(format!("project_is_open lock poisoned: {e}")))?;
    let snapshot = super::project::get_project_snapshot_inner(&state.project, is_open)?;
    let _ = app.emit("project:modified", &snapshot);

    Ok(tool)
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
    app: tauri::AppHandle,
) -> Result<Tool, AppError> {
    let tool = edit_tool_inner(&id, input, &state.project)?;

    let is_open = *state
        .project_is_open
        .read()
        .map_err(|e| AppError::Io(format!("project_is_open lock poisoned: {e}")))?;
    let snapshot = super::project::get_project_snapshot_inner(&state.project, is_open)?;
    let _ = app.emit("project:modified", &snapshot);

    Ok(tool)
}

/// Remove a tool from the project tool library.
///
/// Returns [`AppError::NotFound`] if `id` does not match any tool.
#[tauri::command]
pub async fn delete_tool(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    delete_tool_inner(&id, &state.project)?;

    let is_open = *state
        .project_is_open
        .read()
        .map_err(|e| AppError::Io(format!("project_is_open lock poisoned: {e}")))?;
    let snapshot = super::project::get_project_snapshot_inner(&state.project, is_open)?;
    let _ = app.emit("project:modified", &snapshot);

    Ok(())
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
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
            cutting_length: None,
            shank_diameter: None,
            overall_length: None,
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
                material: Some("hss".to_string()),
                diameter: 6.0,
                flute_count: Some(2),
                default_spindle_speed: None,
                default_feed_rate: None,
                cutting_length: None,
                shank_diameter: None,
                overall_length: None,
                corner_radius: None,
                included_angle: None,
                point_angle: None,
                pilot_diameter: None,
                pilot_length: None,
                thread_pitch: None,
                min_bore_diameter: None,
                taper_half_angle: None,
            },
            &state.project,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, tool.id);
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.tool_type, ToolType::BallNose);
        assert_eq!(updated.material, Some("hss".to_string()));
        assert_eq!(updated.diameter, 6.0);
        assert_eq!(updated.flute_count, Some(2));
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

    // ── Geometry IPC tests ───────────────────────────────────────────────────

    #[test]
    fn add_tool_with_explicit_geometry_values() {
        let state = AppState::default();
        let input = ToolInput {
            name: "Bull Nose".to_string(),
            tool_type: ToolType::BullNose,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: Some(25.0),
            shank_diameter: Some(10.0),
            overall_length: Some(80.0),
            corner_radius: Some(2.0),
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: Some(1.5),
        };
        let tool = add_tool_inner(input, &state.project).expect("add should succeed");
        assert_eq!(tool.cutting_length, 25.0);
        assert_eq!(tool.shank_diameter, 10.0);
        assert_eq!(tool.overall_length, Some(80.0));
        assert_eq!(tool.corner_radius, Some(2.0));
        assert_eq!(tool.taper_half_angle, Some(1.5));
    }

    #[test]
    fn add_tool_omitted_geometry_gets_heuristic_defaults() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("Defaulted"), &state.project).expect("add should succeed");
        // Universal: resolve_defaults fills these from diameter.
        assert_eq!(tool.cutting_length, 30.0);
        assert_eq!(tool.shank_diameter, 10.0);
        assert!(tool.overall_length.is_none());
        // FlatEndmill has no type-specific defaults.
        assert_eq!(tool.corner_radius, None);
    }

    #[test]
    fn edit_tool_preserves_geometry() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("Before"), &state.project).expect("add should succeed");
        let updated = edit_tool_inner(
            &tool.id.to_string(),
            ToolInput {
                name: "After".to_string(),
                tool_type: ToolType::Drill,
                material: Some("hss".to_string()),
                diameter: 8.0,
                flute_count: Some(2),
                default_spindle_speed: None,
                default_feed_rate: None,
                cutting_length: Some(20.0),
                shank_diameter: Some(8.0),
                overall_length: Some(60.0),
                corner_radius: None,
                included_angle: None,
                point_angle: Some(135.0),
                pilot_diameter: None,
                pilot_length: None,
                thread_pitch: None,
                min_bore_diameter: None,
                taper_half_angle: None,
            },
            &state.project,
        )
        .expect("edit should succeed");
        assert_eq!(updated.cutting_length, 20.0);
        assert_eq!(updated.shank_diameter, 8.0);
        assert_eq!(updated.overall_length, Some(60.0));
        assert_eq!(updated.point_angle, Some(135.0));
    }

    #[test]
    fn validation_rejects_negative_cutting_length() {
        let state = AppState::default();
        let mut input = make_input("Bad");
        input.cutting_length = Some(-5.0);
        let result = add_tool_inner(input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn validation_rejects_negative_diameter() {
        let state = AppState::default();
        let mut input = make_input("Bad");
        input.diameter = -1.0;
        let result = add_tool_inner(input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn validation_rejects_overall_less_than_cutting() {
        let state = AppState::default();
        let mut input = make_input("Bad");
        input.cutting_length = Some(50.0);
        input.overall_length = Some(30.0);
        let result = add_tool_inner(input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn validation_rejects_corner_radius_exceeding_half_diameter() {
        let state = AppState::default();
        let mut input = make_input("Bad");
        input.corner_radius = Some(6.0); // diameter = 10, so max = 5
        let result = add_tool_inner(input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn validation_rejects_negative_angles() {
        let state = AppState::default();
        let mut input = make_input("Bad");
        input.included_angle = Some(-10.0);
        let result = add_tool_inner(input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    // ── Optional-field tests ─────────────────────────────────────────────────

    #[test]
    fn add_tool_with_all_three_optional_fields_omitted() {
        let state = AppState::default();
        let input = ToolInput {
            name: "Bare".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: None,
            diameter: 10.0,
            flute_count: None,
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: None,
            shank_diameter: None,
            overall_length: None,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        };
        let tool = add_tool_inner(input, &state.project).expect("add should succeed");
        assert!(tool.material.is_none());
        assert!(tool.flute_count.is_none());
        assert!(tool.overall_length.is_none());
    }

    #[test]
    fn blank_string_material_becomes_none() {
        let state = AppState::default();
        let input = ToolInput {
            name: "Whitespace".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("  ".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: None,
            shank_diameter: None,
            overall_length: None,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        };
        let tool = add_tool_inner(input, &state.project).expect("add should succeed");
        assert!(
            tool.material.is_none(),
            "blank-string material should become None"
        );
    }

    #[test]
    fn edit_tool_clears_previously_set_optional_values() {
        let state = AppState::default();
        let tool =
            add_tool_inner(make_input("Original"), &state.project).expect("add should succeed");
        assert_eq!(tool.material, Some("carbide".to_string()));
        assert_eq!(tool.flute_count, Some(4));

        let updated = edit_tool_inner(
            &tool.id.to_string(),
            ToolInput {
                name: "Cleared".to_string(),
                tool_type: ToolType::FlatEndmill,
                material: None,
                diameter: 10.0,
                flute_count: None,
                default_spindle_speed: None,
                default_feed_rate: None,
                cutting_length: None,
                shank_diameter: None,
                overall_length: None,
                corner_radius: None,
                included_angle: None,
                point_angle: None,
                pilot_diameter: None,
                pilot_length: None,
                thread_pitch: None,
                min_bore_diameter: None,
                taper_half_angle: None,
            },
            &state.project,
        )
        .expect("edit should succeed");

        assert!(updated.material.is_none(), "material should be cleared");
        assert!(
            updated.flute_count.is_none(),
            "flute_count should be cleared"
        );
        assert!(
            updated.overall_length.is_none(),
            "overall_length should be cleared"
        );
    }

    #[test]
    fn validation_passes_with_none_overall_length() {
        let state = AppState::default();
        let input = ToolInput {
            name: "No OAL".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: Some(50.0),
            shank_diameter: None,
            overall_length: None,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        };
        let tool = add_tool_inner(input, &state.project)
            .expect("should succeed — OAL >= CL check is skipped when OAL is None");
        assert_eq!(tool.cutting_length, 50.0);
        assert!(tool.overall_length.is_none());
    }

    #[test]
    fn edit_validation_rejects_invalid_input() {
        let state = AppState::default();
        let tool = add_tool_inner(make_input("Good"), &state.project).expect("add should succeed");
        let mut bad_input = make_input("Bad Edit");
        bad_input.cutting_length = Some(-1.0);
        let result = edit_tool_inner(&tool.id.to_string(), bad_input, &state.project);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }
}
