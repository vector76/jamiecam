//! Atomic save and validated load for the `.jcam` project file format.
//!
//! # Save
//! 1. Build [`ProjectFile`] from the in-memory [`Project`].
//! 2. Write a complete ZIP archive to `<target>.tmp` (same directory → same
//!    filesystem as the final path).
//! 3. Atomically rename the temp file over the target.
//!
//! On any failure the temp file is deleted and the original is left intact.
//!
//! # Load
//! 1. Open the ZIP and read `project.json`.
//! 2. Validate `schema_version` is 1 or 2; reject anything else with a clear error.
//! 3. Reconstruct the in-memory [`Project`].  [`LoadedModel::mesh_data`] is
//!    initialised empty — the IPC `open_model` command re-tessellates when the
//!    viewport needs geometry.

use std::io::{Read, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use super::types::{ProjectFile, ProjectMeta, SourceModelRef};
use crate::error::AppError;
use crate::geometry::MeshData;
use crate::state::{LoadedModel, Project};

/// Name of the project manifest inside every `.jcam` ZIP.
const PROJECT_JSON: &str = "project.json";

/// JamieCam version embedded in every saved file.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Save `project` to a `.jcam` file at `path` using an atomic write.
///
/// The ZIP is written to `<path>.tmp` in the same directory (guaranteeing
/// same-filesystem placement), then renamed over `path`.  On any error the
/// temp file is removed and `path` is left unchanged.
pub fn save(project: &Project, path: &Path) -> Result<(), AppError> {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let tmp_path = path.with_file_name(format!("{file_name}.tmp"));

    if let Err(e) = write_archive(project, &tmp_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        AppError::ProjectSave(format!("rename to final path failed: {e}"))
    })
}

