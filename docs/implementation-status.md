# Implementation Status

_Last updated: 2026-03-12. Based on git history (177 commits, branch `main`)._

This document describes what is actually implemented in the codebase, as
distinct from the planned architecture in `development-roadmap.md`. It is
intended to give a quick, honest picture of where the project stands.

---

## Summary

Phase 0 (Foundation) is complete. The architectural seams — OCCT build, Rust
FFI, IPC bridge, Three.js viewport, and `.jcam` file I/O — are all validated
and working on all three target platforms (Linux, macOS, Windows).

Phase 1 (2D Operations MVP) is **complete**. All planned deliverables are
implemented and tested end-to-end.

The data layer for tools, stock, WCS, and operations is fully implemented on
both the Rust backend and the TypeScript frontend. The post-processor engine and
G-code export pipeline are complete, including four built-in post-processor
configs, golden-file integration tests, IPC commands, and a G-code preview
panel with export functionality. The pocket clearing, profile contouring, and
drilling CAM algorithms, Clipper2 polygon integration, toolpath linking,
planner, IPC calculate/geometry commands, toolpath visualization in the
viewport, operation editor forms (pocket, profile, and drill), per-operation
feed/speed overrides, and Calculate button are all implemented and tested. The
toolpath cache system is complete: SHA-256 cache keys are computed and stored
after calculate, toolpaths are persisted as JSON entries in the `.jcam` ZIP
archive, restored on load, and displayed in the viewport immediately;
`needs_recalculate` is computed via a real cache-key comparison and shown as a
"(stale)" indicator in the UI. The tool library UI, stock panel, and WCS panel
are mounted in the right sidebar and fully tested.

Geometry selection is fully implemented: face fingerprinting via SHA-256 of
OCCT face properties, face enumeration over IPC (`get_model_faces`), per-face
triangle groups in `MeshData`, face highlight overlay in the viewport (yellow
hover, blue selected), `viewportStore` selection state, `OperationEditorForm`
geometry section (Select Faces / Done / Clear), and planner geometry resolution
(face fingerprints resolved to 2-D boundary polygons via OCCT before pass
generation).

Progress events are fully implemented: `ToolpathProgressEvent` emitted at five
milestones during `calculate_toolpath` via a Tauri `AppHandle`, a
`listenToolpathProgress` frontend API wrapper, and a `<progress>` element in
`OperationListPanel` that subscribes to events filtered to the active
calculation row.

Phase 2 (2.5D Operations) is **partially complete**. The following Phase 2
deliverables have been implemented: the OCCT Z-section primitive
(`cg_shape_section_at_z`), Z-Level Roughing operation (data model, algorithm,
IPC, UI, golden test), viewport standard view snap shortcuts (T/F/R/I keys with
animated transitions), perspective/orthographic projection toggle (P key and
toolbar button), and display mode selector (Shaded / Shaded+Edges / Wireframe /
Transparent). Additionally: the `LinkingParams` struct refactor, multi-level
profile stepdown (`ProfileParams.stepdown: Option<f64>`), entry motion data
model fields (`arc_lead_in_radius`, `arc_lead_out_radius`, `helical_entry_radius`,
`helical_entry_pitch`, `ramp_entry_angle_deg`) on Profile/Pocket/ZLevelRoughing
params, IPC plumbing of those fields, UI controls for them in the operation forms,
helical entry (spiral descent into closed pockets), ramp entry (linear angled
descent for open contours), and arc lead-in / lead-out (quarter-circle approach
and departure tangent arcs) are all implemented and tested.

Three more Phase 2 deliverables are now complete: **drill sorting** (greedy
nearest-neighbor hole ordering in `drill_passes`), **canned cycle emission**
(G81/G83 blocks emitted by the post-processor assembler when
`cycles.supported = true`), and **canned cycle expansion** (confirmed correct
for GRBL with `cycles.supported = false`). New golden fixtures cover Fanuc 0i,
LinuxCNC, and GRBL drill output. The Phase 2 built-in post-processors
(`mach4.toml`, `grbl.toml`) were delivered early and are documented under
Phase 1.

Two additional Phase 2 deliverables are now complete: **arc fitting** (detect
circular-arc sequences in toolpath chord segments and emit G2/G3 instead of
linear G1 moves; `arc_fitting::fit_arcs()` runs on every pass after linking
with 0.01 mm tolerance; pocket and profile G-code golden files regenerated with
G02/G03 arc commands) and **hole auto-detection** (`cg_shape_find_holes` C++
function using OCCT cylindrical surface analysis, Rust wrapper, `detect_holes`
IPC command, and "Detect Holes" button in the drill operation editor that
populates drill points from model geometry).

Full test suite: 464 Rust tests, 332 frontend tests — all passing.

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
| Rust geometry loaders (STEP, IGES, STL dispatch) | Done | `c4efd9b` |
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

## Phase 1: 2D Operations — Complete

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

**Operations** (`97c6ac2`, `9c04c01`, updated `eebfd3d`, `f5e0efc`, `88473eb`)
- `Operation` struct (common fields: `id`, `name`, `enabled`, `tool_id`,
  `spindle_speed_override`, `feed_rate_override`, `cache: CacheState`) +
  `OperationParams` enum (`Profile`, `Pocket`, `Drill`; extended with
  `ZLevelRoughing` in Phase 2) flattened alongside it
- `PocketParams` and `ProfileParams` both carry `geometry: Option<Vec<String>>`
  — the face fingerprints that define the machining boundary; serialized with
  `#[serde(default, skip_serializing_if = "Option::is_none")]` for backward
  compatibility; TypeScript: `geometry?: string[] | null`
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
  spindle speed, tool number, coordinate words (1e-6 mm tolerance), plane,
  distance mode, feed mode
- `postprocessor/program.rs` — full program assembler: header, tool changes,
  pass emission, footer, percent delimiters; returns `NotSupported` (not panic)
  when a 5-axis tool orientation is encountered
- `postprocessor/mod.rs` — public API: `PostProcessor::builtin()`,
  `from_file()`, `list_builtins()`, `generate()`; `PostProcessorMeta` struct;
  `PostProcessorError` enum (`Config`, `NotSupported`, `ArcError`, `Assembly`)

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

**Golden file integration tests** (`4877867`, updated `f241586`)
- `src-tauri/tests/gcode_golden.rs`: `fanuc_0i_golden_matches` and
  `linuxcnc_golden_matches`; each generates a pocket toolpath through the full
  production pipeline (planner → linking → arc fitting), produces G-code, and
  asserts byte-for-byte match against checked-in `.nc` golden file; gated on
  `cam_geometry_bindings` (planner dependency)
- `fanuc_0i_pocket_contains_arcs`: regression test asserting G02/G03 arc
  commands appear in the Fanuc 0i pocket output (gated)
- Toolpath JSON files: `tests/integration/golden_gcode/fanuc-0i/simple_pocket.toolpath.json`,
  same for `linuxcnc/` (written by the auto-write pattern for human inspection)
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
- `src-tauri/src/toolpath/operations/pocket.rs`: `pocket_passes(stock, params, tool_diameter, boundary)`
- Receives caller-supplied `boundary` polygon; inward-offsets by tool radius
  for first contour, then repeatedly by stepover until polygon collapses
- Repeats per Z depth level (`stepdown` increments down to `depth`)
- Returns `AppError::GeometryImport` if tool is too large for stock
- Unit tests (all gated on `cam_geometry_bindings`): Z-level count,
  non-empty output for valid tool, error propagation for oversized tool

**Toolpath planner** (`3babbb6`, updated `1b405df`, `b529693`, `4db8300`, `544e758`, `c3531c8`)
- `src-tauri/src/toolpath/planner.rs`: `plan(operation, tool, stock, shape)`
- Derives stock boundary rectangle once, passes it to `pocket_passes` /
  `profile_passes` / `drill_passes` (refactored in `4db8300`: pass generators
  no longer derive the boundary themselves; `pocket_passes` and `profile_passes`
  accept `boundary: &[(f64, f64)]` as a caller-supplied argument)
- Geometry resolution (`544e758`): for Pocket and Profile operations with a
  non-empty `geometry` field, `resolve_geometry_boundary()` is called with the
  optional `OcctShape`; returns `GeometryImport` when shape is `None`; under
  `cam_geometry_bindings` calls `enumerate_faces`, matches each fingerprint to
  a `FaceDescriptor`, retrieves `face_boundary`, and unions multiple polygons
  via `poly_boolean(BoolOp::Union)`; non-bindings stub returns a clear error;
  ZLevelRoughing does not use `resolve_geometry_boundary` — it passes `shape`
  directly to `zlevel_roughing_passes`, which calls `shape_section_at_z` at
  each Z level internally
- `plan()` return type changed in `c3531c8`: returns `(Vec<Pass>, ToolpathStats)`
  — `Toolpath` assembly and `link_passes` call moved to `calculate_toolpath_inner`
  so the emit callback can be fired at intermediate milestones
- Feed/speed override logic: operation-level `spindle_speed_override` /
  `feed_rate_override` take priority over tool defaults, which fall back to
  hardcoded values (8000 RPM / 500 mm/min)
- Unit tests: stats non-zero for Pocket and Profile (gated on bindings);
  error for Profile without geometry bindings (stub path); six override/fallback
  tests (ungated); `plan_pocket_with_geometry_none_uses_stock_boundary`;
  `plan_pocket_with_geometry_some_and_no_shape_returns_error`

**Pocket algorithm golden file test** (`f52cdc5`)
- `src-tauri/tests/pocket_golden.rs`: `pocket_algorithm_golden_matches`
  gated via `#![cfg(cam_geometry_bindings)]`
- Exercises full planner pipeline (50×50×10 mm stock, 10 mm flat endmill,
  depth=10/stepdown=2/stepover=50%); compares serialized toolpath JSON
  against committed golden fixture
- Golden fixture: `tests/integration/pocket/toolpath.json` (1177 lines)
- `[[test]]` entries added to `src-tauri/Cargo.toml` for both
  `gcode_golden` and `pocket_golden`

**Profile contouring algorithm** (`ad562cd`)
- `src-tauri/src/toolpath/operations/profile.rs`: `profile_passes(stock, params, tool_diameter, boundary)`
- Receives caller-supplied `boundary` polygon; offsets inward (Left) or
  outward (Right) by tool radius, or uses raw boundary (Center) for the
  single cutting contour; repeats contour per Z depth level (`stepdown`
  down to `depth`)
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
- Golden fixture: `tests/integration/profile/toolpath.json` (197 lines)
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
- For each hole in `params.points`, produces one `PassKind::Linking` pass and
  one `PassKind::Cutting` pass; the first hole's Linking is a single rapid to
  clearance above that hole; subsequent holes get two rapids — lift to clearance
  above the previous hole, then traverse to clearance above the current hole
- Each Cutting pass opens with a `Rapid` approach to clearance height, then:
  full-depth mode (no `peck_depth`): `Feed` plunge to `drill_z`, `Rapid`
  retract to clearance (3 cut points total)
- Peck mode (`peck_depth` set): after the opening `Rapid`, repeated
  feed/retract cycles decrementing by `peck_depth` until `drill_z` is
  reached; uses `.max(drill_z)` to avoid overshooting the target depth
- Returns `AppError::GeometryImport` if `params.points` is empty or if
  `peck_depth` is ≤ 0
- 6 unit tests: empty-points error, zero peck depth error, negative peck
  depth error, single non-peck hole geometry (Z values and move kinds),
  peck hole Z-levels (7-point sequence for 3 pecks), two-hole pass ordering
  and linking structure

