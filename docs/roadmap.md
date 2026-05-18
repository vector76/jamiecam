# JamieCam Roadmap

> **Post-pivot note:** This is the forward-looking multi-mode plan.
> After the Tauri→web pivot, **Mode 1 (G-code Viewer)** and **Mode 2
> (2D Profile cuts, MVP)** ship in the web build. Shared-infrastructure
> rows annotated "Done — web" describe code that is live and
> WASM-compatible. Rows annotated "Done (Tauri only)" describe deleted
> desktop code that must be reintroduced in WASM-compatible form before
> the modes that depend on them can land.
>
> The Phase 4 design decisions that scope the Mode 2 MVP — pure-Rust
> `clipper2-rust`, profile-cuts only, separate machine-setup model,
> GRBL-only emitter, Canvas2D workspace, **no speculative
> `GeometrySource` abstraction in the planner** — are recorded in
> `phase-4-design.md`. The forward-looking notes below should be read in
> light of that document.
>
> See `web-port-handoff.md` for the live state of the codebase.

## Guiding Principles

**Mode independence.** Each machining mode is almost a separate application
with its own geometry pipeline, operations, and UI. Modes share infrastructure
but do not depend on each other. A bug in Mode 5 never blocks Mode 2.

**Incremental delivery.** Each mode is shipped as it becomes ready. There is no
release that requires all modes to be complete. A user who only needs 2D
profiling should not wait for 5-axis kinematics.

**Runnable at every stage.** Every merge produces a working application.
No merge leaves the application in a half-assembled state.

**Defer complexity, not correctness.** Simple operations produce correct output
from day one. Better algorithms are added later. A wrong toolpath is always
worse than a slow one.

**User value at every milestone.** Each mode unlocks a category of real
machining work. The complexity gradient matches the user base: most hobbyist
CNC work is 2D and 2.5D.

**Validate infrastructure through modes.** Shared infrastructure (G-code
parser, dexel engine) is validated by shipping Mode 1, which exercises both
without needing CAM generation.

---

## Shared Infrastructure

These components are used by multiple modes. Items marked "done" are
implemented and tested. Items marked "spec exists" have a design document
but no implementation yet.

| Component | Status | Used by modes |
|---|---|---|
| Tool library (data model, CRUD, UI) | Done — web (minimal CRUD modal; `working_env` Rust module + IndexedDB `workingEnv` store; see `phase-4-design.md` §6) | All |
| Machine setups + tool↔setup availability matrix | Done — web (same modal + `working_env`) | All |
| Post-processor engine (GRBL emitter) | Done — web (hardcoded GRBL only — see `phase-4-design.md` §7) | 2 (today); all (eventually) |
| Pluggable / multi-dialect post-processor | Not started — deliberately deferred until a second dialect is demanded (§7) | All |
| Toolpath types, linking, arc fitting | Done (Tauri only) — reintroduce as Mode 2+ follow-ups require | 2 (extensions), 3+ |
| Clipper2 integration (offset, boolean) | Done — web (`clipper2-rust` pure-Rust port; `src-rust/src/clipper/`) | 2, 3 (required); 4-6 (optional); 7 |
| `.jcam` project file I/O | Done — web (zip with required `mode` field; Mode 2 round-trip persists original imported bytes) | All |
| OCCT build, FFI, tessellation | Done (Tauri only) — needs WASM strategy before Mode 7 (see `web-port-handoff.md` "OCCT possibilities") | 4-6 (optional); 7 (required) |
| OCCT surface evaluation | Done (Tauri only) | 4-6 (optional); 7 (required) |
| Viewport shell (orbit, views, display modes) | Done — web (Three.js; toolbar has dead state, see handoff) | 1, 2 (sim preview), 3+ |
| Canvas2D viewport | Done — web (`src/viewport2d/`) | 2 (primary workspace), 3+ |
| Simulation playback (tool animation) | Done (Tauri only) | All |
| Toolpath cache (SHA-256, persistence) | Done (Tauri only) | All |
| Progress events | Done (Tauri only) | All |
| G-code parser | Done — web (`gcode-parser.md`) | 1 (primary), all (viewer) |
| Dexel material removal engine | Done — web (`dexel-material-removal.md`) | 1 (primary), 2 (via GRBL→worker, route (a)), all (sim) |
| Tool geometry model (revolution profile) | Done — web (`tool-geometry-model.md`) | All (via dexel) |
| Project format: mode field | Done — web (required field, legacy files default to `gcode-viewer`) | All |
| SVG/DXF input parser | Done — web (`usvg` + `dxf` crates; `src-rust/src/parsers/`) | 2, 3, 5, 6 |
| 2D geometry pipeline (`Point2`/`Polyline`/`Region`) | Done — web (`src-rust/src/geometry2d/`) | 2, 3, 5, 6 |
| Profile toolpath generator (offset + step-down passes) | Done — web (`src-rust/src/profile/`) | 2 |
| Pocket / drill / island / tab Mode 2 operations | Not started — Mode 2 MVP is profile-only (`phase-4-design.md` §5) | 2 |
| Heightmap input loader | Not started | 4, 5 |
| STL/OBJ mesh parser | Not started | 4, 5, 7 |

