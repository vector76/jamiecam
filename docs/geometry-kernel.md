# JamieCam Geometry Kernel

## Overview

JamieCam's geometry kernel is built on **OpenCASCADE Technology (OCCT)** — the
open-source C++ B-rep geometry kernel used by FreeCAD, Salome, and many commercial
CAM tools. OCCT is called from Rust via a thin **plain-C wrapper library** that
isolates the Rust codebase from C++ ABI details.

Supplementary geometry (2D polygon operations) is handled by **Clipper2**, called
through the same C wrapper mechanism.

---

## OCCT Architecture

OCCT is organized into functional modules (called *toolkits*, each a compiled library):

```
┌─────────────────────────────────────────────────────────────────────┐
│  Data Exchange          TKSTEP  TKIGES  TKXSBase  TKXCAF           │
├─────────────────────────────────────────────────────────────────────┤
│  Modeling Algorithms    TKBO  TKOffset  TKTopAlgo  TKShHealing      │
│                         TKFillet  TKPrim                            │
├─────────────────────────────────────────────────────────────────────┤
│  Meshing                TKMesh                                      │
├─────────────────────────────────────────────────────────────────────┤
│  Modeling Data          TKBRep  TKGeom3d  TKGeom2d                 │
│                         TKGeomBase  TKG3d  TKG2d                   │
├─────────────────────────────────────────────────────────────────────┤
│  Foundation             TKernel  TKMath                             │
└─────────────────────────────────────────────────────────────────────┘
```

JamieCam uses the toolkits needed for import, topology traversal, surface evaluation,
tessellation, and intersection. It does **not** use OCCT's visualization stack (V3d,
AIS) — rendering is handled by Three.js.

### The B-Rep Data Model

OCCT represents solids as Boundary Representation: a solid is defined entirely
by its bounding surfaces.

```
TopoDS_Compound           (assembly of shapes)
└── TopoDS_Solid          (closed volume)
    └── TopoDS_Shell      (closed surface bounding the solid)
        └── TopoDS_Face   (a bounded surface patch)
            └── TopoDS_Wire   (closed boundary of a face)
                └── TopoDS_Edge   (a bounded curve)
                    └── TopoDS_Vertex  (a point)
```

Topology and geometry are separate layers:

| Topological entity | Underlying geometry |
|---|---|
| `TopoDS_Face` | `Geom_Surface` (plane, cylinder, NURBS, ...) |
| `TopoDS_Edge` | `Geom_Curve` (line, circle, B-spline, ...) |
| `TopoDS_Vertex` | `gp_Pnt` (3D point) |

`BRep_Tool` provides the bridge between topology and geometry.

### OCCT Handle System

OCCT uses intrusive reference counting via `Handle<T>` (similar to `std::shared_ptr`).
Objects managed by handles are heap-allocated and freed when the last handle goes
out of scope. `TopoDS_Shape` objects use value semantics (copyable structs) and
internally reference the counted geometry objects.

---

## Integration Architecture

### The C Wrapper Pattern

Rust cannot safely call C++ directly — name mangling, exceptions, and ABI details
make the boundary fragile. Instead, a dedicated C++ translation unit exposes a
**plain-C API** that Rust calls via `bindgen`-generated FFI bindings.

```
┌─────────────────────────────────────────────────────────┐
│  Rust                                                   │
│  geometry/ffi.rs    (bindgen-generated raw bindings)    │
│  geometry/safe.rs   (safe Rust wrappers around raw)     │
└────────────────────────┬────────────────────────────────┘
                         │  extern "C"
┌────────────────────────▼────────────────────────────────┐
│  C++ (compiled as static library: libcam_geometry.a)   │
│  cam_geometry.h     (the public C API — plain C types)  │
│  cam_geometry.cpp   (implementation — uses OCCT C++ API)│
│  handle_registry.cpp (maps uint64 IDs to C++ objects)  │
└────────────────────────┬────────────────────────────────┘
                         │  C++ API
┌────────────────────────▼────────────────────────────────┐
│  OCCT static libraries                                  │
│  TKBRep, TKSTEP, TKMesh, TKTopAlgo, ...                │
└─────────────────────────────────────────────────────────┘
```