**Drill algorithm golden file test** (`7e436df`)
- `src-tauri/tests/drill_golden.rs`: `drill_algorithm_golden_matches`
  (ungated — drill algorithm requires no geometry bindings)
- Exercises full planner pipeline (50×50×10 mm stock, 5 mm drill, 5 holes,
  depth=10/peck_depth=3); validates peck cycling for each of 5 holes
- Golden fixture: `tests/integration/drill/toolpath.json` (645 lines)
- `[[test]]` entry added to `src-tauri/Cargo.toml` for `drill_golden`

### IPC commands (calculate and geometry)

**`calculate_toolpath`** and **`get_toolpath_geometry`** (`0c1d802`, updated `e7b5da1`, `b90c440`, `544e758`, `c3531c8`)
- `calculate_toolpath_inner`: parses operation UUID; holds the read lock through `planner::plan()` so the `OcctShape` can be borrowed from the persisted `LoadedModel` without cloning (non-Clone handle); reads operation/stock/tool/model SHA under the same read lock; calls `planner::plan(operation, tool, stock, shape)`; after plan returns, calls `link_passes` for Pocket/Profile/ZLevelRoughing operations (Drill skips `link_passes` — `drill_passes()` handles its own linking internally); runs `arc_fitting::fit_arcs` on every pass (0.01 mm tolerance) to replace linearized arc segments with `MoveKind::Arc` moves; assembles `Toolpath`; computes SHA-256 cache key; stores `Toolpath` and populates `operation.cache` (`key`, `valid`, `computed_at`, `stats`; `binary_file` remains `None`) under write lock; returns `ToolpathStats`
- Progress events (`c3531c8`): `ToolpathProgressEvent { operation_id: String, percent: u32, message: String }` (pub, camelCase serde); `calculate_toolpath_inner` accepts `emit: Option<&dyn Fn(ToolpathProgressEvent)>` and fires it at five milestones (0% / 50% / 80% / 95% / 100%); the `#[tauri::command]` wrapper receives `AppHandle` and emits `"toolpath:progress"` events to the frontend
- `get_toolpath_geometry_inner`: retrieves stored `Toolpath` and operation
  index (for palette colouring); converts passes to flat-array
  `LineGeometryData`; pre-allocates buffers using segment count
- Both registered in `tauri::generate_handler!` list in `lib.rs`
- Unit tests (all in `commands/toolpath.rs`): `list_post_processors_inner`
  returns 4 entries; `calculate_toolpath_inner`: NotFound with no operation,
  NotFound with no stock, stores toolpath for pocket and asserts cache fields
  populated (gated); `get_toolpath_geometry_inner`: NotFound when no toolpath
  stored; `get_gcode_preview_inner`: generates G-code containing rapid and
  feed moves when toolpath exists

### UI (complete)

**Operation editor form** (`53c4f49`, updated `70e8318`, `1318d96`, `bec3737`, `d53a1c0`)
- `OperationEditorForm` in `src/components/operations/OperationEditorForm.tsx`
- Pocket operations: tool select (saves on change) + depth / stepdown /
  stepover / spindle speed override / feed rate override inputs (save on blur)
  + geometry section (Select Faces / Done Selecting / Clear)
- Profile operations: tool select (saves on change) + depth / stepdown /
  compensation side (Left/Center/Right) select + spindle speed override /
  feed rate override inputs (save on blur) + geometry section
- Drill operations: tool select + depth + peck depth + spindle speed override /
  feed rate override + dynamic drill-points table (Add Point / Remove per row,
  each row has X and Y inputs that save on blur); no geometry section
- Geometry section (`d53a1c0`): "Select Faces" calls `getModelFaces()` and
  enters viewport selection mode; selected fingerprints shown as count; "Done
  Selecting" saves `selectedFaceFingerprints` into the operation's `geometry`
  field via `editOperation` and exits selection mode; "Clear" resets `geometry`
  to `null` (falling back to stock boundary)
- `save()` base always carries current `spindleSpeedOverride` and
  `feedRateOverride` values to prevent silent clearing on unrelated saves
- Uses `key={operation.id}` on the rendered div to remount uncontrolled
  inputs when the selected operation changes
- Tests in `OperationEditorForm.test.tsx` cover pocket, profile, and drill
  forms; including add/remove point, override inputs, and geometry section
  (Select Faces / Done Selecting / Clear) — 9 new tests for geometry section;
  z_level_roughing branch added in Phase 2; entry motion fields added (Phase 2);
  detect holes tests added (Phase 2) — 58 tests across 10 describe blocks

**Operation list panel — row selection and Calculate** (`f94a19a`, updated `4f62a9d`, `d178a20`, `1318d96`, `9706ff9`, `085504f`, `8491f7f`, `31e1286`, `d9139d1`)
- Row click sets `selectedOperationId`; selected row highlighted
- `OperationEditorForm` mounted below the list, driven by `selectedOperationId`
- Checkbox per row toggles `enabled`: fetches full operation via
  `listOperations()`, flips `enabled`, calls `editOperation`, refreshes snapshot
- Delete button per row: calls `deleteOperation`, refreshes snapshot
- Add operation buttons at the bottom (+ Profile, + Pocket, + Drill; extended
  with + Z-Level Roughing in Phase 2): disabled when no tools exist; uses
  first available tool; calls `addOperation` with sensible defaults, refreshes
  snapshot
- Reorder buttons (▲/▼) per row: call `reorderOperations` to move the operation
  up or down in the list; ▲ disabled for the first row, ▼ disabled for the last
- Calculate button per row: enabled for pocket, profile, and Z-Level Roughing
  operations when stock is defined; enabled for drill operations when stock is
  defined AND the operation has ≥ 1 drill point; calls `calculateToolpath` →
  `getToolpathGeometry` → `setToolpathGeometry` → `getProjectSnapshot` →
  `setSnapshot` and pushes a stats notification string
- Calculate loading state: a `calculatingId` state tracks the in-flight
  operation ID; the active row's Calculate button shows '…' while calculating;
  all Calculate buttons are disabled while any calculation is running
- `drillPointCounts: Record<string, number>` state maintained via `useEffect`
  that short-circuits (resets to `{}`) when no drill operations exist, and
  otherwise triggers a full `listOperations()` fetch whenever the operations
  list changes; used to gate the Calculate button for drill rows
- Progress bar (`d9139d1`): subscribes to `toolpath:progress` events via
  `listenToolpathProgress`, filtered to the active `calculatingId`; a `<progress>`
  element appears adjacent to the Calc button for the row being calculated;
  resets to 0 at calculation start and disappears when calculation completes;
  active-flag guard prevents the cleanup/resolve race condition; 2 tests
  (element appears and updates on event; disappears after calculation completes)
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
  `useSelectedOperationId` + `useNotifications` (returns active notification
  messages array) + `dismissNotification` added to `projectStore.ts`

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
- "Set WCS" button calls `setWcs([payload])` then refreshes the project snapshot;
  when a WCS already exists, it merges the edited origin into the existing record;
  when no WCS exists, it generates a new UUID and uses G54/standard-axis defaults
- "Clear WCS" button calls `setWcs([])` (empty array) then refreshes the snapshot
  (only rendered when a WCS is defined)
- Error notifications via `usePushNotification` for all failure paths
- Mounted in `AppShell` between `StockPanel` and `OperationListPanel`
- Tests (`WCSPanel.test.tsx`): display when empty ('No WCS defined' / no Clear
  button), display with WCS (shows origin values / Clear button present), Set WCS
  when updating existing record, Set WCS when creating a new record (checks
  generated UUID + default axes), Clear WCS calls `setWcs([])` and refreshes,
  error notification when `setWcs` rejects — 6 tests

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
- Key covers: full operation object (`id`, `name`, `enabled`, `toolId`, `params`) minus `spindleSpeedOverride`, `feedRateOverride`, and `cache`; tool geometry subset (`diameter`, `flute_count`, `material`, `type` — explicitly excluding display-only `id`, `name`, `default_spindle_speed`, `default_feed_rate`); stock definition; optional model content SHA; engine version string
- Feed/speed override fields (`spindle_speed_override`, `feed_rate_override`) intentionally excluded — changing overrides alone does not invalidate a cached toolpath; `cache` field excluded to avoid circular dependency
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
- Tests: `snapshot_needs_recalculate_false_when_cache_key_current`, `snapshot_needs_recalculate_true_after_model_checksum_change`

**Stale indicator in OperationListPanel** (`9706ff9`)
- Rows with `needsRecalculate: true` display an amber "(stale)" label
- Snapshot refreshed via `getProjectSnapshot()` after a successful toolpath calculate so the stale indicator clears immediately
- Tests: stale indicator rendering, post-calculate snapshot refresh

**End-to-end cache integration test** (`89a3d90`)
- `src-tauri/tests/toolpath_cache.rs`: 2 scenarios (ungated — uses drill operations requiring no geometry bindings): save/load round-trip preserves toolpath and cache validity; mutating a param after load marks operation stale
- `[[test]]` entry added to `src-tauri/Cargo.toml`
- `calculate_toolpath_inner` and `get_project_snapshot_inner` exposed as `pub` for integration test access

### Geometry selection (complete)

Face-level geometry selection is fully implemented across the C++ layer, Rust
backend, IPC bridge, frontend store, viewport, and operation editor form.

**C++ face API** (`224fedd`)
- `CgFaceInfo { centroid[3], normal[3], area }` and
  `CgFaceGroup { start_triangle, triangle_count }` structs added to `cam_geometry.h`
- `CgMeshData` extended with a `face_groups` vector; `cg_shape_tessellate`
  records one `CgFaceGroup` entry per face (null/degenerate faces get a
  zero-count entry), maintaining strict 1:1 alignment with face index
- New C functions: `cg_mesh_face_group_count`, `cg_mesh_copy_face_groups`
  (retrieve per-face triangle group table), `cg_shape_face_count` (via
  `TopExp_Explorer`), `cg_face_info` (area, centroid, unit normal via
  `BRepGProp` and `GeomAdaptor_Surface`; handles `Geom_RectangularTrimmedSurface`),
  `cg_face_boundary_poly` (outer-wire XY pairs via `BRepTools_WireExplorer`;
  all edges discretized with `GCPnts_TangentialDeflection` (chord=0.1 mm,
  angular=0.1 rad); FORWARD edges sampled start-to-end skipping the last
  shared vertex; REVERSED edges sampled end-to-start skipping the first
  shared vertex — preserving wire winding order)

**`MeshData` face groups** (`a677827`)
- `FaceGroup { start_triangle: u32, triangle_count: u32 }` added to Rust;
  populated from `cg_mesh_face_group_count` / `cg_mesh_copy_face_groups` in
  `OcctMesh::to_mesh_data()`; stub build returns an empty vec
- `MeshData` gains `face_groups: Vec<FaceGroup>` with `#[serde(rename_all = "camelCase")]`
  → IPC delivers `faceGroups`; TypeScript `MeshData` interface updated to match
- Integration test asserts 6 face groups for the box fixture with bounds checks

**Rust face API** (`bb21234`)
- `src-tauri/src/geometry/faces.rs`:
  - `FaceInfo { centroid: [f64; 3], normal: [f64; 3], area: f64 }` — raw OCCT
    face properties
  - `FaceDescriptor { fingerprint: String, face_idx: usize, centroid: [f64; 3],
    normal: [f64; 3], area: f64 }` — augmented with fingerprint and face index
  - `enumerate_faces(shape) -> Result<Vec<FaceDescriptor>, GeometryError>` —
    iterates all faces; non-planar faces are silently skipped
  - `face_boundary(shape, face_idx: usize) -> Result<Vec<(f64, f64)>, GeometryError>`
    — outer-wire XY polygon for a given face index
  - `face_fingerprint(info: &FaceInfo) -> String` — SHA-256 of a canonical
    comma-separated `key:value` string encoding centroid (cx/cy/cz), normal
    (nx/ny/nz), and area (a) each to 4 decimal places; returns a 64-character
    lowercase hex string (no `"sha256:"` prefix)
