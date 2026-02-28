//! File and project lifecycle command handlers.
//!
//! All handlers follow the pattern of an `_inner` function (testable without
//! Tauri) wrapped by the `#[tauri::command]` entry point that extracts the
//! managed state.
//!
//! # Error contract
//! Every fallible path returns `Result<_, AppError>`. No `unwrap()` or
//! `expect()` calls are present outside of `#[cfg(test)]`.

use std::path::PathBuf;
use std::sync::RwLock;

use sha2::Digest as _;

use crate::error::AppError;
use crate::geometry::MeshData;
use crate::state::{AppState, LoadedModel, Project};

use crate::postprocessor::{program::GenerateOptions, PostProcessor};

use super::project::ProjectSnapshot;
use super::{build_tool_infos, parse_entity_id, read_project, write_project};

// ── open_model ────────────────────────────────────────────────────────────────

/// Testable inner logic for [`open_model`].
///
/// 1. Returns [`AppError::FileNotFound`] if `path_str` does not exist.
/// 2. Offloads tessellation + checksum computation to a blocking thread pool.
/// 3. Stores the resulting [`LoadedModel`] in `project_lock`.
/// 4. Returns the [`MeshData`] for the frontend to render.
pub(crate) async fn open_model_inner(
    path_str: &str,
    project_lock: &RwLock<Project>,
) -> Result<MeshData, AppError> {
    let path_buf = PathBuf::from(path_str);

    if !path_buf.exists() {
        return Err(AppError::FileNotFound);
    }

    // Tessellation is CPU-bound; run it on the blocking thread pool so the
    // async runtime is not starved.
    let path_clone = path_buf.clone();
    let blocking_result = tokio::task::spawn_blocking(move || {
        let mesh = crate::geometry::import(&path_clone).map_err(AppError::from)?;
        let bytes = std::fs::read(&path_clone).map_err(|e| AppError::Io(e.to_string()))?;
        let digest = sha2::Sha256::digest(&bytes);
        Ok::<(MeshData, String), AppError>((mesh, format!("{digest:x}")))
    })
    .await
    .map_err(|e| AppError::GeometryImport(format!("import task panicked: {e}")))?;

    let (mesh, checksum) = blocking_result?;

    let mut project = write_project(project_lock)?;
    project.source_model = Some(LoadedModel {
        path: path_buf,
        checksum,
        mesh_data: mesh.clone(),
    });

    Ok(mesh)
}

// ── save_project ──────────────────────────────────────────────────────────────

/// Testable inner logic for [`save_project`].
///
/// Updates `modified_at` (and `created_at` on first save) to the current UTC
/// time, then serialises the project to `path_str`.
pub(crate) fn save_project_inner(
    path_str: &str,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let path_buf = PathBuf::from(path_str);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    {
        let mut project = write_project(project_lock)?;
        if project.created_at.is_empty() {
            project.created_at = now.clone();
        }
        project.modified_at = now;
    }

    let project = read_project(project_lock)?;
    crate::project::serialization::save(&project, &path_buf)
}

// ── load_project ──────────────────────────────────────────────────────────────

/// Testable inner logic for [`load_project`].
///
/// Loads the `.jcam` file, replaces the active project in `project_lock`, and
/// returns a [`ProjectSnapshot`] for immediate display.
pub(crate) fn load_project_inner(
    path_str: &str,
    project_lock: &RwLock<Project>,
) -> Result<ProjectSnapshot, AppError> {
    let path_buf = PathBuf::from(path_str);
    let new_project = crate::project::serialization::load(&path_buf)?;
    let snapshot = ProjectSnapshot::from(&new_project);
    let mut project = write_project(project_lock)?;
    *project = new_project;
    Ok(snapshot)
}

// ── new_project ───────────────────────────────────────────────────────────────