### Handle Registry

C++ objects cannot be passed across the C boundary by value or by C++ pointer
without exposing the C++ ABI. Instead, C++ objects are stored in a registry
and identified by opaque `uint64_t` handles on the C side.

```cpp
// handle_registry.h
#pragma once
#include <cstdint>

// All OCCT objects referenced from Rust are stored here.
// Thread-safe: protected by a shared mutex.

uint64_t registry_store_shape(const TopoDS_Shape& shape);
uint64_t registry_store_mesh(Handle(Poly_Triangulation) mesh);

const TopoDS_Shape& registry_get_shape(uint64_t id);
Handle(Poly_Triangulation) registry_get_mesh(uint64_t id);

void registry_free_shape(uint64_t id);
void registry_free_mesh(uint64_t id);
```

On the Rust side, handles are wrapped in structs with `Drop` implementations that
call `registry_free_*`. This ensures no handle is ever leaked.

---

## C API Design

The public API surface is kept intentionally narrow — only what JamieCam
actually needs. It expands as new operations require new geometry queries.

### cam_geometry.h

```c
#pragma once
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Error handling ──────────────────────────────────────────────── */

/* Error codes returned by all functions that can fail.
   Functions that return a handle return 0 on failure.
   Functions returning int return CG_OK (0) on success. */
typedef enum {
    CG_OK                = 0,
    CG_ERR_FILE_NOT_FOUND = 1,
    CG_ERR_PARSE_FAILED  = 2,
    CG_ERR_NULL_HANDLE   = 3,
    CG_ERR_INVALID_ARG   = 4,
    CG_ERR_OCCT_EXCEPTION = 5,
    CG_ERR_NO_RESULT     = 6,
} CgError;

/* Retrieve a human-readable description of the last error on this thread. */
const char* cg_last_error_message(void);

/* ── Primitive types ─────────────────────────────────────────────── */

typedef struct { double x, y, z;    } CgPoint3;
typedef struct { double x, y, z;    } CgVec3;
typedef struct { double u, v;       } CgPoint2;
typedef struct { double xmin, ymin, zmin, xmax, ymax, zmax; } CgBbox;
typedef struct { double umin, umax, vmin, vmax; } CgUVBounds;

/* Opaque handles — uint64 IDs into the handle registry. */
typedef uint64_t CgShapeId;
typedef uint64_t CgFaceId;
typedef uint64_t CgEdgeId;
typedef uint64_t CgMeshId;
typedef uint64_t CgCurveId;

#define CG_NULL_ID  UINT64_C(0)

/* Surface type classification */
typedef enum {
    CG_SURF_PLANE      = 0,
    CG_SURF_CYLINDER   = 1,
    CG_SURF_CONE       = 2,
    CG_SURF_SPHERE     = 3,
    CG_SURF_TORUS      = 4,
    CG_SURF_BSPLINE    = 5,
    CG_SURF_BEZIER     = 6,
    CG_SURF_OFFSET     = 7,
    CG_SURF_OTHER      = 8,
} CgSurfaceType;

/* ── Shape import ────────────────────────────────────────────────── */

/* Load a STEP file. Returns CG_NULL_ID on failure. */
CgShapeId cg_load_step(const char* path);

/* Load an IGES file. Returns CG_NULL_ID on failure. */
CgShapeId cg_load_iges(const char* path);

/* Load an STL file (mesh-only — no topology). Returns CG_NULL_ID on failure. */
CgMeshId  cg_load_stl(const char* path);

/* Free a shape and remove it from the registry. */
void cg_shape_free(CgShapeId id);

/* ── Shape healing ───────────────────────────────────────────────── */

/* Attempt to repair a shape (fix tolerances, sew shells, remove duplicates).
   Returns a new handle to the healed shape. Caller must free original. */
CgShapeId cg_shape_heal(CgShapeId id);

/* ── Shape topology traversal ────────────────────────────────────── */

CgBbox  cg_shape_bounding_box(CgShapeId id);

/* Write face handles into out_faces (caller allocates). Returns face count.
   Pass NULL for out_faces to query the count first. */
size_t  cg_shape_faces(CgShapeId id, CgFaceId* out_faces, size_t capacity);
size_t  cg_shape_edges(CgShapeId id, CgEdgeId* out_edges, size_t capacity);

/* Free face/edge handles when done with them. */
void    cg_face_free(CgFaceId id);
void    cg_edge_free(CgEdgeId id);

/* ── Tessellation ────────────────────────────────────────────────── */

/* Tessellate the entire shape into a single triangle mesh.
   chord_tol: max deviation from true surface (mm).
   angle_tol: max angular deviation (radians).
   Returns CG_NULL_ID on failure. */
CgMeshId cg_shape_tessellate(CgShapeId id,
                              double chord_tol,
                              double angle_tol);

/* Query mesh buffer sizes before copying. */
size_t cg_mesh_vertex_count(CgMeshId id);
size_t cg_mesh_triangle_count(CgMeshId id);

/* Copy mesh data into caller-allocated buffers.
   vertices and normals: 3 doubles per vertex [x,y,z, x,y,z, ...]
   indices: 3 uint32 per triangle [i0,i1,i2, ...] */
CgError cg_mesh_copy_vertices(CgMeshId id, double* out_vertices);
CgError cg_mesh_copy_normals (CgMeshId id, double* out_normals);
CgError cg_mesh_copy_indices (CgMeshId id, uint32_t* out_indices);

void    cg_mesh_free(CgMeshId id);

/* ── Surface evaluation ──────────────────────────────────────────── */

CgSurfaceType cg_face_surface_type(CgFaceId id);
CgUVBounds    cg_face_uv_bounds(CgFaceId id);

/* Evaluate a point on the surface at (u, v). */
CgPoint3 cg_face_eval_point(CgFaceId id, double u, double v);

/* Evaluate the surface normal at (u, v). Always points outward. */
CgVec3   cg_face_eval_normal(CgFaceId id, double u, double v);

/* Evaluate first derivatives at (u, v). */
CgVec3   cg_face_eval_du(CgFaceId id, double u, double v);
CgVec3   cg_face_eval_dv(CgFaceId id, double u, double v);

/* Project a 3D point onto the face. Returns the nearest (u, v) parameter.
   out_dist: distance from point to surface (can be NULL). */
CgPoint2 cg_face_project_point(CgFaceId id, CgPoint3 point, double* out_dist);

/* For planar faces: return the plane normal and a point on the plane. */
CgError  cg_face_plane(CgFaceId id, CgVec3* out_normal, CgPoint3* out_origin);

/* For cylindrical faces: return axis direction, origin, and radius. */
CgError  cg_face_cylinder(CgFaceId id, CgVec3* out_axis,
                           CgPoint3* out_origin, double* out_radius);

/* ── Edge / curve evaluation ─────────────────────────────────────── */

void     cg_edge_param_range(CgEdgeId id, double* out_tmin, double* out_tmax);
CgPoint3 cg_edge_eval_point(CgEdgeId id, double t);
CgVec3   cg_edge_eval_tangent(CgEdgeId id, double t);
double   cg_edge_length(CgEdgeId id);

/* Is this edge a circle/arc? If so, returns center, axis, and radius. */
int      cg_edge_is_circle(CgEdgeId id, CgPoint3* out_center,
                            CgVec3* out_axis, double* out_radius);

/* ── Geometric queries ───────────────────────────────────────────── */

/* Minimum distance between two shapes. */
double  cg_shape_distance(CgShapeId a, CgShapeId b);

/* Intersect a shape with a horizontal plane at z_value.
   Returns a mesh of the resulting wires as polyline segments.
   out_points: flat array of 3D points [x,y,z, ...] forming connected segments.
   out_count: number of CgPoint3 values written.
   Returns CG_ERR_NO_RESULT if no intersection. */
CgError cg_shape_section_at_z(CgShapeId id, double z_value,
                               CgPoint3** out_points, size_t* out_count);

/* Free memory allocated by cg_shape_section_at_z. */
void    cg_section_free(CgPoint3* points);

/* ── Feature detection ───────────────────────────────────────────── */

typedef struct {
    CgPoint3 center;       /* hole center at top face level */
    CgVec3   axis;         /* hole axis direction */
    double   diameter;     /* mm */
    double   depth;        /* mm, positive downward */
    int      is_through;   /* 1 if through-hole, 0 if blind */
} CgHoleInfo;

typedef struct {
    CgFaceId face_id;
    CgVec3   normal;
    double   area;         /* mm² */
    double   z_height;     /* Z coordinate of the plane (Z-up WCS) */
} CgPlanarFaceInfo;

/* Detect cylindrical holes. out_holes is caller-freed via cg_holes_free. */
size_t  cg_shape_find_holes(CgShapeId id,
                             double min_diameter, double max_diameter,
                             CgHoleInfo** out_holes);
void    cg_holes_free(CgHoleInfo* holes);

/* Detect planar (flat) faces. out_faces is caller-freed via cg_planar_faces_free. */
size_t  cg_shape_find_planar_faces(CgShapeId id,
                                   CgPlanarFaceInfo** out_faces);
void    cg_planar_faces_free(CgPlanarFaceInfo* faces);

/* ── 2D polygon operations (Clipper2) ───────────────────────────── */

/* Offset a closed 2D polygon by delta mm (positive = outward, negative = inward).
   Input/output: flat [x,y, x,y, ...] arrays.
   out_paths: array of path arrays; out_counts: point count per path.
   Caller frees via cg_poly_free. */
CgError cg_poly_offset(const double* points, size_t point_count,
                        double delta, double arc_tolerance,
                        double** out_points, size_t* out_count);

void    cg_poly_free(double* points);

/* Boolean operations on polygon sets. */
typedef enum { CG_BOOL_UNION, CG_BOOL_DIFFERENCE, CG_BOOL_INTERSECTION } CgBoolOp;

CgError cg_poly_boolean(const double* a_points, size_t a_count,
                         const double* b_points, size_t b_count,
                         CgBoolOp op,
                         double** out_points, size_t* out_count);

#ifdef __cplusplus
}
#endif
```

