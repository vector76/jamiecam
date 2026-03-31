//! Global tool library CRUD IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<GlobalToolLibrary>` and `&Path`, and
//!   contain the business logic. They are synchronous and directly testable
//!   without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::path::Path;
use std::sync::RwLock;

use tauri::Emitter;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Tool;
use crate::state::{AppState, GlobalToolLibrary, Project};

use super::tools::{tool_from_input, validate_tool_geometry, ToolInput};
use super::{parse_entity_id, read_library, read_project, write_library, write_project};

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

// ── import_from_library ─────────────────────────────────────────────────────

/// Testable inner logic for [`import_from_library`].
///
/// Finds the tool with the given `id` in the global library, clones it with a
/// new UUID, and pushes it into the project tool list. Returns the newly
/// created project tool.
pub(crate) fn import_from_library_inner(
    id: &str,
    library_lock: &RwLock<GlobalToolLibrary>,
    project_lock: &RwLock<Project>,
) -> Result<Tool, AppError> {
    let uuid = parse_entity_id(id, "global tool")?;

    let library = read_library(library_lock)?;
    let source = library
        .tools
        .iter()
        .find(|t| t.id == uuid)
        .ok_or_else(|| AppError::NotFound(format!("global tool {id} not found")))?;

    let mut cloned = source.clone();
    cloned.id = Uuid::new_v4();
    drop(library);

    let mut project = write_project(project_lock)?;
    project.tools.push(cloned.clone());

    Ok(cloned)
}

// ── export_to_library ───────────────────────────────────────────────────────

