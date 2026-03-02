# Implementation Status

_Last updated: 2026-03-02. Based on git history (101 commits, branch `main`)._

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
TypeScript frontend. The post-processor engine and G-code export pipeline are
complete, including four built-in post-processor configs, golden-file
integration tests, IPC commands, and a G-code preview panel with export
functionality. The pocket clearing, profile contouring, and drilling CAM
algorithms, Clipper2 polygon integration, toolpath linking, planner, IPC
calculate/geometry commands, toolpath visualization in the viewport, operation
editor forms (pocket, profile, and drill), per-operation feed/speed overrides,
and Calculate button are all implemented and tested end-to-end. The toolpath
cache system is complete: SHA-256 cache keys are computed and stored after
calculate, toolpaths are persisted as JSON entries in the `.jcam` ZIP archive,
restored on load, and displayed in the viewport immediately; `needs_recalculate`
is computed via a real cache-key comparison and shown as a "(stale)" indicator
in the UI. The tool library UI (`ToolLibraryPanel`), stock definition UI (`StockPanel`),
and WCS panel (`WCSPanel`) are implemented and mounted in the right sidebar; all
three panels are fully tested. Operation reorder controls (▲/▼ buttons per row)
and a calculate loading state (in-flight '…' indicator with all Calculate
buttons disabled while any calculation is running) are also implemented and
tested in `OperationListPanel`. Geometry selection and progress events are the
remaining items for Phase 1.

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

**Operations** (`97c6ac2`, `9c04c01`, updated `eebfd3d`, `f5e0efc`)
- `Operation` struct (common fields: `id`, `name`, `enabled`, `tool_id`,
  `spindle_speed_override`, `feed_rate_override`, `cache: CacheState`) +
  `OperationParams` enum (`Profile`, `Pocket`, `Drill`) flattened alongside it
- Project integration — operations stored in `Vec<Operation>`; each carries a
  UUID `id` field
- `ProjectSnapshot` carries `Vec<OperationSummary>` (id, name, operationType, enabled, needsRecalculate) to frontend (`7695e8b`)
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

### CAM algorithms (complete for pocket, profile, and drill)

**Clipper2 C++ implementation** (`4700298`)
- `cg_poly_offset` and `cg_poly_boolean` stubs replaced with real Clipper2
  implementations in `cam_geometry.cpp`
- Coordinates scaled ×1e6 into integer space before calling
  `InflatePaths`/`Union`/`Difference`/`Intersect`, then scaled back
- When multiple paths result, the largest-area path is returned
- Doctest suite (`src-tauri/cpp/tests/test_geometry.cpp`): inward/outward
  offset, collapse-to-empty, intersection, union, difference,
  non-overlapping intersection

**Safe Rust wrappers for Clipper2** (`74ef19c`)
- `src-tauri/src/geometry/clipper.rs`: `BoolOp` enum, `poly_offset()`,
  `poly_boolean()`
- Dual-compiled: real FFI behind `#[cfg(cam_geometry_bindings)]`; stub
  returning `GeometryError::ImportFailed` behind `#[cfg(not(...))]`
- `geometry/mod.rs` declares and re-exports the new module
- Unit tests for both the stub path and (gated) integration path

**Toolpath types extension** (`edf7fe0`)
- `ToolpathStats` struct: `total_pass_count`, `total_point_count`,
  `total_path_length_mm`; camelCase serde, inline round-trip tests
- `LineGeometryData` struct: flat `positions`/`colours`/`types` arrays
  for Three.js; serde-serializable

**Toolpath linking** (`99a92ab`)
- `src-tauri/src/toolpath/linking.rs`: `link_passes()` wraps each cutting
  pass with `LeadOut`, `Linking` (three rapids: lift/traverse/descend),
  and `LeadIn` passes; `LeadIn` is skipped when the current cutting pass
  has ≤1 point; `LeadOut` is skipped for the first pass and when the
  previous cutting pass has ≤1 point

**Pocket clearing algorithm** (`646154b`)
- `src-tauri/src/toolpath/operations/pocket.rs`: `pocket_passes(stock, params, tool_diameter)`
- Constructs stock boundary rectangle; inward-offsets by tool radius for
  first contour, then repeatedly by stepover until polygon collapses
- Repeats per Z depth level (`stepdown` increments down to `depth`)
- Returns `AppError::GeometryImport` if tool is too large for stock
- Unit tests (all gated on `cam_geometry_bindings`): Z-level count,
  non-empty output for valid tool, error propagation for oversized tool