---

## C++ Implementation Notes

### Exception Handling

OCCT signals errors by throwing `Standard_Failure` exceptions. Since exceptions
cannot safely cross C boundaries, all C++ wrapper functions catch them and
convert to error codes:

```cpp
CgShapeId cg_load_step(const char* path) {
    try {
        STEPControl_Reader reader;
        IFSelect_ReturnStatus status = reader.ReadFile(path);
        if (status != IFSelect_RetDone) {
            set_last_error("STEP reader: file not found or unreadable");
            return CG_NULL_ID;
        }
        reader.TransferRoots();
        TopoDS_Shape shape = reader.OneShape();
        return registry_store_shape(shape);
    } catch (const Standard_Failure& ex) {
        set_last_error(ex.GetMessageString());
        return CG_NULL_ID;
    } catch (...) {
        set_last_error("Unknown exception in cg_load_step");
        return CG_NULL_ID;
    }
}
```

`set_last_error` writes to a thread-local string. `cg_last_error_message` reads it.
This pattern is used in every C wrapper function without exception.

### Shape Healing

STEP/IGES files from other software often contain tolerance violations, gaps
between faces, or degenerate edges. OCCT's `ShapeFix` module repairs these.
Healing is run automatically on every import:

```cpp
CgShapeId cg_load_step(const char* path) {
    // ... load ...
    TopoDS_Shape raw = reader.OneShape();

    // Always attempt healing on import
    ShapeFix_Shape fixer(raw);
    fixer.Perform();
    TopoDS_Shape healed = fixer.Shape();

    return registry_store_shape(healed);
}
```

