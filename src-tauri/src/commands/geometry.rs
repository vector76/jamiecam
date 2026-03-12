//! Geometry-related IPC command handlers.
//!
//! Exposes face enumeration from a loaded B-rep model to the frontend.

use std::sync::RwLock;

use crate::error::AppError;
use crate::state::{AppState, Project};

use super::read_project;

// ── IPC return types ──────────────────────────────────────────────────────────

/// Serialisable face descriptor sent to the frontend.
///
/// Uses `f32` values (downcast from the internal `f64`) to match the
/// `MeshData` convention and reduce payload size.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceDescriptorIpc {
    pub fingerprint: String,
    pub face_idx: usize,
    pub centroid: [f32; 3],
    pub normal: [f32; 3],
    pub area: f32,
}

/// Serialisable hole descriptor sent to the frontend.
///
/// Uses `f32` values (downcast from the internal `f64`) to reduce payload size.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoleDescriptorIpc {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub depth: f32,
    pub is_through: bool,
}

// ── get_model_faces ───────────────────────────────────────────────────────────

/// Testable inner logic for [`get_model_faces`].
///
/// Returns the list of planar faces for the currently loaded model, or an
/// appropriate [`AppError`] if no model is loaded or no B-rep shape is
/// available.
pub(crate) fn get_model_faces_inner(
    project_lock: &RwLock<Project>,
) -> Result<Vec<FaceDescriptorIpc>, AppError> {
    let descriptors = {
        let project = read_project(project_lock)?;

        let loaded = project
            .source_model
            .as_ref()
            .ok_or_else(|| AppError::NotFound("no model loaded".into()))?;

        let shape = loaded.shape.as_ref().ok_or_else(|| {
            AppError::NotFound("no shape available (stub build or STL import)".into())
        })?;

        crate::geometry::enumerate_faces(shape).map_err(AppError::from)?
        // `project` (read lock guard) drops here.
    };

    let ipc = descriptors
        .into_iter()
        .map(|d| FaceDescriptorIpc {
            fingerprint: d.fingerprint,
            face_idx: d.face_idx,
            centroid: [
                d.centroid[0] as f32,
                d.centroid[1] as f32,
                d.centroid[2] as f32,
            ],
            normal: [d.normal[0] as f32, d.normal[1] as f32, d.normal[2] as f32],
            area: d.area as f32,
        })
        .collect();

    Ok(ipc)
}

