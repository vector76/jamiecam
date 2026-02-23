# JamieCam Technology Stack

## Overview

JamieCam is a cross-platform CAM (Computer-Aided Manufacturing) application targeting
2D, 2.5D, and full 5-axis toolpath generation. The architecture is designed around a
hard separation between the UI layer (web technologies) and the compute layer (Rust/C++),
with offline-first operation and a future web deployment path.

---

## Application Shell: Tauri

**Version:** Tauri 2.x

Tauri provides the desktop application wrapper. It hosts a native OS webview for the UI
and a Rust process for all backend logic. The two communicate over a typed IPC bridge.

| Concern | Provided by |
|---|---|
| Window management | Tauri core |
| Native file dialogs | Tauri `dialog` plugin |
| Native menu bar | Tauri `menu` API |
| Auto-updater | Tauri `updater` plugin |
| Code signing / packaging | Tauri bundler |
| IPC (frontend ↔ backend) | Tauri `invoke` / `emit` |

**Platform webviews:**

| OS | Webview engine |
|---|---|
| Windows | WebView2 (Chromium-based, ships with Win 10/11) |
| macOS | WebKit (WKWebView) |
| Linux | WebKitGTK |

---

## Frontend Layer

### Framework
**React 19** with **TypeScript**

React is chosen for its mature ecosystem, strong TypeScript support, and the availability
of component libraries suited to dense tool UIs.

### Build Tooling
**Vite** — Tauri's default and recommended bundler. Fast HMR during development.

### UI Component Library
**TBD** — candidates include:
- `shadcn/ui` (Radix primitives, fully customizable, no runtime dependency)
- `Mantine` (batteries-included, good for complex forms and panels)

A dark-theme-first, dense tool UI is the target aesthetic (similar to Blender, FreeCAD).
This favors a library that allows full visual control over components.

### State Management
**Zustand** — lightweight, minimal boilerplate, works well for the mix of global app state
(active tool, current operation, selection) and local component state.

### Frontend ↔ Rust API Layer
A thin typed wrapper module (`src/api/`) wraps all `invoke()` calls and event listeners.
The frontend never calls `invoke()` directly — it goes through typed functions that mirror
the Rust command signatures. This creates a single point of change when backend APIs evolve.

---

## 3D Viewport

**Three.js** (r3f — React Three Fiber optional wrapper)

Three.js handles all 3D visualization in the webview via WebGL.

| Responsibility | Approach |
|---|---|
| Model display | BufferGeometry built from tessellation mesh sent by Rust |
| Toolpath display | LineSegments geometry, color-coded by operation type |
| Stock / fixture display | Transparent overlaid mesh |
| Tool animation | Animated Object3D following toolpath point sequence |
| Camera | OrbitControls with configurable up-axis (Y or Z) |
| Selection | Raycasting against model faces / operation regions |
| Measurement overlays | Custom shader or CSS2DRenderer for labels |

Three.js geometry data is received from Rust as flat typed arrays
(`Float32Array` for vertices/normals, `Uint32Array` for indices) to minimize
serialization overhead across the IPC boundary.

---

## Backend Layer: Rust

The Rust process is the authoritative owner of all data and computation.
The frontend is a view and input layer only — it holds no canonical state.

### Async Runtime
**Tokio** — Tauri's default async runtime. Long-running toolpath computations run as
async tasks, streaming progress events back to the frontend via `window.emit()`.

### Key Responsibilities

| Responsibility | Notes |
|---|---|
| File parsing (STL, OBJ, DXF, SVG) | Native Rust crates |
| File parsing (STEP, IGES) | Via OpenCASCADE FFI |
| Solid/surface geometry kernel | Via OpenCASCADE FFI |
| Toolpath algorithm engine | Pure Rust |
| G-code post-processor | Pure Rust |
| Project file serialization | `serde` + JSON or binary format TBD |
| Preferences / config | `serde` + TOML |

### Rust Crates (anticipated)

| Crate | Purpose |
|---|---|
| `tauri` | App framework and IPC |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `nalgebra` | Linear algebra (vectors, matrices, quaternions) |
| `bindgen` | Auto-generate FFI bindings from C headers |
| `thiserror` | Structured error types |
| `tracing` | Structured logging |
| `rayon` | Data-parallel iteration for toolpath computation |

---

## Geometry Kernel: OpenCASCADE Technology (OCCT)

**OCCT 7.7.x** (open source edition)