- Dual-compiled: real OCCT implementation behind `#[cfg(cam_geometry_bindings)]`;
  stub returning `GeometryImport` error in non-bindings builds

**`OcctShape` persistence in `AppState`** (`b90c440`)
- `LoadedModel` gains `shape: Option<OcctShape>` field
- `import_with_shape(path) -> Result<(MeshData, Option<OcctShape>)>` in
  `importer.rs`; STEP/IGES return a live handle, STL returns `None`
- `open_model_inner` calls `import_with_shape` inside `spawn_blocking`, moves
  the `OcctShape` out of the closure, stores it in `LoadedModel.shape`
- `unsafe impl Sync for OcctShape` (required for `AppState: Sync`; C++ handle
  registry is protected by `std::shared_mutex`)

**IPC `get_model_faces` command** (`ce05bee`)
- `src-tauri/src/commands/geometry.rs`: new file with `FaceDescriptorIpc
  { fingerprint: String, face_idx: usize, centroid: [f32; 3], normal: [f32; 3],
  area: f32 }` (camelCase; f64→f32 downcast from internal `FaceDescriptor`) and
  `get_model_faces_inner` / `get_model_faces` pair
- Returns `NotFound` when no model is loaded or the shape is absent (stub build
  or STL import)
- Four tests: camelCase serialization, no-model, no-shape, OCCT integration
  test against box.step

**Frontend API layer and `viewportStore` selection state** (`6d1fe41`)
- `src/api/types.ts`: `FaceDescriptor { fingerprint, faceIdx, centroid: [n,n,n],
  normal: [n,n,n], area }` and `ToolpathProgressEvent { operationId, percent, message }` interfaces
- `src/api/geometry.ts`: new module with `getModelFaces()` IPC wrapper
- `src/api/mock.ts`: stubs for `getModelFaces` (returns `[]`) and
  `listenToolpathProgress` (no-op)
- `viewportStore`: `selectionMode: boolean`, `hoveredFaceIdx: number | null`,
  `selectedFaceFingerprints: string[]`, `faceDescriptors: FaceDescriptor[]`
  state fields with corresponding setters; `setSelectionMode(false)` clears
  hover and descriptors but preserves the fingerprint selection
- 10 new viewportStore tests covering all new state and actions

**Three.js face highlighting in the viewport** (`ce6a005`)
- `SceneManager` gains `setOrbitEnabled()` and a camera getter; `scene.ts`
  updated accordingly
- Highlight overlay mesh (`MeshBasicMaterial`, vertexColors, `depthTest: false`)
  that shares position/normal attributes with the model mesh; disposed
  separately to avoid freeing GPU buffers still owned by the model mesh
- Orbit controls disabled while face-selection mode is active
- `mousemove` raycast resolves triangle → `faceGroups` index; calls
  `setHoveredFaceIdx` on hit or `null` on miss
- Click handler calls `toggleFaceSelection` for the hovered face descriptor
- Mutable refs (selectionModeRef, hoveredFaceIdxRef, etc.) keep event handlers
  free of stale closures while registered once at mount
- Highlight rebuild effect recomputes index buffer and per-vertex color array
  (yellow for hovered, blue for selected) on every relevant state change

**Integration tests** (`9e2f97a`)
- `calculate_toolpath_inner_with_geometry_selection_bounds_passes_within_face`:
  loads `box.step`, enumerates faces, selects the first face fingerprint, runs
  `calculate_toolpath_inner` with geometry selection, asserts resulting passes
  are bounded within the face XY extents (smaller than 400×400 stock)
- `calculate_toolpath_inner_with_invalid_fingerprint_returns_geometry_import_error`:
  runs `calculate_toolpath_inner` with a bogus 64-char fingerprint, asserts
  `AppError::GeometryImport`
- Both tests gated on `#[cfg(cam_geometry_bindings)]`

### Progress events (complete)

**Backend** (`c3531c8`)
- `ToolpathProgressEvent { operation_id: String, percent: u32, message: String }` — pub struct,
  `#[serde(rename_all = "camelCase")]`
- `calculate_toolpath_inner` extended with `emit: Option<&dyn Fn(ToolpathProgressEvent)>`;
  fired at five milestones: 0% (start) / 50% (plan complete) / 80% (passes
  linked) / 95% (toolpath and cache written to state) / 100% (complete)
- `#[tauri::command]` wrapper receives `AppHandle` and emits
  `"toolpath:progress"` events; test: `calculate_toolpath_inner_emits_progress_events`
  (drill, ungated)
- `plan()` return type changed to `(Vec<Pass>, ToolpathStats)`; `Toolpath`
  assembly and `link_passes` moved to `calculate_toolpath_inner` to allow
  milestone emission between steps

**Frontend** (`6d1fe41`, `d9139d1`)
- `listenToolpathProgress(cb)` wrapper in `api/toolpath.ts` — subscribes to the
  `"toolpath:progress"` Tauri event; returns the unsubscribe function
- `OperationListPanel` subscribes at calculation start, filtered to the active
  `calculatingId`; a `<progress>` element appears for the active row, resets
  to 0 at start, updates on each event, and disappears when calculation
  completes; active-flag guard prevents the cleanup/resolve race condition

---

## Phase 2: 2.5D Operations — Partial

### OCCT Z-section primitive (`ca53015`, `0924ca7`)

**C++ implementation** (`cam_geometry.cpp`)
- `cg_shape_section_at_z(id, z_value, CgPoint3** out_points, size_t* out_count)
  → CgError` — slices an OCCT B-rep solid at a given Z plane using
  `BRepAlgoAPI_Section`, walks result edges, and writes a heap-allocated flat
  array of `CgPoint3` pairs (connected edge segments) into `*out_points`;
  caller frees with `cg_section_free(CgPoint3*)`
- New error guards: null output-pointer guard, `CG_NULL_ID` guard, Standard_Failure
  exception chain
- 5 new C++ doctests: mid-height section of box fixture, out-of-bounds section,
  null out_points guard, CG_NULL_ID guard, `cg_section_free(nullptr)` safety

**Rust safe wrapper** (`src-tauri/src/geometry/safe.rs`)
- `shape_section_at_z(shape: &OcctShape, z: f64) -> Result<Vec<Vec<(f64, f64)>>, GeometryError>`
- Private helper `stitch_segments_into_loops()` handles mixed-orientation edge
  segments from OCCT by traversing each segment in either direction and
  chain-linking them into closed (or open) loops; a chain that closes on itself
  has the duplicate endpoint removed; a chain with no continuation is kept as-is
  (open)
- `GeometryError::NotImplemented` variant added for the stub path
- Dual-compile: real FFI behind `#[cfg(cam_geometry_bindings)]`; stub returning
  `GeometryError::NotImplemented` otherwise
- 2 new Rust unit tests: stub returns NotImplemented (ungated); box section at
  mid-height returns a single loop (OCCT-gated)

### Z-Level Roughing (ZLR) — complete end-to-end (`db86fdf`–`87b89ce`)

**Data model** (`src-tauri/src/models/operation.rs`)
- `ZLevelRoughingParams` struct: `depth: f64`, `stepdown: f64`, `stepover: f64`
  (fraction 0–1), `geometry: Option<Vec<String>>`; tagged as `"type":
  "z_level_roughing"` in the `OperationParams` enum; note: the `geometry` field
  is stored and round-tripped in serde (and shown in the UI) but is not yet
  wired to the planner — `plan()` never calls `resolve_geometry_boundary` for
  ZLR, so selecting faces has no effect on the computed toolpath; the algorithm
  always sections the full loaded OcctShape at each Z level
- `AppError::InvalidInput(String)` new variant for parameter validation errors
- Serde round-trip test: `zlevel_roughing_round_trips` verifies discriminant and
  camelCase serialization

**Algorithm** (`src-tauri/src/toolpath/operations/zlevel_roughing.rs`)
- `zlevel_roughing_passes(stock, params, tool_diameter, shape)` → `Result<Vec<Pass>>`
- Validates all params (stepdown > 0, depth > 0, stepover in (0, 1])
- Integer-indexed Z-level loop ensures floor depth is always machined even when
  depth is not a multiple of stepdown
