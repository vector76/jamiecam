# Implementation Status

_Last updated: 2026-02-28. Based on git history (57 commits, branch `main`)._

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
TypeScript frontend. The operation list panel UI is in place. The complete
post-processor engine and G-code export pipeline are implemented and tested,
including four built-in post-processor configs, golden-file integration tests,
IPC commands, and a G-code preview panel with export functionality. No CAM
algorithms (toolpath calculation) have been implemented yet.

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
- `Tool` struct: `id`, `name`, `tool_type` (`ToolType` enum), `material`,
  `diameter`, `flute_count`, `default_spindle_speed`, `default_feed_rate`
- Project integration — tools persisted in `project.json` inside `.jcam`
- IPC commands: `add_tool`, `edit_tool`, `delete_tool`, `list_tools`
- Frontend API wrappers in `src/api/tools.ts`

**Stock definition** (`b632b69`)
- `StockDefinition` enum (currently one variant: `Box(BoxDimensions)`) with
  `BoxDimensions` fields: `origin`, `width`, `depth`, `height`
- IPC commands: `get_stock`, `set_stock`
- Frontend API wrappers in `src/api/stock.ts`

**WCS setup** (`b632b69`)
- `WorkCoordinateSystem` struct: `id`, `name`, `origin`, `x_axis`, `z_axis`
  (in `src-tauri/src/models/wcs.rs`)
- IPC commands: `get_wcs`, `set_wcs`
- Frontend API wrappers in `src/api/stock.ts` (combined with stock commands)

**Operations** (`97c6ac2`, `9c04c01`)
- `Operation` struct (common fields: `id`, `name`, `enabled`, `tool_id`) +
  `OperationParams` enum (`Profile`, `Pocket`, `Drill`) flattened alongside it
- Project integration — operations stored in `Vec<Operation>`; each carries a
  UUID `id` field
- `ProjectSnapshot` carries full operations list to frontend (`7695e8b`)
- IPC commands: `add_operation`, `edit_operation`, `delete_operation`,
  `reorder_operations`, `list_operations`
- Frontend API wrappers in `src/api/operations.ts`

### Post-processor engine (complete)

The complete post-processor engine and G-code output pipeline is implemented
and tested. This is the final stage of the CAM pipeline; it was built before
the CAM algorithms using fixture toolpath data so that G-code export will work
immediately once algorithms are written.

**Toolpath abstract types** (`9935b84`)
- `src-tauri/src/toolpath/types.rs`: `Toolpath`, `Pass`, `PassKind`,
  `CutPoint`, `MoveKind` (Rapid/Feed/Arc/Dwell), `ToolOrientation`
  (ThreeAxis/FiveAxis); all serde-serializable for golden file fixtures
- `Project.toolpaths: HashMap<Uuid, Toolpath>` storage slot added to state
- `AppError::PostProcessor(String)` variant added for engine errors
- `toml` crate added to `Cargo.toml`

**Post-processor engine modules** (`13b8244`–`31831fa`)
- `postprocessor/config.rs` — full TOML schema deserialization
  (`PostProcessorConfig`) with post-load validation
- `postprocessor/formatter.rs` — number formatting (decimal places, trailing
  zero stripping, leading zero suppression) and template variable substitution
- `postprocessor/block.rs` — `Block`, `Word`, `WordValue`, `BlockBuilder` with
  standard word ordering and configurable rendering
- `postprocessor/arcs.rs` — IJK arc computation; R-format with major/minor arc
  sign logic; guard for 180° arcs in R format
- `postprocessor/modal.rs` — `ModalState` suppression for motion code, feed,
  spindle speed, coordinate words (1e-6 mm tolerance), plane, distance mode
- `postprocessor/program.rs` — full program assembler: header, tool changes,
  pass emission, footer, percent delimiters; returns `NotSupported` (not panic)
  for 5-axis paths or unsupported cycle types