### Project format

The `.jcam` `project.json` has a required `mode` field set at project
creation (currently `"gcode-viewer"` or `"2d-profile"`). The mode
determines which file formats, operations, and UI panels are available
and is immutable after creation. Pre-Phase-4 `.jcam` files default to
`"gcode-viewer"` on load. See `phase-4-design.md` §8.

### Viewport shell

Mode 1 and Mode 2 (sim preview) use the existing Three.js viewport.
Mode 2's primary workspace is a separate Canvas2D component
(`src/viewport2d/Canvas2DViewport.tsx`), not the Three.js viewport with
a locked orthographic camera — see `phase-4-design.md` §9 for the
rationale. Mode 4 will need a heightmap displacement mesh; whether to
extend the existing 3-D viewport or introduce another presentation
class is an open question for that work.

---

## Per-Mode Roadmap

### Mode 1: G-code Viewer / Simulation

Load and visualize G-code from any source. Material removal simulation. No
CAM generation -- this mode is a viewer, not a programmer.

**Reuses:** post-processor engine (for G-code structure knowledge), viewport
shell, simulation playback, tool library (tool geometry for simulation).

**Needs new:**
- G-code parser (spec exists in `gcode-parser.md`)
- Dexel material removal engine (spec exists in `dexel-material-removal.md`)
- Tool geometry revolution profile (spec exists in `tool-geometry-model.md`)
- Viewport mesh rendering for the evolving dexel workpiece
- File open dialog for `.nc` / `.gcode` / `.tap` files
- Mode-specific UI: no operation list, no geometry selection; instead a
  G-code text panel, playback controls, tool selection for simulation

**Dependencies on shared infra:** G-code parser, dexel engine, tool geometry
model. All three are implemented (`gcode-parser.md`,
`dexel-material-removal.md`, `tool-geometry-model.md`) and ship in the
current web build.

**Suggested order within mode:**
1. G-code parser with unit tests
2. Tool geometry revolution profile
3. Dexel engine with unit tests
4. Wire parser output into dexel engine; integration tests
5. Viewport rendering of dexel mesh
6. UI shell (file open, playback, tool selector)

---

### Mode 2: 2D (SVG/DXF, fixed-depth operations)

2D vector artwork input. Each operation has a fixed Z depth. Primary
workspace is a Canvas2D component.

**Shipped (Phase 4 MVP):**
- SVG + DXF import (`usvg`, `dxf` crates → `geometry2d::Polyline`).
- Path selection UI and per-path open/closed display.
- Profile cut operation: tool-radius offset + step-down passes
  (`src-rust/src/profile/`).
- Hardcoded GRBL G-code emitter (`src-rust/src/grbl/`).
- Working environment editor (machine setups, tools, availability matrix)
  persisted to IndexedDB.
- Simulation via the existing dexel worker fed with the emitted GRBL
  G-code (route (a) in `phase-4-design.md` §5).
- Canvas2D primary workspace; Three.js sim preview swap-in.
- `.jcam` round-trip with original imported bytes preserved.