- At each Z level: calls `shape_section_at_z()` to get OCCT section boundary;
  falls back to stock extents when shape is `None` or OCCT returns
  `GeometryError::NotImplemented` (no bindings); skips (continues to next level)
  when OCCT returns an empty section (geometry doesn't intersect that Z plane);
  other geometry errors propagate as `AppError`
- Applies inward tool-radius offset then concentric stepover offsets until polygon
  collapses (identical strategy to pocket clearing)
- Registered in `planner.rs` (new `ZLevelRoughing` arm), `commands/project.rs`,
  and `commands/toolpath.rs`
- 3 ungated unit tests: zero stepdown rejection, zero depth rejection,
  out-of-range stepover rejection
- 6 OCCT-gated unit tests: produces passes for simple stock, Z levels span depth,
  floor Z always machined when depth not multiple of stepdown,
  `zlr_stock_boundary_produces_correct_z_levels`, `zlr_geometry_uses_section_boundaries`,
  collapses when tool too large

**IPC and TypeScript types** (`a3f4ce1`)
- `z_level_roughing` case handled in `calculate_toolpath_inner` (linking
  dispatch match arm) and in `planner::plan()` (ZLevelRoughing arm);
  `get_toolpath_geometry_inner` requires no ZLR-specific code — it operates
  generically on any stored `Toolpath`; no extra IPC commands needed
- `get_project_snapshot` updated to emit `"z_level_roughing"` as the operation
  type string for `OperationSummary`
- `plan_zlevel_roughing_invalid_params_returns_invalid_input` planner test
  (in `planner.rs`): passes ZLR with `stepdown=0.0` to `planner::plan()` and
  asserts `AppError::InvalidInput` is returned
- `ZLevelRoughingParams` interface in `src/api/types.ts`; `'z_level_roughing'`
  added to `Operation`, `OperationInput`, and `OperationSummary` union types

**Operation editor UI** (`39211ad`, `26a171d`)
- `OperationEditorForm.tsx`: new `z_level_roughing` branch with tool selector,
  depth/stepdown/stepover (displayed as % → stored as fraction), geometry section
  (Select Faces / Done / Clear — stored but not yet used by the algorithm; see
  data model note above), and feed/speed override inputs
- `OperationListPanel.tsx`: `+ Z-Level Roughing` button (disabled when no tools),
  default params depth=5.0, stepdown=1.0, stepover=0.5
- 10 new OperationEditorForm tests; 2 new OperationListPanel tests

**Golden test** (`87b89ce`)
- `src-tauri/tests/zlevel_roughing_golden.rs` (gated on `cam_geometry_bindings`) —
  loads `box.step`, runs ZLR with depth=5/stepdown=2/stepover=0.4; compares
  serialized passes against committed golden fixture
- Golden fixture: `tests/fixtures/zlevel_roughing_golden.json` (197 lines)

### Viewport standard views (`1a752b2`, `728476a`)

**SceneManager methods** (`src/viewport/scene.ts`)
- `snapToView(position, up)` — smoothly animates camera position and up-vector
  over 300 ms using `@tweenjs/tween.js`; cancels any in-flight tween; preserves
  orbit distance; `_tweenGroup` driven each animation frame
- Convenience methods: `snapTop()` (camera at +Z, up=(0,1,0), looks down),
  `snapFront()` (camera at -Y, up=(0,0,1), looks in from front),
  `snapRight()` (camera at +X, up=(0,0,1), looks in from right),
  `snapIsometric()` (camera at (1,−1,1) normalized, up=(0,0,1))

**Keyboard handler** (`src/viewport/Viewport.tsx`)
- `T` / `F` / `R` / `I` keys call the corresponding SceneManager snap methods
- Guards against `INPUT`, `TEXTAREA`, `SELECT` focus; cleanup on unmount
- 8 Viewport keyboard-shortcut tests (T/F/R/I routing, uppercase T, INPUT guard,
  TEXTAREA guard, remove-listener-on-unmount); note: the 2 P-key tests also live
  in the `keyboard shortcuts` describe block but are counted under projection toggle
- 5 new SceneManager mock tests (`scene-snap-mock.test.ts`): verify tween target
  position for each view including no-op when already at target
- 12 real-tween SceneManager tests in `scene.test.ts`: position/up animation,
  orbit distance preservation, mid-flight cancellation, dispose cleanup

### Viewport projection toggle (`d25ef8a`, `4ce177d`, `1f07a1c`)

**SceneManager methods** (`src/viewport/scene.ts`)
- `getProjectionMode()` → `'perspective' | 'orthographic'`
- `toggleProjection()` — switches between `THREE.PerspectiveCamera` and
  `THREE.OrthographicCamera`; syncs position, up vector, and frustum size;
  reassigns `OrbitControls.object`; calls `controls.update()`
- `_animate()` dispatches render calls through a private `_activeCamera()` helper
- Bug fix (`1f07a1c`): `_onResize()` ortho frustum used hardcoded 250 half-size
  instead of `orthographicCamera.top`; fixed to use current ortho half-height

**Store and UI** (`src/store/viewportStore.ts`, `src/viewport/Viewport.tsx`)
- `projectionMode: 'perspective' | 'orthographic'` field + `setProjectionMode()`
  action in `viewportStore`
- `P` key toggles projection via store; toolbar button shows current mode
- `useEffect` syncs store → SceneManager only when modes disagree (avoids flicker)
- 14 SceneManager projection tests (13 in `projection toggle` describe block +
  1 `orthographic resize consistency` test verifying the `_onResize` bug fix);
  6 Viewport projection tests: P-key ×2 (in keyboard shortcuts describe), toolbar
  button ×3, projection mode sync ×3 — across three describe blocks

### Viewport display mode selector (`8c0b2a8`, `b5edf28`, `eccd2f1`)

**SceneManager** (`src/viewport/scene.ts`)
- `setModelMesh(mesh)` — stores reference for display-mode changes; disposes
  stale edge overlay when mesh changes
- `setDisplayMode(mode: DisplayMode)` — implements all four modes:
  - `'shaded'`: `MeshStandardMaterial` opaque, wireframe=false, transparent=false, opacity=1
  - `'shaded-edges'`: shaded fill + lazy `EdgesGeometry` / `LineSegments` overlay
    (built once, reused)
  - `'wireframe'`: `material.wireframe = true`, no fill
  - `'transparent'`: `material.transparent = true`, `opacity = 0.3`
  - Each mode fully resets material state to avoid stale values
- `dispose()` tears down edge overlay to prevent GPU leaks

**Store and UI** (`src/store/viewportStore.ts`, `src/viewport/Viewport.tsx`)
- `DisplayMode = 'shaded' | 'shaded-edges' | 'wireframe' | 'transparent'`
  type and `displayMode` field (default `'shaded'`) in `viewportStore`;
  replaces the previous string literal
- `setModelMesh` + `setDisplayMode` called in mesh-update effect so current mode
  applies immediately to newly loaded models
- `<select>` toolbar control: Shaded / Shaded + Edges / Wireframe / Transparent
- 6 Viewport display-mode tests (select rendered, all four options present, store
  sync on change, SceneManager `setDisplayMode` called, `setModelMesh` on mesh
  load/clear); SceneManager display-mode tests in `scene.test.ts`

### LinkingParams struct refactor (`00726e9`)

**Types** (`src-tauri/src/toolpath/types.rs`)
- New `LinkingParams` struct with fields `tool_diameter: f64`, `clearance_z: f64`,
  `lead_ratio: f64` — replaces the previous separate positional arguments and
  hardcoded `LEAD_RATIO` constant in `link_passes`
- `link_passes(passes, params: &LinkingParams)` — signature updated; no behaviour
  change

**Command handler** (`src-tauri/src/commands/toolpath.rs`)
- `calculate_toolpath_inner` constructs `LinkingParams` from operation context
  and passes it to `link_passes`

### Multi-level profile stepdown (`d093656`, `3bc5b09`)

**Data model** (`src-tauri/src/models/operation.rs`)
- `ProfileParams.stepdown` changed from `f64` to `Option<f64>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`
- `None` or `Some(v <= 0)` → single pass at full depth (backward compatible)
- `Some(v > 0)` → integer-indexed Z loop identical to `zlevel_roughing` pattern:
  `Z = stock_top - n*sd` clamped to `floor_z`; final pass is always at floor

**Algorithm** (`src-tauri/src/toolpath/operations/profile.rs`)
- `profile_passes` matches on `params.stepdown`; multi-level loop when
  `Some(sd > 0)`, single-pass otherwise
- 8 new ungated tests (in `tests_no_bindings`): `None` → one pass at floor,
  `Some(0)` → one pass, `Some(-1)` → one pass, JSON absence when None,
  backward-compat deserialization, `None` → one Z level, `stepdown=2/depth=8` →
  four passes at Z=8/6/4/2, `stepdown=3/depth=8` → three passes, last clamped to floor

**TypeScript** (`src/api/types.ts`)
- `ProfileParams.stepdown` changed to `stepdown?: number | null`

### Entry motion data model and plumbing (`c8a8b0d`, `5cef144`)

**Data model** (`src-tauri/src/models/operation.rs`)
- Five new `Option<f64>` fields added to `ProfileParams`, `PocketParams`, and
  `ZLevelRoughingParams` (all with `#[serde(default, skip_serializing_if = "Option::is_none")]`
  for backward compatibility):
  - `arc_lead_in_radius` — radius for arc lead-in; `None` = straight lead-in
  - `arc_lead_out_radius` — radius for arc lead-out; `None` = straight lead-out
  - `helical_entry_radius` — helix radius for closed-pocket entry; `None` = plunge
  - `helical_entry_pitch` — Z descent per revolution (mm); uses tool_diameter/3
    default when `None` but radius is `Some`
  - `ramp_entry_angle_deg` — ramp angle (degrees) for open-contour entry; `None` = plunge

**LinkingParams** (`src-tauri/src/toolpath/types.rs`)
- Same five fields added to `LinkingParams` struct

**Command handler** (`src-tauri/src/commands/toolpath.rs`)
- `calculate_toolpath_inner` now extracts all five entry motion fields from the
  active operation's params and forwards them into `LinkingParams`; previously
  all were hardcoded `None`

**TypeScript** (`src/api/types.ts`)
- camelCase variants added to `ProfileParams`, `PocketParams`, `ZLevelRoughingParams`
  interfaces: `arcLeadInRadius?`, `arcLeadOutRadius?`, `helicalEntryRadius?`,
  `helicalEntryPitch?`, `rampEntryAngleDeg?` (all `number | null`)

### Entry motion UI (`e6b5af3`)

**OperationEditorForm** (`src/components/operations/OperationEditorForm.tsx`)
- Profile and Pocket forms each gain five optional numeric inputs:
  Arc lead-in radius (mm), Arc lead-out radius (mm), Helical entry radius (mm),
  Helical entry pitch (mm), Ramp entry angle (°)
- All fields use the `value === '' ? null : Number(...)` pattern on blur
- Profile `stepdown` field corrected to send `null` (not `NaN`) when cleared
- `ProfileParams.stepdown` type corrected to `number | null` in the form
- 8 new OperationEditorForm tests:
  - Profile: all five fields render, all five empty by default, arc lead-in blur
    sets value, arc lead-in blur with blank sends null, stepdown blank sends null
  - Pocket: all five fields render, arc lead-out blur sets value, arc lead-out blur
    with blank sends null

### Helical entry (`a7feffc`, `734afe2`)

**Algorithm** (`src-tauri/src/toolpath/linking.rs`)
- `segments_per_revolution(radius)` — number of chord segments per revolution for
  ≤0.01 mm chordal error; returns 0 when radius ≤ 0.005 mm (avoids divide-by-zero)
- `chord_len_for_radius(radius)` — chord length formula
- `helical_descent_moves(center, radius, pitch, start_z, end_z)` → `Vec<CutPoint>` —
  CCW spiral from `start_z` to `end_z`, `pitch` mm per revolution; all moves are
  `MoveKind::Feed`
- `cleanup_arc_moves(center, radius, z, start_angle)` — full CCW cleanup circle at
  cutting depth `z` starting from the helix endpoint angle (positional continuity)
- `centroid_xy(points)` — closed-contour centroid; excludes closing duplicate point
  (within 0.001 mm) to avoid skew
- `link_passes` integration: when `params.helical_entry_radius` is `Some(r)` and
  the cutting pass is a closed contour, replaces the straight plunge with
  `helical_descent_moves` centred on the contour centroid + `cleanup_arc_moves`;
  falls back to plunge when radius is too large for contour or contour is degenerate
- 8 helical-specific unit tests: Z monotonically decreasing, XY on circle, pitch
  controls revolution count, `None` radius falls back to plunge, closed contour uses
  helix, cleanup arc starts at helix endpoint, helix fallback when radius too large,
  degenerate radius (≤ 0.005 mm, `segments_per_revolution` returns 0) falls back to
  3-point plunge

### Ramp entry (`734afe2`, `d02acd1`, `a0a97d4`)

**Algorithm** (`src-tauri/src/toolpath/linking.rs`)
- `ramp_descent_moves(xy_start, xy_end, retract_z, cut_z, angle_deg)` → `Vec<CutPoint>` —
  linear XY+Z descent for open contours; guards against zero-length segments,
  angle ≥ 90°, and inverted Z (returns empty when `retract_z < cut_z`, i.e. the
  retract point is below the cut depth so the tool would have to move upward —
  `a0a97d4` fixed a `.abs()` bug that previously generated wrong-direction moves
  in this case); clamps horizontal distance to segment length when the ramp
  would require more travel than available
- `link_passes` integration: when `params.ramp_entry_angle_deg` is `Some(a)` and
  the cutting pass is an open contour, uses ramp descent; `None` or closed contour
  falls back to plunge/helical as appropriate
- 12 ramp/combination unit tests: Z spans retract to cut depth, Z decreases
  monotonically, horizontal distance consistent with angle and depth, `None` angle
  falls back to plunge, short segment clamps and still reaches cut depth, angles
  ≥ 90° produce no ramp moves, inverted Z (retract_z < cut_z) produces no moves,
  zero-length segment stays at start XY, open-contour ramp integration, non-clamped
  path reaches cut depth, invalid angle falls back to plunge in `link_passes`,
  ramp+arc-lead-in combination continuity (when both are active, `Linking` ends at
  arc start S so there is no XY gap at cutting depth)

### Arc lead-in / lead-out (`f658547`, `a5125f5`, `d02acd1`)

**Algorithm** (`src-tauri/src/toolpath/linking.rs`)
- `arc_approach_moves(first_cut_point, second_cut_point, radius, cut_z)` → `Vec<CutPoint>` —
  generates a quarter-circle CCW arc at `cut_z`, tangent to the direction from
  `first_cut_point` toward `second_cut_point`; chord-approximated to ≤0.01 mm
  chordal error; S = P − R·T + R·N; all moves are `MoveKind::Feed`; degenerate
  arcs (tiny radius, coincident points) return empty (fall back to straight feed
  at the `link_passes` call site rather than silently omitting the pass)
- `arc_departure_moves(last_cut_point, second_to_last_cut_point, radius, cut_z)` →
  `Vec<CutPoint>` — symmetric quarter-circle CCW departure arc tangent to the
  direction from `second_to_last_cut_point` toward `last_cut_point`
- `link_passes` integration:
  - When `params.arc_lead_in_radius` is `Some(r)`: the `LeadIn` pass replaces
    the straight lead-in feed with arc approach moves; the `Linking` pass descent
    targets the arc start point S (not the first cut point) so the arc entry lands
    exactly on the cut
  - When `params.arc_lead_out_radius` is `Some(r)`: the `LeadOut` pass replaces
    the straight lead-out feed with arc departure moves; `from_x/from_y` after
    a lead-out arc tracks the true arc endpoint for subsequent linking
  - `None` values preserve the existing behaviour exactly
- 11 arc-specific unit tests: `None` radius preserves straight lead-in, `Some`
  radius produces arc approach moves, last approach move lands exactly at first cut
  point (≤1e-9 mm), first approach move is ≥ radius from first cut point, all
  approach moves are `Feed`, departure `None`/`Some` variants, all departure moves
  are `Feed`, linking descent targets arc start point, lift after lead-out departs
  from arc end, ramp not applied to closed contours

### Drill sorting — nearest-neighbor hole order (`58a58a7`, `7d0066f`)

**Algorithm** (`src-tauri/src/toolpath/operations/drill.rs`)
- `drill_passes()` now sorts `params.points` using a greedy nearest-neighbor
  algorithm before generating any passes
- Starting from the first point (preserves user intent for the initial hole),
  each subsequent hole is chosen as the closest unvisited point by squared
  Euclidean XY distance; `swap_remove` for O(1) pool removal; `total_cmp` for
  NaN-safe comparison without `unwrap`
- Sorting is purely internal to `drill_passes`; the `DrillParams` struct and
  IPC types are unchanged
- 2 new unit tests: `test_sort_single` (single hole unchanged), `test_sort_grid`
  (4 collinear holes — `(0,0),(40,0),(10,0),(30,0)` → sorted
  `(0,0),(10,0),(30,0),(40,0)`); existing golden fixture `tests/integration/drill/toolpath.json`
  regenerated to reflect sorted order

### Canned cycle support (`003393d`, `3c09517`, `3e46415`, `8b1186e`)

**New module: `src-tauri/src/postprocessor/cycles.rs`** (`003393d`, `3e46415`)
- `is_drill_cutting_pass(pass: &Pass) -> bool` — detects the Rapid/Feed
  alternating signature produced by `drill_passes()`: odd count ≥ 3, first
  cut is `Rapid`, subsequent pairs alternate `Feed`/`Rapid`, all cuts share
  the same XY position (1e-9 mm tolerance)
- `DrillCycleKind` enum: `Simple` (3 cut-points: Rapid/Feed/Rapid) and
  `Peck { increment: f64 }` (1 + 2N cut-points, N ≥ 2)
- `DrillCycleParams { kind, r_plane_z, drill_depth_z }` — extracted cycle
  parameters; `r_plane_z` from `cuts[0].z`, `drill_depth_z` from
  `cuts[cuts.len()-2].z`; `increment` reconstructed from `r_plane_z -
  DEFAULT_CLEARANCE_OFFSET - first_peck_z`
- `classify_drill_pass(pass: &Pass) -> Option<DrillCycleParams>` — wraps
  `is_drill_cutting_pass` and extracts parameters
- Config accessors: `cycles_supported`, `drill_cycle_code`, `peck_cycle_code`,
  `cycle_cancel_code`, `r_plane_abs_code`
- `format_cycle_header(params, feed_rate, config) -> Result<String, ...>` —
  formats the cycle activation line; R-plane return mode code comes from
  `config.cycles.r_plane_abs` (e.g. "G98") and is emitted only when that
  field is set; cycle code (G81/G83/G73) comes from config; Z axis letter
  and feed word letter come from config (`config.axes.z`, `config.words.feed`);
  R and Q word letters are hardcoded; number formatting from config; returns
  `NotSupported` error when required cycle codes are absent
- `format_cycle_cancel(config) -> Result<String, ...>` — formats G80
- 10 unit tests (4 in `003393d` + 6 in `3e46415`): `test_is_drill_pass_simple`,
  `test_is_drill_pass_peck`, `test_is_not_drill_pass_mixed_xy`,
  `test_is_not_drill_pass_wrong_count`, `test_format_header_simple_g81`,
  `test_format_header_peck_g83`, `test_cycles_not_supported_returns_false`,
  `test_peck_retract_mode_selects_g83`, `test_format_cycle_cancel_returns_g80`,
  `test_format_cycle_cancel_err_when_not_configured`

**`PeckRetractMode` enum and config** (`3c09517`)
- `PeckRetractMode { Full, ChipBreak }` enum added to `config.rs`
  (`snake_case` serde)
- `peck_retract_mode: Option<PeckRetractMode>` added to `CyclesConfig`; absent
  is treated as `Full` (G83); `ChipBreak` selects G73 (chip-break partial
  retract, reserved for future scope)
- `fanuc-0i.toml`, `linuxcnc.toml`, `mach4.toml` updated with
  `peck_retract_mode = "full"` under `[cycles]`
- Additional `CyclesConfig` fields present in the struct and populated in all
  three TOMLs (Fanuc/LinuxCNC/Mach4) but not yet read by the assembler:
  `r_plane_r` (G99 retract-to-R-plane mode), `boring_feed`, `boring_dwell`,
  `reaming`, `tapping`, `tapping_ccw` — reserved for future canned cycle types
- 4 new config unit tests: `full` parses, `chip_break` parses, absent is `None`,
  invalid value returns parse error

**Canned cycle emission in assembler** (`8b1186e`)
- `assemble()` in `program.rs` now detects "drill toolpaths": when
  `config.cycles.supported` is true and every `Cutting` pass satisfies
  `is_drill_cutting_pass()`, the toolpath is emitted as a canned cycle block
  rather than individual G0/G1 moves
- Emission logic:
  - First `Linking` pass: emitted normally (positions tool over first hole)
  - First `Cutting` pass: replaced by `format_cycle_header()` output; motion
    modal updated with the cycle code so subsequent lines suppress it;
    `cycle_active` flag set
  - Subsequent `Linking` passes (cycle active): only the last cut-point (XY
    over next hole) emitted; Z managed by G98
  - Subsequent `Cutting` passes (cycle active): suppressed entirely (cycle
    triggers on the preceding XY move)
  - `LeadIn`/`LeadOut`/`SpringPass` kinds: emitted normally via existing path
  - `format_cycle_cancel()` emitted (G80) after the last pass when cycle was
    active
- Non-drill toolpaths fall through to the existing cut-by-cut path unchanged
- 3 assembler-level unit tests in `gcode_golden.rs`: `test_assemble_nonpeck_cycle_g81`
  (Fanuc 0i, simple drill → G81/G80, no G01), `test_assemble_peck_cycle_g83`
  (Fanuc 0i, peck=3mm → G83/Q/G80, no G01),
  `test_assemble_cycles_not_supported_uses_linear` (GRBL → G00/G01, no cycle codes)

**Canned cycle expansion for non-supporting controllers**
- Confirmed correct: when `cycles.supported = false` (e.g. GRBL), the
  assembler falls through to the existing per-move linear path, producing
  explicit Rapid/Feed peck sequences for each hole. No additional code was
  needed — the existing path already produced correct expansion.

### Drill golden fixtures — canned cycles and expansion (`bdc6db7`, `64ad0d3`, `e8ce7ac`)

**New golden fixtures and tests** in `src-tauri/tests/gcode_golden.rs`:
- `grbl_drill_expansion_golden_matches` — 5-hole peck drill via GRBL
  (`canned_cycles = false`); verifies explicit G0/G1 peck sequences and
  NN-sorted XY order; golden file:
  `tests/integration/golden_gcode/grbl/drill_expansion.nc`
- `linuxcnc_drill_cycle_golden_matches` — same 5-hole peck scenario via
  LinuxCNC; verifies G83/Q/G80 output; golden file:
  `tests/integration/golden_gcode/linuxcnc/drill_cycle.nc`
- `fanuc_0i_drill_cycle_golden_matches` — same scenario via Fanuc 0i; verifies
  G83/Q/G80 with Fanuc number formatting and line numbers; golden file:
  `tests/integration/golden_gcode/fanuc-0i/drill_cycle.nc`
- Each test generates its toolpath from `five_hole_peck_toolpath()` (5 holes,
  peck depth 3 mm, depth 10 mm, 50×50×10 mm stock) — **not** read from a
  fixture file. On the first run (when the `.nc` golden doesn't exist yet),
  the test writes both `.toolpath.json` (for human inspection) and the `.nc`
  golden to its directory, then panics prompting a manual review and re-run
  to lock the fixture. The `.toolpath.json` files are identical in content
  across all three directories but are separate files.

### Arc fitting (`379a89f`, `5e576ca`, `f241586`)

**Algorithm** (`src-tauri/src/toolpath/arc_fitting.rs`)
- `fit_arcs(cuts: Vec<CutPoint>, tolerance: f64) -> Vec<CutPoint>` — replaces
  qualifying sequences of linear `Feed` moves with `MoveKind::Arc` moves
- Scans for consecutive `Feed` moves at constant Z; fits circles through
  sliding windows of 3 points via `fit_circle_3pt()` (algebraic circumscribed
  circle); greedily extends arcs while all points remain within `tolerance` of
  the fitted circle and CW/CCW direction is consistent
- Minimum 4 points (3 segments) required for arc replacement; prevents false
  positives on short straight runs
- Z-change breaks detection (2D arcs only, constant Z within each run)
- Determines CW/CCW direction from center-relative cross product via
  `direction_from_center()`
- Preserves existing `Arc`, `Rapid`, and `Dwell` moves unchanged (no
  double-processing of entry motion arcs)
- Arc `CutPoint` convention: `position` = arc start (used for IJK offset
  computation), `end` inside `MoveKind::Arc` = arc destination
- 18 unit tests: `points_on_known_circle_produce_single_arc`,
  `cw_circle_detected_correctly`, `mixed_straight_and_curved_only_curve_replaced`,
  `tolerance_boundary_just_inside_merges`, `tolerance_boundary_just_outside_breaks_arc`,
  `fewer_than_3_segments_no_replacement`, `z_change_breaks_detection`,
  `full_360_circle_detection`, `existing_arc_moves_pass_through`,
  `straight_collinear_points_no_false_positive`, `empty_input_returns_empty`,
  `single_point_returns_unchanged`, `all_rapid_moves_pass_through`,
  `dwell_moves_pass_through`, `arc_center_and_end_are_correct`,
  `all_feed_points_on_circle_no_leading_rapid`, `arc_after_dwell_interruption`,
  `two_arcs_different_radii`

**Pipeline integration** (`src-tauri/src/commands/toolpath.rs`)
- `calculate_toolpath_inner` calls `arc_fitting::fit_arcs(pass.cuts, 0.01)`
  on every pass after `link_passes` and before `Toolpath` assembly
- Applied to all pass kinds (Cutting, Linking, LeadIn, LeadOut, SpringPass)
- Hardcoded tolerance 0.01 mm

**Golden file regeneration** (`src-tauri/tests/gcode_golden.rs`)
- `pocket_toolpath()` helper generates toolpaths through the full production
  pipeline (planner → linking → arc fitting) instead of loading hand-crafted
  fixture JSON; ensures golden tests match the actual `calculate_toolpath` flow
- `simple_pocket_golden(controller)` with auto-write pattern for regeneration
- `fanuc_0i_pocket_contains_arcs` assertion test: verifies G02/G03 arc
  commands appear in the Fanuc 0i pocket G-code output
- Fanuc 0i and LinuxCNC `simple_pocket.nc` golden files regenerated with arc
  commands; pocket golden tests gated on `cam_geometry_bindings` (planner
  dependency)

### Hole auto-detection (`8b58e3b`, `0dc273d`, `ad08963`, `c1f76e0`, `d740dfc`, `a1f9f1b`)

**Test fixture** (`tests/fixtures/plate_with_holes.step`)
- 100×100×20 mm plate with four holes:
  - Two through-holes (diameter 10 mm at (25,25) and 6 mm at (75,25))
  - One blind hole (diameter 8 mm, 12 mm deep at (50,75))
  - One tilted hole (diameter 5 mm, 30° from Z) — used to test axis filtering
- Generated via C++ doctest using OCCT BRepPrimAPI_MakeCylinder and
  BRepAlgoAPI_Cut; exported to STEP and verified via round-trip load

**C++ implementation** (`cam_geometry.cpp`)
- `cg_shape_find_holes(id, min_diameter, max_diameter, CgHoleInfo** out_holes)
  → size_t` — walks all faces via `TopExp_Explorer`; identifies
  `Geom_CylindricalSurface` faces; filters by Z-axis parallelism (1° angular
  tolerance via `cos_tol = cos(1°)`); filters by diameter range; computes
  depth from face bounding box Z-span; classifies through vs. blind by
  comparing face Z-extent to solid bounding box (0.1 mm tolerance); applies
  face location transforms for axis direction and center position; merges
  duplicate faces from seam-split cylinders (same center ±0.5 mm, same
  diameter ±0.01 mm) by keeping maximum depth
- `CgHoleInfo` struct: `center` (CgPoint3), `axis` (CgVec3), `diameter`,
  `depth`, `is_through` (int: 1=through, 0=blind)
- `cg_holes_free(CgHoleInfo*)` — caller frees via `delete[]`
- 3 C++ doctests: plate fixture returns 3 holes (tilted hole filtered),
  diameter filter test, no-holes model test

**Rust safe wrapper** (`src-tauri/src/geometry/holes.rs`)
- `HoleDescriptor { center_x: f64, center_y: f64, radius: f64, depth: f64,
  is_through: bool }` struct with `#[serde(rename_all = "camelCase")]`
- `find_holes(shape: &OcctShape, min_diameter: f64, max_diameter: f64)
  -> Result<Vec<HoleDescriptor>, GeometryError>` — calls FFI, converts
  diameter to radius, converts `is_through: int` to bool
- Dual-compiled: real FFI behind `#[cfg(cam_geometry_bindings)]`; stub
  returning `GeometryError::NotImplemented` otherwise
- 2 tests: `find_holes_stub_returns_not_implemented` (ungated),
  `find_holes_in_plate_fixture` (OCCT-gated: verifies 3 holes with correct
  centers, radii, depths, and through/blind flags)

**IPC command** (`src-tauri/src/commands/geometry.rs`)
- `HoleDescriptorIpc { center_x: f32, center_y: f32, radius: f32, depth: f32,
  is_through: bool }` — f64→f32 downcast, camelCase serde
- `detect_holes_inner(state) -> Result<Vec<HoleDescriptorIpc>, AppError>` —
  reads project lock, validates loaded model + shape; calls `find_holes` with
  full diameter range (0..MAX); returns `NotFound` when no model/shape
- `detect_holes` Tauri command — thin async wrapper
- 4 tests: `hole_descriptor_ipc_serializes_camel_case`, `detect_holes_inner_returns_not_found_when_no_model`,
  `detect_holes_inner_returns_not_found_when_shape_is_none`,
  `detect_holes_inner_returns_holes_for_plate_step` (OCCT-gated)

**Frontend integration** (`d740dfc`)
- `src/api/types.ts`: `HoleDescriptor { centerX, centerY, radius, depth,
  isThrough }` TypeScript interface
- `src/api/geometry.ts`: `detectHoles()` IPC wrapper via `typedInvoke`
- `OperationEditorForm.tsx`: "Detect Holes" button for drill operations;
  calls `detectHoles()`, maps results to `DrillPoint[]` (centerX→x, centerY→y),
  shows "No holes detected" notification when empty, prompts confirmation via
  `window.confirm` when replacing existing points, saves updated points via
  `editOperation`
- 8 new OperationEditorForm tests in "detect holes" describe block:
  `Detect Holes button renders only for drill operations`,
  `does not render for pocket operations`,
  `does not render for profile operations`,
  `clicking Detect Holes calls detectHoles API and populates points`,
  `shows confirmation dialog when existing points would be replaced`,
  `does not replace points when user cancels confirmation`,
  `shows notification when no holes are detected`,
  `shows notification when detectHoles API rejects`

**End-to-end integration test** (`src-tauri/tests/hole_detection_e2e.rs`)
- `detect_holes_returns_expected_geometry` (OCCT-gated): loads
  `plate_with_holes.step`, verifies 3 holes with correct centers, radii,
  depths, and through/blind flags (tilted hole correctly filtered out)
- `detected_holes_produce_valid_drill_toolpath` (OCCT-gated): feeds detected
  holes into a drill operation, runs `calculate_toolpath_inner`, verifies
  toolpath XY positions match detected hole centers within 0.5 mm tolerance
- `open_model_inner` and `detect_holes_inner` promoted to `pub` for
  integration test access

### Phase 2 remaining items

The following Phase 2 deliverables from `development-roadmap.md` are **not yet
implemented**:

- **Adaptive (trochoidal) clearing** — constant engagement angle, high-speed
  machining (operation + algorithm)
- **3D contour / Z-level finishing** — wall finishing via OCCT
  `BRepAlgoAPI_Section` (operation + algorithm)
- **Rest machining (basic)** — compute stock remaining after roughing pass,
  clip finishing paths to un-machined regions only

All other Phase 2 items (15 of 18) are complete.

---

## Phases 3–5

Nothing from Phases 3–5 is implemented. `src-tauri/src/simulation/` does not
yet exist. The post-processor engine returns `PostProcessorError::NotSupported`
when a 5-axis path is encountered.

---

## Test coverage

| Test file | Coverage |
|---|---|
| `src/viewport/scene.test.ts` | SceneManager: scene graph (lights, GridHelper), cameras (perspective/orthographic, Z-up, FOV, clip planes), OrbitControls config, dispose (idempotent), frameModel; projection toggle (mode state, camera sync, frustum sizing, controls reassignment); orthographic resize consistency (left/right respect toggled top/bottom after resize); snap view animation (position, up, distance preservation, mid-flight cancel); display mode (shaded/wireframe/transparent/edges, edge overlay reuse, dispose cleanup) — 55 tests |
| `src/viewport/scene-snap-mock.test.ts` | SceneManager snap direction tests using Tween mock: snapTop/Front/Right/Isometric target positions, no-op when already at target — 5 tests |
| `src/viewport/controls.test.ts` | `createAxisTriad`: returns named Group, 3 ArrowHelper children, correct axis directions and colours (red/green/blue), independent instances — 11 tests |
| `src/viewport/modelMesh.test.ts` | `buildModelMesh`: returns THREE.Mesh, vertex/index counts, position attribute data, normals, bounding sphere, MeshStandardMaterial, single-triangle edge case — 9 tests |
| `src/viewport/toolpathLines.test.ts` | `buildToolpathLines`: null input, empty positions, LineSegments instance type, position attribute count, color attribute count, vertexColors on material — 6 tests |
| `src/store/projectStore.test.ts` | Zustand store: state transitions (setSnapshot), `useModelPath`, `useModelChecksum`, `useOperations`, `useTools`, `useStock` selectors — 22 tests across 6 describe blocks. Note: `useWcs`, `useNotifications`, and `useSelectedOperationId` selectors are implemented but not directly tested here (covered implicitly via component tests). |
| `src/store/viewportStore.test.ts` | Viewport store: initial state (`meshData`, `orbitTarget`, `zoom`, `displayMode`, `projectionMode`), setters, `selectionMode`, `hoveredFaceIdx`, `selectedFaceFingerprints`, `faceDescriptors`, `setSelectionMode`, `toggleFaceSelection`, `clearFaceSelection`, `setFaceDescriptors`, `setProjectionMode`, `setDisplayMode` — 32 tests. Note: `toolpathGeometry` and `setToolpathGeometry` are covered implicitly via Viewport component tests. |
| `src/components/toolbar/Toolbar.test.tsx` | Toolbar: Open Model (calls openModel, updates meshData+snapshot, cancellation, error+dismiss), New Project (clears meshData, updates snapshot, error), Save Project (calls saveProject, cancellation, error), Open Project (loadProject, model reload, meshData clear, error, getToolpathGeometry for non-stale, skip stale) — 22 tests across 4 describe blocks |
| `src/components/operations/OperationListPanel.test.tsx` | Operation list: rendering (5), add buttons disabled/enabled/addOperation calls per type/snapshot refresh (8: disabled/enabled/profile/pocket/drill/zlr-disabled/zlr-calls/snapshot-refresh), enable/disable toggle (2), delete (2), row selection and OperationEditorForm mount (3), stale indicator (2), Calculate button gates and behaviour (12), calculate loading state (4), reorder (7), progress bar (2) — 47 tests across 10 describe blocks |
| `src/components/operations/OperationEditorForm.test.tsx` | OperationEditorForm: null state, profile form (depth/stepdown/compensation/entry motions/blank-sends-null), pocket form (entry motions), tool change saves, input blur saves, drill form, geometry section, z_level_roughing form (tool select/depth/stepdown/stepover/geometry/overrides), detect holes (button renders/hidden per op type, API call + point population, confirmation dialog, cancel confirmation, empty-result notification, API-error notification), error handling — 58 tests across 10 describe blocks |
| `src/components/common/Notifications.test.tsx` | Notifications: no toasts when empty, renders on add, renders multiple, click × dismisses, auto-dismisses after 5 s — 5 tests |
| `src/components/stock/StockPanel.test.tsx` | StockPanel: null state/'No stock defined', stock defined shows values/Clear button, Set Stock submit calls correct payload, Clear Stock calls setStock(null), error notification on Set Stock reject — 5 tests |
| `src/components/wcs/WCSPanel.test.tsx` | WCSPanel: display (empty / with WCS), Set WCS (update existing / create new), Clear WCS calls `setWcs([])`, error notification — 6 tests |
| `src/components/tools/ToolLibraryPanel.test.tsx` | ToolLibraryPanel: rendering, add/cancel/submit form, edit pre-populate/submit, delete, error notifications — 12 tests |
| `src/viewport/Viewport.test.tsx` | Seven describe blocks: mount/unmount (4), mesh updates (4), face selection mode (1), keyboard shortcuts — T/F/R/I routing, uppercase T, INPUT/TEXTAREA focus guards, remove-listener on unmount, P-key projection toggle ×2 (10), toolbar button — renders Perspective label, click toggles, label updates (3), projection mode sync — no-op on mount, syncs on store change, skips when modes agree (3), display mode — select rendered, all options, store sync, SceneManager calls, setModelMesh on load/clear (6) — 31 tests |
| `src/App.test.tsx` | App smoke test (renders AppShell) |
| `src/components/gcode/GCodePreviewPanel.test.tsx` | GCodePreviewPanel: placeholder when no op selected, placeholder when NotFound, renders G-code text, Export button calls exportGcode, PP selector populated from listPostProcessors — 5 tests |
| `src-tauri/cpp/tests/` | C++ geometry wrapper: OCCT loaders + Clipper2 offset/boolean + `cg_shape_section_at_z` + `cg_shape_find_holes` (plate fixture 3-hole detection, diameter filter, no-holes model) + plate_with_holes.step fixture generation (doctest) |
| `src-tauri/tests/gcode_golden.rs` | Golden-file integration: fanuc-0i simple pocket (full pipeline with arc fitting, gated), linuxcnc simple pocket (full pipeline, gated), `fanuc_0i_pocket_contains_arcs` (G02/G03 assertion); assembler-level canned cycle tests (`test_assemble_nonpeck_cycle_g81`, `test_assemble_peck_cycle_g83`, `test_assemble_cycles_not_supported_uses_linear`); GRBL drill expansion golden; LinuxCNC drill cycle golden; Fanuc 0i drill cycle golden — 9 tests |
| `src-tauri/tests/pocket_golden.rs` | Golden-file integration: pocket algorithm JSON output (`#[cfg(cam_geometry_bindings)]`) |
| `src-tauri/tests/profile_golden.rs` | Golden-file integration: profile algorithm JSON output (`#[cfg(cam_geometry_bindings)]`) |
| `src-tauri/tests/drill_golden.rs` | Golden-file integration: drill algorithm JSON output (ungated; 5 holes, peck drilling) |
| `src-tauri/tests/toolpath_cache.rs` | End-to-end cache round-trip: save/load preserves toolpath + validity; param mutation marks stale (ungated; uses drill operations) |
| `src-tauri/tests/zlevel_roughing_golden.rs` | Golden-file integration: Z-Level Roughing algorithm JSON output (`#[cfg(cam_geometry_bindings)]`; box.step, depth=5/stepdown=2/stepover=0.4) |
| `src-tauri/tests/hole_detection_e2e.rs` | End-to-end hole detection: `detect_holes_returns_expected_geometry` (3 holes from plate fixture, tilted filtered), `detected_holes_produce_valid_drill_toolpath` (detected holes → drill op → toolpath XY match) — 2 OCCT-gated tests |
| `src-tauri/src/postprocessor/` (inline) | Config parse (incl. `PeckRetractMode` — full/chip_break/absent/invalid), formatter, modal, arcs, block, program, public API; `cycles.rs` — `is_drill_cutting_pass` (simple/peck/mixed-XY/wrong-count), `classify_drill_pass`, `format_cycle_header` (G81/G83), `format_cycle_cancel` (G80/err-when-absent), `cycles_not_supported`, `peck_retract_mode_selects_g83` |
| `src-tauri/src/commands/` (inline) | All command handlers: file ops (save/load/new/export_gcode round-trip and error tests; 5 `#[tokio::test]` for `open_model` — file-not-found, geometry-error without OCCT, full mesh load, shape stored, shape absent without OCCT), tool CRUD, stock/WCS, operations CRUD, project snapshot (snapshot fields, camelCase serialization, real `needs_recalculate` comparison), toolpath (calculate + cache populate + progress events, get_geometry, G-code preview), geometry (get_model_faces: camelCase, no-model, no-shape, OCCT integration; detect_holes: camelCase, no-model, no-shape, OCCT integration with plate fixture); plus three OCCT-gated tests in `commands/toolpath.rs`: pocket toolpath end-to-end, geometry-selection boundary clamping, and invalid-fingerprint error |
| `src-tauri/src/models/` (inline) | Tool, stock, WCS, operation — serde round-trips and field invariants (profile/pocket/drill/zlr op round-trips; `operation_type_field_at_top_level`; `operation_fields_are_camel_case`; `operation_enabled_defaults_to_true_when_absent`); `drill_peck_depth_absent_when_none`; `DrillPoint` round-trip/non-empty/default-empty; `Operation` feed/speed override absent-None/present-set/default-None; `CacheState` defaults-when-absent/round-trip; `ZLevelRoughingParams` round-trip + type-field assertion; geometry field serde for Pocket/Profile/ZLevelRoughing (absent-when-None, present-when-set, round-trip with fingerprints, defaults-absent-in-old-JSON — 12 tests) |
| `src-tauri/src/toolpath/` (inline) | `types.rs` serde (Toolpath/MoveKind/PassKind/ToolpathStats/LineGeometryData round-trips and tag/camelCase assertions; 8 tests), `cache.rs` key stability + sensitivity (4 tests), `arc_fitting.rs` 18 tests (known circle CW/CCW, mixed straight+curved, tolerance boundary inside/outside, min-segment count, Z-change breaks, full 360°, existing arc passthrough, collinear rejection, empty/single-point, rapid/dwell passthrough, center+end correctness, dwell interruption, two arcs different radii), `linking.rs` 34 tests total — 3 pass-wrapping (sequence, rapid moves, single-point skip) + 31 entry-motion: 8 helical (Z monotone, XY on circle, pitch, fallback, closed-contour integration, cleanup arc, radius-too-large fallback, degenerate-radius fallback) + 12 ramp (Z span, Z monotone, horizontal distance, fallback, short-segment clamp, angle≥90, inverted-Z produces no moves, zero-length, open-contour integration, non-clamped, invalid-angle plunge-fallback, ramp+arc-combination continuity) + 11 arc lead-in/out (None straight, Some arc, last-move lands at cut point, first-move outside, all Feed, departure None/Some/Feed, linking-descent-to-arc-start, lift-from-arc-end, closed-contour-no-ramp), `planner.rs` 12 tests: stats non-zero for Pocket/Profile (gated) + profile-error without bindings (stub path) + feed/speed override/fallback (6: spindle/feed × override/tool-default/unset) + geometry-none-uses-stock + geometry-some-no-shape-error + ZLR-invalid-params-returns-InvalidInput, `operations/pocket.rs` Z-levels/output/error, `operations/profile.rs` Z-levels/compensation/collapse/Left-vs-Center-differ (gated) + Center-uses-raw-boundary (ungated) + 8 stepdown tests (None→single-pass, Some(0)→single, Some(-1)→single, JSON-absent, backward-compat, None→1 Z-level, stepdown=2/depth=8→4 passes, stepdown=3/depth=8→3 passes floor-clamped), `operations/drill.rs` empty/bad-peck errors + non-peck geometry + peck Z-levels + multi-hole ordering + `test_sort_single` + `test_sort_grid`, `operations/zlevel_roughing.rs` 3 ungated param validation tests (zero stepdown/depth/out-of-range stepover) + 6 OCCT-gated tests (produces passes, Z-level span, floor depth guarantee, stock boundary Z-level count, geometry section boundary, tool-too-large collapse) |
| `src-tauri/src/project/` (inline) | `serialization.rs` (13 tests): multiple round-trips (with/without model, with tool, with stock+WCS, with operations); schema version rejection (`load_rejects_unknown_schema_version`); graceful missing file (`load_fails_gracefully_on_missing_file`); ZIP structure validation (`save_creates_valid_zip`); backward-compat load without `tools` field (`load_phase0_schema_without_tools_field_succeeds`); toolpath ZIP entry write (positive + negative); round-trip with valid toolpath; graceful load with missing toolpath entry |
| `src-tauri/src/geometry/` (inline) | `clipper.rs`: 2 stub tests (offset + boolean, ungated/not-bindings-only) + 2 OCCT-gated integration tests (offset shrinks square, offset returns error on collapse) — 4 tests; `safe.rs`: GeometryError Display + externally-tagged serde serialization (8), MeshData accessibility + serialization + FaceGroup camelCase (3), OcctShape/Mesh Send + null-drop safety (4), loader stubs return correct error variants (5), OCCT-gated fixture load + bounding-box + tessellate (3), `section_at_z` stub returns NotImplemented (ungated) + box midheight returns single loop (gated) (25 tests); `importer.rs`: missing file + unknown/no/uppercase extension dispatch (7 ungated) + OCCT-gated fixture load and `import_with_shape` (2 gated) — 9 tests; `faces.rs`: fingerprint determinism + fingerprint sensitivity (differs for different inputs) + known value + two stub-path error tests (5 tests); `holes.rs`: stub returns NotImplemented (ungated) + plate fixture 3-hole detection with centers/radii/depths/through-flags (OCCT-gated) — 2 tests; `mod.rs`: FFI constant sizes and enum discriminants incl. `cg_hole_info_size` (18 tests) |
| `src-tauri/src/` (inline) | `error.rs`: all AppError variant serde format tests incl. `InvalidInput` + adjacently-tagged encoding + Display (11 tests); `state.rs`: Project/AppState defaults and `RwLock` write access (7 tests); `lib.rs`: 2 placeholder tests (sanity arithmetic + serde round-trip) |

Golden-file tests cover the post-processor output stage (G-code) and all four
CAM algorithm output stages (pocket, profile, drill, and Z-Level Roughing
toolpath JSON). The pocket, profile, and ZLR golden tests are gated on
`cam_geometry_bindings`; the drill algorithm golden test is ungated since
drilling requires no geometry bindings. The G-code golden tests for simple
pocket (fanuc-0i and linuxcnc) are now gated on `cam_geometry_bindings` as
they generate toolpaths through the full planner pipeline including arc
fitting; the drill cycle golden tests (fanuc-0i, linuxcnc, grbl) remain
ungated. The `fanuc_0i_pocket_contains_arcs` regression test verifies that
arc fitting produces G02/G03 commands in the output.

---

## Key files by area

### Rust backend
| File | Purpose |
|---|---|
| `src-tauri/src/main.rs` | Thin binary entry point (calls `lib.rs::run()`) |
| `src-tauri/src/lib.rs` | Tauri app init, IPC command registration |
| `src-tauri/src/state.rs` | `AppState`, `Project` (in-memory document with all fields), `LoadedModel` (path + checksum + `MeshData` + `shape: Option<OcctShape>`), `UserPreferences` (recent files list, not yet persisted); `Project.toolpaths: HashMap<Uuid, Toolpath>` |
| `src-tauri/src/error.rs` | `AppError` enum (thiserror, adjacently-tagged serde); variants: `FileNotFound`, `GeometryImport`, `Io`, `ProjectLoad`, `ProjectSave`, `UnsupportedFormat`, `NotFound`, `PostProcessor`, `InvalidInput` |
| `src-tauri/src/models/tool.rs` | `Tool`, `ToolType` |
| `src-tauri/src/models/stock.rs` | `StockDefinition`, `BoxDimensions`, `Vec3` |
| `src-tauri/src/models/wcs.rs` | `WorkCoordinateSystem` |
| `src-tauri/src/models/operation.rs` | `Operation` struct, `OperationParams` enum (`Profile`/`Pocket`/`Drill`/`ZLevelRoughing`), `ProfileParams` (incl. `stepdown: Option<f64>` and five entry motion fields), `PocketParams` (incl. five entry motion fields), `DrillParams`, `DrillPoint`, `ZLevelRoughingParams` (incl. five entry motion fields), `CompensationSide`, `CacheState`, `CachedStats` |
| `src-tauri/src/toolpath/types.rs` | `Toolpath`, `Pass`, `PassKind`, `CutPoint`, `MoveKind`, `ToolOrientation`, `ToolpathStats`, `LineGeometryData`, `LinkingParams` |
| `src-tauri/src/toolpath/linking.rs` | `link_passes(passes, params: &LinkingParams)` — lift/traverse/descend between cutting passes with optional helical entry, ramp entry, arc lead-in, and arc lead-out |
| `src-tauri/src/toolpath/planner.rs` | `plan()` — resolves geometry boundary, dispatches to algorithm, returns passes + stats; linking, arc fitting, and `Toolpath` assembly happen in `calculate_toolpath_inner` |
| `src-tauri/src/toolpath/operations/pocket.rs` | Pocket clearing algorithm (concentric offset contours per Z level) |
| `src-tauri/src/toolpath/operations/profile.rs` | Profile contouring algorithm (single offset contour per Z level) |
| `src-tauri/src/toolpath/operations/drill.rs` | Drill cycle algorithm (nearest-neighbor sort, linking + cutting passes per hole, peck support) |
| `src-tauri/src/toolpath/operations/zlevel_roughing.rs` | Z-Level Roughing algorithm (OCCT section at each Z level + concentric offset per level; stock-boundary fallback) |
| `src-tauri/src/toolpath/arc_fitting.rs` | `fit_arcs(cuts, tolerance)` — replaces qualifying linear Feed sequences with Arc moves; 3-point circle fitting, direction consistency, constant-Z constraint |
| `src-tauri/src/toolpath/cache.rs` | `compute_cache_key()` — deterministic SHA-256 cache key for toolpath operations |
| `src-tauri/src/geometry/clipper.rs` | Safe Rust wrappers: `poly_offset`, `poly_boolean`, `BoolOp` |
| `src-tauri/src/postprocessor/config.rs` | TOML schema deserialization + validation |
| `src-tauri/src/postprocessor/formatter.rs` | Number formatting + template substitution |
| `src-tauri/src/postprocessor/block.rs` | Block/word assembly and rendering |
| `src-tauri/src/postprocessor/arcs.rs` | IJK and R-format arc computation |
| `src-tauri/src/postprocessor/modal.rs` | Modal G-code state suppression |
| `src-tauri/src/postprocessor/cycles.rs` | Canned cycle detection (`is_drill_cutting_pass`, `classify_drill_pass`, `DrillCycleKind`, `DrillCycleParams`) and formatting (`format_cycle_header`, `format_cycle_cancel`); config accessors for cycle codes |
| `src-tauri/src/postprocessor/program.rs` | Full G-code program assembler; drill toolpath detection emits G81/G83/G80 canned cycle blocks when `cycles.supported = true`; non-drill toolpaths use per-move linear path |
| `src-tauri/src/postprocessor/mod.rs` | `PostProcessor` public API; `PostProcessorError` enum (variants: `Config`, `NotSupported`, `ArcError`, `Assembly`); `PostProcessorMeta` struct; re-exports `program::ToolInfo` |
| `src-tauri/src/postprocessor/builtins/` | `fanuc-0i.toml`, `linuxcnc.toml`, `mach4.toml`, `grbl.toml` (first three have `peck_retract_mode = "full"` under `[cycles]`) |
| `src-tauri/src/commands/file.rs` | `open_model`, `save_project`, `load_project`, `new_project`, `export_gcode` |
| `src-tauri/src/commands/geometry.rs` | `get_model_faces` — enumerates faces from persisted `OcctShape`; returns `Vec<FaceDescriptorIpc>` (fingerprint, face_idx, centroid/normal as `[f32;3]`, area); `detect_holes` — finds cylindrical holes in loaded shape; returns `Vec<HoleDescriptorIpc>` (center_x/y, radius, depth, is_through as f32+bool) |
| `src-tauri/src/commands/toolpath.rs` | `list_post_processors`, `get_gcode_preview`, `calculate_toolpath` (with progress events), `get_toolpath_geometry` |
| `src-tauri/src/commands/tools.rs` | Tool CRUD commands |
| `src-tauri/src/commands/stock.rs` | Stock/WCS commands |
| `src-tauri/src/commands/operations.rs` | Operation CRUD commands; `OperationInput` (add/edit input type) |
| `src-tauri/src/commands/project.rs` | `get_project_snapshot`; `ProjectSnapshot`, `ToolSummary`, `OperationSummary` IPC output types |
| `src-tauri/src/geometry/importer.rs` | Format dispatch (STEP/IGES/STL); `import_with_shape()` returns live `OcctShape` for STEP/IGES |
| `src-tauri/src/geometry/safe.rs` | Safe Rust wrappers: `OcctShape` (with `unsafe impl Sync`), `OcctMesh` (with `Drop` impls); `MeshData` struct (incl. `face_groups`); `FaceGroup` struct; `GeometryError` enum (variants incl. `NotImplemented`); `shape_section_at_z()` + `stitch_segments_into_loops()` |
| `src-tauri/src/geometry/faces.rs` | `FaceInfo`, `FaceDescriptor` structs; `enumerate_faces()` (skips non-planar); `face_boundary(shape, face_idx)`; `face_fingerprint()` (64-char hex SHA-256); dual-compiled (OCCT / stub) |
| `src-tauri/src/geometry/holes.rs` | `HoleDescriptor` struct; `find_holes(shape, min_diameter, max_diameter)` — cylindrical hole detection via OCCT; dual-compiled (OCCT / stub) |
| `src-tauri/src/geometry/ffi.rs` | FFI bindings module: includes bindgen output written to `$OUT_DIR` at build time |
| `src-tauri/src/project/types.rs` | On-disk serialization types: `ProjectMeta`, `SourceModelRef`, `ProjectFile` (mirrors `project.json` schema; distinct from the in-memory `Project` in `state.rs`) |
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
| `tests/integration/golden_gcode/fanuc-0i/simple_pocket.toolpath.json` | Auto-generated pocket toolpath JSON (full pipeline: planner → linking → arc fitting) for human inspection |
| `tests/integration/golden_gcode/fanuc-0i/simple_pocket.nc` | Golden G-code output for Fanuc 0i (includes G02/G03 arcs) |
| `tests/integration/golden_gcode/linuxcnc/simple_pocket.toolpath.json` | Same auto-generated pocket toolpath for LinuxCNC |
| `tests/integration/golden_gcode/linuxcnc/simple_pocket.nc` | Golden G-code output for LinuxCNC (includes G02/G03 arcs) |
| `tests/integration/pocket/toolpath.json` | Golden pocket algorithm output (50×50×10 mm, 10 mm tool) |
| `tests/integration/profile/toolpath.json` | Golden profile algorithm output (50×50×10 mm, 6 mm tool, Left compensation) |
| `tests/integration/drill/toolpath.json` | Golden drill algorithm output (50×50×10 mm, 5 mm drill, 5 holes, 3 mm peck) |
| `tests/integration/golden_gcode/fanuc-0i/drill_cycle.nc` | Golden G-code for Fanuc 0i canned cycle drill (G83/Q/G80, 5 holes, peck=3 mm) |
| `tests/integration/golden_gcode/linuxcnc/drill_cycle.nc` | Golden G-code for LinuxCNC canned cycle drill (G83/Q/G80, 5 holes, peck=3 mm) |
| `tests/integration/golden_gcode/grbl/drill_expansion.nc` | Golden G-code for GRBL drill expansion (G0/G1 peck sequences, no cycle codes) |
| `tests/fixtures/zlevel_roughing_golden.json` | Golden Z-Level Roughing output (box.step, depth=5/stepdown=2/stepover=0.4) |
| `tests/fixtures/plate_with_holes.step` | 100×100×20 mm plate with 4 holes (2 through, 1 blind, 1 tilted) for hole detection tests |

### TypeScript frontend
| File | Purpose |
|---|---|
| `src/api/types.ts` | TypeScript mirrors of Rust types (incl. `PostProcessorMeta`, `ExportParams`, `FaceDescriptor`, `HoleDescriptor`, `ToolpathProgressEvent`, `ZLevelRoughingParams`); operation union types include `'z_level_roughing'`; `ProfileParams.stepdown` is `number | null`; `ProfileParams`, `PocketParams`, `ZLevelRoughingParams` include five optional entry motion fields (`arcLeadInRadius`, `arcLeadOutRadius`, `helicalEntryRadius`, `helicalEntryPitch`, `rampEntryAngleDeg`) |
| `src/api/file.ts` | File operation IPC wrappers |
| `src/api/tools.ts` | Tool CRUD IPC wrappers |
| `src/api/stock.ts` | Stock/WCS IPC wrappers |
| `src/api/operations.ts` | Operation CRUD IPC wrappers |
| `src/api/geometry.ts` | `getModelFaces()` and `detectHoles()` IPC wrappers |
| `src/api/toolpath.ts` | `listPostProcessors`, `getGcodePreview`, `exportGcode`, `calculateToolpath`, `getToolpathGeometry`, `listenToolpathProgress` |
| `src/store/projectStore.ts` | Project Zustand store; selector hooks: `useModelPath`, `useModelChecksum`, `useOperations`, `useTools`, `useStock`, `useWcs`, `useNotifications`, `useSelectedOperationId`, `usePushNotification` |
| `src/store/viewportStore.ts` | Viewport Zustand store (incl. `meshData`, `orbitTarget`, `zoom`, `toolpathGeometry`, `selectionMode`, `hoveredFaceIdx`, `selectedFaceFingerprints`, `faceDescriptors`, `projectionMode`, `displayMode`) |
| `src/viewport/scene.ts` | Three.js renderer + scene + `toolpathGroup` + `setToolpathLines()` + `setOrbitEnabled()` + camera getter; projection toggle (`toggleProjection`, `getProjectionMode`); standard view snaps (`snapTop/Front/Right/Isometric`) with Tween animation; display mode (`setDisplayMode`, `setModelMesh`) |
| `src/viewport/controls.ts` | `createAxisTriad()` — RGB ArrowHelper group (X=red, Y=green, Z=blue) added to the main scene |
| `src/viewport/modelMesh.ts` | `buildModelMesh(meshData)` → `ModelMeshResult { mesh: THREE.Mesh, boundingSphere }` |
| `src/viewport/toolpathLines.ts` | `buildToolpathLines()` → `THREE.LineSegments` from `LineGeometryData` |
| `src/components/layout/AppShell.tsx` | Top-level layout |
| `src/components/operations/OperationListPanel.tsx` | Operation list: add/delete/toggle/reorder operations (incl. Z-Level Roughing), row selection, stale indicator, Calculate button with loading state, `OperationEditorForm` mount |
| `src/components/operations/OperationEditorForm.tsx` | Pocket, profile, drill, and z_level_roughing parameter forms; feed/speed override inputs; dynamic drill-points table; geometry section; five optional entry motion inputs on Profile and Pocket forms; Detect Holes button (drill only) |
| `src/components/stock/StockPanel.tsx` | Stock definition form: origin, dimensions, Set/Clear Stock buttons |
| `src/components/wcs/WCSPanel.tsx` | WCS panel: origin X/Y/Z editing, Set WCS and Clear WCS buttons |
| `src/components/tools/ToolLibraryPanel.tsx` | Tool library: list, add form, edit form, delete; refreshes project snapshot after each mutation |
| `src/components/toolbar/Toolbar.tsx` | File operation toolbar (Open Model, New Project, Save Project, Open Project) |
| `src/components/common/Notifications.tsx` | Dismissible toast overlay with auto-dismiss after 5 s |
| `src/components/gcode/GCodePreviewPanel.tsx` | G-code preview with PP selector + Export |

---

*Related documents: `development-roadmap.md`, `system-architecture.md`, `geometry-kernel.md`, `viewport-design.md`*