- `postprocessor/mod.rs` — public API: `PostProcessor::builtin()`,
  `from_file()`, `list_builtins()`, `generate()`; `PostProcessorMeta` struct

**Built-in post-processor configs** (`2afdc6f`)
- `fanuc-0i.toml` — Fanuc 0i-MD/MF, metric, `\r\n` EOL, IJK arcs, canned
  cycles, percent delimiters
- `linuxcnc.toml` — LinuxCNC 2.x, metric, `\n` EOL, IJK arcs, canned cycles
- `mach4.toml` — Mach4 Mill, metric, IJK arcs, canned cycles
- `grbl.toml` — GRBL 1.1, metric, no line numbers, no canned cycles (expand to
  linear moves)

**IPC commands** (`5c27561`, `1712316`)
- `list_post_processors` → `Vec<PostProcessorMeta>` (no state access needed)
- `get_gcode_preview(operation_id, post_processor_id)` → `String` (returns
  `AppError::NotFound` when no toolpath exists for the operation)
- `export_gcode(ExportParams)` → writes `.nc` to disk; `ExportParams` includes
  operation IDs, post-processor ID, output path, program number, include-comments

**Golden file integration tests** (`4877867`)
- `src-tauri/tests/gcode_golden.rs`: `fanuc_0i_golden_matches` and
  `linuxcnc_golden_matches`; each reads fixture JSON, generates G-code, asserts
  byte-for-byte match against checked-in `.nc` golden file
- Fixture toolpath JSON: `tests/integration/golden_gcode/fanuc-0i/simple_pocket.toolpath.json`,
  same for `linuxcnc/` (covers: rapid + arc lead-in pass, feed cutting pass, rapid lead-out pass)
- Golden files: `tests/integration/golden_gcode/fanuc-0i/simple_pocket.nc`,
  `linuxcnc/simple_pocket.nc`

### UI (partially complete)

**Operation list panel** (`0d7f3fa`)
- `OperationListPanel` component renders the list of operations
- Per-item display: name, type label, enable/disable checkbox, delete button
- Three add buttons (`+ Profile`, `+ Pocket`, `+ Drill`) create operations
  with default parameters directly; no parameter editor form yet
- Reads operations from Zustand store (populated via `get_project_snapshot`);
  `list_operations` is called only within the toggle-enabled mutation flow

**Error notifications** (`42bd7dc`)
- `Notifications` component in `src/components/common/Notifications.tsx`:
  dismissible toasts for IPC errors
- All IPC error paths in `OperationListPanel` route through the notification
  system
- `selectedOperationId` + `setSelectedOperationId` + `usePushNotification` +
  `useSelectedOperationId` added to `projectStore.ts`

**G-code preview panel** (`75733b9`, `3b94300`)
- `GCodePreviewPanel` in `src/components/gcode/GCodePreviewPanel.tsx`:
  post-processor selector dropdown, scrollable G-code `<pre>` view, Export
  button with native save dialog
- Shows placeholder when no operation selected or no toolpath computed yet
  (backend returns `NotFound`)
- `src/api/toolpath.ts`: `listPostProcessors()`, `getGcodePreview()`,
  `exportGcode()` API wrappers
- `PostProcessorMeta` and `ExportParams` TypeScript types added to
  `src/api/types.ts`
- Panel mounted in `AppShell.tsx` sidebar below `OperationListPanel`

**App shell** (`79c78bf`, updated `75733b9`)
- `AppShell` layout with Toolbar + Viewport + right sidebar
  (`OperationListPanel` + `GCodePreviewPanel`) + `Notifications`
- `Toolbar` component with file operations (New, Open, Save, Save As)

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
| Toolpath visualization | Three.js `LineSegments` per segment type |
| Progress events | Tokio task progress → `emit()` → frontend progress bar |
| Cache invalidation | SHA-256 cache key, stale detection; `needs_recalculate` is a placeholder (`true` always) |
| Toolpath binary format | `toolpaths/*.bin` in `.jcam` |
| Feeds and speeds | Tool defaults + per-operation override |