/// Load a `.jcam` file from `path` and return the reconstructed [`Project`].
///
/// Returns [`AppError::ProjectLoad`] if the file cannot be read, is not a
/// valid ZIP, contains no `project.json`, or has an unsupported
/// `schema_version`.
pub fn load(path: &Path) -> Result<Project, AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::ProjectLoad(format!("cannot open file: {e}")))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::ProjectLoad(format!("not a valid ZIP archive: {e}")))?;

    // Read project.json inside a block so the borrow on `archive` is released
    // before we might need it again (e.g. for embedded model extraction later).
    let json_str = {
        let mut entry = archive.by_name(PROJECT_JSON).map_err(|e| {
            AppError::ProjectLoad(format!("{PROJECT_JSON} not found in archive: {e}"))
        })?;
        let mut s = String::new();
        entry
            .read_to_string(&mut s)
            .map_err(|e| AppError::ProjectLoad(format!("cannot read {PROJECT_JSON}: {e}")))?;
        s
    };

    let pf: ProjectFile = serde_json::from_str(&json_str)
        .map_err(|e| AppError::ProjectLoad(format!("cannot parse {PROJECT_JSON}: {e}")))?;

    if pf.schema_version != 1 && pf.schema_version != 2 {
        return Err(AppError::ProjectLoad(format!(
            "unsupported schema version {}; only schema versions 1 and 2 are supported",
            pf.schema_version
        )));
    }

    let source_model = pf.source_model.map(|r| LoadedModel {
        path: std::path::PathBuf::from(&r.path),
        checksum: r.checksum,
        // Mesh data is not persisted in the project file.  The IPC
        // `open_model` command re-tessellates the geometry when needed.
        mesh_data: MeshData {
            vertices: vec![],
            normals: vec![],
            indices: vec![],
            face_groups: vec![],
        },
        // Shape handle is not persisted; must be re-loaded via open_model.
        shape: None,
    });

    let mut project = Project {
        name: pf.project.name,
        description: pf.project.description,
        units: pf.project.units,
        schema_version: pf.schema_version,
        created_at: pf.created_at,
        modified_at: pf.modified_at,
        source_model,
        stock: pf.stock,
        wcs: pf.wcs,
        tools: pf
            .tools
            .into_iter()
            .map(|mut t| {
                t.resolve_defaults();
                t
            })
            .collect(),
        operations: pf.operations,
        toolpaths: std::collections::HashMap::new(),
        mode: pf.mode,
        source_2d_artwork: pf.source_2d_artwork,
        safe_height: pf.safe_height,
        artwork_origin: pf.artwork_origin,
        file_path: None,
    };

    // Restore persisted toolpaths for operations that have a binary_file reference.
    for op in &project.operations {
        if let Some(ref path) = op.cache.binary_file {
            match archive.by_name(path) {
                Ok(mut entry) => {
                    let mut s = String::new();
                    match entry.read_to_string(&mut s) {
                        Ok(_) => match serde_json::from_str::<crate::toolpath::Toolpath>(&s) {
                            Ok(tp) => {
                                project.toolpaths.insert(op.id, tp);
                            }
                            Err(e) => {
                                tracing::warn!("failed to parse toolpath {path}: {e}");
                            }
                        },
                        Err(e) => {
                            tracing::warn!("failed to read toolpath entry {path}: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("toolpath entry {path} missing: {e}");
                }
            }
        }
    }

    Ok(project)
}

/// Write the ZIP archive to `path` (the temp file location).
///
/// Separated from [`save`] so that cleanup on error is handled entirely by
/// the caller.
fn write_archive(project: &Project, path: &Path) -> Result<(), AppError> {
    let file = std::fs::File::create(path)
        .map_err(|e| AppError::ProjectSave(format!("cannot create temp file: {e}")))?;

    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // Build the on-disk SourceModelRef from the in-memory LoadedModel.
    // Phase 0: embedding is always false; the toggle is added in a later phase.
    let source_model_ref = project.source_model.as_ref().map(|m| SourceModelRef {
        path: m.path.to_string_lossy().into_owned(),
        checksum: m.checksum.clone(),
        embedded: false,
    });

    // Write toolpath ZIP entries for operations with a valid cache, and record
    // the in-archive path in each operation's cache.binary_file so the
    // serialised project.json carries the correct path.
    let mut ops = project.operations.clone();
    for op in &mut ops {
        if op.cache.valid {
            if let Some(toolpath) = project.toolpaths.get(&op.id) {
                let entry_path = format!("toolpaths/{}.json", op.id);
                let tp_json = serde_json::to_string(toolpath).map_err(|e| {
                    AppError::ProjectSave(format!("cannot serialize toolpath: {e}"))
                })?;
                zip.start_file(&entry_path, opts).map_err(|e| {
                    AppError::ProjectSave(format!("cannot create toolpath ZIP entry: {e}"))
                })?;
                zip.write_all(tp_json.as_bytes()).map_err(|e| {
                    AppError::ProjectSave(format!("cannot write toolpath entry: {e}"))
                })?;
                op.cache.binary_file = Some(entry_path);
            }
        }
    }

    let pf = ProjectFile {
        schema_version: 2,
        app_version: APP_VERSION.to_string(),
        created_at: project.created_at.clone(),
        modified_at: project.modified_at.clone(),
        mode: project.mode.clone(),
        project: ProjectMeta {
            name: project.name.clone(),
            description: project.description.clone(),
            units: project.units.clone(),
        },
        source_model: source_model_ref.clone(),
        stock: project.stock.clone(),
        wcs: project.wcs.clone(),
        tools: project.tools.clone(),
        operations: ops,
        source_2d_artwork: project.source_2d_artwork.clone(),
        safe_height: project.safe_height,
        artwork_origin: project.artwork_origin,
    };

    // Serialize and write project.json.
    let json = serde_json::to_string_pretty(&pf)
        .map_err(|e| AppError::ProjectSave(format!("cannot serialize project: {e}")))?;

    zip.start_file(PROJECT_JSON, opts)
        .map_err(|e| AppError::ProjectSave(format!("cannot create {PROJECT_JSON} entry: {e}")))?;
    zip.write_all(json.as_bytes())
        .map_err(|e| AppError::ProjectSave(format!("cannot write {PROJECT_JSON}: {e}")))?;

    // Embed model if requested (Phase 0: embedded is always false, so this
    // branch never executes — it is here for correctness when the toggle is
    // wired up in a later bead).
    if let Some(model_ref) = &source_model_ref {
        if model_ref.embedded {
            if let Some(loaded) = &project.source_model {
                let ext = loaded
                    .path
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                let entry_name = format!("model/source{ext}");

                let model_bytes = std::fs::read(&loaded.path).map_err(|e| {
                    AppError::ProjectSave(format!("cannot read model file for embedding: {e}"))
                })?;

                zip.start_file(&entry_name, opts).map_err(|e| {
                    AppError::ProjectSave(format!("cannot create model ZIP entry: {e}"))
                })?;
                zip.write_all(&model_bytes).map_err(|e| {
                    AppError::ProjectSave(format!("cannot write embedded model: {e}"))
                })?;
            }
        }
    }

    zip.finish()
        .map_err(|e| AppError::ProjectSave(format!("cannot finalize ZIP: {e}")))?;

    Ok(())
}

#[cfg(test)]
// Test fixtures deliberately use `let mut x = T::default(); x.field = ...;`
// for readability when only a couple of fields differ from defaults.
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::models::{Tool, ToolType};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap(),
            name: "10mm 4F Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: Some(90.0),
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

    fn make_project_with_model() -> Project {
        let mut p = Project::default();
        p.name = "Test Project".to_string();
        p.description = "A test description".to_string();
        p.created_at = "2026-01-01T00:00:00Z".to_string();
        p.modified_at = "2026-01-02T12:00:00Z".to_string();
        p.source_model = Some(LoadedModel {
            path: PathBuf::from("/home/user/model.step"),
            checksum: "abc123def456abc123def456abc123def456abc123def456abc123def456abc1"
                .to_string(),
            mesh_data: MeshData {
                vertices: vec![],
                normals: vec![],
                indices: vec![],
                face_groups: vec![],
            },
            shape: None,
        });
        p
    }

    #[test]
    fn round_trip_with_model() {
        let project = make_project_with_model();
        let tmp = std::env::temp_dir().join("jcam_test_round_trip_model.jcam");

        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.name, project.name);
        assert_eq!(loaded.description, project.description);
        assert_eq!(loaded.units, project.units);
        assert_eq!(loaded.schema_version, 2);
        assert_eq!(loaded.created_at, project.created_at);
        assert_eq!(loaded.modified_at, project.modified_at);

        let orig = project.source_model.as_ref().unwrap();
        let got = loaded
            .source_model
            .as_ref()
            .expect("source_model should survive round-trip");
        assert_eq!(got.path, orig.path);
        assert_eq!(got.checksum, orig.checksum);
    }

    #[test]
    fn round_trip_no_model() {
        let project = Project::default();
        let tmp = std::env::temp_dir().join("jcam_test_round_trip_empty.jcam");

        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.schema_version, 2);
        assert_eq!(loaded.units, "mm");
        assert!(loaded.source_model.is_none());
    }

    #[test]
    fn load_rejects_unknown_schema_version() {
        let tmp = std::env::temp_dir().join("jcam_test_bad_schema.jcam");

        // Write a minimal ZIP with schema_version = 99.
        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("project.json", opts).unwrap();
            let json = r#"{
                "schema_version": 99,
                "app_version": "0.1.0",
                "created_at": "",
                "modified_at": "",
                "project": { "name": "", "description": "", "units": "mm" }
            }"#;
            zip.write_all(json.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        match result.expect_err("should fail for schema_version 99") {
            AppError::ProjectLoad(msg) => {
                assert!(
                    msg.to_lowercase().contains("schema"),
                    "error message should mention 'schema', got: {msg}"
                );
            }
            other => panic!("expected AppError::ProjectLoad, got {other:?}"),
        }
    }

    #[test]
    fn load_fails_gracefully_on_missing_file() {
        let result = load(Path::new("/nonexistent/path/project.jcam"));
        assert!(matches!(result, Err(AppError::ProjectLoad(_))));
    }

    #[test]
    fn save_creates_valid_zip() {
        let project = make_project_with_model();
        let tmp = std::env::temp_dir().join("jcam_test_zip_valid.jcam");

        save(&project, &tmp).expect("save should succeed");

        // Verify the file is a readable ZIP containing project.json.
        let file = std::fs::File::open(&tmp).unwrap();
        let mut archive = zip::ZipArchive::new(file).expect("should be a valid ZIP");
        assert!(
            archive.by_name("project.json").is_ok(),
            "project.json must be present in the archive"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn round_trip_project_with_tool() {
        let mut project = Project::default();
        project.name = "Tool Round-Trip Test".to_string();
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-02T00:00:00Z".to_string();
        let tool = make_tool();
        project.tools.push(tool.clone());

        let tmp = std::env::temp_dir().join("jcam_test_round_trip_tool.jcam");
        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.tools.len(), 1);
        let rt = &loaded.tools[0];
        assert_eq!(rt.id, tool.id);
        assert_eq!(rt.name, tool.name);
        assert_eq!(rt.tool_type, tool.tool_type);
        assert_eq!(rt.material, tool.material);
        assert_eq!(rt.diameter, tool.diameter);
        assert_eq!(rt.flute_count, tool.flute_count);
        assert_eq!(rt.default_spindle_speed, tool.default_spindle_speed);
        assert_eq!(rt.default_feed_rate, tool.default_feed_rate);
    }

    #[test]
    fn load_phase0_schema_without_tools_field_succeeds() {
        // A Phase 0 .jcam archive that has no "tools" key in project.json.
        // Because ProjectFile uses #[serde(default)] the field should default
        // to an empty vec and load without error.
        let tmp = std::env::temp_dir().join("jcam_test_phase0_compat.jcam");

        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("project.json", opts).unwrap();
            let json = r#"{
                "schema_version": 1,
                "app_version": "0.1.0",
                "created_at": "2026-01-01T00:00:00Z",
                "modified_at": "2026-01-01T00:00:00Z",
                "project": { "name": "Phase0 Project", "description": "", "units": "mm" }
            }"#;
            zip.write_all(json.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        let project = result.expect("Phase 0 schema without tools should load successfully");
        assert!(
            project.tools.is_empty(),
            "tools should default to empty vec"
        );
        assert!(project.stock.is_none(), "stock should default to None");
        assert!(project.wcs.is_empty(), "wcs should default to empty vec");
        assert!(
            project.operations.is_empty(),
            "operations should default to empty vec"
        );
        assert_eq!(project.name, "Phase0 Project");
    }

    #[test]
    fn round_trip_project_with_stock_and_wcs() {
        use crate::models::stock::{BoxDimensions, Vec3};
        use crate::models::{StockDefinition, WorkCoordinateSystem};

        let mut project = Project::default();
        project.name = "Stock/WCS Round-Trip Test".to_string();
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-02T00:00:00Z".to_string();

        project.stock = Some(StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: -5.0,
                y: -5.0,
                z: -2.0,
            },
            width: 120.0,
            depth: 80.0,
            height: 30.0,
        }));

        let wcs_id = Uuid::parse_str("3f8a2b00-0000-0000-0000-000000000001").unwrap();
        project.wcs.push(WorkCoordinateSystem {
            id: wcs_id,
            name: "G54".to_string(),
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            x_axis: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            z_axis: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        });

        let tmp = std::env::temp_dir().join("jcam_test_round_trip_stock_wcs.jcam");
        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        // Stock round-trip
        let stock = loaded.stock.expect("stock should survive round-trip");
        let StockDefinition::Box(b) = stock;
        assert_eq!(b.width, 120.0);
        assert_eq!(b.depth, 80.0);
        assert_eq!(b.height, 30.0);
        assert_eq!(b.origin.x, -5.0);

        // WCS round-trip
        assert_eq!(loaded.wcs.len(), 1);
        assert_eq!(loaded.wcs[0].id, wcs_id);
        assert_eq!(loaded.wcs[0].name, "G54");
    }

    #[test]
    fn toolpath_entry_written_to_zip() {
        use crate::models::operation::{CacheState, DrillParams, OperationParams};
        use crate::models::Operation;
        use crate::toolpath::Toolpath;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();
        let op_id = Uuid::parse_str("dddd0000-0000-0000-0000-000000000004").unwrap();

        let op = Operation {
            id: op_id,
            name: "Test Drill".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: 10.0,
                points: vec![],
                peck_depth: None,
            }),
            cache: CacheState {
                key: Some("sha256:abc123".to_string()),
                valid: true,
                computed_at: Some("2026-03-01T00:00:00Z".to_string()),
                stats: None,
                binary_file: None,
            },
        };

        let toolpath = Toolpath {
            operation_id: op_id,
            tool_number: 1,
            spindle_speed: 12000.0,
            feed_rate: 500.0,
            passes: vec![],
        };

        let mut project = Project::default();
        project.operations.push(op);
        project.toolpaths.insert(op_id, toolpath);

        let tmp = std::env::temp_dir().join("jcam_test_toolpath_entry.jcam");
        save(&project, &tmp).expect("save should succeed");

        let expected_entry = format!("toolpaths/{op_id}.json");

        // Verify the toolpath entry exists in the ZIP.
        {
            let file = std::fs::File::open(&tmp).unwrap();
            let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
            assert!(
                archive.by_name(&expected_entry).is_ok(),
                "toolpath ZIP entry {expected_entry} must exist"
            );
        }

        // Verify binary_file path appears in the deserialized project.json.
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.operations.len(), 1);
        assert_eq!(
            loaded.operations[0].cache.binary_file.as_deref(),
            Some(expected_entry.as_str()),
            "binary_file must be set in the persisted operation"
        );
    }

    #[test]
    fn invalid_cache_not_written_to_zip() {
        use crate::models::operation::{CacheState, DrillParams, OperationParams};
        use crate::models::Operation;
        use crate::toolpath::Toolpath;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();
        let op_id = Uuid::parse_str("eeee0000-0000-0000-0000-000000000005").unwrap();

        let op = Operation {
            id: op_id,
            name: "Stale Drill".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: 10.0,
                points: vec![],
                peck_depth: None,
            }),
            cache: CacheState {
                key: Some("sha256:stale".to_string()),
                valid: false,
                computed_at: None,
                stats: None,
                binary_file: None,
            },
        };

        let toolpath = Toolpath {
            operation_id: op_id,
            tool_number: 1,
            spindle_speed: 12000.0,
            feed_rate: 500.0,
            passes: vec![],
        };

        let mut project = Project::default();
        project.operations.push(op);
        project.toolpaths.insert(op_id, toolpath);

        let tmp = std::env::temp_dir().join("jcam_test_invalid_cache.jcam");
        save(&project, &tmp).expect("save should succeed");

        let not_expected_entry = format!("toolpaths/{op_id}.json");

        // Verify no toolpath ZIP entry was written for the invalid-cache op.
        {
            let file = std::fs::File::open(&tmp).unwrap();
            let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
            assert!(
                archive.by_name(&not_expected_entry).is_err(),
                "toolpath ZIP entry must NOT exist for cache.valid=false"
            );
        }

        // Verify binary_file remains None after round-trip.
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.operations.len(), 1);
        assert!(
            loaded.operations[0].cache.binary_file.is_none(),
            "binary_file must remain None for cache.valid=false"
        );
    }

    #[test]
    fn round_trip_project_with_operations() {
        use crate::models::operation::{
            CacheState, CompensationSide, DrillParams, OperationParams, PocketParams, ProfileParams,
        };
        use crate::models::Operation;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();

        let op_profile = Operation {
            id: Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap(),
            name: "Outer Profile".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: Some(2.5),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let op_pocket = Operation {
            id: Uuid::parse_str("bbbb0000-0000-0000-0000-000000000002").unwrap(),
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 15.0,
                stepdown: 3.0,
                stepover_percent: 45.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let op_drill = Operation {
            id: Uuid::parse_str("cccc0000-0000-0000-0000-000000000003").unwrap(),
            name: "Drill Holes".to_string(),
            enabled: false,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                points: vec![],
                peck_depth: Some(5.0),
            }),
            cache: CacheState::default(),
        };

        let mut project = Project::default();
        project.name = "Operations Round-Trip Test".to_string();
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-02T00:00:00Z".to_string();
        project.operations.push(op_profile.clone());
        project.operations.push(op_pocket.clone());
        project.operations.push(op_drill.clone());

        let tmp = std::env::temp_dir().join("jcam_test_round_trip_operations.jcam");
        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.operations.len(), 3, "all 3 operations must survive");

        // Verify order and identity.
        assert_eq!(loaded.operations[0].id, op_profile.id);
        assert_eq!(loaded.operations[0].name, "Outer Profile");
        assert_eq!(loaded.operations[0].params, op_profile.params);

        assert_eq!(loaded.operations[1].id, op_pocket.id);
        assert_eq!(loaded.operations[1].name, "Rough Pocket");
        assert_eq!(loaded.operations[1].params, op_pocket.params);

        assert_eq!(loaded.operations[2].id, op_drill.id);
        assert_eq!(loaded.operations[2].name, "Drill Holes");
        assert!(
            !loaded.operations[2].enabled,
            "enabled=false must round-trip"
        );
        assert_eq!(loaded.operations[2].params, op_drill.params);
    }

    #[test]
    fn round_trip_with_valid_toolpath() {
        use crate::models::operation::{CacheState, DrillParams, OperationParams};
        use crate::models::Operation;
        use crate::toolpath::Toolpath;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();
        let op_id = Uuid::parse_str("f1110000-0000-0000-0000-000000000001").unwrap();

        let op = Operation {
            id: op_id,
            name: "Drill Round-Trip".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: 8.0,
                points: vec![],
                peck_depth: None,
            }),
            cache: CacheState {
                key: Some("sha256:validkey".to_string()),
                valid: true,
                computed_at: Some("2026-03-01T00:00:00Z".to_string()),
                stats: None,
                binary_file: None,
            },
        };

        let toolpath = Toolpath {
            operation_id: op_id,
            tool_number: 2,
            spindle_speed: 10000.0,
            feed_rate: 300.0,
            passes: vec![],
        };

        let mut project = Project::default();
        project.operations.push(op);
        project.toolpaths.insert(op_id, toolpath.clone());

        let tmp = std::env::temp_dir().join("jcam_test_round_trip_valid_toolpath.jcam");
        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert!(
            loaded.toolpaths.contains_key(&op_id),
            "toolpath must be restored after round-trip"
        );
        assert_eq!(
            loaded.toolpaths[&op_id], toolpath,
            "restored toolpath must equal the original"
        );
    }

    #[test]
    fn v2_round_trip_preserves_mode() {
        let mut project = Project::default();
        project.mode = crate::state::Mode::TwoD;
        project.created_at = "2026-01-01T00:00:00Z".to_string();
        project.modified_at = "2026-01-01T00:00:00Z".to_string();

        let tmp = std::env::temp_dir().join("jcam_test_v2_round_trip_mode.jcam");
        save(&project, &tmp).expect("save should succeed");
        let loaded = load(&tmp).expect("load should succeed");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(loaded.mode, crate::state::Mode::TwoD);
        assert_eq!(loaded.schema_version, 2);
    }

    #[test]
    fn v1_schema_without_mode_defaults_to_three_d() {
        let tmp = std::env::temp_dir().join("jcam_test_v1_no_mode.jcam");
        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("project.json", opts).unwrap();
            let json = r#"{
                "schema_version": 1,
                "app_version": "0.1.0",
                "created_at": "2026-01-01T00:00:00Z",
                "modified_at": "2026-01-01T00:00:00Z",
                "project": { "name": "V1 No Mode", "description": "", "units": "mm" }
            }"#;
            zip.write_all(json.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        let loaded = result.expect("v1 schema without mode field should load successfully");
        assert_eq!(
            loaded.mode,
            crate::state::Mode::ThreeD,
            "missing mode field should default to ThreeD"
        );
    }

    #[test]
    fn v2_schema_with_explicit_rotary_two_mode() {
        let tmp = std::env::temp_dir().join("jcam_test_v2_rotary_two.jcam");
        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("project.json", opts).unwrap();
            let json = r#"{
                "schema_version": 2,
                "app_version": "0.0.2",
                "created_at": "2026-01-01T00:00:00Z",
                "modified_at": "2026-01-01T00:00:00Z",
                "mode": "rotary_2",
                "project": { "name": "V2 Rotary2", "description": "", "units": "mm" }
            }"#;
            zip.write_all(json.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        let loaded = result.expect("v2 schema with rotary_2 mode should load successfully");
        assert_eq!(loaded.mode, crate::state::Mode::RotaryTwo);
    }

    #[test]
    fn unsupported_schema_v3_rejected() {
        let tmp = std::env::temp_dir().join("jcam_test_schema_v3.jcam");
        {
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("project.json", opts).unwrap();
            let json = r#"{
                "schema_version": 3,
                "app_version": "0.0.2",
                "created_at": "2026-01-01T00:00:00Z",
                "modified_at": "2026-01-01T00:00:00Z",
                "project": { "name": "Future", "description": "", "units": "mm" }
            }"#;
            zip.write_all(json.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        match result.expect_err("schema_version 3 should be rejected") {
            AppError::ProjectLoad(msg) => {
                assert!(
                    msg.to_lowercase().contains("unsupported schema version"),
                    "error message should contain 'unsupported schema version', got: {msg}"
                );
            }
            other => panic!("expected AppError::ProjectLoad, got {other:?}"),
        }
    }

    #[test]
    fn load_ignores_missing_toolpath_entry_gracefully() {
        use crate::models::operation::{CacheState, DrillParams, OperationParams};
        use crate::models::Operation;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();
        let op_id = Uuid::parse_str("f2220000-0000-0000-0000-000000000002").unwrap();

        // The op has a binary_file reference to a path that will not exist in
        // the ZIP, because we build the archive manually without writing that entry.
        let op = Operation {
            id: op_id,
            name: "Ghost Drill".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Drill(DrillParams {
                depth: 5.0,
                points: vec![],
                peck_depth: None,
            }),
            cache: CacheState {
                key: Some("sha256:ghost".to_string()),
                valid: false,
                computed_at: None,
                stats: None,
                binary_file: Some("toolpaths/nonexistent.json".to_string()),
            },
        };

        // Build the ZIP manually: write project.json with the binary_file
        // reference but omit the actual toolpath entry.
        let tmp = std::env::temp_dir().join("jcam_test_missing_toolpath_entry.jcam");
        {
            use crate::project::types::{ProjectFile, ProjectMeta};
            let ops = vec![op];
            let pf = ProjectFile {
                schema_version: 1,
                app_version: "0.0.0".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                modified_at: "2026-01-01T00:00:00Z".to_string(),
                mode: crate::state::Mode::default(),
                project: ProjectMeta {
                    name: "Ghost".to_string(),
                    description: String::new(),
                    units: "mm".to_string(),
                },
                source_model: None,
                stock: None,
                wcs: vec![],
                tools: vec![],
                operations: ops,
                source_2d_artwork: None,
                safe_height: None,
                artwork_origin: [0.0, 0.0],
            };
            let json = serde_json::to_string_pretty(&pf).unwrap();
            let file = std::fs::File::create(&tmp).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file(PROJECT_JSON, opts).unwrap();
            zip.write_all(json.as_bytes()).unwrap();
            // Deliberately omit "toolpaths/nonexistent.json".
            zip.finish().unwrap();
        }

        let result = load(&tmp);
        let _ = std::fs::remove_file(&tmp);

        let loaded = result.expect("load must succeed even when toolpath entry is absent");
        assert!(
            loaded.toolpaths.is_empty(),
            "toolpaths must be empty when ZIP entry is missing"
        );
    }
}
