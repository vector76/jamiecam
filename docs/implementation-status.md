# Implementation Status

_Last updated: 2026-04-10._

This document summarizes what is implemented and working in the codebase,
organized by shared infrastructure and CAM capability. It replaces the
detailed per-phase changelog that tracked the old linear development plan.

---

## Shared Infrastructure (all implemented)

**App shell and build system.** Tauri 2.x + React 19 + Vite frontend. Rust
backend with module structure. OCCT built as static libraries on all three
platforms (Linux, macOS, Windows). C++ wrapper compiled via `build.rs` with
bindgen-generated FFI. GitHub Actions CI on all three platforms.

**OCCT geometry kernel.** C wrapper (`cam_geometry.h/.cpp`) with handle
registry. Safe Rust wrappers (`OcctShape`, `OcctMesh` with `Drop`). STEP,
IGES, and STL import. B-rep tessellation. Surface evaluation (point, normal,
curvature, UV bounds, projection). Shape section at Z. Face enumeration. Hole
auto-detection. Planar face detection deferred.

**Clipper2 integration.** Vendored in `cpp/third_party/`. Polygon offset
(inward/outward) and boolean operations (union, difference, intersection).
Integer-scaled coordinates. Dual-compiled Rust wrappers.

**Tool library.** `Tool` struct with full data model. CRUD IPC commands.
Frontend API, Zustand store, and editor UI. Per-project persistence in
`.jcam`.

**Stock and WCS.** `StockDefinition` (box variant). `WorkCoordinateSystem`
struct with origin and axes. IPC commands and frontend UI for both.

**Operations data model.** `Operation` struct with `OperationParams` enum
covering 10 operation types: Pocket, Profile, Drill, ZLevelRoughing,
ZLevelFinishing, AdaptiveClearing, ParallelFinishing, ScallopFinishing,
FlowlineFinishing, PencilMilling. Feed/speed overrides per operation.
`ProjectSnapshot` with operation summaries. Full IPC and frontend wiring.

**Post-processor engine.** TOML config loader, modal state tracker, block
formatter, number formatting, arc computation (IJK and R-format). Built-in
GRBL configuration (`grbl.toml`). Other configs (`fanuc-0i.toml`,
`linuxcnc.toml`, `mach4.toml`) and advanced features (canned cycles) are
slated for removal; additional controller support is deferred to a future
release. G-code export to `.nc` file. G-code preview panel.

**Toolpath infrastructure.** Abstract types (`Toolpath`, `Pass`, `CutPoint`,
`MoveKind`). Linking (retract/traverse/descend, arc lead-in/out, helical
entry, ramp entry). Arc fitting (G2/G3 emission). Cache system with SHA-256
keys, `.jcam` persistence, stale detection.

**Geometry selection.** Face fingerprinting via SHA-256. Face enumeration
IPC. Per-face triangle groups. Viewport highlight (hover/selected). Selection
state in `viewportStore`. Planner resolution of fingerprints to boundary
polygons via OCCT.

**Viewport.** Three.js scene with orbit controls (Z-up). Standard view
snaps (T/F/R/I keys). Perspective/orthographic toggle. Display modes
(Shaded, Shaded+Edges, Wireframe, Transparent). Toolpath visualization
with per-segment-type coloring. Toolpath LOD (decimation at low zoom).
Simulation playback (tool mesh animation, scrub, speed control). Measurement
overlays (CSS2DRenderer).

**Progress events.** `ToolpathProgressEvent` emitted during calculation.
Frontend progress bar per operation.

**Error handling.** `AppError` with adjacently-tagged serde. `toAppError()`
frontend pattern. Toast notifications.

---

## CAM Algorithms (all implemented)