**Follow-ups (deliberately out of MVP scope, see `phase-4-design.md` §5):**
- Pocket clearing algorithm.
- Drill operations.
- Island pocket algorithm (pocket with interior keep-out regions).
- Tab retention (bridges left at intervals during profile cuts).
- Editor UX polish for setups/tools beyond the minimal CRUD modal.
- Multi-dialect post-processor (GRBL is sufficient for now, see §7).

**Suggested order for follow-ups:**
1. Pocket clearing on top of the existing `geometry2d` + clipper offset
   facade.
2. Drill operation.
3. Island pocket (pocket + interior boundaries via clipper).
4. Tab retention on the profile generator.
5. Editor polish.

---

### Mode 3: 2.5D (V-carve)

Same 2D vector artwork input as Mode 2, but the toolpath is 3D. A V-bit
descends to varying depths based on local shape width. Standard V-carve,
flat-bottom V-carve, inlay, and paint-fill variants.

**Reuses:** everything from Mode 2 (SVG/DXF parsers, 2D geometry pipeline,
Clipper2, pocket algorithm for flat-bottom clearing), post-processor, linking.

**Needs new:**
- V-carve algorithm (`toolpath/operations/vcarve.rs`)
- Medial axis computation (progressive inward offset via Clipper2 initially;
  exact straight skeleton later)
- Inlay computation (mirror geometry, gap offset, male piece profile)
- Flat-bottom V-carve (intersection of medial axis depth field with max depth;
  pocket clearing of center area)
- 3D preview mesh from V-carve geometry (swept V-bit profile)

**Dependencies on shared infra:** SVG/DXF parsers (from Mode 2), Clipper2
(done).

**Suggested order within mode:**
1. Medial axis via progressive Clipper2 offset with unit tests
2. Standard V-carve algorithm with golden tests
3. Flat-bottom variant (V-carve + pocket clearing)
4. Inlay variant (female + male piece generation)
5. 3D preview mesh generation
6. UI (V-bit angle, max depth, glue gap parameters)

---

### Mode 4: 3D (Heightmaps, relief, lithophanes)

3-axis surface machining reachable from the top. Input is a heightmap,
STL mesh, or STEP solid model. No undercuts.

**Reuses:** post-processor, linking, viewport shell, simulation playback.
The existing parallel and scallop finishing algorithms could be reused if
refactored behind a `SurfaceModel` trait (see `modes-overview.md`). OCCT
(done) for optional STEP import.

**Needs new:**
- Heightmap loader (PNG/TIFF grayscale, 16-bit RAW)
- STL/OBJ mesh parser
- `HeightmapSurface` struct implementing bilinear Z sampling and finite-
  difference normals
- `MeshSurface` struct implementing ray-cast Z sampling
- `SurfaceModel` trait abstracting over heightmaps, meshes, and OCCT faces,
  enabling reuse of parallel/scallop algorithms
- Roughing passes for deep reliefs (coarse Z-level removal before finishing)
- Viewport: heightmap as `THREE.PlaneGeometry` with Z displacement;
  STL/STEP as triangle mesh
- Mode-specific UI (physical size, depth range, invert flag for heightmaps;
  model import for STL/STEP)

**Dependencies on shared infra:** post-processor (done), OCCT (done, for
optional STEP import), Clipper2 (done, for Z-level roughing on mesh/STEP).
Optionally dexel engine for material removal preview.

**Suggested order within mode:**
1. Heightmap loader with tests
2. STL/OBJ mesh parser with tests
3. `SurfaceModel` trait, `HeightmapSurface`, and `MeshSurface` implementations
4. Parallel raster finishing over heightmap with golden tests
5. Scallop finishing over heightmap
6. Roughing passes
7. Extend to STL/STEP input via `SurfaceModel`
8. Viewport rendering (heightmap + mesh)
9. UI shell

---

### Mode 5: 2+Rotary (X, Z, theta)

One linear axis replaced by a rotary axis. Used for cylindrical objects:
table legs, balusters, cylindrical signs. Accepts SVG/DXF, heightmaps,
STL, or STEP input.

**Reuses:** post-processor (extended with rotary axis config), linking,
tool library, viewport shell, SVG/DXF parsers (from Mode 2), heightmap
loader and STL parser (from Mode 4), OCCT (done, for optional STEP import).