**Toolpath planner** (`3babbb6`, updated `1b405df`, `b529693`)
- `src-tauri/src/toolpath/planner.rs`: `plan(operation, tool, stock)`
- Dispatches to `pocket_passes` or `profile_passes` (then `link_passes`)
  or `drill_passes` (no link pass wrapping); assembles `Toolpath` and
  computes `ToolpathStats` separately; returns both as a tuple
- Feed/speed override logic: operation-level `spindle_speed_override` /
  `feed_rate_override` take priority over tool defaults, which fall back to
  hardcoded values (8000 RPM / 500 mm/min)
- Unit tests: stats non-zero for Pocket and Profile (gated on bindings);
  error for Profile without geometry bindings (stub path); six tests
  covering all override/fallback combinations for spindle speed and feed rate
  (ungated — use a drill operation with a single point so no geometry
  bindings are needed)

**Pocket algorithm golden file test** (`f52cdc5`)
- `src-tauri/tests/pocket_golden.rs`: `pocket_algorithm_golden_matches`
  gated via `#![cfg(cam_geometry_bindings)]`
- Exercises full planner pipeline (50×50×10 mm stock, 10 mm flat endmill,
  depth=10/stepdown=2/stepover=50%); compares serialized toolpath JSON
  against committed golden fixture
- Golden fixture: `tests/integration/pocket/toolpath.json` (2847 lines)
- `[[test]]` entries added to `src-tauri/Cargo.toml` for both
  `gcode_golden` and `pocket_golden`

**Profile contouring algorithm** (`ad562cd`)
- `src-tauri/src/toolpath/operations/profile.rs`: `profile_passes(stock, params, tool_diameter)`
- Constructs stock boundary rectangle; offsets inward (Left) or outward
  (Right) by tool radius, or uses raw boundary (Center) for the single
  cutting contour; repeats contour per Z depth level (`stepdown` down to
  `depth`)
- Returns `AppError::GeometryImport` if Left/Right offset collapses entirely
  (tool too large); Center never fails since it skips `poly_offset`
- Geometry-gated tests: Z-level count, non-empty for Left, collapse when
  tool too large, Left vs Center produce different contours
- Ungated test: Center compensation uses raw boundary (compiles without
  geometry bindings)

**Profile algorithm golden file test** (`7e7da0f`)
- `src-tauri/tests/profile_golden.rs`: `profile_algorithm_golden_matches`
  gated via `#![cfg(cam_geometry_bindings)]`
- Exercises full planner pipeline (50×50×10 mm stock, 6 mm flat endmill,
  depth=10/stepdown=2.5, Left compensation); 4 Z levels with one
  rectangular contour per level offset inward by 3 mm
- Golden fixture: `tests/integration/profile/toolpath.json` (467 lines)
- `[[test]]` entry added to `src-tauri/Cargo.toml` for `profile_golden`

**`DrillPoint` struct and `points` field** (`13b3bc7`)
- `DrillPoint { x: f64, y: f64 }` struct added to `src-tauri/src/models/operation.rs`
- `points: Vec<DrillPoint>` field with `#[serde(default)]` added to `DrillParams`
- 3 serde unit tests: round-trip, non-empty points, default-to-empty behaviour

**Feed/speed override fields on `Operation`** (`eebfd3d`)
- `spindle_speed_override: Option<u32>` and `feed_rate_override: Option<f64>`
  added to `Operation` struct and `OperationInput`
- `#[serde(default, skip_serializing_if = "Option::is_none")]` on both
  fields — absent from JSON when `None` (skip_serializing_if), and default
  to `None` when absent on deserialize (default); both halves needed for
  backward compatibility with existing project files
- Wired through `add_operation_inner` and `edit_operation_inner`; all
  existing `Operation` / `OperationInput` struct literals across the codebase
  updated
- 3 serde unit tests: absent when `None`, present when set, defaults to
  `None` on deserialize when field is absent

**Drill algorithm** (`3169f31`)
- `src-tauri/src/toolpath/operations/drill.rs`: `drill_passes(stock, params)`
- For each hole in `params.points`, produces one `PassKind::Linking` pass
  (rapid to clearance height above the hole) and one `PassKind::Cutting` pass
- Each Cutting pass opens with a `Rapid` approach to clearance height, then:
  full-depth mode (no `peck_depth`): `Feed` plunge to `drill_z`, `Rapid`
  retract to clearance (3 cut points total)
- Peck mode (`peck_depth` set): after the opening `Rapid`, repeated
  feed/retract cycles decrementing by `peck_depth` until `drill_z` is
  reached; uses `.max(drill_z)` to avoid overshooting the target depth
- Returns `AppError::GeometryImport` if `params.points` is empty or if
  `peck_depth` is ≤ 0