| Algorithm | Description |
|---|---|
| Pocket clearing | Inward offset by tool radius, repeated by stepover, per Z level |
| Profile contouring | Left/Right/Center compensation, multi-level stepdown |
| Drilling | Full-depth and peck modes, nearest-neighbor sorting, canned cycles |
| Z-level roughing | Horizontal slice strategy with pocket fill per level |
| Z-level finishing | Single offset contour per Z level, finishing allowance, spring pass |
| Adaptive (trochoidal) clearing | Constant engagement angle, trochoidal loops, per-point feed scaling |
| Rest machining | Cross-operation data flow, polygon boolean difference for un-machined regions |
| Parallel (raster) finishing | Rotated scan frame, surface projection, boustrophedon ordering |
| Scallop finishing | Curvature-adaptive stepover, target scallop height, min/max bounds |
| Flowline finishing | UV iso-curve sampling, normal offset, run splitting |
| Pencil milling | Curvature grid sampling, concave region BFS, nearest-neighbor ordering |
| 3-axis gouge detection | Surface projection check with auto-lift correction |

All algorithms have golden file tests (toolpath JSON and G-code output).

---

## Mapping to the Mode Architecture

The codebase was built under a linear phase plan (Phases 0-3). Here is how
the existing work maps to the new mode-based architecture:

**Directly reusable by all modes:** tool library, post-processor engine,
toolpath types, linking, arc fitting, cache system, progress events, viewport
shell, simulation playback.

**Reusable by Mode 2 (2D) with refactoring:** pocket clearing, profile
contouring, and drilling algorithms. These currently receive boundary
polygons that are resolved from OCCT face fingerprints. For Mode 2, the
boundaries will come from SVG/DXF parsed paths instead. The algorithms
themselves are geometry-source agnostic -- the refactoring is in the planner's
boundary resolution step.

**Reusable by Mode 3 (2.5D):** everything from Mode 2, plus Clipper2 for
the progressive-offset medial axis computation.

**Reusable by Mode 4 (3D heightmap) with abstraction:** the parallel and
scallop finishing algorithms could work on heightmaps if refactored behind
a `SurfaceModel` trait. Currently they call OCCT surface evaluation directly.

**Reusable by Modes 6-7 (4-axis, 5-axis):** OCCT surface evaluation, all 3D
surface finishing algorithms, gouge detection framework.

**Not reusable (Mode 1 specific):** Mode 1 (G-code viewer) needs the G-code
parser and dexel engine, neither of which exists yet.

---

## What Is Missing per Mode

**Mode 1 (G-code viewer):** G-code parser, dexel material removal engine,
tool geometry revolution profile. All three have design specs.

**Mode 2 (2D):** SVG/DXF parsers, 2D geometry pipeline adapter (bypass OCCT),
island pocket algorithm, tab retention, 2D canvas viewport mode.

**Mode 3 (2.5D):** V-carve algorithm, medial axis computation, inlay
computation, flat-bottom variant. Depends on Mode 2's SVG/DXF pipeline.

**Mode 4 (3D):** Heightmap loader, STL/OBJ mesh parser, `SurfaceModel`
trait abstraction (over heightmaps, meshes, and OCCT faces), viewport
rendering for heightmaps and meshes. OCCT integration already done (for
optional STEP import).

**Mode 5 (2+rotary):** Rotary coordinate transform, profile turning
operations, fluting, rotary-aware feed rate, post-processor rotary config.
Depends on SVG/DXF parsers (from Mode 2) and heightmap/STL loaders (from
Mode 4). OCCT integration already done (for optional STEP import).

**Mode 6 (3+rotary):** 4-axis kinematics solver, rotary tilt extensions to
3D algorithms, 4-axis gouge detection.

**Mode 7 (5-axis):** 5-axis algorithms (point milling, swarf, multi-axis
contour), full kinematics solver, tool orientation strategies, singularity
handling, holder collision detection, RTCP support, inverse time feed,
5-axis post-processor configs.

**Cross-mode:** project format mode field, planner refactoring to support
non-OCCT geometry sources.

---

## Test Suite

Test counts are approximate and need a deep-dive audit. Golden file tests
cover pocket, profile, drill, Z-level roughing, Z-level finishing, adaptive
clearing, parallel finishing, scallop finishing, flowline finishing, pencil
milling, gouge detection, and G-code output. All passing on three platforms.

---

*Document status: Draft*
*Related documents: `roadmap.md`, `modes-overview.md`, `shared-engine-design-choices.md`*