/// Return the planar faces of the currently loaded model.
#[tauri::command]
pub async fn get_model_faces(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FaceDescriptorIpc>, AppError> {
    get_model_faces_inner(&state.project)
}

// ── detect_holes ─────────────────────────────────────────────────────────────

/// Testable inner logic for [`detect_holes`].
///
/// Returns the list of detected holes for the currently loaded model, or an
/// appropriate [`AppError`] if no model is loaded or no B-rep shape is
/// available.
pub fn detect_holes_inner(
    project_lock: &RwLock<Project>,
) -> Result<Vec<HoleDescriptorIpc>, AppError> {
    let descriptors = {
        let project = read_project(project_lock)?;

        let loaded = project
            .source_model
            .as_ref()
            .ok_or_else(|| AppError::NotFound("no model loaded".into()))?;

        let shape = loaded.shape.as_ref().ok_or_else(|| {
            AppError::NotFound("no shape available (stub build or STL import)".into())
        })?;

        crate::geometry::find_holes(shape, 0.0, f64::MAX).map_err(AppError::from)?
        // `project` (read lock guard) drops here.
    };

    let ipc = descriptors
        .into_iter()
        .map(|d| HoleDescriptorIpc {
            center_x: d.center_x as f32,
            center_y: d.center_y as f32,
            radius: d.radius as f32,
            depth: d.depth as f32,
            is_through: d.is_through,
        })
        .collect();

    Ok(ipc)
}

/// Detect cylindrical holes in the currently loaded model.
#[tauri::command]
pub async fn detect_holes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<HoleDescriptorIpc>, AppError> {
    detect_holes_inner(&state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn face_descriptor_ipc_serializes_camel_case() {
        let desc = FaceDescriptorIpc {
            fingerprint: "abc".into(),
            face_idx: 3,
            centroid: [1.0, 2.0, 3.0],
            normal: [0.0, 0.0, 1.0],
            area: 25.0,
        };
        let value = serde_json::to_value(&desc).expect("serialize FaceDescriptorIpc");
        assert!(
            value.get("faceIdx").is_some(),
            "face_idx should serialize as faceIdx"
        );
        assert_eq!(value["faceIdx"], 3);
        assert!(
            value.get("face_idx").is_none(),
            "snake_case key must not appear"
        );
    }

    #[test]
    fn get_model_faces_inner_returns_not_found_when_no_model() {
        let state = AppState::default();
        let result = get_model_faces_inner(&state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn get_model_faces_inner_returns_not_found_when_shape_is_none() {
        use crate::geometry::MeshData;
        use crate::state::{LoadedModel, Project};
        use std::sync::RwLock;

        let project = Project {
            source_model: Some(LoadedModel {
                path: std::path::PathBuf::from("/fake/model.step"),
                checksum: "abc".into(),
                mesh_data: MeshData {
                    vertices: vec![],
                    normals: vec![],
                    indices: vec![],
                    face_groups: vec![],
                },
                shape: None,
            }),
            ..Project::default()
        };
        let lock = RwLock::new(project);
        let result = get_model_faces_inner(&lock);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn hole_descriptor_ipc_serializes_camel_case() {
        let desc = HoleDescriptorIpc {
            center_x: 1.0,
            center_y: 2.0,
            radius: 3.0,
            depth: 4.0,
            is_through: true,
        };
        let value = serde_json::to_value(&desc).expect("serialize HoleDescriptorIpc");
        assert!(
            value.get("centerX").is_some(),
            "center_x should serialize as centerX"
        );
        assert!(
            value.get("isThrough").is_some(),
            "is_through should serialize as isThrough"
        );
        assert!(
            value.get("center_x").is_none(),
            "snake_case key must not appear"
        );
    }

    #[test]
    fn detect_holes_inner_returns_not_found_when_no_model() {
        let state = AppState::default();
        let result = detect_holes_inner(&state.project);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn detect_holes_inner_returns_not_found_when_shape_is_none() {
        use crate::geometry::MeshData;
        use crate::state::{LoadedModel, Project};
        use std::sync::RwLock;

        let project = Project {
            source_model: Some(LoadedModel {
                path: std::path::PathBuf::from("/fake/model.step"),
                checksum: "abc".into(),
                mesh_data: MeshData {
                    vertices: vec![],
                    normals: vec![],
                    indices: vec![],
                    face_groups: vec![],
                },
                shape: None,
            }),
            ..Project::default()
        };
        let lock = RwLock::new(project);
        let result = detect_holes_inner(&lock);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn detect_holes_inner_returns_holes_for_plate_step() {
        use std::sync::RwLock;

        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/plate_with_holes.step"
        );
        let project_lock = RwLock::new(crate::state::Project::default());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(crate::commands::file::open_model_inner(
            fixture,
            &project_lock,
        ))
        .expect("open_model_inner should succeed");

        let result = detect_holes_inner(&project_lock);
        let holes = result.expect("should return Ok");
        assert_eq!(holes.len(), 3, "plate_with_holes.step should have 3 holes");

        // Verify f32 downcast happened (values should be finite and reasonable)
        for h in &holes {
            assert!(h.radius > 0.0, "radius should be positive");
            assert!(h.depth > 0.0, "depth should be positive");
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn get_model_faces_inner_returns_faces_for_box_step() {
        use std::sync::RwLock;

        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/box.step");
        let project_lock = RwLock::new(crate::state::Project::default());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(crate::commands::file::open_model_inner(
            fixture,
            &project_lock,
        ))
        .expect("open_model_inner should succeed");

        let result = get_model_faces_inner(&project_lock);
        let faces = result.expect("should return Ok");
        assert!(!faces.is_empty(), "box.step should have at least one face");
        assert_eq!(
            faces[0].fingerprint.len(),
            64,
            "fingerprint should be a 64-char hex string"
        );
    }
}