- 6 unit tests: empty-points error, zero/negative peck error, single
  non-peck hole geometry (Z values and move kinds), peck hole Z-levels
  (7-point sequence for 3 pecks), two-hole pass ordering and linking structure

**Drill algorithm golden file test** (`7e436df`)
- `src-tauri/tests/drill_golden.rs`: `drill_algorithm_golden_matches`
  (ungated — drill algorithm requires no geometry bindings)
- Exercises full planner pipeline (50×50×10 mm stock, 5 mm drill, 5 holes,
  depth=10/peck_depth=3); validates peck cycling for each of 5 holes
- Golden fixture: `tests/integration/drill/toolpath.json` (651 lines)
- `[[test]]` entry added to `src-tauri/Cargo.toml` for `drill_golden`

### IPC commands (calculate and geometry)

**`calculate_toolpath`** and **`get_toolpath_geometry`** (`0c1d802`, updated `e7b5da1`)
- `calculate_toolpath_inner`: parses operation UUID; reads operation/stock/tool and captures model SHA under read lock; calls `planner::plan`; computes SHA-256 cache key; stores `Toolpath` and populates `operation.cache` (`key`, `valid`, `computed_at`, `stats`; `binary_file` remains `None`) under write lock; returns `ToolpathStats`
- `get_toolpath_geometry_inner`: retrieves stored `Toolpath` and operation
  index (for palette colouring); converts passes to flat-array
  `LineGeometryData`; pre-allocates buffers using segment count
- Both registered in `generate_handler!` list in `lib.rs`
- Unit tests (all in `commands/toolpath.rs`): `list_post_processors_inner`
  returns 4 entries; `calculate_toolpath_inner`: NotFound with no operation,
  NotFound with no stock, stores toolpath for pocket and asserts cache fields
  populated (gated); `get_toolpath_geometry_inner`: NotFound when no toolpath
  stored; `get_gcode_preview_inner`: generates G-code containing rapid and
  feed moves when toolpath exists

### UI (substantially complete for all three operation types)

**Operation editor form** (`53c4f49`, updated `70e8318`, `1318d96`, `bec3737`)
- `OperationEditorForm` in `src/components/operations/OperationEditorForm.tsx`
- Pocket operations: tool select (saves on change) + depth / stepdown /
  stepover / spindle speed override / feed rate override inputs (save on blur)
- Profile operations: tool select (saves on change) + depth / stepdown /
  compensation side (Left/Center/Right) select + spindle speed override /
  feed rate override inputs (save on blur)
- Drill operations: tool select + depth + peck depth + spindle speed override /
  feed rate override + dynamic drill-points table (Add Point / Remove per row,
  each row has X and Y inputs that save on blur)
- `save()` base always carries current `spindleSpeedOverride` and
  `feedRateOverride` values to prevent silent clearing on unrelated saves
- Uses `key={operation.id}` on the rendered div to remount uncontrolled
  inputs when the selected operation changes
- Tests in `OperationEditorForm.test.tsx` cover pocket, profile, and drill
  forms; including add/remove point and override inputs

**Operation list panel — row selection and Calculate** (`f94a19a`, updated `4f62a9d`, `d178a20`, `1318d96`, `9706ff9`, `085504f`, `8491f7f`, `31e1286`)
- Row click sets `selectedOperationId`; selected row highlighted
- `OperationEditorForm` mounted below the list, driven by `selectedOperationId`
- Checkbox per row toggles `enabled`: fetches full operation via
  `listOperations()`, flips `enabled`, calls `editOperation`, refreshes snapshot
- Delete button per row: calls `deleteOperation`, refreshes snapshot
- Add operation buttons at the bottom (+ Profile, + Pocket, + Drill): disabled
  when no tools exist; uses first available tool; calls `addOperation` with
  sensible defaults, refreshes snapshot
- Reorder buttons (▲/▼) per row: call `reorderOperations` to move the operation
  up or down in the list; ▲ disabled for the first row, ▼ disabled for the last
- Calculate button per row: enabled for pocket and profile operations when
  stock is defined; enabled for drill operations when stock is defined AND
  the operation has ≥ 1 drill point; calls `calculateToolpath` →
  `getToolpathGeometry` → `setToolpathGeometry` → `getProjectSnapshot` →
  `setSnapshot` and pushes a stats notification string
- Calculate loading state: a `calculatingId` state tracks the in-flight
  operation ID; the active row's Calculate button shows '…' while calculating;
  all Calculate buttons are disabled while any calculation is running
