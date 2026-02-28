# Implementation Status

_Last updated: 2026-02-28. Based on git history (39 commits, branch `main`)._

This document describes what is actually implemented in the codebase, as
distinct from the planned architecture in `development-roadmap.md`. It is
intended to give a quick, honest picture of where the project stands.

---

## Summary

Phase 0 (Foundation) is complete. The architectural seams — OCCT build, Rust
FFI, IPC bridge, Three.js viewport, and `.jcam` file I/O — are all validated
and working on all three target platforms (Linux, macOS, Windows).

Phase 1 (2D Operations MVP) is in progress. The data layer for tools, stock,
WCS, and operations is fully implemented on both the Rust backend and the
TypeScript frontend. The operation list panel UI is in place. No CAM algorithms
(toolpath calculation, G-code output) have been implemented yet.

---

## Phase 0: Foundation — Complete

### Infrastructure

| Deliverable | Status | Notes |
|---|---|---|
| Tauri 2.x + React 19 + Vite scaffold | Done | `8fea2aa` |
| Environment check scripts + pre-commit hook | Done | `249c688` |
| C++ handle registry (`handle_registry.h/cpp`) | Done | `a9c284c` |
| C++ geometry API header (`cam_geometry.h`) | Done | `a9c284c` |
| C++ geometry implementation (`cam_geometry.cpp`) | Done | `0fa67ff` |
| Clipper2 vendored in `cpp/third_party/` | Done | `0fa67ff` |
| C++ build system (`CMakeLists.txt`) + doctest fixtures | Done | `2de44e7` |
| Rust `build.rs` — bindgen + C++ compile + link | Done | `bd98b92` |
| Safe Rust geometry types (`OcctShape`, `OcctMesh`, `Drop`) | Done | `c7a35d3` |
| Rust geometry loaders (STEP, STL dispatch) | Done | `c4efd9b` |
| Rust tessellator (B-rep → triangle mesh) | Done | `c4efd9b` |
| `AppState` with `RwLock<Project>` | Done | `b0ea703` |
| `AppError` (thiserror + adjacently-tagged serde) | Done | `b0ea703` |
| `.jcam` ZIP save/load (project metadata + model ref) | Done | `0eec381` |
| IPC command handlers (`open_model`, `save_project`, …) | Done | `b608e32` |
| Frontend API layer (`src/api/`) | Done | `3f4bb72` |
| Zustand stores (`projectStore`, `viewportStore`) | Done | `3f4bb72` |
| Three.js scene: renderer, cameras, controls, lighting | Done | `3368928` |
| Three.js mesh, axis triad, Viewport component | Done | `6cca2f3` |
| App shell UI + native file open/save dialogs | Done | `79c78bf` |
| GitHub Actions CI (Linux, macOS, Windows) | Done | `0676bbf` + 9 fix commits |

### CI stabilization

The CI required significant work after the initial commit due to platform
differences in OCCT library names (7.7 vs 7.8+), missing `TKGeomAlgo` on
Windows, macOS `.dylib` detection failures, `LIBCLANG_PATH` on Ubuntu 24.04,
vcpkg warnings, and CMake flag corrections. All CI jobs now pass on all three
platforms.

### Acceptance criteria status

| Criterion | Status |
|---|---|
| Open STEP file → shaded model in viewport, orbit works | Done |
| Open STL file → same result | Done |
| Save project → `.jcam` created; reopen → model reference restored | Done |
| CI passes on Linux, macOS, Windows | Done |

---

## Phase 1: 2D Operations — In Progress

### Data layer (complete)

The full data model for Phase 1 project entities is implemented in Rust and
mirrored in TypeScript. Rust structs live in `src-tauri/src/models/`; the
frontend types are in `src/api/types.ts`.

**Tool library** (`72e520d`, `1b608b1`)
- `Tool` struct with all cutting geometry fields (diameter, flutes, material,
  coatings, geometry coefficients)
- `ProjectFile` integration — tools persisted in `project.json`
- IPC commands: `add_tool`, `edit_tool`, `delete_tool`, `list_tools`
- Frontend API wrappers in `src/api/tools.ts`

**Stock definition** (`b632b69`)
- `Stock` struct: box dimensions + offset from model origin + material tag
- IPC commands: `get_stock`, `set_stock`
- Frontend API wrappers in `src/api/stock.ts`

**WCS setup** (`b632b69`)
- `Wcs` struct: origin point + axis orientation + label
- IPC commands: `get_wcs`, `set_wcs`
- Frontend API wrappers in `src/api/stock.ts` (combined with stock commands)

**Operations** (`97c6ac2`, `9c04c01`)
- `Operation` enum covering all planned operation types (`Profile`, `Pocket`,
  `Drill`, plus stubs for later phases)
- `OperationParams` per-type parameter structs
- `Project` integration — operations stored in `Vec<Operation>` with UUID keys
- `ProjectSnapshot` carries full operations list to frontend (`7695e8b`)
- IPC commands: `add_operation`, `edit_operation`, `delete_operation`,
  `reorder_operations`, `list_operations`
- Frontend API wrappers in `src/api/operations.ts`

### UI (partially complete)

**Operation list panel** (`0d7f3fa`)
- `OperationListPanel` component renders the list of operations
- Per-item display: name, type badge, enable/disable toggle, delete button
- Add-operation button (opens placeholder; no editor form yet)
- Zustand `projectStore` wired to backend — loads operations from `list_operations` on mount