**Needs new:**
- Rotary coordinate transformer (Cartesian XYZ to X/Z/A)
- Profile turning operations (rough and finish from a revolved profile curve)
- Fluting operation (longitudinal or helical channels)
- Cylindrical relief carving (heightmap wrapped around cylinder)
- Rotary-aware feed rate calculation (surface speed depends on current
  diameter)
- Post-processor `[rotary]` section (axis letter, feed mode, diameter word)
- Viewport: cylindrical workpiece display, unwrapped 2D view
- Rotary-specific UI

**Dependencies on shared infra:** post-processor (done), SVG/DXF parsers
(from Mode 2), heightmap loader and STL parser (from Mode 4), OCCT (done,
for optional STEP import), Clipper2 (done, for SVG/DXF offset ops).

**Suggested order within mode:**
1. Rotary coordinate transform with unit tests
2. Post-processor rotary extension
3. Profile turning (rough/finish) with golden tests
4. Fluting
5. Cylindrical relief (reuses heightmap infrastructure from Mode 4)
6. STL/STEP model input for rotary
7. Viewport cylindrical display
8. UI shell

---

### Mode 6: 3+Rotary (X, Y, Z, A simultaneous)

Four simultaneous axes. Three linear plus one continuous rotary. Used for
cam lobes, spiral flutes, port machining.

**Reuses:** OCCT surface evaluation (done), parallel/scallop/flowline
algorithms (done, need 4-axis tool orientation extension), gouge detection
(done, needs 4-axis extension), post-processor, linking.

**Needs new:**
- 4-axis kinematics solver (simpler than 5-axis: one rotary DOF)
- Extension of existing 3D surface algorithms with rotary tool tilt
- 4-axis gouge detection
- Post-processor 4-axis word emission (X, Y, Z, A)
- Viewport: tool tilt visualization

**Dependencies on shared infra:** OCCT (done), 3D surface algorithms (done).

**Suggested order within mode:**
1. 4-axis kinematics solver with unit tests
2. Extend parallel finishing with rotary tilt
3. 4-axis gouge detection
4. Post-processor 4-axis emission with golden tests
5. Viewport tilt visualization
6. UI shell

---

### Mode 7: 5-Axis

Full simultaneous 5-axis. Unlocks undercuts, turbine blades, impellers,
deep cavity work. (The detailed toolpath-engine and post-processor specs
lived in pre-pivot docs that have been removed.)

**Reuses:** OCCT surface evaluation (done), all 3D surface algorithms (done),
gouge detection framework (done), post-processor (done), linking.

**Needs new:**
- 5-axis point milling, swarf milling, multi-axis contour algorithms
- Tool orientation strategies (fixed tilt, surface normal, smoothed, auto-tilt)
- Singularity detection and handling (gimbal lock freeze)
- 5-axis gouge detection (discretized disc model)
- Holder collision detection
- Full kinematics solver (A-C, B-C, A-B table configurations)
- RTCP/TCP support (G43.4 / TRAORI)
- Inverse time feed mode (G93)
- Fixture definition for collision avoidance
- Post-processor configs: `fanuc-30i.toml`, `siemens-840d.toml`
- Viewport: 5-axis simulation with tool tilt, holder visualization

**Dependencies on shared infra:** OCCT (done), 3D algorithms (done).

**Suggested order within mode:**
1. Kinematics solver (one config at a time) with unit tests
2. 5-axis point milling algorithm
3. Swarf milling
4. 5-axis gouge detection + auto-tilt
5. Holder collision detection
6. RTCP support and inverse time feed
7. Post-processor configs with golden tests
8. Viewport 5-axis simulation
9. UI shell

---

## Suggested Implementation Order

**Mode 1 (G-code viewer) first.** It needs only the G-code parser, dexel
engine, and viewport -- all shared infrastructure. Shipping it validates those
shared components before any mode depends on them. It also delivers immediate
user value: anyone with G-code can use it, regardless of what CAM software
produced the code.