- `drillPointCounts: Record<string, number>` state maintained via `useEffect`
  that short-circuits (resets to `{}`) when no drill operations exist, and
  otherwise triggers a full `listOperations()` fetch whenever the operations
  list changes; used to gate the Calculate button for drill rows
- `stopPropagation` on checkbox, delete, reorder, and Calculate buttons

**Toolpath visualization** (`63028d8`, `108548a`)
- `src/viewport/toolpathLines.ts`: `buildToolpathLines(data)` builds
  `THREE.LineSegments` with shared `LineBasicMaterial` (vertexColors) and
  per-vertex position/color attributes from `LineGeometryData`
- `viewportStore` gains `toolpathGeometry: LineGeometryData | null` field
  and `setToolpathGeometry` setter
- `SceneManager` gains private `toolpathGroup: THREE.Group` added to scene;
  `setToolpathLines(lines)` disposes previous geometry and replaces group child
- `Viewport.tsx` subscribes to `toolpathGeometry` via `useEffect` and drives
  `setToolpathLines` via `buildToolpathLines`
- Frontend API: `calculateToolpath()` and `getToolpathGeometry()` wrappers
  added to `src/api/toolpath.ts`; `ToolpathStats` and `LineGeometryData`
  TypeScript interfaces added to `src/api/types.ts`
- Tests: `toolpathLines.test.ts` (6 tests: null input, empty positions,
  LineSegments instance type, position attribute count, color attribute count,
  vertexColors on material)

**Error notifications** (`42bd7dc`)
- `Notifications` component in `src/components/common/Notifications.tsx`:
  dismissible toasts with auto-dismiss after 5 seconds
- `usePushNotification` used by OperationListPanel, StockPanel, WCSPanel,
  ToolLibraryPanel, OperationEditorForm, and GCodePreviewPanel; `Toolbar` uses
  its own local `errorMsg: string | null` state and an inline dismissible banner
  (not the shared notification system)
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

**Stock panel** (`1dc1b62`, `dba633a`)
- `StockPanel` in `src/components/stock/StockPanel.tsx`: form UI for box stock
  definition with six numeric inputs (origin X/Y/Z, width, depth, height)
- Shows current stock dimensions (origin and size) when stock is defined; shows
  "No stock defined" when null
- "Set Stock" button calls `setStock(payload)` then refreshes the project
  snapshot via `getProjectSnapshot()` → `setSnapshot()`
- "Clear Stock" button (only rendered when stock is defined) calls
  `setStock(null)` then refreshes the snapshot
- Error notifications via `usePushNotification` for all failure paths
- Tests (5): null state/"No stock defined", stock defined shows values and Clear
  button, Set Stock submit calls correct payload, Clear Stock calls setStock(null),
  error notification on Set Stock reject

**WCS panel** (`cc8a5da`, `b45da2c`)
- `WCSPanel` in `src/components/wcs/WCSPanel.tsx`: form UI for WCS origin
  editing with three numeric inputs (origin X, Y, Z)
- "Set WCS" button calls `setWcs(payload)` then refreshes the project snapshot
- "Clear WCS" button calls `setWcs(null)` then refreshes the snapshot (only
  rendered when a WCS is defined)
- Error notifications via `usePushNotification` for all failure paths
- Mounted in `AppShell` between `StockPanel` and `OperationListPanel`
- Tests (`WCSPanel.test.tsx`): display, Set WCS, Clear WCS, error — 6 tests

**Tool library panel** (`0859606`, `58f6965`)
- `ToolLibraryPanel` in `src/components/tools/ToolLibraryPanel.tsx`: three-mode
  component (list, add form, edit form) for managing the project tool library
- List mode: renders a row per tool from the snapshot (name + type label + Edit
  and Delete buttons); "Add Tool" button switches to add form
- Add form: inputs for name, type (dropdown from 10 tool type values), material,
  diameter, flute count, and optional default spindle speed / feed rate; submits
  via `addTool()`, refreshes snapshot, returns to list mode; Cancel discards
- Edit: clicking Edit fetches full tool data via `listTools()` and pre-populates
  the shared `ToolForm` with existing values; submits via `editTool(id, input)`,
  refreshes snapshot, returns to list
- Delete: calls `deleteTool(id)` then refreshes snapshot
- All mutations call `getProjectSnapshot()` → `setSnapshot()` on success
- Error notifications via `usePushNotification` for all failure paths (addTool,
  editTool, deleteTool, and listTools on edit click)
- Tests (12): renders tool names/types, renders empty with null snapshot, opens
  add form, submits add form, cancel add form, fetches and pre-populates edit
  form, submits edit form with correct args, delete calls deleteTool + refreshes,
  and error notification for each of the four error paths (addTool, editTool,
  deleteTool, listTools on edit click)