OCCT is the industry-standard open-source B-rep geometry kernel. It is written in C++
and required for robust handling of STEP/IGES files and for surface-based toolpath
operations needed by 5-axis machining.

### Integration Strategy

OCCT is not called directly from Rust. A thin C wrapper library (`src-tauri/cpp/`) is
maintained that exposes only the OCCT functionality JamieCam needs via a plain C API.
`bindgen` generates the Rust FFI bindings from those C headers at build time.

```
Rust code
  └─ calls ──► cam_geometry.h  (plain C API)
                  └─ calls ──► OCCT C++ API
                                  └─ links ──► OCCT static libraries
```

This isolation means:
- Rust never directly depends on C++ name mangling or ABI
- The C wrapper is the only place that needs to know OCCT internals
- The surface area of the FFI boundary stays small and auditable

### OCCT Responsibilities

| Operation | Used for |
|---|---|
| STEP / IGES import | Loading customer solid models |
| Tessellation | Converting B-rep solids to triangle meshes for display |
| Surface offset | Tool envelope / gouge detection |
| Face / edge classification | Identifying machinable features |
| Intersection computation | Tool-surface contact point calculation |

### Supplementary Geometry Libraries

| Library | Purpose |
|---|---|
| `Clipper2` (C++) | 2D polygon offsetting for pocket clearing passes |
| `libfive` (optional) | Implicit surface representation for adaptive strategies |

---

## G-code Post-Processor

Written entirely in Rust. Post-processors are data-driven: each machine/controller
is described by a configuration file (TOML) that defines:

- Axis naming and orientation
- Modal and non-modal command syntax
- Feed/speed units
- Header / footer templates
- Canned cycle support

Built-in post-processors (planned): Fanuc, Heidenhain, Siemens 840D, Mach4, LinuxCNC.

---

## File Format Support

### Input

| Format | Handler | Notes |
|---|---|---|
| STEP (.stp, .step) | OCCT | Primary solid model format |
| IGES (.igs, .iges) | OCCT | Legacy solid model format |
| STL (.stl) | Rust (native) | Mesh-only, no topology |
| OBJ (.obj) | Rust (native) | Mesh-only |
| DXF (.dxf) | Rust (`dxf` crate) | 2D drawing input |
| SVG (.svg) | Rust (`svg` crate) | 2D profile input |

### Output

| Format | Notes |
|---|---|
| G-code (.nc, .ngc, .tap) | Via post-processor, controller-specific |
| JamieCam project (.jcam) | Proprietary JSON/binary project file |
| Toolpath preview mesh | Exported for verification in external tools |

---

## Project File Format

The `.jcam` project file stores:
- Reference to the source model file (path + checksum)
- All machining operations and their parameters
- Tool library (for this project)
- Stock definition
- WCS (Work Coordinate System) setup
- Computed toolpaths (cached, invalidated when inputs change)

Format: JSON envelope (human-readable, version-tagged) with optional binary payload
for large toolpath arrays. Serialized via `serde`.

---

## Build System

| Tool | Purpose |
|---|---|
| `cargo` | Rust package manager and build |
| `npm` / `pnpm` | Frontend package management |
| `Vite` | Frontend build and dev server |
| `tauri-cli` | App packaging, dev mode orchestration |
| `cmake` | Building OCCT and the C++ wrapper |
| `bindgen` | Generating Rust FFI from C headers (runs in `build.rs`) |

OCCT is built once as a set of static libraries and cached. The C wrapper is compiled
as a static library linked into the Rust binary. The final distributable is a single
native executable with the frontend assets embedded.

---

## Web Deployment Path

The architecture preserves a future web deployment option:

- The frontend (React + Three.js) is pure web technology with no desktop-only dependencies
- The typed `src/api/` layer can be given a second implementation that calls a server-side
  API instead of Tauri `invoke()`
- Rust compute code can be compiled to **WebAssembly** for in-browser execution,
  though OCCT's WASM story is limited — a server-side Rust process is the more realistic
  web backend
- No web deployment is planned for the initial release

---

## Development Environment

| Tool | Purpose |
|---|---|
| Rust (stable toolchain) | Backend development |
| Node.js 20+ / pnpm | Frontend development |
| OCCT development headers | Geometry kernel |
| `clang` | Required by `bindgen` |
| VS Code + `rust-analyzer` | Recommended IDE setup |

---

*Document status: Draft — decisions marked TBD are open for discussion.*
