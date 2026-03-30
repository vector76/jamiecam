//! Global tool library CRUD IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<GlobalToolLibrary>` and `&Path`, and
//!   contain the business logic. They are synchronous and directly testable
//!   without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::path::Path;
use std::sync::RwLock;

use uuid::Uuid;

use crate::error::AppError;
use crate::models::Tool;
use crate::state::{AppState, GlobalToolLibrary};

use super::tools::{tool_from_input, validate_tool_geometry, ToolInput};
use super::{parse_entity_id, read_library, write_library};

// ── list_global_tools ────────────────────────────────────────────────────────

/// Testable inner logic for [`list_global_tools`].
///
/// Returns a snapshot of the global tool library (cloned to release the lock).
pub(crate) fn list_global_tools_inner(
    library_lock: &RwLock<GlobalToolLibrary>,
) -> Result<Vec<Tool>, AppError> {
    let library = read_library(library_lock)?;
    Ok(library.tools.clone())
}

// ── add_global_tool ──────────────────────────────────────────────────────────

/// Testable inner logic for [`add_global_tool`].
///
/// Generates a new UUID for the tool, inserts it into the global library,
/// persists to disk, and returns the created [`Tool`].
pub(crate) fn add_global_tool_inner(
    input: ToolInput,
    library_lock: &RwLock<GlobalToolLibrary>,
    save_path: &Path,
) -> Result<Tool, AppError> {
    validate_tool_geometry(&input)?;
    let tool = tool_from_input(Uuid::new_v4(), input);
    let mut library = write_library(library_lock)?;
    library.tools.push(tool.clone());
    library.save(save_path)?;
    Ok(tool)
}

// ── edit_global_tool ─────────────────────────────────────────────────────────

/// Testable inner logic for [`edit_global_tool`].
///
/// Finds the tool with the given `id`, replaces all its fields with `input`,
/// persists to disk, and returns the updated [`Tool`]. Returns
/// [`AppError::NotFound`] if no tool with that ID exists.
pub(crate) fn edit_global_tool_inner(
    id: &str,
    input: ToolInput,
    library_lock: &RwLock<GlobalToolLibrary>,
    save_path: &Path,
) -> Result<Tool, AppError> {
    let uuid = parse_entity_id(id, "global tool")?;
    validate_tool_geometry(&input)?;

    let mut library = write_library(library_lock)?;

    let entry = library
        .tools
        .iter_mut()
        .find(|t| t.id == uuid)
        .ok_or_else(|| AppError::NotFound(format!("global tool {id} not found")))?;

    *entry = tool_from_input(uuid, input);
    let updated = entry.clone();

    library.save(save_path)?;
    Ok(updated)
}

// ── delete_global_tool ───────────────────────────────────────────────────────

/// Testable inner logic for [`delete_global_tool`].
///
/// Removes the tool with the given `id` and persists to disk. Returns
/// [`AppError::NotFound`] if no tool with that ID exists.
pub(crate) fn delete_global_tool_inner(
    id: &str,
    library_lock: &RwLock<GlobalToolLibrary>,
    save_path: &Path,
) -> Result<(), AppError> {
    let uuid = parse_entity_id(id, "global tool")?;

    let mut library = write_library(library_lock)?;

    let before = library.tools.len();
    library.tools.retain(|t| t.id != uuid);
    if library.tools.len() == before {
        return Err(AppError::NotFound(format!("global tool {id} not found")));
    }

    library.save(save_path)?;
    Ok(())
}

// ── Tauri command wrappers ──────────────────────────────────────────────────

/// Return all tools in the global tool library.
#[tauri::command]
pub async fn list_global_tools(state: tauri::State<'_, AppState>) -> Result<Vec<Tool>, AppError> {
    list_global_tools_inner(&state.global_tool_library)
}

/// Add a new tool to the global tool library.
///
/// The tool ID is generated server-side. Returns the created [`Tool`] so the
/// frontend can immediately display it with its assigned ID.
#[tauri::command]
pub async fn add_global_tool(
    input: ToolInput,
    state: tauri::State<'_, AppState>,
) -> Result<Tool, AppError> {
    add_global_tool_inner(
        input,
        &state.global_tool_library,
        &state.global_library_path,
    )
}