**App shell** (`79c78bf`, updated `75733b9`, `c302972`, `6a911ac`, `cc8a5da`)
- `AppShell` layout with Toolbar + Viewport + right sidebar
  (`ToolLibraryPanel` + `StockPanel` + `WCSPanel` + `OperationListPanel` + `GCodePreviewPanel`) + `Notifications`
- `Toolbar` component with file operations (Open Model, New Project, Save Project, Open Project)
- `handleOpenProject` calls `getToolpathGeometry` → `setToolpathGeometry` for
  each non-stale operation after load so cached toolpaths appear in viewport
  immediately; failures per-operation are caught and silently skipped

### Toolpath cache system (complete)

**SHA-256 cache key module** (`1a8475e`)
- `src-tauri/src/toolpath/cache.rs`: `compute_cache_key(operation, tool, stock, model_sha, engine_version) -> String`
- Key covers: operation geometry (all params), tool geometry (diameter, flute_count, material, type — excluding display-only `id`, `name`, `default_spindle_speed`, `default_feed_rate`), stock definition, optional model content SHA, engine version string
- Feed/speed override fields (`spindle_speed_override`, `feed_rate_override`) intentionally excluded — changing overrides alone does not invalidate a cached toolpath
- Returns `"sha256:<lowercase hex digest>"` format
- 4 unit tests: stability (same inputs → same key), sensitivity to tool diameter change, operation param change, and model SHA presence/absence

**CachedStats and CacheState data model** (`f5e0efc`)
- `CachedStats` struct: `total_pass_count: u32`, `total_point_count: u32`, `total_path_length_mm: f64`; camelCase serde with `#[serde(default)]`
- `CacheState` struct: `key: Option<String>`, `valid: bool`, `computed_at: Option<String>`, `stats: Option<CachedStats>`, `binary_file: Option<String>`; camelCase serde with `#[serde(default)]`
- `cache: CacheState` field added to `Operation` struct with `#[serde(default)]` for backward compatibility with existing project files
- 2 serde tests: `cache_field_defaults_when_absent`, `cache_state_round_trip`

**Populate cache after successful calculate** (`e7b5da1`)
- `calculate_toolpath_inner` captures model checksum during read-lock phase, computes SHA-256 cache key after `planner::plan` returns, then writes `key`, `valid: true`, `computed_at` (UTC ISO-8601 `SecondsFormat::Secs`), and `stats` into `operation.cache` inside the write-lock; `binary_file` is left `None` until the project is saved
- Existing pocket toolpath test extended to assert all cache fields are correctly populated, including asserting `binary_file` remains `None`

**Toolpath persistence in `.jcam`** (`46f98ed`, `9544226`)
- `write_archive`: clones the operations list; for each cloned op with `cache.valid = true` and a matching toolpath in `project.toolpaths`, writes the toolpath JSON as `toolpaths/<uuid>.json` inside the ZIP and sets `cache.binary_file` on the clone; the clones (with `binary_file` set) are serialized into `project.json`; the live in-memory `project.operations` are unchanged — `binary_file` remains `None` in memory until the project is reloaded; operations with `cache.valid = false` are skipped; propagates `AppError::ProjectSave` on write errors
- `load()`: after constructing the Project, iterates operations; for each with `cache.binary_file` set, reads and deserializes the toolpath JSON entry from the ZIP; missing or unparseable entries emit `tracing::warn` and are silently skipped — load never fails due to stale or absent toolpath data
- 4 tests: `toolpath_entry_written_to_zip` (positive), `invalid_cache_not_written_to_zip` (negative), `round_trip_with_valid_toolpath` (full save/load cycle), `load_ignores_missing_toolpath_entry_gracefully`

**Real cache-key comparison for `needs_recalculate`** (`edf9a61`)
- `From<&Project> for ProjectSnapshot` now performs a real SHA-256 comparison instead of returning hardcoded `true`
- Logic: short-circuits to `true` when `cache.key` is absent or `cache.valid` is false, or when the operation's tool or project stock is missing; otherwise recomputes the SHA-256 key and compares against stored key
- 2 new tests: `snapshot_needs_recalculate_false_when_cache_key_current`, `snapshot_needs_recalculate_true_after_model_checksum_change`

**Stale indicator in OperationListPanel** (`9706ff9`)
- Rows with `needsRecalculate: true` display an amber "(stale)" label
- Snapshot refreshed via `getProjectSnapshot()` after a successful toolpath calculate so the stale indicator clears immediately
- 2 new tests: stale indicator rendering, post-calculate snapshot refresh

