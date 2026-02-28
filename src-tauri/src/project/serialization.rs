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
//! 2. Validate `schema_version == 1`; reject anything else with a clear error.
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

    if pf.schema_version != 1 {
        return Err(AppError::ProjectLoad(format!(
            "unsupported schema version {}; only schema version 1 is supported",
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
        },
    });

    Ok(Project {
        name: pf.project.name,
        description: pf.project.description,
        units: pf.project.units,
        schema_version: pf.schema_version,
        created_at: pf.created_at,
        modified_at: pf.modified_at,
        source_model,
        stock: pf.stock,
        wcs: pf.wcs,
        tools: pf.tools,
        operations: pf.operations,
    })
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

    let pf = ProjectFile {
        schema_version: 1,
        app_version: APP_VERSION.to_string(),
        created_at: project.created_at.clone(),
        modified_at: project.modified_at.clone(),
        project: ProjectMeta {
            name: project.name.clone(),
            description: project.description.clone(),
            units: project.units.clone(),
        },
        source_model: source_model_ref.clone(),
        stock: project.stock.clone(),
        wcs: project.wcs.clone(),
        tools: project.tools.clone(),
        operations: project.operations.clone(),
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
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
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
            },
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
        assert_eq!(loaded.schema_version, project.schema_version);
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

        assert_eq!(loaded.schema_version, 1);
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
    fn round_trip_project_with_operations() {
        use crate::models::operation::{
            CompensationSide, DrillParams, OperationParams, PocketParams, ProfileParams,
        };
        use crate::models::Operation;

        let tool_id = Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap();

        let op_profile = Operation {
            id: Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap(),
            name: "Outer Profile".to_string(),
            enabled: true,
            tool_id,
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: 2.5,
                compensation_side: CompensationSide::Left,
            }),
        };
        let op_pocket = Operation {
            id: Uuid::parse_str("bbbb0000-0000-0000-0000-000000000002").unwrap(),
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id,
            params: OperationParams::Pocket(PocketParams {
                depth: 15.0,
                stepdown: 3.0,
                stepover_percent: 45.0,
            }),
        };
        let op_drill = Operation {
            id: Uuid::parse_str("cccc0000-0000-0000-0000-000000000003").unwrap(),
            name: "Drill Holes".to_string(),
            enabled: false,
            tool_id,
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                peck_depth: Some(5.0),
            }),
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
}