**Mode 2 (2D) — profile cuts shipped (Phase 4).** This is the largest
user base -- flat panel work, signs, PCB cutouts. The MVP covers profile
cuts only on a pure-Rust stack (`clipper2-rust`, `usvg`, `dxf`). Pocket,
drill, island pocket, and tab retention are the next slice — see the
Mode 2 entry above for the suggested order. The Phase 4 work also
delivered the shared `geometry2d` types, the working-environment data
model, the GRBL emitter, and the Canvas2D component, all of which
later 2D-input modes (3, 5, 6) will reuse.

**Mode 3 (2.5D) next.** It shares the vector artwork pipeline with
Mode 2 and adds the V-carve algorithm. Natural extension after Mode 2's
profile-cut follow-ups (pocket/drill/island/tab) ship.

**Mode 4 (3D heightmap) could run in parallel with Modes 2-3.** It is
independent of the 2D pipeline. The main new work is the heightmap loader
and the `SurfaceModel` trait abstraction.

**Modes 5-7 in order of complexity.** All require OCCT (already done) or
build on prior rotary/multi-axis work. Mode 5 (2+rotary) introduces the
rotary coordinate transform. Mode 6 (3+rotary) extends it to 4 simultaneous
axes. Mode 7 (5-axis) is the most complex and requires the full kinematics
solver.

---

## Cross-Cutting Concerns

### Testing strategy

Each mode has its own test suite that can run independently. Shared
infrastructure tests run on every PR. Golden file tests validate end-to-end
correctness (input geometry through G-code output).

OCCT-gated tests (`cam_geometry_bindings`) apply to Mode 7 and shared
geometry infrastructure. Modes 4-6 may have OCCT-gated tests for their
optional STEP import paths. Modes 1-3 should have zero OCCT-gated tests,
keeping their test suites fast and portable.

### CI

The existing GitHub Actions matrix (Linux, macOS, Windows) continues. No
changes to CI structure are needed for the mode-based approach. Each mode's
tests are automatically included when its code is merged.

### Documentation

Each mode gets a section in `modes-overview.md` (already written). Shared
infrastructure specs live in their own documents (`gcode-parser.md`,
`dexel-material-removal.md`, etc.). This roadmap replaces `development-roadmap.md`.

---

## What Existing Code Needs to Change

The pre-pivot version of this section described a refactor of the
OCCT-coupled toolpath planner so that 2D and surface inputs could share
the same planner entry point. **That refactor was retired by the Phase 4
design** (see `phase-4-design.md` §4) for two reasons:

1. The original OCCT-coupled planner was deleted in the Tauri→web
   pivot, so there is nothing left to decouple.
2. We do not introduce speculative abstractions ahead of a concrete
   second consumer; the Mode 2 planner therefore takes
   `&[Polyline]`-shaped 2D input directly. A `GeometrySource` enum that
   tries to model OCCT faces, heightmaps, meshes, and polylines before
   any of those non-polyline consumers exist would be designed against
   imaginary requirements and impossible to test honestly under our TDD
   convention.

**Current shape of the planner surface:**
- `profile::generate_profile(input: &ProfileOperationInput)`
  (`src-rust/src/profile/mod.rs`) bundles its inputs into one struct:
  `boundaries: Vec<Polyline>` (from `geometry2d`), `tool: Tool` (from
  `working_env`), `cut_side`, depth/feed/spindle params. It returns
  `ToolpathOutput = Vec<ToolpathMotion>` (Rapid / Linear moves). Any
  `geometry2d::Region`s with holes appear only internally as the
  intermediate result of `clipper::offset_region`.
- `grbl::emit_grbl(toolpath, tool, stock)` (`src-rust/src/grbl/mod.rs`)
  takes that `ToolpathOutput` plus the active `Tool` and
  `types::BoxDimensions` stock, and produces a G-code string.

**When a non-2D geometry consumer actually appears** — e.g. Mode 4
heightmap finishing or Mode 7 STEP-fed roughing — the right move is to
design the abstraction against *that* concrete second consumer plus the
existing 2D consumer, not to invent it now. The candidate shapes
mentioned previously (`GeometrySource::Heightmap(grid)`,
`SurfaceModel` trait, etc.) are still reasonable starting points to
revisit at that time.

---

*Document status: Draft*
*Related documents: `modes-overview.md`, `web-port-handoff.md`, `gcode-parser.md`, `dexel-material-removal.md`, `tool-geometry-model.md`*
