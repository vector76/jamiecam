//! Safe Rust wrappers around the raw cam_geometry C API.
//!
//! All `unsafe` code in the geometry module is isolated here. Every type in
//! this file upholds Rust's safety invariants at the boundary with the C++
//! handle registry.

// ── RAII handle owners ────────────────────────────────────────────────────────

/// Safe owner of a loaded B-rep shape handle.
///
/// The handle is released via `cg_shape_free` when this value is dropped.
/// The underlying C++ object lives in the handle registry.
///
/// # Thread safety
///
/// `OcctShape` is [`Send`] — it is safe to move to a worker thread because
/// the C++ handle registry is protected by a `std::shared_mutex`.
///
/// It is **not** [`Sync`] — concurrent method calls on the same shape from
/// multiple threads are not safe without external locking.
#[derive(Debug)]
pub struct OcctShape {
    // CgShapeId is typedef uint64_t; we store it as u64 so this struct
    // compiles regardless of whether the FFI bindings were generated.
    id: u64,
    // PhantomData<*mut ()> opts out of the Sync auto-trait (raw pointers are
    // neither Send nor Sync).  We restore Send manually below.
    _marker: std::marker::PhantomData<*mut ()>,
}

impl Drop for OcctShape {
    fn drop(&mut self) {
        // Only call into OCCT when the C++ library was actually compiled in.
        // Without OCCT the id was never issued, so doing nothing is correct.
        #[cfg(cam_geometry_bindings)]
        // SAFETY: `id` was obtained from `cg_load_step` / `cg_load_iges` and
        // has not previously been freed (Rust ownership ensures a single
        // owner).
        unsafe {
            super::ffi::cg_shape_free(self.id);
        }
    }
}

// SAFETY: The C++ handle registry is protected by a shared_mutex; moving the
// handle value to another thread is safe.
unsafe impl Send for OcctShape {}

// ── OcctMesh ──────────────────────────────────────────────────────────────────

/// Safe owner of a tessellated mesh handle.
///
/// Released via `cg_mesh_free` on drop. Same `Send`-not-`Sync` contract as
/// [`OcctShape`].
#[derive(Debug)]
pub struct OcctMesh {
    // CgMeshId is typedef uint64_t.
    id: u64,
    // Same as OcctShape: opt out of Sync, restore Send explicitly.
    _marker: std::marker::PhantomData<*mut ()>,
}

impl Drop for OcctMesh {
    fn drop(&mut self) {
        #[cfg(cam_geometry_bindings)]
        // SAFETY: same as OcctShape::drop.
        unsafe {
            super::ffi::cg_mesh_free(self.id);
        }
    }
}

// SAFETY: same reasoning as OcctShape.
unsafe impl Send for OcctMesh {}

// ── GeometryError ─────────────────────────────────────────────────────────────

/// Errors produced by the geometry kernel layer.
///
/// Implements [`serde::Serialize`] so it can be returned to the frontend
/// through Tauri IPC commands as a JSON value.
#[derive(thiserror::Error, Debug, serde::Serialize)]
pub enum GeometryError {
    /// The requested file does not exist on disk.
    #[error("File not found")]
    FileNotFound,

    /// The file was found but could not be parsed as a supported format.
    #[error("Import failed: {message}")]
    ImportFailed { message: String },

    /// Tessellation was attempted but produced no usable mesh.
    #[error("Tessellation failed: {message}")]
    TessellationFailed { message: String },

    /// The file extension is not handled by any available importer.
    #[error("Unsupported format: {extension}")]
    UnsupportedFormat { extension: String },
}

// ── MeshData ──────────────────────────────────────────────────────────────────

/// Tessellated triangle mesh ready for transfer to the frontend.
///
/// Buffers use `f32` vertices/normals (sufficient precision for Three.js
/// rendering) and `u32` indices. All geometry computation in Rust uses `f64`;
/// the downcast to `f32` happens only at the IPC boundary.
#[derive(Debug, serde::Serialize)]
pub struct MeshData {
    /// XYZ interleaved vertex positions — 3 `f32` values per vertex.
    pub vertices: Vec<f32>,
    /// XYZ interleaved normals — 3 `f32` values per vertex.
    pub normals: Vec<f32>,
    /// Triangle indices — 3 `u32` values per triangle.
    pub indices: Vec<u32>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── GeometryError display ─────────────────────────────────────────────

