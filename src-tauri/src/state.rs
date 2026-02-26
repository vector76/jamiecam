//! Application state managed by Tauri.
//!
//! [`AppState`] is registered with `tauri::Builder::manage` and accessed from
//! command handlers via `tauri::State<AppState>`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::geometry::MeshData;
use crate::models::{Operation, StockDefinition, Tool, WorkCoordinateSystem};

/// A geometry model that has been loaded into memory.
#[derive(Debug)]
pub struct LoadedModel {
    /// Absolute path to the source file on disk.
    pub path: PathBuf,
    /// SHA-256 hex digest of the file at load time (for cache validation).
    pub checksum: String,
    /// Tessellated mesh ready for rendering.
    pub mesh_data: MeshData,
}

/// The active project document.
///
/// All optional/vec scaffolding fields (stock, wcs, tools, operations) are
/// present as typed placeholders so later phases can populate them without
/// changing the struct layout.
#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub description: String,
    /// Unit system in use (e.g. `"mm"` or `"inch"`).
    pub units: String,
    /// Monotonically increasing schema version; starts at 1.
    pub schema_version: u32,
    /// ISO-8601 creation timestamp (empty string when not yet persisted).
    pub created_at: String,
    /// ISO-8601 last-modified timestamp (empty string when not yet persisted).
    pub modified_at: String,
    /// The geometry model currently loaded, if any.
    pub source_model: Option<LoadedModel>,
    // ── Scaffolding — remaining types will be replaced in later beads ────────
    /// Stock solid definition.
    pub stock: Option<StockDefinition>,
    /// Work coordinate systems.
    pub wcs: Vec<WorkCoordinateSystem>,
    /// Tool library entries.
    pub tools: Vec<Tool>,
    /// Machining operations.
    pub operations: Vec<Operation>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            units: "mm".to_string(),
            schema_version: 1,
            created_at: String::new(),
            modified_at: String::new(),
            source_model: None,
            stock: None,
            wcs: Vec::new(),
            tools: Vec::new(),
            operations: Vec::new(),
        }
    }
}

/// In-memory user preferences.
///
/// Phase 0: no disk persistence.  The list is rebuilt from scratch each
/// session.  A persistence layer will be added in a future phase.
#[derive(Default)]
pub struct UserPreferences {
    /// Most-recently-used file paths, newest first.
    pub recent_files: VecDeque<PathBuf>,
}

/// Root application state managed by Tauri.
///
/// Both fields are wrapped in [`RwLock`] so that multiple concurrent read
/// commands (e.g. "get current project name" alongside "list recent files")
/// do not block each other.
pub struct AppState {
    /// The active project, guarded for concurrent read access.
    pub project: RwLock<Project>,
    /// User preferences, guarded for concurrent read access.
    pub preferences: RwLock<UserPreferences>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: RwLock::new(Project::default()),
            preferences: RwLock::new(UserPreferences::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_default_constructs_without_panic() {
        let state = AppState::default();
        // Both locks should be accessible immediately after construction.
        let _project = state.project.read().expect("read project lock");
        let _prefs = state.preferences.read().expect("read preferences lock");
    }

    #[test]
    fn project_default_schema_version_is_one() {
        let project = Project::default();
        assert_eq!(project.schema_version, 1);
    }

    #[test]
    fn project_default_units_are_mm() {
        let project = Project::default();
        assert_eq!(project.units, "mm");
    }

    #[test]
    fn project_default_has_no_source_model() {
        let project = Project::default();
        assert!(project.source_model.is_none());
    }

    #[test]
    fn project_default_scaffolding_fields_are_empty() {
        let project = Project::default();
        assert!(project.stock.is_none());
        assert!(project.wcs.is_empty());
        assert!(project.tools.is_empty());
        assert!(project.operations.is_empty());
    }

    #[test]
    fn user_preferences_default_has_empty_recent_files() {
        let prefs = UserPreferences::default();
        assert!(prefs.recent_files.is_empty());
    }

    #[test]
    fn app_state_project_lock_allows_write() {
        let state = AppState::default();
        {
            let mut project = state.project.write().expect("write project lock");
            project.name = "Test Project".to_string();
        }
        let project = state.project.read().expect("read project lock");
        assert_eq!(project.name, "Test Project");
    }
}