`cg_shape_heal` is also exposed publicly for re-healing after operations.

### Tessellation

OCCT's incremental mesh algorithm tessellates each face independently, then
the results are gathered and merged into a single flat buffer:

```cpp
CgMeshId cg_shape_tessellate(CgShapeId id, double chord_tol, double angle_tol) {
    const TopoDS_Shape& shape = registry_get_shape(id);

    // Compute the mesh (stored inside the shape's topology)
    BRepMesh_IncrementalMesh mesher(shape, chord_tol, false, angle_tol, true);
    if (!mesher.IsDone()) { /* ... */ }

    // Gather all face triangulations into a single buffer
    // De-duplicate shared edge vertices using a spatial hash map
    // ...
    // Store assembled CgMeshData in registry
}
```

Face normals are computed from the per-face `Poly_Triangulation::HasNormals()`
if available, otherwise derived from the surface normal at each triangle's
parametric centroid using `BRep_Tool::Surface`.

Winding order is corrected per face orientation (`TopAbs_Orientation`) so
all outward normals point consistently away from the solid.

### Section at Z (for Z-level toolpaths)

```cpp
CgError cg_shape_section_at_z(CgShapeId id, double z_value, ...) {
    const TopoDS_Shape& shape = registry_get_shape(id);

    // Construct the cutting plane
    gp_Pln plane(gp_Pnt(0, 0, z_value), gp_Dir(0, 0, 1));
    BRepBuilderAPI_MakeFace faceBuilder(plane, -1e6, 1e6, -1e6, 1e6);

    // Compute the section
    BRepAlgoAPI_Section section(shape, faceBuilder.Face(), Standard_False);
    section.ComputePCurveOn1(Standard_False);
    section.Approximation(Standard_True);
    section.Build();

    // Extract edges from the section result
    // Discretize each edge to points using BRepAdaptor_Curve
    // ...
}
```