**End-to-end cache integration test** (`89a3d90`)
- `src-tauri/tests/toolpath_cache.rs`: 2 scenarios (ungated — uses drill operations requiring no geometry bindings): save/load round-trip preserves toolpath and cache validity; mutating a param after load marks operation stale
- `[[test]]` entry added to `src-tauri/Cargo.toml`
- `calculate_toolpath_inner` and `get_project_snapshot_inner` exposed as `pub` for integration test access

### Not yet implemented (Phase 1)

| Item | Notes |
|---|---|
| Geometry selection | Click faces/edges in viewport; face fingerprinting |
| Progress events | Tokio task progress → `emit()` → frontend progress bar |

---

## Phases 2–5

Nothing from Phases 2–5 is implemented. `src-tauri/src/simulation/` does not
yet exist. `kinematics.rs` and `cycles.rs` are not created; the post-processor
engine returns `PostProcessorError::NotSupported` when a 5-axis path is
encountered.

---

## Test coverage

| Test file | Coverage |
|---|---|
| `src/viewport/scene.test.ts` | Three.js scene setup |
| `src/viewport/controls.test.ts` | Camera controls |
| `src/viewport/modelMesh.test.ts` | Mesh construction |
| `src/viewport/toolpathLines.test.ts` | `buildToolpathLines`: null input, empty positions, LineSegments instance type, position attribute count, color attribute count, vertexColors on material — 6 tests |
| `src/store/projectStore.test.ts` | Zustand store actions (including `selectedOperationId`, `useTools`, `useStock` selectors) |
| `src/store/viewportStore.test.ts` | Viewport store (including `toolpathGeometry`) |
| `src/components/toolbar/Toolbar.test.tsx` | Toolbar: Open Model (calls openModel, updates meshData+snapshot, cancellation, error+dismiss), New Project (clears meshData, updates snapshot, error), Save Project (calls saveProject, cancellation, error), Open Project (loadProject, model reload, meshData clear, error, getToolpathGeometry for non-stale, skip stale) — 22 tests across 4 describe blocks |
| `src/components/operations/OperationListPanel.test.tsx` | Operation list: rendering (5), add buttons disabled/enabled/addOperation calls per type/snapshot refresh (6), enable/disable toggle (2), delete (2), row selection and OperationEditorForm mount (3), stale indicator (2), Calculate button gates and behaviour (12), reorder (7), calculate loading state (4) — 43 tests across 9 describe blocks |
| `src/components/operations/OperationEditorForm.test.tsx` | OperationEditorForm: null state, profile form (inputs/defaults/save-on-blur+change/overrides), pocket form (tool select/inputs/overrides/remount-on-id-change), tool change saves, input blur saves, drill form (inputs/add-point/remove-point/overrides), error handling — 23 tests across 7 describe blocks |
| `src/components/common/Notifications.test.tsx` | Notifications: no toasts when empty, renders on add, renders multiple, click × dismisses, auto-dismisses after 5 s — 5 tests |
| `src/components/stock/StockPanel.test.tsx` | StockPanel: null state, stock defined, Set Stock submit, Clear Stock, error notification |
| `src/components/wcs/WCSPanel.test.tsx` | WCSPanel: display, Set WCS, Clear WCS, error notification — 6 tests |
| `src/components/tools/ToolLibraryPanel.test.tsx` | ToolLibraryPanel: rendering, add/cancel/submit form, edit pre-populate/submit, delete, error notifications (12 tests) |
| `src/viewport/Viewport.test.tsx` | Viewport: mount/unmount (4), mesh updates — add/replace/remove ModelGroup, frameModel called (4); SceneManager mock includes `setToolpathLines` — 8 tests |
| `src/App.test.tsx` | App smoke test (renders AppShell) |
| `src/components/gcode/GCodePreviewPanel.test.tsx` | GCodePreviewPanel: placeholder when no op selected, placeholder when NotFound, renders G-code text, Export button calls exportGcode, PP selector populated from listPostProcessors — 5 tests |
| `src-tauri/cpp/tests/` | C++ geometry wrapper: OCCT loaders + Clipper2 offset/boolean (doctest) |
| `src-tauri/tests/gcode_golden.rs` | Golden-file integration: fanuc-0i, linuxcnc |
| `src-tauri/tests/pocket_golden.rs` | Golden-file integration: pocket algorithm JSON output (`#[cfg(cam_geometry_bindings)]`) |
| `src-tauri/tests/profile_golden.rs` | Golden-file integration: profile algorithm JSON output (`#[cfg(cam_geometry_bindings)]`) |
| `src-tauri/tests/drill_golden.rs` | Golden-file integration: drill algorithm JSON output (ungated; 5 holes, peck drilling) |
| `src-tauri/tests/toolpath_cache.rs` | End-to-end cache round-trip: save/load preserves toolpath + validity; param mutation marks stale (ungated; uses drill operations) |
| `src-tauri/src/postprocessor/` (inline) | Config parse, formatter, modal, arcs, block, program, public API |
| `src-tauri/src/commands/` (inline) | All command handlers: file ops, tool CRUD, stock/WCS, operations CRUD, project snapshot (incl. real `needs_recalculate` comparison + 2 new tests), toolpath (calculate + cache populate, get_geometry, G-code preview) |
| `src-tauri/src/models/` (inline) | Tool, stock, WCS, operation — serde round-trips and field invariants; `DrillPoint` round-trip/non-empty/default-empty; `Operation` feed/speed override absent-None/present-set/default-None; `CacheState` defaults-when-absent/round-trip |
| `src-tauri/src/toolpath/` (inline) | `types.rs` serde, `cache.rs` key stability + sensitivity (4 tests), `linking.rs` pass wrapping, `planner.rs` dispatch + feed/speed override/fallback, `operations/pocket.rs` Z-levels/output/error, `operations/profile.rs` Z-levels/compensation/collapse, `operations/drill.rs` empty/bad-peck errors, non-peck geometry, peck Z-levels, multi-hole ordering |
| `src-tauri/src/project/` (inline) | `serialization.rs` round-trips, toolpath ZIP entry write (positive + negative), round-trip with valid toolpath, graceful load with missing entry |
| `src-tauri/src/geometry/clipper.rs` (inline) | Stub path always; integration path (offset/boolean) gated on bindings |
| `src-tauri/src/` (inline) | `error.rs` variants, `state.rs` defaults |

