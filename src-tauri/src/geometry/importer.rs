//! High-level import dispatcher for supported 3D file formats.
//!
//! [`import_with_shape`] is the primary entry point: it returns both a
//! tessellated [`MeshData`] and an optional live [`OcctShape`] handle for
//! B-rep formats. [`import`] is a thin wrapper that discards the shape for
//! callers that only need the mesh.

use std::path::Path;

use super::safe::{GeometryError, MeshData, OcctMesh, OcctShape};

/// Load a 3D file and return both a tessellated mesh and an optional B-rep shape.
///
/// STEP and IGES files produce a live [`OcctShape`] handle; STL files do not.
///
/// Supported extensions (case-insensitive):
///
/// | Extension    | Path                                  |
/// |--------------|---------------------------------------|
/// | `.step`/`.stp` | B-rep → tessellate → mesh + shape  |
/// | `.iges`/`.igs` | B-rep → tessellate → mesh + shape  |
/// | `.stl`       | Triangle mesh (loaded directly); shape is `None` |
///
/// # Errors
///
/// - [`GeometryError::FileNotFound`] — path does not exist.
/// - [`GeometryError::UnsupportedFormat`] — extension not recognised.
/// - [`GeometryError::ImportFailed`] — loader rejected the file.
/// - [`GeometryError::TessellationFailed`] — B-rep produced no triangles.
pub fn import_with_shape(path: &Path) -> Result<(MeshData, Option<OcctShape>), GeometryError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("step") | Some("stp") => {
            let shape = OcctShape::load_step(path)?;
            let mesh = shape.tessellate(0.1, 0.1)?;
            Ok((mesh.to_mesh_data(), Some(shape)))
        }
        Some("iges") | Some("igs") => {
            let shape = OcctShape::load_iges(path)?;
            let mesh = shape.tessellate(0.1, 0.1)?;
            Ok((mesh.to_mesh_data(), Some(shape)))
        }
        Some("stl") => {
            let mesh = OcctMesh::load_stl(path)?;
            Ok((mesh.to_mesh_data(), None))
        }
        Some(ext) => Err(GeometryError::UnsupportedFormat {
            extension: ext.to_string(),
        }),
        None => Err(GeometryError::UnsupportedFormat {
            extension: "(no extension)".to_string(),
        }),
    }
}

/// Load a 3D file and return a tessellated mesh ready for the frontend.
///
/// This is a thin wrapper around [`import_with_shape`] that discards the shape.
/// Prefer [`import_with_shape`] when the B-rep handle is needed downstream.
pub fn import(path: &Path) -> Result<MeshData, GeometryError> {
    import_with_shape(path).map(|(mesh, _shape)| mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── Extension dispatch and error paths (no OCCT required) ─────────────

    #[test]
    fn import_missing_step_file_returns_file_not_found() {
        let result = import(Path::new("/nonexistent/path/model.step"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    #[test]
    fn import_missing_stl_file_returns_file_not_found() {
        let result = import(Path::new("/nonexistent/path/model.stl"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    #[test]
    fn import_unknown_extension_returns_unsupported_format() {
        // Extension check happens before file-existence check, so path need
        // not exist on disk.
        let result = import(Path::new("model.obj"));
        assert!(matches!(
            result,
            Err(GeometryError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn import_no_extension_returns_unsupported_format() {
        let result = import(Path::new("noextension"));
        assert!(matches!(
            result,
            Err(GeometryError::UnsupportedFormat { extension })
            if extension == "(no extension)"
        ));
    }

    #[test]
    fn import_uppercase_extension_is_unsupported() {
        // Extensions are lowercased before matching, so .OBJ is still
        // unsupported (not a supported format).
        let result = import(Path::new("model.OBJ"));
        assert!(matches!(
            result,
            Err(GeometryError::UnsupportedFormat { .. })
        ));
    }

    // ── OCCT integration tests ────────────────────────────────────────────

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn import_step_fixture_returns_nonempty_mesh() {
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let mesh = import(&path).expect("import box.step");
        assert!(!mesh.vertices.is_empty(), "vertices must not be empty");
        assert_eq!(
            mesh.vertices.len(),
            mesh.normals.len(),
            "vertices and normals must have equal length"
        );
        assert!(!mesh.indices.is_empty(), "indices must not be empty");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn import_with_shape_step_returns_shape() {
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let (mesh, shape) = import_with_shape(&path).expect("import_with_shape box.step");
        assert!(!mesh.vertices.is_empty(), "vertices must not be empty");
        assert!(shape.is_some(), "STEP import must return a shape");
    }

    #[test]
    fn import_with_shape_missing_step_returns_file_not_found() {
        let result = import_with_shape(std::path::Path::new("/nonexistent/path/model.step"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    #[test]
    fn import_with_shape_unknown_extension_returns_unsupported_format() {
        let result = import_with_shape(std::path::Path::new("model.obj"));
        assert!(matches!(
            result,
            Err(GeometryError::UnsupportedFormat { .. })
        ));
    }
}