---

## Rust Safe Wrapper Layer

Raw FFI bindings (generated by `bindgen`) live in `geometry/ffi_generated.rs`
and are never used directly outside `geometry/`. Safe wrappers in `geometry/safe.rs`
provide:

- Rust ownership semantics via `Drop`
- `Result<T, GeometryError>` instead of null handles and error codes
- Typed structs instead of raw `f64` arrays

```rust
/// Safe Rust wrapper around a CgShapeId.
pub struct OcctShape {
    id: sys::CgShapeId,
}

impl OcctShape {
    pub fn load_step(path: &Path) -> Result<Self, GeometryError> {
        let c_path = CString::new(path.to_str().ok_or(GeometryError::InvalidPath)?)?;
        let id = unsafe { sys::cg_load_step(c_path.as_ptr()) };
        if id == 0 {
            let msg = unsafe {
                CStr::from_ptr(sys::cg_last_error_message()).to_string_lossy()
            };
            return Err(GeometryError::ImportFailed(msg.into_owned()));
        }
        Ok(Self { id })
    }

    pub fn tessellate(&self, chord_tol: f64, angle_tol: f64) -> Result<OcctMesh, GeometryError> {
        let mesh_id = unsafe { sys::cg_shape_tessellate(self.id, chord_tol, angle_tol) };
        if mesh_id == 0 {
            return Err(GeometryError::TessellationFailed);
        }
        Ok(OcctMesh { id: mesh_id })
    }

    pub fn section_at_z(&self, z: f64) -> Result<Vec<[f64; 3]>, GeometryError> {
        // ... calls cg_shape_section_at_z, copies result, frees C memory ...
    }

    pub fn bounding_box(&self) -> BoundingBox {
        let bbox = unsafe { sys::cg_shape_bounding_box(self.id) };
        BoundingBox {
            min: Vec3::new(bbox.xmin, bbox.ymin, bbox.zmin),
            max: Vec3::new(bbox.xmax, bbox.ymax, bbox.zmax),
        }
    }

    pub fn faces(&self) -> Vec<OcctFace> { /* ... */ }
    pub fn edges(&self) -> Vec<OcctEdge> { /* ... */ }
    pub fn find_holes(&self, min_d: f64, max_d: f64) -> Vec<HoleInfo> { /* ... */ }
    pub fn find_planar_faces(&self) -> Vec<PlanarFaceInfo> { /* ... */ }
}

impl Drop for OcctShape {
    fn drop(&mut self) {
        unsafe { sys::cg_shape_free(self.id); }
    }
}

// OcctShape is Send (operations are internally thread-safe via registry mutex)
// but not Sync (parallel calls into the same shape are not safe without locking)
unsafe impl Send for OcctShape {}
```

---

## Tessellation Data Flow

The path from OCCT to the GPU:

