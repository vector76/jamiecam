//! Safe Rust wrappers around the raw cam_geometry C API.
//!
//! All `unsafe` code in the geometry module is isolated here. Every type in
//! this file upholds Rust's safety invariants at the boundary with the C++
//! handle registry.

use std::path::Path;

// ── Module-level helpers (OCCT only) ──────────────────────────────────────────

/// Convert a [`Path`] to a null-terminated C string for FFI.
///
/// Returns [`GeometryError::ImportFailed`] if the path contains a null byte.
#[cfg(cam_geometry_bindings)]
fn path_to_cstring(path: &Path) -> Result<std::ffi::CString, GeometryError> {
    std::ffi::CString::new(path.to_string_lossy().as_ref()).map_err(|_| {
        GeometryError::ImportFailed {
            message: "path contains a null byte".into(),
        }
    })
}

/// Copy the last C-layer error message into an owned [`String`].
#[cfg(cam_geometry_bindings)]
fn last_error_message() -> String {
    // SAFETY: `cg_last_error_message` returns a thread-local pointer valid
    // until the next FFI call on this thread.  We copy it into an owned
    // String immediately.
    unsafe {
        let ptr = super::ffi::cg_last_error_message();
        if ptr.is_null() {
            return "unknown error".into();
        }
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

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

impl OcctShape {
    /// Load a STEP file from `path`.
    ///
    /// Returns [`GeometryError::FileNotFound`] if the path does not exist on disk.
    /// Returns [`GeometryError::ImportFailed`] if the OCCT importer rejects it.
    pub fn load_step(path: &Path) -> Result<OcctShape, GeometryError> {
        if !path.exists() {
            return Err(GeometryError::FileNotFound);
        }
        Self::load_step_inner(path)
    }

    #[cfg(cam_geometry_bindings)]
    fn load_step_inner(path: &Path) -> Result<OcctShape, GeometryError> {
        let c_path = path_to_cstring(path)?;
        let id = unsafe { super::ffi::cg_load_step(c_path.as_ptr()) };
        if id == 0 {
            return Err(GeometryError::ImportFailed {
                message: last_error_message(),
            });
        }
        Ok(OcctShape {
            id,
            _marker: std::marker::PhantomData,
        })
    }

    #[cfg(not(cam_geometry_bindings))]
    fn load_step_inner(_path: &Path) -> Result<OcctShape, GeometryError> {
        Err(GeometryError::ImportFailed {
            message: "OCCT not available".into(),
        })
    }

    /// Load an IGES file from `path`.
    ///
    /// Returns [`GeometryError::FileNotFound`] if the path does not exist on disk.
    /// Returns [`GeometryError::ImportFailed`] if the OCCT importer rejects it.
    pub fn load_iges(path: &Path) -> Result<OcctShape, GeometryError> {
        if !path.exists() {
            return Err(GeometryError::FileNotFound);
        }
        Self::load_iges_inner(path)
    }

    #[cfg(cam_geometry_bindings)]
    fn load_iges_inner(path: &Path) -> Result<OcctShape, GeometryError> {
        let c_path = path_to_cstring(path)?;
        let id = unsafe { super::ffi::cg_load_iges(c_path.as_ptr()) };
        if id == 0 {
            return Err(GeometryError::ImportFailed {
                message: last_error_message(),
            });
        }
        Ok(OcctShape {
            id,
            _marker: std::marker::PhantomData,
        })
    }

    #[cfg(not(cam_geometry_bindings))]
    fn load_iges_inner(_path: &Path) -> Result<OcctShape, GeometryError> {
        Err(GeometryError::ImportFailed {
            message: "OCCT not available".into(),
        })
    }

    /// Tessellate the shape into a triangle mesh.
    ///
    /// - `chord_tol`: maximum chord deviation from the true surface (mm).
    /// - `angle_tol`: maximum angular deviation (radians).
    ///
    /// Returns [`GeometryError::TessellationFailed`] if tessellation produces
    /// no usable triangles.
    #[cfg(cam_geometry_bindings)]
    pub fn tessellate(&self, chord_tol: f64, angle_tol: f64) -> Result<OcctMesh, GeometryError> {
        let id = unsafe { super::ffi::cg_shape_tessellate(self.id, chord_tol, angle_tol) };
        if id == 0 {
            return Err(GeometryError::TessellationFailed {
                message: last_error_message(),
            });
        }
        Ok(OcctMesh {
            id,
            _marker: std::marker::PhantomData,
        })
    }

    #[cfg(not(cam_geometry_bindings))]
    pub fn tessellate(&self, _chord_tol: f64, _angle_tol: f64) -> Result<OcctMesh, GeometryError> {
        Err(GeometryError::TessellationFailed {
            message: "OCCT not available".into(),
        })
    }

    /// Return the axis-aligned bounding box as `(xmin, ymin, zmin, xmax, ymax, zmax)`.
    #[cfg(cam_geometry_bindings)]
    pub fn bounding_box(&self) -> (f64, f64, f64, f64, f64, f64) {
        let bb = unsafe { super::ffi::cg_shape_bounding_box(self.id) };
        (bb.xmin, bb.ymin, bb.zmin, bb.xmax, bb.ymax, bb.zmax)
    }

    #[cfg(not(cam_geometry_bindings))]
    pub fn bounding_box(&self) -> (f64, f64, f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    }
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

impl OcctMesh {
    /// Load an STL file from `path` directly as a triangle mesh.
    ///
    /// Returns [`GeometryError::FileNotFound`] if the path does not exist on disk.
    /// Returns [`GeometryError::ImportFailed`] if the STL importer rejects it.
    pub fn load_stl(path: &Path) -> Result<OcctMesh, GeometryError> {
        if !path.exists() {
            return Err(GeometryError::FileNotFound);
        }
        Self::load_stl_inner(path)
    }

    #[cfg(cam_geometry_bindings)]
    fn load_stl_inner(path: &Path) -> Result<OcctMesh, GeometryError> {
        let c_path = path_to_cstring(path)?;
        let id = unsafe { super::ffi::cg_load_stl(c_path.as_ptr()) };
        if id == 0 {
            return Err(GeometryError::ImportFailed {
                message: last_error_message(),
            });
        }
        Ok(OcctMesh {
            id,
            _marker: std::marker::PhantomData,
        })
    }

    #[cfg(not(cam_geometry_bindings))]
    fn load_stl_inner(_path: &Path) -> Result<OcctMesh, GeometryError> {
        Err(GeometryError::ImportFailed {
            message: "OCCT not available".into(),
        })
    }

    /// Copy the mesh buffers into a [`MeshData`] value for transfer to the
    /// frontend.
    ///
    /// The C API stores coordinates as `f64`; they are downcast to `f32` here
    /// because that is sufficient precision for Three.js rendering.
    #[cfg(cam_geometry_bindings)]
    pub fn to_mesh_data(&self) -> MeshData {
        let vertex_count = unsafe { super::ffi::cg_mesh_vertex_count(self.id) };
        let tri_count = unsafe { super::ffi::cg_mesh_triangle_count(self.id) };

        let mut verts_f64 = vec![0.0_f64; vertex_count * 3];
        let mut norms_f64 = vec![0.0_f64; vertex_count * 3];
        let mut indices = vec![0_u32; tri_count * 3];

        // SAFETY: buffers are sized exactly as required by the C API contracts:
        //   cg_mesh_copy_vertices  → vertex_count * 3 doubles
        //   cg_mesh_copy_normals   → vertex_count * 3 doubles
        //   cg_mesh_copy_indices   → tri_count * 3 uint32s
        unsafe {
            super::ffi::cg_mesh_copy_vertices(self.id, verts_f64.as_mut_ptr());
            super::ffi::cg_mesh_copy_normals(self.id, norms_f64.as_mut_ptr());
            super::ffi::cg_mesh_copy_indices(self.id, indices.as_mut_ptr());
        }

        let vertices: Vec<f32> = verts_f64.iter().map(|&v| v as f32).collect();
        let normals: Vec<f32> = norms_f64.iter().map(|&v| v as f32).collect();

        MeshData {
            vertices,
            normals,
            indices,
        }
    }

    #[cfg(not(cam_geometry_bindings))]
    pub fn to_mesh_data(&self) -> MeshData {
        MeshData {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        }
    }
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
    use std::path::Path;

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

    // ── OcctShape loaders — file-not-found (always run) ───────────────────

    #[test]
    fn load_step_returns_file_not_found() {
        let result = OcctShape::load_step(Path::new("/nonexistent/path/model.step"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    #[test]
    fn load_iges_returns_file_not_found() {
        let result = OcctShape::load_iges(Path::new("/nonexistent/path/model.iges"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    #[test]
    fn load_stl_returns_file_not_found() {
        let result = OcctMesh::load_stl(Path::new("/nonexistent/path/model.stl"));
        assert!(matches!(result, Err(GeometryError::FileNotFound)));
    }

    // ── Stub behaviour (no OCCT) ──────────────────────────────────────────

    /// Without OCCT, tessellate() returns TessellationFailed.
    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn tessellate_stub_returns_tessellation_failed() {
        let shape = OcctShape {
            id: 0,
            _marker: std::marker::PhantomData,
        };
        assert!(matches!(
            shape.tessellate(0.1, 0.1),
            Err(GeometryError::TessellationFailed { .. })
        ));
    }

    /// Without OCCT, to_mesh_data() returns an empty MeshData.
    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn to_mesh_data_stub_returns_empty() {
        let mesh = OcctMesh {
            id: 0,
            _marker: std::marker::PhantomData,
        };
        let data = mesh.to_mesh_data();
        assert!(data.vertices.is_empty());
        assert!(data.normals.is_empty());
        assert!(data.indices.is_empty());
    }

    // ── OCCT integration tests ────────────────────────────────────────────

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn load_step_fixture_returns_shape() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        assert!(OcctShape::load_step(path).is_ok(), "box.step should load");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn bounding_box_is_ordered() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let shape = OcctShape::load_step(path).expect("load box.step");
        let (xmin, ymin, zmin, xmax, ymax, zmax) = shape.bounding_box();
        assert!(xmax > xmin, "xmax > xmin");
        assert!(ymax > ymin, "ymax > ymin");
        assert!(zmax > zmin, "zmax > zmin");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn tessellate_produces_nonempty_mesh() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let shape = OcctShape::load_step(path).expect("load box.step");
        let mesh = shape.tessellate(0.1, 0.1).expect("tessellate");
        let data = mesh.to_mesh_data();
        assert!(!data.vertices.is_empty(), "vertices must not be empty");
        assert_eq!(
            data.vertices.len(),
            data.normals.len(),
            "vertices and normals must have equal length"
        );
        assert_eq!(
            data.vertices.len() % 3,
            0,
            "vertex count must be divisible by 3"
        );
        assert!(!data.indices.is_empty(), "indices must not be empty");
        assert_eq!(
            data.indices.len() % 3,
            0,
            "index count must be divisible by 3"
        );
    }
}