/// Testable inner logic for [`new_project`].
///
/// Replaces the active project with [`Project::default()`] and returns a
/// [`ProjectSnapshot`] for immediate display.
pub(crate) fn new_project_inner(
    project_lock: &RwLock<Project>,
) -> Result<ProjectSnapshot, AppError> {
    let new_project = Project::default();
    let snapshot = ProjectSnapshot::from(&new_project);
    let mut project = write_project(project_lock)?;
    *project = new_project;
    Ok(snapshot)
}

// ── Tauri command wrappers ────────────────────────────────────────────────────

/// Open a 3D model file, tessellate it, and store it in the active project.
///
/// Tessellation is offloaded to a blocking thread pool because it is
/// CPU-bound. Returns the [`MeshData`] so the frontend can begin rendering
/// immediately.
#[tauri::command]
pub async fn open_model(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<MeshData, AppError> {
    open_model_inner(&path, &state.project).await
}

/// Serialize the active project to a `.jcam` file at `path`.
#[tauri::command]
pub async fn save_project(path: String, state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    save_project_inner(&path, &state.project)
}

/// Load a `.jcam` file and replace the active project.
///
/// Returns a [`ProjectSnapshot`] for immediate display in the frontend.
#[tauri::command]
pub async fn load_project(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<ProjectSnapshot, AppError> {
    load_project_inner(&path, &state.project)
}

/// Reset the active project to a fresh default state.
///
/// Returns a [`ProjectSnapshot`] for immediate display in the frontend.
#[tauri::command]
pub async fn new_project(state: tauri::State<'_, AppState>) -> Result<ProjectSnapshot, AppError> {
    new_project_inner(&state.project)
}

// ── export_gcode ──────────────────────────────────────────────────────────────

/// Input parameters for [`export_gcode`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportParams {
    pub operation_ids: Vec<String>,
    pub post_processor_id: String,
    pub output_path: String,
    pub program_number: Option<u32>,
    pub include_comments: bool,
}

/// Testable inner logic for [`export_gcode`].
///
/// 1. Parses all operation UUIDs.
/// 2. Verifies each operation exists in the project.
/// 3. Looks up each toolpath by operation UUID.
/// 4. Builds [`crate::postprocessor::ToolInfo`] from matching operations and tools.
/// 5. Loads the named builtin post-processor.
/// 6. Generates G-code and writes it to `params.output_path`.
pub(crate) fn export_gcode_inner(
    params: ExportParams,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let op_uuids = params
        .operation_ids
        .iter()
        .map(|id| parse_entity_id(id, "operation"))
        .collect::<Result<Vec<_>, _>>()?;

    let (toolpaths, tool_infos) = {
        let project = read_project(project_lock)?;

        let mut toolpaths = Vec::new();
        for op_uuid in &op_uuids {
            if !project.operations.iter().any(|op| op.id == *op_uuid) {
                return Err(AppError::NotFound(format!("operation {op_uuid} not found")));
            }
            let toolpath = project
                .toolpaths
                .get(op_uuid)
                .ok_or_else(|| AppError::NotFound(format!("no toolpath for operation {op_uuid}")))?
                .clone();
            toolpaths.push(toolpath);
        }

        let tool_infos = build_tool_infos(&toolpaths, &project);

        (toolpaths, tool_infos)
    }; // read lock released here

    let pp = PostProcessor::builtin(&params.post_processor_id)
        .map_err(|e| AppError::PostProcessor(e.to_string()))?;

    let gcode = pp
        .generate(
            &toolpaths,
            &tool_infos,
            GenerateOptions {
                program_number: params.program_number,
                include_comments: params.include_comments,
            },
        )
        .map_err(|e| AppError::PostProcessor(e.to_string()))?;

    std::fs::write(&params.output_path, gcode).map_err(AppError::from)?;

    Ok(())
}

/// Generate G-code for the given operations and write it to the output path.
#[tauri::command]
pub async fn export_gcode(
    params: ExportParams,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    export_gcode_inner(params, &state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    // ── new_project ───────────────────────────────────────────────────────

    #[test]
    fn new_project_resets_state_to_default() {
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.name = "Old Project".to_string();
        }
        let snap = new_project_inner(&state.project).expect("new_project should succeed");
        assert_eq!(snap.project_name, "");
        assert!(snap.model_path.is_none());
        let project = state.project.read().expect("read lock");
        assert_eq!(project.schema_version, 1);
        assert_eq!(project.units, "mm");
        assert!(project.source_model.is_none());
    }

    // ── save_project / load_project ───────────────────────────────────────

    #[test]
    fn save_and_load_project_round_trip() {
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.name = "Round Trip".to_string();
            // Leave created_at and modified_at empty — save_project_inner must fill them.
        }

        let tmp = std::env::temp_dir().join("jcam_cmd_test_round_trip.jcam");
        save_project_inner(&tmp.to_string_lossy(), &state.project).expect("save should succeed");

        // After save, both timestamps must be non-empty ISO-8601 strings.
        {
            let p = state.project.read().expect("read lock");
            assert!(
                !p.created_at.is_empty(),
                "created_at must be set after first save"
            );
            assert!(
                !p.modified_at.is_empty(),
                "modified_at must be set after save"
            );
        }

        // Reset state, then load the saved file.
        new_project_inner(&state.project).expect("new_project should succeed");

        let snap = load_project_inner(&tmp.to_string_lossy(), &state.project)
            .expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(snap.project_name, "Round Trip");
        let project = state.project.read().expect("read lock");
        assert_eq!(project.name, "Round Trip");
        assert_eq!(project.schema_version, 1);
        assert!(
            !project.created_at.is_empty(),
            "created_at must survive round-trip"
        );
        assert!(
            !project.modified_at.is_empty(),
            "modified_at must survive round-trip"
        );
    }

    #[test]
    fn save_preserves_created_at_on_subsequent_saves() {
        let state = AppState::default();
        let tmp = std::env::temp_dir().join("jcam_cmd_test_created_at.jcam");

        // First save: sets created_at.
        save_project_inner(&tmp.to_string_lossy(), &state.project).expect("first save");
        let created_at_1 = state.project.read().expect("read").created_at.clone();
        assert!(!created_at_1.is_empty());

        // Second save: created_at must not change; modified_at may change.
        save_project_inner(&tmp.to_string_lossy(), &state.project).expect("second save");
        let _ = std::fs::remove_file(&tmp);
        let created_at_2 = state.project.read().expect("read").created_at.clone();

        assert_eq!(
            created_at_1, created_at_2,
            "created_at must not change on re-save"
        );
    }

    #[test]
    fn load_project_returns_err_for_missing_file() {
        let state = AppState::default();
        let result = load_project_inner("/nonexistent/path/project.jcam", &state.project);
        assert!(matches!(result, Err(AppError::ProjectLoad(_))));
    }

    #[test]
    fn save_project_to_invalid_path_returns_err() {
        let state = AppState::default();
        let result = save_project_inner("/nonexistent_dir_jamiecam/project.jcam", &state.project);
        assert!(matches!(result, Err(AppError::ProjectSave(_))));
    }

    // ── open_model ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn open_model_returns_file_not_found_for_missing_path() {
        let state = AppState::default();
        let result = open_model_inner("/nonexistent/path/model.step", &state.project).await;
        assert!(matches!(result, Err(AppError::FileNotFound)));
    }

    /// Without OCCT bindings, importing an existing STEP file fails with
    /// GeometryImport because the C++ backend is unavailable.
    #[tokio::test]
    #[cfg(not(cam_geometry_bindings))]
    async fn open_model_returns_geometry_error_without_occt() {
        let fixture = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step",
        ));
        if !fixture.exists() {
            return; // fixture absent in this environment — skip
        }
        let state = AppState::default();
        let result = open_model_inner(&fixture.to_string_lossy(), &state.project).await;
        assert!(
            matches!(result, Err(AppError::GeometryImport(_))),
            "expected GeometryImport, got: {result:?}",
        );
        // State must not be modified on failure.
        let project = state.project.read().expect("read lock");
        assert!(project.source_model.is_none());
    }

    /// With full OCCT bindings, importing box.step must return a non-empty
    /// mesh and store the model + checksum in state.
    #[tokio::test]
    #[cfg(cam_geometry_bindings)]
    async fn open_model_with_occt_stores_mesh_and_returns_nonempty() {
        let fixture = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step",
        ));
        let state = AppState::default();
        let mesh = open_model_inner(&fixture.to_string_lossy(), &state.project)
            .await
            .expect("open_model should succeed with OCCT");
        assert!(!mesh.vertices.is_empty(), "vertices must not be empty");
        assert_eq!(
            mesh.vertices.len(),
            mesh.normals.len(),
            "vertices and normals must have equal length"
        );
        let project = state.project.read().expect("read lock");
        let model = project
            .source_model
            .as_ref()
            .expect("source_model must be set");
        assert!(!model.checksum.is_empty());
        assert_eq!(model.path, fixture);
    }

    // ── get_project_snapshot (cross-module) ───────────────────────────────

    #[test]
    fn get_project_snapshot_reflects_updated_name() {
        use super::super::project::get_project_snapshot_inner;
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.name = "Snapshot Test".to_string();
        }
        let snap = get_project_snapshot_inner(&state.project).expect("should succeed");
        assert_eq!(snap.project_name, "Snapshot Test");
    }

    // ── export_gcode ──────────────────────────────────────────────────────

    fn make_export_state() -> (AppState, uuid::Uuid) {
        use crate::models::{
            operation::{OperationParams, PocketParams},
            tool::ToolType,
            Operation, Tool, Vec3,
        };
        use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};
        use crate::toolpath::Toolpath;
        use uuid::Uuid;

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

        (state, op_id)
    }

    #[test]
    fn export_gcode_inner_writes_file_to_temp_path() {
        let (state, op_id) = make_export_state();
        let tmp = std::env::temp_dir().join("jcam_export_gcode_test.nc");
        let params = ExportParams {
            operation_ids: vec![op_id.to_string()],
            post_processor_id: "fanuc-0i".to_string(),
            output_path: tmp.to_string_lossy().to_string(),
            program_number: Some(1),
            include_comments: true,
        };

        export_gcode_inner(params, &state.project).expect("export should succeed");

        assert!(tmp.exists(), "output file must exist after export");
        let content = std::fs::read_to_string(&tmp).expect("read output file");
        assert!(!content.is_empty(), "output file must not be empty");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn export_gcode_inner_returns_not_found_when_toolpath_absent() {
        use crate::models::{
            operation::{OperationParams, PocketParams},
            Operation,
        };
        use uuid::Uuid;

        let state = AppState::default();
        let op_id = Uuid::new_v4();

        let operation = Operation {
            id: op_id,
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id: Uuid::new_v4(),
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
            }),
        };

        {
            let mut project = state.project.write().expect("write lock");
            project.operations.push(operation);
            // toolpath intentionally NOT inserted
        }

        let params = ExportParams {
            operation_ids: vec![op_id.to_string()],
            post_processor_id: "fanuc-0i".to_string(),
            output_path: "/tmp/should_not_be_created.nc".to_string(),
            program_number: None,
            include_comments: false,
        };

        let result = export_gcode_inner(params, &state.project);
        assert!(
            matches!(result, Err(AppError::NotFound(_))),
            "expected NotFound, got: {result:?}"
        );
    }

    #[test]
    fn export_gcode_inner_returns_io_error_for_unwritable_path() {
        let (state, op_id) = make_export_state();
        let params = ExportParams {
            operation_ids: vec![op_id.to_string()],
            post_processor_id: "fanuc-0i".to_string(),
            output_path: "/nonexistent_dir_jamiecam/output.nc".to_string(),
            program_number: None,
            include_comments: false,
        };

        let result = export_gcode_inner(params, &state.project);
        assert!(
            matches!(result, Err(AppError::Io(_))),
            "expected Io error, got: {result:?}"
        );
    }
}
