//! High-level import dispatcher for supported 3D file formats.
//!
//! [`import`] is the single entry point used by Tauri commands. It dispatches
//! to the appropriate loader based on file extension, tessellates B-rep shapes
//! where needed, and returns a [`MeshData`] ready for the frontend.

use std::path::Path;

use super::safe::{GeometryError, MeshData, OcctMesh, OcctShape};

/// Load a 3D file and return a tessellated mesh ready for the frontend.
///
/// Supported extensions (case-insensitive):
///
/// | Extension    | Path                                  |
/// |--------------|---------------------------------------|
/// | `.step`/`.stp` | B-rep → tessellate → mesh           |
/// | `.iges`/`.igs` | B-rep → tessellate → mesh           |
/// | `.stl`       | Triangle mesh (loaded directly)       |
///
/// # Errors
///
/// - [`GeometryError::FileNotFound`] — path does not exist.
/// - [`GeometryError::UnsupportedFormat`] — extension not recognised.
/// - [`GeometryError::ImportFailed`] — loader rejected the file.
/// - [`GeometryError::TessellationFailed`] — B-rep produced no triangles.
pub fn import(path: &Path) -> Result<MeshData, GeometryError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("step") | Some("stp") => {
            let shape = OcctShape::load_step(path)?;
            let mesh = shape.tessellate(0.1, 0.1)?;
            Ok(mesh.to_mesh_data())
        }
        Some("iges") | Some("igs") => {
            let shape = OcctShape::load_iges(path)?;
            let mesh = shape.tessellate(0.1, 0.1)?;
            Ok(mesh.to_mesh_data())
        }
        Some("stl") => {
            let mesh = OcctMesh::load_stl(path)?;
            Ok(mesh.to_mesh_data())
        }
        Some(ext) => Err(GeometryError::UnsupportedFormat {
            extension: ext.to_string(),
        }),
        None => Err(GeometryError::UnsupportedFormat {
            extension: String::new(),
        }),
    }
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
            if extension.is_empty()
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
}
