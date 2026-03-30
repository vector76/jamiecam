//! Application state managed by Tauri.
//!
//! [`AppState`] is registered with `tauri::Builder::manage` and accessed from
//! command handlers via `tauri::State<AppState>`.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::RwLock;

use uuid::Uuid;

use crate::error::AppError;
use crate::feed_library::FeedLibrary;
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
    /// Live B-rep shape handle, present for STEP/IGES imports.
    /// `None` for STL or when OCCT bindings are unavailable.
    pub shape: Option<crate::geometry::OcctShape>,
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
    /// Generated toolpaths keyed by operation UUID.
    pub toolpaths: HashMap<Uuid, crate::toolpath::Toolpath>,
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
            toolpaths: HashMap::new(),
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

/// Global tool library persisted as a JSON file in the user's data directory.
///
/// The library contains a flat list of [`Tool`] entries shared across all
/// projects.  It is loaded at startup and saved back to disk whenever the
/// user modifies the library through IPC commands.
#[derive(Debug, Default)]
pub struct GlobalToolLibrary {
    pub tools: Vec<Tool>,
}

impl GlobalToolLibrary {
    /// Load the library from a JSON file at `path`.
    ///
    /// - If the file does not exist, returns an empty library.
    /// - If the file exists but cannot be parsed, logs a warning and returns
    ///   an empty library.
    /// - Never propagates errors to the caller.
    pub fn load(path: &std::path::Path) -> Self {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!(
                    "failed to read global tool library at {}: {e}",
                    path.display()
                );
                return Self::default();
            }
        };

        match serde_json::from_slice::<Vec<Tool>>(&bytes) {
            Ok(tools) => Self { tools },
            Err(e) => {
                tracing::warn!(
                    "failed to parse global tool library at {}: {e}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Save the library to `path` as a JSON array of tools.
    ///
    /// Parent directories are created if they do not already exist.
    pub fn save(&self, path: &std::path::Path) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json =
            serde_json::to_string_pretty(&self.tools).map_err(|e| AppError::Io(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }
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
    /// Feed/speed library loaded from the bundled TOML at startup.
    pub feed_library: FeedLibrary,
    /// Global tool library shared across all projects.
    pub global_tool_library: RwLock<GlobalToolLibrary>,
    /// Resolved path to the global tool library JSON file (immutable after init).
    pub global_library_path: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: RwLock::new(Project::default()),
            preferences: RwLock::new(UserPreferences::default()),
            feed_library: FeedLibrary::from_toml(crate::feed_library::FEEDS_TOML)
                .expect("bundled feed library must parse"),
            global_tool_library: RwLock::new(GlobalToolLibrary::default()),
            global_library_path: PathBuf::new(),
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
        assert!(project.toolpaths.is_empty());
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

    // ── GlobalToolLibrary tests ─────────────────────────────────────────

    use crate::models::tool::ToolType;

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap(),
            name: "10mm 4F Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: 90.0,
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
    fn global_tool_library_default_is_empty() {
        let lib = GlobalToolLibrary::default();
        assert!(lib.tools.is_empty());
    }

    #[test]
    fn global_tool_library_save_then_load_round_trips() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("tools.json");

        let mut lib = GlobalToolLibrary::default();
        lib.tools.push(make_tool());
        lib.save(&path).expect("save should succeed");

        let loaded = GlobalToolLibrary::load(&path);
        assert_eq!(loaded.tools.len(), 1);
        assert_eq!(loaded.tools[0], lib.tools[0]);
    }

    #[test]
    fn global_tool_library_load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("nonexistent.json");

        let lib = GlobalToolLibrary::load(&path);
        assert!(lib.tools.is_empty());
    }

    #[test]
    fn global_tool_library_load_corrupt_file_returns_empty() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("tools.json");
        std::fs::write(&path, "this is not valid json!!!").expect("write corrupt file");

        let lib = GlobalToolLibrary::load(&path);
        assert!(lib.tools.is_empty());
    }

    #[test]
    fn global_tool_library_save_writes_valid_json() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("tools.json");

        let mut lib = GlobalToolLibrary::default();
        lib.tools.push(make_tool());
        lib.save(&path).expect("save should succeed");

        let raw = std::fs::read_to_string(&path).expect("read file");
        let parsed: Vec<Tool> = serde_json::from_str(&raw).expect("should be valid JSON array");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "10mm 4F Flat Endmill");
    }

    #[test]
    fn global_tool_library_save_creates_parent_dirs() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("nested").join("deep").join("tools.json");

        let lib = GlobalToolLibrary {
            tools: vec![make_tool()],
        };
        lib.save(&path).expect("save should create parent dirs");
        assert!(path.exists());
    }
}