/// Testable inner logic for [`export_to_library`].
///
/// Finds the tool with the given `id` in the project, clones it with a new
/// UUID, pushes it into the global library, saves to disk, and returns the
/// newly created global tool.
pub(crate) fn export_to_library_inner(
    id: &str,
    project_lock: &RwLock<Project>,
    library_lock: &RwLock<GlobalToolLibrary>,
    save_path: &Path,
) -> Result<Tool, AppError> {
    let uuid = parse_entity_id(id, "project tool")?;

    let project = read_project(project_lock)?;
    let source = project
        .tools
        .iter()
        .find(|t| t.id == uuid)
        .ok_or_else(|| AppError::NotFound(format!("project tool {id} not found")))?;

    let mut cloned = source.clone();
    cloned.id = Uuid::new_v4();
    drop(project);

    let mut library = write_library(library_lock)?;
    library.tools.push(cloned.clone());
    library.save(save_path)?;

    Ok(cloned)
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

/// Import a tool from the global library into the current project.
///
/// Clones the global tool with a new UUID and adds it to the project.
/// Returns the newly created project tool.
#[tauri::command]
pub async fn import_from_library(
    id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Tool, AppError> {
    let tool = import_from_library_inner(&id, &state.global_tool_library, &state.project)?;

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

    Ok(tool)
}

/// Export a tool from the current project to the global library.
///
/// Clones the project tool with a new UUID and adds it to the global library.
/// Returns the newly created global tool.
#[tauri::command]
pub async fn export_to_library(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Tool, AppError> {
    export_to_library_inner(
        &id,
        &state.project,
        &state.global_tool_library,
        &state.global_library_path,
    )
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
            &library,
            &path,
        )
        .expect("edit should succeed");

        assert_eq!(updated.id, tool.id);
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.tool_type, ToolType::BallNose);
        assert_eq!(updated.material, Some("hss".to_string()));
        assert_eq!(updated.diameter, 6.0);
        assert_eq!(updated.flute_count, Some(2));
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
        assert!(tool.overall_length.is_none());
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

    // ── import_from_library tests ───────────────────────────────────────

    #[test]
    fn import_creates_project_tool_with_new_uuid() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        let global_tool = add_global_tool_inner(make_input("Endmill"), &library, &path)
            .expect("add should succeed");

        let imported = import_from_library_inner(&global_tool.id.to_string(), &library, &project)
            .expect("import should succeed");

        assert_ne!(
            imported.id, global_tool.id,
            "imported tool must have a new UUID"
        );
        assert_eq!(imported.name, global_tool.name);
        assert_eq!(imported.tool_type, global_tool.tool_type);
        assert_eq!(imported.material, global_tool.material);
        assert_eq!(imported.diameter, global_tool.diameter);
        assert_eq!(imported.flute_count, global_tool.flute_count);
        assert_eq!(imported.cutting_length, global_tool.cutting_length);
        assert_eq!(imported.shank_diameter, global_tool.shank_diameter);
        assert_eq!(imported.overall_length, global_tool.overall_length);
    }

    #[test]
    fn import_does_not_modify_source_global_tool() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        let global_tool = add_global_tool_inner(make_input("Source"), &library, &path)
            .expect("add should succeed");
        let original_id = global_tool.id;

        import_from_library_inner(&global_tool.id.to_string(), &library, &project)
            .expect("import should succeed");

        let tools = list_global_tools_inner(&library).expect("list should succeed");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, original_id);
        assert_eq!(tools[0].name, "Source");
    }

    #[test]
    fn import_nonexistent_id_returns_not_found() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let fake_id = Uuid::new_v4().to_string();

        let result = import_from_library_inner(&fake_id, &library, &project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn import_same_tool_twice_creates_two_copies_with_different_uuids() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        let global_tool = add_global_tool_inner(make_input("Shared"), &library, &path)
            .expect("add should succeed");

        let imp1 = import_from_library_inner(&global_tool.id.to_string(), &library, &project)
            .expect("first import");
        let imp2 = import_from_library_inner(&global_tool.id.to_string(), &library, &project)
            .expect("second import");

        assert_ne!(imp1.id, imp2.id, "each import must have a unique UUID");
        assert_ne!(imp1.id, global_tool.id);
        assert_ne!(imp2.id, global_tool.id);
        assert_eq!(imp1.name, imp2.name);

        let proj = project.read().expect("read project");
        assert_eq!(proj.tools.len(), 2);
    }

    // ── export_to_library tests ─────────────────────────────────────────

    #[test]
    fn export_creates_global_tool_with_new_uuid() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        // Add a tool to the project directly.
        let project_tool = {
            use crate::commands::tools::add_tool_inner;
            add_tool_inner(make_input("Project Endmill"), &project).expect("add should succeed")
        };

        let exported =
            export_to_library_inner(&project_tool.id.to_string(), &project, &library, &path)
                .expect("export should succeed");

        assert_ne!(
            exported.id, project_tool.id,
            "exported tool must have a new UUID"
        );
        assert_eq!(exported.name, project_tool.name);
        assert_eq!(exported.tool_type, project_tool.tool_type);
        assert_eq!(exported.material, project_tool.material);
        assert_eq!(exported.diameter, project_tool.diameter);
        assert_eq!(exported.flute_count, project_tool.flute_count);
        assert_eq!(exported.cutting_length, project_tool.cutting_length);
    }

    #[test]
    fn export_does_not_modify_source_project_tool() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        let project_tool = {
            use crate::commands::tools::add_tool_inner;
            add_tool_inner(make_input("Original"), &project).expect("add should succeed")
        };
        let original_id = project_tool.id;

        export_to_library_inner(&project_tool.id.to_string(), &project, &library, &path)
            .expect("export should succeed");

        let proj = project.read().expect("read project");
        assert_eq!(proj.tools.len(), 1);
        assert_eq!(proj.tools[0].id, original_id);
        assert_eq!(proj.tools[0].name, "Original");
    }

    #[test]
    fn export_nonexistent_id_returns_not_found() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();
        let fake_id = Uuid::new_v4().to_string();

        let result = export_to_library_inner(&fake_id, &project, &library, &path);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    // ── Optional-field tests ─────────────────────────────────────────────────

    #[test]
    fn add_global_tool_with_all_three_optional_fields_omitted() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
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
        let tool = add_global_tool_inner(input, &library, &path).expect("add should succeed");
        assert!(tool.material.is_none());
        assert!(tool.flute_count.is_none());
        assert!(tool.overall_length.is_none());
    }

    #[test]
    fn blank_string_material_becomes_none_in_global_tools() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
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
        let tool = add_global_tool_inner(input, &library, &path).expect("add should succeed");
        assert!(
            tool.material.is_none(),
            "blank-string material should become None"
        );
    }

    #[test]
    fn none_values_survive_disk_round_trip() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let path = temp_path();
        let input = ToolInput {
            name: "Disk RT".to_string(),
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
        add_global_tool_inner(input, &library, &path).expect("add should succeed");

        let loaded = GlobalToolLibrary::load(&path);
        assert_eq!(loaded.tools.len(), 1);
        let tool = &loaded.tools[0];
        assert!(
            tool.material.is_none(),
            "material should survive disk round-trip as None"
        );
        assert!(
            tool.flute_count.is_none(),
            "flute_count should survive disk round-trip as None"
        );
        assert!(
            tool.overall_length.is_none(),
            "overall_length should survive disk round-trip as None"
        );
    }

    #[test]
    fn import_export_preserves_none_values() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        // Add a global tool with None optional fields.
        let input = ToolInput {
            name: "None Fields".to_string(),
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
        let global_tool =
            add_global_tool_inner(input, &library, &path).expect("add should succeed");

        // Import into project — None values should be preserved.
        let imported = import_from_library_inner(&global_tool.id.to_string(), &library, &project)
            .expect("import should succeed");
        assert!(imported.material.is_none());
        assert!(imported.flute_count.is_none());
        assert!(imported.overall_length.is_none());

        // Export back to library — None values should still be preserved.
        let exported = export_to_library_inner(&imported.id.to_string(), &project, &library, &path)
            .expect("export should succeed");
        assert!(exported.material.is_none());
        assert!(exported.flute_count.is_none());
        assert!(exported.overall_length.is_none());
    }

    // ── Dirty-flag tests ─────────────────────────────────────────────────

    #[test]
    fn import_from_library_then_dirty_snapshot_shows_dirty() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let state = crate::state::AppState::default();
        let path = temp_path();

        let global_tool = add_global_tool_inner(make_input("Endmill"), &library, &path)
            .expect("add should succeed");

        import_from_library_inner(&global_tool.id.to_string(), &library, &state.project)
            .expect("import should succeed");

        {
            let mut flag = state.dirty.write().expect("write lock");
            *flag = true;
        }

        let snapshot =
            crate::commands::project::get_project_snapshot_inner(&state.project, true, true)
                .expect("snapshot should succeed");
        assert!(snapshot.dirty, "snapshot must reflect dirty = true");
    }

    #[test]
    fn export_persists_to_disk() {
        let library = RwLock::new(GlobalToolLibrary::default());
        let project = RwLock::new(Project::default());
        let path = temp_path();

        let project_tool = {
            use crate::commands::tools::add_tool_inner;
            add_tool_inner(make_input("Persisted Export"), &project).expect("add should succeed")
        };

        export_to_library_inner(&project_tool.id.to_string(), &project, &library, &path)
            .expect("export should succeed");

        let loaded = GlobalToolLibrary::load(&path);
        assert_eq!(loaded.tools.len(), 1);
        assert_eq!(loaded.tools[0].name, "Persisted Export");
    }
}