Golden-file tests cover the post-processor output stage (G-code) and all three
CAM algorithm output stages (pocket, profile, and drill toolpath JSON). The
pocket and profile golden tests are gated on `cam_geometry_bindings`; the drill
golden test is ungated since drilling requires no geometry bindings.

---

## Key files by area

### Rust backend
| File | Purpose |
|---|---|
| `src-tauri/src/main.rs` | Thin binary entry point (calls `lib.rs::run()`) |
| `src-tauri/src/lib.rs` | Tauri app init, IPC command registration |
| `src-tauri/src/state.rs` | `AppState`, `RwLock<Project>`, `Project.toolpaths` |
| `src-tauri/src/error.rs` | `AppError` enum (thiserror, adjacently-tagged serde); variants: `FileNotFound`, `GeometryImport`, `Io`, `ProjectLoad`, `ProjectSave`, `UnsupportedFormat`, `NotFound`, `PostProcessor` |
| `src-tauri/src/models/tool.rs` | `Tool`, `ToolType` |
| `src-tauri/src/models/stock.rs` | `StockDefinition`, `BoxDimensions`, `Vec3` |
| `src-tauri/src/models/wcs.rs` | `WorkCoordinateSystem` |
| `src-tauri/src/models/operation.rs` | `Operation` struct, `OperationParams` enum, `CacheState`, `CachedStats` |
| `src-tauri/src/toolpath/types.rs` | `Toolpath`, `Pass`, `PassKind`, `CutPoint`, `MoveKind`, `ToolOrientation`, `ToolpathStats`, `LineGeometryData` |
| `src-tauri/src/toolpath/linking.rs` | `link_passes()` — retract/traverse/descend between cutting passes |
| `src-tauri/src/toolpath/planner.rs` | `plan()` — dispatches to algorithm, links passes, computes stats |
| `src-tauri/src/toolpath/operations/pocket.rs` | Pocket clearing algorithm (concentric offset contours per Z level) |
| `src-tauri/src/toolpath/operations/profile.rs` | Profile contouring algorithm (single offset contour per Z level) |
| `src-tauri/src/toolpath/operations/drill.rs` | Drill cycle algorithm (linking + cutting passes per hole, peck support) |
| `src-tauri/src/toolpath/cache.rs` | `compute_cache_key()` — deterministic SHA-256 cache key for toolpath operations |
| `src-tauri/src/geometry/clipper.rs` | Safe Rust wrappers: `poly_offset`, `poly_boolean`, `BoolOp` |
| `src-tauri/src/postprocessor/config.rs` | TOML schema deserialization + validation |
| `src-tauri/src/postprocessor/formatter.rs` | Number formatting + template substitution |
| `src-tauri/src/postprocessor/block.rs` | Block/word assembly and rendering |
| `src-tauri/src/postprocessor/arcs.rs` | IJK and R-format arc computation |
| `src-tauri/src/postprocessor/modal.rs` | Modal G-code state suppression |
| `src-tauri/src/postprocessor/program.rs` | Full G-code program assembler |
| `src-tauri/src/postprocessor/mod.rs` | `PostProcessor` public API |
| `src-tauri/src/postprocessor/builtins/` | `fanuc-0i.toml`, `linuxcnc.toml`, `mach4.toml`, `grbl.toml` |
| `src-tauri/src/commands/file.rs` | `open_model`, `save_project`, `load_project`, `new_project`, `export_gcode` |
| `src-tauri/src/commands/toolpath.rs` | `list_post_processors`, `get_gcode_preview`, `calculate_toolpath`, `get_toolpath_geometry` |
| `src-tauri/src/commands/tools.rs` | Tool CRUD commands |
| `src-tauri/src/commands/stock.rs` | Stock/WCS commands |
| `src-tauri/src/commands/operations.rs` | Operation CRUD commands |
| `src-tauri/src/commands/project.rs` | `get_project_snapshot` |
| `src-tauri/src/geometry/importer.rs` | Format dispatch (STEP/STL) |
| `src-tauri/src/geometry/safe.rs` | Safe Rust wrappers: `OcctShape`, `OcctMesh` (with `Drop` impls) |
| `src-tauri/src/geometry/ffi.rs` | FFI bindings module: includes bindgen output written to `$OUT_DIR` at build time |
| `src-tauri/src/project/serialization.rs` | `.jcam` ZIP read/write; toolpath JSON persistence per operation |

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
| `tests/integration/pocket/toolpath.json` | Golden pocket algorithm output (50×50×10 mm, 10 mm tool) |
| `tests/integration/profile/toolpath.json` | Golden profile algorithm output (50×50×10 mm, 6 mm tool, Left compensation) |
| `tests/integration/drill/toolpath.json` | Golden drill algorithm output (50×50×10 mm, 5 mm drill, 5 holes, 3 mm peck) |