---

## Phases 2–5

Nothing from Phases 2–5 is implemented. The file structure in
`src-tauri/src/simulation/` does not yet exist (a design target, not current
code). `src-tauri/src/toolpath/` exists but contains only type definitions —
no planner or algorithm code. `src-tauri/src/postprocessor/` is complete (see
above). `kinematics.rs` and `cycles.rs` are explicitly not created yet;
the engine returns `PostProcessorError::NotSupported` when a 5-axis path is
encountered.

---

## Test coverage

| Test file | Coverage |
|---|---|
| `src/viewport/scene.test.ts` | Three.js scene setup |
| `src/viewport/controls.test.ts` | Camera controls |
| `src/viewport/modelMesh.test.ts` | Mesh construction |
| `src/store/projectStore.test.ts` | Zustand store actions (including `selectedOperationId`) |
| `src/store/viewportStore.test.ts` | Viewport store |
| `src/components/toolbar/Toolbar.test.tsx` | Toolbar component |
| `src/components/operations/OperationListPanel.test.tsx` | Operation list panel |
| `src/components/common/Notifications.test.tsx` | Error notification toasts |
| `src/viewport/Viewport.test.tsx` | Viewport component mount/unmount, mesh updates |
| `src/App.test.tsx` | App smoke test (renders AppShell) |
| `src/components/gcode/GCodePreviewPanel.test.tsx` | G-code preview panel |
| `src-tauri/cpp/tests/` | C++ geometry wrapper (doctest) |
| `src-tauri/tests/gcode_golden.rs` | Golden-file integration: fanuc-0i, linuxcnc |
| `src-tauri/src/postprocessor/` (inline) | Config parse, formatter, modal, arcs, block, program, public API |
| `src-tauri/src/commands/` (inline) | All command handlers: file ops (open/save/load/new project, export G-code), tool CRUD, stock/WCS, operations CRUD, project snapshot, toolpath/G-code preview |
| `src-tauri/src/models/` (inline) | Tool, stock, WCS, operation — serde round-trips and field invariants |
| `src-tauri/src/` (inline) | `error.rs` variants, `state.rs` defaults, `toolpath/types.rs` serde, `project/serialization.rs` round-trips |

The golden toolpath integration tests described in the roadmap now exist for
the post-processor output stage. The golden toolpath-geometry tests (CAM
algorithm output) do not yet exist — they depend on the CAM algorithms not yet
written.

---

## Key files by area

### Rust backend
| File | Purpose |
|---|---|
| `src-tauri/src/main.rs` | Thin binary entry point (calls `lib.rs::run()`) |
| `src-tauri/src/lib.rs` | Tauri app init, IPC command registration |
| `src-tauri/src/state.rs` | `AppState`, `RwLock<Project>`, `Project.toolpaths` |
| `src-tauri/src/error.rs` | `AppError` enum (incl. `PostProcessor` variant) |
| `src-tauri/src/models/tool.rs` | `Tool`, `ToolType` |
| `src-tauri/src/models/stock.rs` | `StockDefinition`, `BoxDimensions`, `Vec3` |
| `src-tauri/src/models/wcs.rs` | `WorkCoordinateSystem` |
| `src-tauri/src/models/operation.rs` | `Operation` struct, `OperationParams` enum |
| `src-tauri/src/toolpath/types.rs` | `Toolpath`, `Pass`, `CutPoint`, `MoveKind` |
| `src-tauri/src/postprocessor/config.rs` | TOML schema deserialization + validation |
| `src-tauri/src/postprocessor/formatter.rs` | Number formatting + template substitution |
| `src-tauri/src/postprocessor/block.rs` | Block/word assembly and rendering |
| `src-tauri/src/postprocessor/arcs.rs` | IJK and R-format arc computation |
| `src-tauri/src/postprocessor/modal.rs` | Modal G-code state suppression |
| `src-tauri/src/postprocessor/program.rs` | Full G-code program assembler |
| `src-tauri/src/postprocessor/mod.rs` | `PostProcessor` public API |
| `src-tauri/src/postprocessor/builtins/` | `fanuc-0i.toml`, `linuxcnc.toml`, `mach4.toml`, `grbl.toml` |
| `src-tauri/src/commands/file.rs` | `open_model`, `save_project`, `load_project`, `new_project`, `export_gcode` |
| `src-tauri/src/commands/toolpath.rs` | `list_post_processors`, `get_gcode_preview` |
| `src-tauri/src/commands/tools.rs` | Tool CRUD commands |
| `src-tauri/src/commands/stock.rs` | Stock/WCS commands |
| `src-tauri/src/commands/operations.rs` | Operation CRUD commands |
| `src-tauri/src/commands/project.rs` | `get_project_snapshot` |
| `src-tauri/src/geometry/importer.rs` | Format dispatch (STEP/STL) |
| `src-tauri/src/geometry/safe.rs` | Safe Rust wrappers: `OcctShape`, `OcctMesh`, `Drop` |
| `src-tauri/src/geometry/ffi.rs` | bindgen-generated FFI |
| `src-tauri/src/project/serialization.rs` | `.jcam` ZIP read/write |