    #[test]
    fn geometry_error_file_not_found_display() {
        assert_eq!(GeometryError::FileNotFound.to_string(), "File not found");
    }

    #[test]
    fn geometry_error_import_failed_display() {
        let e = GeometryError::ImportFailed {
            message: "bad STEP file".into(),
        };
        assert_eq!(e.to_string(), "Import failed: bad STEP file");
    }

    #[test]
    fn geometry_error_tessellation_failed_display() {
        let e = GeometryError::TessellationFailed {
            message: "degenerate mesh".into(),
        };
        assert_eq!(e.to_string(), "Tessellation failed: degenerate mesh");
    }

    #[test]
    fn geometry_error_unsupported_format_display() {
        let e = GeometryError::UnsupportedFormat {
            extension: "stl".into(),
        };
        assert_eq!(e.to_string(), "Unsupported format: stl");
    }

    // ── GeometryError serialization ───────────────────────────────────────

    #[test]
    fn geometry_error_file_not_found_serializes_as_unit_variant() {
        let json = serde_json::to_string(&GeometryError::FileNotFound).unwrap();
        // serde encodes a unit variant as a bare string.
        assert_eq!(json, r#""FileNotFound""#);
    }

    #[test]
    fn geometry_error_import_failed_serializes() {
        let e = GeometryError::ImportFailed {
            message: "oops".into(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(v["ImportFailed"]["message"], "oops");
    }

    #[test]
    fn geometry_error_tessellation_failed_serializes() {
        let e = GeometryError::TessellationFailed {
            message: "bad".into(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(v["TessellationFailed"]["message"], "bad");
    }

    #[test]
    fn geometry_error_unsupported_format_serializes() {
        let e = GeometryError::UnsupportedFormat {
            extension: "obj".into(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(v["UnsupportedFormat"]["extension"], "obj");
    }

    // ── MeshData ──────────────────────────────────────────────────────────

    #[test]
    fn mesh_data_fields_are_accessible() {
        let m = MeshData {
            vertices: vec![0.0, 1.0, 2.0],
            normals: vec![0.0, 0.0, 1.0],
            indices: vec![0, 1, 2],
        };
        assert_eq!(m.vertices.len(), 3);
        assert_eq!(m.normals.len(), 3);
        assert_eq!(m.indices.len(), 3);
    }

    #[test]
    fn mesh_data_serializes_to_expected_shape() {
        let m = MeshData {
            vertices: vec![1.0_f32, 2.0, 3.0],
            normals: vec![0.0_f32, 0.0, 1.0],
            indices: vec![0_u32, 1, 2],
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert!(v["vertices"].is_array());
        assert!(v["normals"].is_array());
        assert!(v["indices"].is_array());
        assert_eq!(v["vertices"].as_array().unwrap().len(), 3);
        assert_eq!(v["indices"][2], 2);
    }

    // ── Handle type properties ────────────────────────────────────────────

    /// OcctShape must implement Send (compile-time check).
    #[test]
    fn occt_shape_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OcctShape>();
    }

    /// OcctMesh must implement Send (compile-time check).
    #[test]
    fn occt_mesh_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OcctMesh>();
    }

    /// Dropping a null-handle OcctShape must not panic.
    ///
    /// Without OCCT, Drop is a no-op; with OCCT, id=0 is CG_NULL_ID which
    /// cg_shape_free handles gracefully.
    #[test]
    fn occt_shape_null_drop_does_not_panic() {
        let shape = OcctShape {
            id: 0,
            _marker: std::marker::PhantomData,
        };
        drop(shape);
    }

    /// Dropping a null-handle OcctMesh must not panic.
    #[test]
    fn occt_mesh_null_drop_does_not_panic() {
        let mesh = OcctMesh {
            id: 0,
            _marker: std::marker::PhantomData,
        };
        drop(mesh);
    }
}
