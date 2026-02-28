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

use super::project::ProjectSnapshot;
use super::{read_project, write_project};

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
}