### C++ geometry wrapper
| File | Purpose |
|---|---|
| `src-tauri/cpp/cam_geometry.h` | Public C API contract |
| `src-tauri/cpp/cam_geometry.cpp` | OCCT implementation |
| `src-tauri/cpp/handle_registry.h/cpp` | uint64 handle → C++ object map |
| `src-tauri/cpp/third_party/Clipper2/` | Vendored 2D polygon library |

### Integration test fixtures
| File | Purpose |
|---|---|
| `tests/fixtures/box.step` | STEP geometry fixture |
| `tests/fixtures/box.stl` | STL geometry fixture |
| `tests/integration/golden_gcode/fanuc-0i/simple_pocket.toolpath.json` | Fixture toolpath (rapids, feeds, arc) |
| `tests/integration/golden_gcode/fanuc-0i/simple_pocket.nc` | Golden G-code output for Fanuc 0i |
| `tests/integration/golden_gcode/linuxcnc/simple_pocket.toolpath.json` | Same fixture for LinuxCNC |
| `tests/integration/golden_gcode/linuxcnc/simple_pocket.nc` | Golden G-code output for LinuxCNC |

### TypeScript frontend
| File | Purpose |
|---|---|
| `src/api/types.ts` | TypeScript mirrors of Rust types (incl. `PostProcessorMeta`, `ExportParams`) |
| `src/api/file.ts` | File operation IPC wrappers |
| `src/api/tools.ts` | Tool CRUD IPC wrappers |
| `src/api/stock.ts` | Stock/WCS IPC wrappers |
| `src/api/operations.ts` | Operation CRUD IPC wrappers |
| `src/api/toolpath.ts` | `listPostProcessors`, `getGcodePreview`, `exportGcode` |
| `src/store/projectStore.ts` | Project Zustand store (incl. `selectedOperationId`) |
| `src/store/viewportStore.ts` | Viewport Zustand store |
| `src/viewport/scene.ts` | Three.js renderer + scene |
| `src/viewport/controls.ts` | OrbitControls (Z-up) |
| `src/viewport/modelMesh.ts` | `MeshData` → `BufferGeometry` |
| `src/components/layout/AppShell.tsx` | Top-level layout |
| `src/components/operations/OperationListPanel.tsx` | Operation list sidebar |
| `src/components/toolbar/Toolbar.tsx` | File operation toolbar |
| `src/components/common/Notifications.tsx` | IPC error toast/snackbar |
| `src/components/gcode/GCodePreviewPanel.tsx` | G-code preview with PP selector + Export |

---

*Related documents: `development-roadmap.md`, `system-architecture.md`, `gcode-postprocessor.md`*