```
OCCT B-Rep solid (C++)
        │
        │  BRepMesh_IncrementalMesh
        ▼
Poly_Triangulation per face (C++)
        │
        │  gather + merge in C++ wrapper
        ▼
CgMeshData { vertices: Vec<f64>, normals: Vec<f64>, indices: Vec<u32> } (C++)
        │
        │  cg_mesh_copy_vertices / normals / indices
        ▼
OcctMesh (Rust) — Vec<f32> vertices, normals; Vec<u32> indices
        │
        │  serde_json / tauri IPC  (Float32Array + Uint32Array via JS typed arrays)
        ▼
MeshData (TypeScript)
        │
        │  THREE.BufferAttribute
        ▼
THREE.BufferGeometry → GPU vertex buffer
```

**Note on f64 → f32:** OCCT works in f64 (double precision). Geometry is stored
in Rust as f64 for algorithm correctness. On transfer to the frontend for *display*,
vertices are downcast to f32 — sufficient for Three.js rendering and halves transfer
size. All toolpath computation in Rust stays in f64.

---

## Surface Evaluation for Toolpath Algorithms

The toolpath engine calls surface evaluation directly on `OcctFace` objects
(which internally call `cg_face_eval_*`). The main operations needed:

### Normal at a Point

Used by scallop finishing to compute the step-perpendicular direction:

```rust
impl OcctFace {
    pub fn normal_at(&self, uv: [f64; 2]) -> Vec3 {
        let n = unsafe { sys::cg_face_eval_normal(self.id, uv[0], uv[1]) };
        Vec3::new(n.x, n.y, n.z)
    }
}
```

### Project Point onto Surface

Used by scallop to snap an offset point back onto the surface:

```rust
pub fn project(&self, point: Vec3) -> ProjectionResult {
    let p = sys::CgPoint3 { x: point.x, y: point.y, z: point.z };
    let mut dist = 0.0_f64;
    let uv = unsafe { sys::cg_face_project_point(self.id, p, &mut dist) };
    ProjectionResult { uv: [uv.u, uv.v], distance: dist }
}
```

### Z-Level Cross-Section

Used by Z-level roughing and contour operations:

```rust
impl OcctShape {
    pub fn section_at_z(&self, z: f64) -> Result<Vec<Polyline>, GeometryError> {
        // calls cg_shape_section_at_z
        // converts flat point array to Vec<Polyline> (connected point sequences)
        // sorts and links wire segments into closed loops
    }
}
```

The resulting `Vec<Polyline>` is passed to Clipper2 (via `cg_poly_offset`) to
offset by the tool radius before generating the actual cut passes.

---

## Feature Detection

Feature detection produces suggestions that the user confirms in the UI.
It is heuristic — not guaranteed to find all features in complex models.

### Hole Detection

```
For each face in the shape:
  if face surface type == CG_SURF_CYLINDER:
    extract axis, origin, radius
    check face spans 360° (full circle arc in UV space)
    find the opposite face at the other end of the cylinder
    compute depth = distance between the two planar end faces
    classify as through-hole if the cylinder exits the bounding box
    record CgHoleInfo
```

Implemented entirely in the C++ wrapper using OCCT's `BRepAdaptor_Surface`
and `TopExp_Explorer`.

### Pocket / Boss Detection

Pockets and bosses are identified by analyzing face adjacency and normal direction:

```
For each planar face F:
  compute face normal direction (Z-component)
  if normal points upward (+Z): candidate floor or top face
  if normal points downward (−Z): candidate ceiling

  traverse adjacent faces (sharing edges with F):
    if adjacent faces are vertical walls → F is a pocket floor or boss top
    if enclosed region → pocket
    if protruding region → boss
```

This is necessarily approximate. Complex interlocking features may not be
correctly classified. The user always reviews and adjusts the result.

---

## Build System Integration

### Directory Layout

```
src-tauri/
├── build.rs                 # Rust build script
└── cpp/
    ├── cam_geometry.h       # Public C API (the contract)
    ├── cam_geometry.cpp     # Implementation (OCCT + Clipper2)
    ├── handle_registry.h
    ├── handle_registry.cpp
    ├── CMakeLists.txt       # Builds libcam_geometry.a
    └── third_party/
        └── Clipper2/        # Vendored
```