### TypeScript frontend
| File | Purpose |
|---|---|
| `src/api/types.ts` | TypeScript mirrors of Rust types (incl. `PostProcessorMeta`, `ExportParams`) |
| `src/api/file.ts` | File operation IPC wrappers |
| `src/api/tools.ts` | Tool CRUD IPC wrappers |
| `src/api/stock.ts` | Stock/WCS IPC wrappers |
| `src/api/operations.ts` | Operation CRUD IPC wrappers |
| `src/api/toolpath.ts` | `listPostProcessors`, `getGcodePreview`, `exportGcode`, `calculateToolpath`, `getToolpathGeometry` |
| `src/store/projectStore.ts` | Project Zustand store (incl. `selectedOperationId`, `useTools`, `useStock`, `useWcs` selectors) |
| `src/store/viewportStore.ts` | Viewport Zustand store (incl. `toolpathGeometry`) |
| `src/viewport/scene.ts` | Three.js renderer + scene + `toolpathGroup` + `setToolpathLines()` |
| `src/viewport/controls.ts` | OrbitControls (Z-up) |
| `src/viewport/modelMesh.ts` | `MeshData` → `BufferGeometry` |
| `src/viewport/toolpathLines.ts` | `buildToolpathLines()` → `THREE.LineSegments` from `LineGeometryData` |
| `src/components/layout/AppShell.tsx` | Top-level layout |
| `src/components/operations/OperationListPanel.tsx` | Operation list: add/delete/toggle/reorder operations, row selection, stale indicator, Calculate button with loading state, `OperationEditorForm` mount |
| `src/components/operations/OperationEditorForm.tsx` | Pocket, profile, and drill parameter forms; feed/speed override inputs; dynamic drill-points table |
| `src/components/stock/StockPanel.tsx` | Stock definition form: origin, dimensions, Set/Clear Stock buttons |
| `src/components/wcs/WCSPanel.tsx` | WCS panel: origin X/Y/Z editing, Set WCS and Clear WCS buttons |
| `src/components/tools/ToolLibraryPanel.tsx` | Tool library: list, add form, edit form, delete; refreshes project snapshot after each mutation |
| `src/components/toolbar/Toolbar.tsx` | File operation toolbar |
| `src/components/common/Notifications.tsx` | Dismissible toast overlay with auto-dismiss after 5 s |
| `src/components/gcode/GCodePreviewPanel.tsx` | G-code preview with PP selector + Export |

---

*Related documents: `development-roadmap.md`, `system-architecture.md`, `gcode-postprocessor.md`*
