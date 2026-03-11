//! Safe Rust wrappers around the raw cam_geometry C API.
//!
//! All `unsafe` code for RAII handle management is isolated here. Every type
//! in this file upholds Rust's safety invariants at the boundary with the C++
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
/// `OcctShape` is both [`Send`] and [`Sync`].  The C++ handle registry is
/// protected by a `std::shared_mutex`, which allows safe concurrent read
/// access from multiple threads.  In practice the shape is always stored
/// behind `RwLock<Project>` in `AppState`, which serialises writes.
#[derive(Debug)]
pub struct OcctShape {
    // CgShapeId is typedef uint64_t; we store it as u64 so this struct
    // compiles regardless of whether the FFI bindings were generated.
    id: u64,
    // PhantomData<*mut ()> opts out of the Send and Sync auto-traits (raw
    // pointers are neither Send nor Sync).  Both are restored manually below.
    _marker: std::marker::PhantomData<*mut ()>,
}

impl OcctShape {
    /// Return the raw handle id for use by sibling modules within `geometry`.
    pub(super) fn raw_id(&self) -> u64 {
        self.id
    }

    /// Construct an `OcctShape` from a raw id — for use in unit tests only.
    #[cfg(test)]
    pub fn new_for_test(id: u64) -> Self {
        OcctShape {
            id,
            _marker: std::marker::PhantomData,
        }
    }

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

// SAFETY: The C++ handle registry uses std::shared_mutex, so read-only access
// to the handle id from multiple threads concurrently is safe.  In practice
// OcctShape is always stored behind RwLock<Project> in AppState, which
// serialises all mutations; this impl is required for AppState: Sync.
unsafe impl Sync for OcctShape {}

// ── OcctMesh ──────────────────────────────────────────────────────────────────

/// Safe owner of a tessellated mesh handle.
///
/// Released via `cg_mesh_free` on drop. [`Send`] but not [`Sync`] — meshes
/// are short-lived temporaries used during tessellation and are never stored
/// in shared state, so concurrent access is not needed.
#[derive(Debug)]
pub struct OcctMesh {
    // CgMeshId is typedef uint64_t.
    id: u64,
    // Opts out of Sync; Send is restored explicitly below.
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
        //   cg_mesh_copy_vertices    → vertex_count * 3 doubles
        //   cg_mesh_copy_normals     → vertex_count * 3 doubles
        //   cg_mesh_copy_indices     → tri_count * 3 uint32s
        // cg_mesh_copy_face_groups is called separately below with its own guard.
        unsafe {
            super::ffi::cg_mesh_copy_vertices(self.id, verts_f64.as_mut_ptr());
            super::ffi::cg_mesh_copy_normals(self.id, norms_f64.as_mut_ptr());
            super::ffi::cg_mesh_copy_indices(self.id, indices.as_mut_ptr());
        }

        let vertices: Vec<f32> = verts_f64.iter().map(|&v| v as f32).collect();
        let normals: Vec<f32> = norms_f64.iter().map(|&v| v as f32).collect();

        let fg_count = unsafe { super::ffi::cg_mesh_face_group_count(self.id) };
        let mut fg_buf = vec![unsafe { std::mem::zeroed::<super::ffi::CgFaceGroup>() }; fg_count];
        if fg_count > 0 {
            unsafe {
                super::ffi::cg_mesh_copy_face_groups(self.id, fg_buf.as_mut_ptr());
            }
        }
        let face_groups: Vec<FaceGroup> = fg_buf
            .into_iter()
            .map(|fg| FaceGroup {
                start_triangle: fg.start_triangle,
                triangle_count: fg.triangle_count,
            })
            .collect();

        MeshData {
            vertices,
            normals,
            indices,
            face_groups,
        }
    }

    #[cfg(not(cam_geometry_bindings))]
    pub fn to_mesh_data(&self) -> MeshData {
        MeshData {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            face_groups: Vec::new(),
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

    /// The operation requires OCCT bindings not available in this build.
    #[error("Not implemented (OCCT not available)")]
    NotImplemented,
}

// ── MeshData ──────────────────────────────────────────────────────────────────

/// Per-face triangle group boundary within a [`MeshData`].
///
/// Each entry corresponds to one B-rep face in tessellation traversal order.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceGroup {
    pub start_triangle: u32,
    pub triangle_count: u32,
}

/// Tessellated triangle mesh ready for transfer to the frontend.
///
/// Buffers use `f32` vertices/normals (sufficient precision for Three.js
/// rendering) and `u32` indices. All geometry computation in Rust uses `f64`;
/// the downcast to `f32` happens only at the IPC boundary.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshData {
    /// XYZ interleaved vertex positions — 3 `f32` values per vertex.
    pub vertices: Vec<f32>,
    /// XYZ interleaved normals — 3 `f32` values per vertex.
    pub normals: Vec<f32>,
    /// Triangle indices — 3 `u32` values per triangle.
    pub indices: Vec<u32>,
    /// Per-face triangle group boundaries — one entry per B-rep face.
    pub face_groups: Vec<FaceGroup>,
}

// ── shape_section_at_z ────────────────────────────────────────────────────────