### build.rs

```rust
fn main() {
    let occt_root = std::env::var("OCCT_DIR")
        .unwrap_or_else(|_| "/usr/lib/x86_64-linux-gnu/opencascade".into());

    // Compile the C++ wrapper
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .files([
            "cpp/cam_geometry.cpp",
            "cpp/handle_registry.cpp",
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.engine.cpp",
            "cpp/third_party/Clipper2/Clipper2Lib/src/clipper.offset.cpp",
        ])
        .include("cpp/")
        .include("cpp/third_party/Clipper2/Clipper2Lib/include")
        .include(format!("{}/include/opencascade", occt_root))
        .compile("cam_geometry");

    // Link OCCT toolkits (static)
    let occt_lib = format!("{}/lib", occt_root);
    println!("cargo:rustc-link-search=native={}", occt_lib);

    for lib in &[
        "TKernel", "TKMath",
        "TKBRep", "TKGeomBase", "TKGeom2d", "TKGeom3d", "TKG2d", "TKG3d",
        "TKTopAlgo", "TKPrim", "TKBO", "TKShHealing", "TKOffset", "TKMesh",
        "TKXSBase", "TKSTEPBase", "TKSTEPAttr", "TKSTEP", "TKIGES", "TKXCAF",
    ] {
        println!("cargo:rustc-link-lib=static={}", lib);
    }

    // Generate bindings from the C header
    let bindings = bindgen::Builder::default()
        .header("cpp/cam_geometry.h")
        .allowlist_function("cg_.*")
        .allowlist_type("Cg.*")
        .generate()
        .expect("bindgen failed");

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("ffi_generated.rs")).unwrap();

    // Rebuild if C++ sources change
    println!("cargo:rerun-if-changed=cpp/cam_geometry.h");
    println!("cargo:rerun-if-changed=cpp/cam_geometry.cpp");
    println!("cargo:rerun-if-env-changed=OCCT_DIR");
}
```

### Platform Notes

| Platform | OCCT source |
|---|---|
| Ubuntu / Debian | `apt install libocct-*-dev` |
| macOS | `brew install opencascade` |
| Windows | vcpkg: `vcpkg install opencascade:x64-windows-static` |

OCCT version must be consistent across developer machines and CI. The
`OCCT_DIR` environment variable or a vendored build (via CMake ExternalProject)
pins the version. A vendored build is recommended for release packaging.

---

## Thread Safety

The handle registry is protected by a `std::shared_mutex` (C++17):
- Multiple concurrent reads are allowed (multiple toolpath threads querying surfaces)
- Writes (store/free) are exclusive

`OcctShape` is `Send` in Rust — it can be moved to a worker thread.
It is not `Sync` — two threads must not call methods on the same `OcctShape`
concurrently without external locking. The toolpath engine clones the input
data it needs before releasing the `AppState` lock, so threads operate on
independent data.

---

## Error Types

```rust
#[derive(thiserror::Error, Debug, Serialize)]
pub enum GeometryError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Import failed: {0}")]
    ImportFailed(String),

    #[error("Tessellation failed")]
    TessellationFailed,

    #[error("Surface evaluation failed at uv=({u}, {v})")]
    EvalFailed { u: f64, v: f64 },

    #[error("No intersection result")]
    NoIntersection,

    #[error("Feature detection error: {0}")]
    FeatureDetection(String),

    #[error("Invalid path")]
    InvalidPath,
}
```

---

## Supplementary: Clipper2

Clipper2 handles all **2D polygon operations**. It is used by:
- Profile operation: offset the input contour by tool radius
- Pocket clearing: successive inward offsets to fill area
- Z-level toolpaths: offset the Z-section curves by tool radius
- Rest machining: subtract previous tool's swept area from stock boundary

Clipper2 is wrapped in the same `cam_geometry.cpp` and exposed via the
`cg_poly_*` functions in the C API. Clipper2 operates in integer coordinates
internally; the wrapper scales by 1e6 (input in mm → internal in nanometers)
to preserve precision.

---

*Document status: Draft*
*Related documents: `technology-stack.md`, `system-architecture.md`, `toolpath-engine.md`*