/// Replace all fields of an existing tool in the global library.
///
/// Returns the updated [`Tool`], or [`AppError::NotFound`] if `id` does not
/// match any tool in the global library.
#[tauri::command]
pub async fn edit_global_tool(
    id: String,
    input: ToolInput,
    state: tauri::State<'_, AppState>,
) -> Result<Tool, AppError> {
    edit_global_tool_inner(
        &id,
        input,
        &state.global_tool_library,
        &state.global_library_path,
    )
}

/// Remove a tool from the global tool library.
///
/// Returns [`AppError::NotFound`] if `id` does not match any tool.
#[tauri::command]
pub async fn delete_global_tool(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    delete_global_tool_inner(&id, &state.global_tool_library, &state.global_library_path)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ToolType;

    fn make_input(name: &str) -> ToolInput {
        ToolInput {
            name: name.to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
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

    fn temp_path() -> std::path::PathBuf {
        tempfile::NamedTempFile::new().unwrap().path().to_owned()
    }

    #[test]
    fn add_global_tool_appears_in_list() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("My Endmill"), &library, &path)
            .expect("add should succeed");

        let tools = list_global_tools_inner(&library).expect("list should succeed");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, tool.id);
        assert_eq!(tools[0].name, "My Endmill");
    }

    #[test]
    fn edit_global_tool_updates_fields() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("Original"), &library, &path)
            .expect("add should succeed");

        let updated = edit_global_tool_inner(
            &tool.id.to_string(),
            ToolInput {
                name: "Renamed".to_string(),
                tool_type: ToolType::BallNose,
                material: "hss".to_string(),
                diameter: 6.0,
                flute_count: 2,
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
            &library,
            &path,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, tool.id);
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.tool_type, ToolType::BallNose);
        assert_eq!(updated.material, "hss");
        assert_eq!(updated.diameter, 6.0);
        assert_eq!(updated.flute_count, 2);
        assert!(updated.default_spindle_speed.is_none());

        let tools = list_global_tools_inner(&library).expect("list should succeed");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Renamed");
    }

    #[test]
    fn delete_global_tool_removes_it() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("To Delete"), &library, &path)
            .expect("add should succeed");

        delete_global_tool_inner(&tool.id.to_string(), &library, &path)
            .expect("delete should succeed");

        let tools = list_global_tools_inner(&library).expect("list should succeed");
        assert!(tools.is_empty());
    }

    #[test]
    fn edit_nonexistent_id_returns_not_found() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
        let fake_id = Uuid::new_v4().to_string();
        let result = edit_global_tool_inner(&fake_id, make_input("X"), &library, &path);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn delete_nonexistent_id_returns_not_found() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
        let fake_id = Uuid::new_v4().to_string();
        let result = delete_global_tool_inner(&fake_id, &library, &path);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn validation_rejects_negative_diameter() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
        let mut input = make_input("Bad");
        input.diameter = -1.0;
        let result = add_global_tool_inner(input, &library, &path);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn add_assigns_new_uuid() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let t1 = add_global_tool_inner(make_input("Tool A"), &library, &path).expect("add t1");
        let t2 = add_global_tool_inner(make_input("Tool B"), &library, &path).expect("add t2");

        assert_ne!(t1.id, t2.id);
    }

    #[test]
    fn edit_preserves_existing_uuid() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("Before"), &library, &path)
            .expect("add should succeed");
        let original_id = tool.id;

        let updated = edit_global_tool_inner(
            &original_id.to_string(),
            make_input("After"),
            &library,
            &path,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, original_id);
    }

    #[test]
    fn resolve_defaults_applied() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("Defaulted"), &library, &path)
            .expect("add should succeed");

        // resolve_defaults fills these from diameter (10.0).
        assert_eq!(tool.cutting_length, 30.0);
        assert_eq!(tool.shank_diameter, 10.0);
        assert_eq!(tool.overall_length, 90.0);
    }

    #[test]
    fn add_persists_to_disk() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        add_global_tool_inner(make_input("Persisted"), &library, &path)
            .expect("add should succeed");

        let loaded = GlobalToolLibrary::load(&path);
        assert_eq!(loaded.tools.len(), 1);
        assert_eq!(loaded.tools[0].name, "Persisted");
    }

    #[test]
    fn delete_persists_to_disk() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();

        let tool = add_global_tool_inner(make_input("ToDelete"), &library, &path)
            .expect("add should succeed");
        delete_global_tool_inner(&tool.id.to_string(), &library, &path)
            .expect("delete should succeed");

        let loaded = GlobalToolLibrary::load(&path);
        assert!(loaded.tools.is_empty());
    }
}
