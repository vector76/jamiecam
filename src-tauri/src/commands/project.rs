//! Project state query commands.
//!
//! [`get_project_snapshot`] returns a lightweight view of the current project
//! for display in the frontend toolbar / title bar. It acquires only a read
//! lock and is safe to call concurrently with other read commands.

use std::sync::RwLock;

use serde::Serialize;

use crate::error::AppError;
use crate::state::{AppState, Project};

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
}

impl From<&Project> for ProjectSnapshot {
    fn from(p: &Project) -> Self {
        Self {
            model_path: p
                .source_model
                .as_ref()
                .map(|m| m.path.to_string_lossy().into_owned()),
            model_checksum: p.source_model.as_ref().map(|m| m.checksum.clone()),
            project_name: p.name.clone(),
            modified_at: p.modified_at.clone(),
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
    use crate::state::AppState;

    #[test]
    fn snapshot_of_default_project_has_no_model() {
        let state = AppState::default();
        let snap =
            get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
        assert!(snap.model_path.is_none());
        assert!(snap.model_checksum.is_none());
        assert_eq!(snap.project_name, "");
        assert_eq!(snap.modified_at, "");
    }

    #[test]
    fn snapshot_reflects_project_name_and_modified_at() {
        let state = AppState::default();
        {
            let mut p = state.project.write().expect("write lock");
            p.name = "My Project".to_string();
            p.modified_at = "2026-01-01T00:00:00Z".to_string();
        }
        let snap =
            get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
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
        let snap =
            get_project_snapshot_inner(&state.project).expect("snapshot should not fail");
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
        };
        let value = serde_json::to_value(&snap).expect("serialize");
        assert!(value.get("modelPath").is_some(), "expected camelCase modelPath");
        assert!(value.get("modelChecksum").is_some(), "expected camelCase modelChecksum");
        assert!(value.get("projectName").is_some(), "expected camelCase projectName");
        assert!(value.get("modifiedAt").is_some(), "expected camelCase modifiedAt");
    }
}