**App shell** (`79c78bf`)
- `AppShell` layout component with sidebar + main viewport area
- `Toolbar` component with file operations (New, Open, Save, Save As)
- `OperationListPanel` mounted in the sidebar

### Not yet implemented (Phase 1)

| Item | Notes |
|---|---|
| Operation editor form | UI for editing per-type parameters |
| Geometry selection | Click faces/edges in viewport; face fingerprinting |
| Profile / contour algorithm | CAM logic in `src-tauri/src/toolpath/` |
| Pocket clearing algorithm | CAM logic |
| Drilling algorithm | CAM logic |
| Clipper2 integration (tool compensation) | `cg_poly_offset`, `cg_poly_boolean` |
| Basic linking (retract, lead-in/out) | Toolpath linking |
| Post-processor engine | TOML loader, modal tracker, block formatter |
| Built-in post-processors (Fanuc, LinuxCNC) | `.toml` configs |
| G-code export | Write `.nc` via save dialog |
| G-code preview panel | Read-only text view |
| Toolpath visualization | Three.js `LineSegments` per segment type |
| Progress events | Tokio task progress → `emit()` → frontend progress bar |
| Cache invalidation | SHA-256 cache key, stale detection |
| Toolpath binary format | `toolpaths/*.bin` in `.jcam` |
| Error notification UI | Toast / snackbar for IPC errors |
| Feeds and speeds | Tool defaults + per-operation override |

---

## Phases 2–5

Nothing from Phases 2–5 is implemented. The file structure in
`src-tauri/src/toolpath/`, `src-tauri/src/simulation/`, and
`src-tauri/src/postprocessor/` does not yet exist (the directories listed in
`system-architecture.md` are design targets, not current code).

---

## Test coverage

Tests exist for the infrastructure layer established in Phase 0:

| Test file | Coverage |
|---|---|
| `src/viewport/scene.test.ts` | Three.js scene setup |
| `src/viewport/controls.test.ts` | Camera controls |
| `src/viewport/modelMesh.test.ts` | Mesh construction |
| `src/store/projectStore.test.ts` | Zustand store actions |
| `src/store/viewportStore.test.ts` | Viewport store |
| `src/components/layout/Toolbar.test.tsx` | Toolbar component |
| `src/components/layout/OperationListPanel.test.tsx` | Operation list panel |
| `src-tauri/cpp/tests/` | C++ geometry wrapper (doctest) |

The golden toolpath and G-code integration tests described in the roadmap do
not yet exist — they depend on the CAM algorithms not yet written.

---

## Key files by area

### Rust backend
| File | Purpose |
|---|---|
| `src-tauri/src/main.rs` | Tauri app init, command registration |
| `src-tauri/src/state.rs` | `AppState`, `RwLock<Project>` |
| `src-tauri/src/error.rs` | `AppError` enum |
| `src-tauri/src/models/tool.rs` | Tool data types |
| `src-tauri/src/models/stock.rs` | Stock + WCS data types |
| `src-tauri/src/models/operation.rs` | Operation enum and params |
| `src-tauri/src/commands/file.rs` | `open_model`, `new_project`, `save_project` |
| `src-tauri/src/commands/tools.rs` | Tool CRUD commands |
| `src-tauri/src/commands/stock.rs` | Stock/WCS commands |
| `src-tauri/src/commands/operations.rs` | Operation CRUD commands |
| `src-tauri/src/commands/project.rs` | `get_project_snapshot` |
| `src-tauri/src/geometry/importer.rs` | Format dispatch (STEP/STL) |
| `src-tauri/src/geometry/ffi.rs` | bindgen-generated FFI |
| `src-tauri/src/project/serialization.rs` | `.jcam` ZIP read/write |

### C++ geometry wrapper
| File | Purpose |
|---|---|
| `src-tauri/cpp/cam_geometry.h` | Public C API contract |
| `src-tauri/cpp/cam_geometry.cpp` | OCCT implementation |
| `src-tauri/cpp/handle_registry.h/cpp` | uint64 handle → C++ object map |
| `src-tauri/cpp/third_party/Clipper2/` | Vendored 2D polygon library |

### TypeScript frontend
| File | Purpose |
|---|---|
| `src/api/types.ts` | TypeScript mirrors of Rust types |
| `src/api/file.ts` | File operation IPC wrappers |
| `src/api/tools.ts` | Tool CRUD IPC wrappers |
| `src/api/stock.ts` | Stock/WCS IPC wrappers |
| `src/api/operations.ts` | Operation CRUD IPC wrappers |
| `src/store/projectStore.ts` | Project Zustand store |
| `src/store/viewportStore.ts` | Viewport Zustand store |
| `src/viewport/scene.ts` | Three.js renderer + scene |
| `src/viewport/controls.ts` | OrbitControls (Z-up) |
| `src/viewport/modelMesh.ts` | `MeshData` → `BufferGeometry` |
| `src/components/layout/AppShell.tsx` | Top-level layout |
| `src/components/layout/OperationListPanel.tsx` | Operation list sidebar |
| `src/components/toolbar/Toolbar.tsx` | File operation toolbar |

---

*Related documents: `development-roadmap.md`, `system-architecture.md`*