/// Stitch a flat list of line segments `(start, end)` into closed (or open) loops.
///
/// Uses a greedy chain walk: repeatedly search for a segment whose start (or
/// end, traversing in reverse) matches the current chain end, then append it.
/// When a chain closes (end == first point) or no continuation is found, the
/// chain is saved and a new one is started from the next unvisited segment.
/// Segments may be traversed in either direction to handle mixed-orientation
/// edge output from OCCT.
fn stitch_segments_into_loops(segments: Vec<((f64, f64), (f64, f64))>) -> Vec<Vec<(f64, f64)>> {
    const EPS: f64 = 1e-6;

    fn approx_eq(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS
    }

    let mut used = vec![false; segments.len()];
    let mut loops: Vec<Vec<(f64, f64)>> = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let (s, e) = segments[start_idx];
        let mut chain: Vec<(f64, f64)> = vec![s, e];

        loop {
            let chain_end = *chain.last().unwrap();
            if approx_eq(chain_end, chain[0]) {
                // Closed loop — remove the duplicate closing point.
                chain.pop();
                break;
            }
            // Find the next unused segment — accept either orientation.
            let next = segments
                .iter()
                .enumerate()
                .find_map(|(i, &(seg_s, seg_e))| {
                    if used[i] {
                        return None;
                    }
                    if approx_eq(seg_s, chain_end) {
                        Some((i, seg_e))
                    } else if approx_eq(seg_e, chain_end) {
                        Some((i, seg_s)) // traverse reversed
                    } else {
                        None
                    }
                });
            match next {
                Some((idx, next_pt)) => {
                    used[idx] = true;
                    chain.push(next_pt);
                }
                None => break, // open chain — keep as-is
            }
        }
        loops.push(chain);
    }
    loops
}

/// Compute the cross-section of `shape` at height `z`.
///
/// Returns a list of closed (or open) 2-D loops, each loop being an ordered
/// sequence of XY points.  An empty `Vec` means the plane did not intersect
/// the shape.
///
/// # Errors
/// - [`GeometryError::NotImplemented`] — OCCT bindings were not compiled in.
/// - [`GeometryError::ImportFailed`] — the kernel reported an unexpected error.
#[cfg(cam_geometry_bindings)]
pub fn shape_section_at_z(
    shape: &OcctShape,
    z: f64,
) -> Result<Vec<Vec<(f64, f64)>>, GeometryError> {
    let mut out_ptr: *mut super::ffi::CgPoint3 = std::ptr::null_mut();
    let mut out_count: usize = 0;
    let rc = unsafe {
        super::ffi::cg_shape_section_at_z(shape.raw_id(), z, &mut out_ptr, &mut out_count)
    };
    if rc == super::ffi::CgError::CG_ERR_NO_RESULT {
        return Ok(vec![]);
    }
    if rc != super::ffi::CgError::CG_OK {
        return Err(GeometryError::ImportFailed {
            message: last_error_message(),
        });
    }
    let segments: Vec<_> = (0..out_count / 2)
        .map(|i| {
            let s = unsafe { *out_ptr.add(i * 2) };
            let e = unsafe { *out_ptr.add(i * 2 + 1) };
            ((s.x, s.y), (e.x, e.y))
        })
        .collect();
    unsafe { super::ffi::cg_section_free(out_ptr) };
    Ok(stitch_segments_into_loops(segments))
}

#[cfg(not(cam_geometry_bindings))]
pub fn shape_section_at_z(
    _shape: &OcctShape,
    _z: f64,
) -> Result<Vec<Vec<(f64, f64)>>, GeometryError> {
    Err(GeometryError::NotImplemented)
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
            face_groups: vec![],
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
            face_groups: vec![],
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert!(v["vertices"].is_array());
        assert!(v["normals"].is_array());
        assert!(v["indices"].is_array());
        assert!(v["faceGroups"].is_array());
        assert_eq!(v["vertices"].as_array().unwrap().len(), 3);
        assert_eq!(v["indices"][2], 2);
    }

    #[test]
    fn face_group_serializes_camel_case_keys() {
        let fg = FaceGroup {
            start_triangle: 4,
            triangle_count: 2,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&fg).unwrap()).unwrap();
        assert_eq!(v["startTriangle"], 4);
        assert_eq!(v["triangleCount"], 2);
        // Rust snake_case keys must not appear in the output.
        assert!(v.get("start_triangle").is_none());
        assert!(v.get("triangle_count").is_none());
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
        assert!(data.face_groups.is_empty());
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

    // ── shape_section_at_z ────────────────────────────────────────────────

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn section_returns_not_implemented_without_bindings() {
        let shape = OcctShape::new_for_test(0);
        assert!(matches!(
            shape_section_at_z(&shape, 5.0),
            Err(GeometryError::NotImplemented)
        ));
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn section_box_at_midheight_returns_single_loop() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let shape = OcctShape::load_step(path).expect("box.step fixture should load");
        let (_, _, zmin, _, _, zmax) = shape.bounding_box();
        let z_mid = (zmin + zmax) / 2.0;
        let loops =
            shape_section_at_z(&shape, z_mid).expect("section at mid-height should succeed");
        assert_eq!(
            loops.len(),
            1,
            "rectangular cross-section should produce one closed loop"
        );
        assert_eq!(
            loops[0].len(),
            4,
            "rectangular loop should have four corners"
        );
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
        // A box has 6 faces; each must appear in face_groups.
        assert_eq!(data.face_groups.len(), 6, "box must have 6 face groups");
        // Sanity-check that all triangle ranges are in bounds.
        let tri_count = (data.indices.len() / 3) as u64;
        for fg in &data.face_groups {
            assert!(
                fg.start_triangle as u64 + fg.triangle_count as u64 <= tri_count,
                "face group overruns triangle buffer"
            );
        }
    }
}
