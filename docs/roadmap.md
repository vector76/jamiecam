# JamieCam Roadmap

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
| Tool library (data model, CRUD, UI) | Done | All |
| Post-processor engine (GRBL config) | Done | All |
| Toolpath types, linking, arc fitting | Done | All |
| Clipper2 integration (offset, boolean) | Done | 2, 3 (required); 4-6 (optional); 7 |
| `.jcam` project file I/O | Done | All |
| OCCT build, FFI, tessellation | Done | 4-6 (optional); 7 (required) |
| OCCT surface evaluation | Done | 4-6 (optional); 7 (required) |
| Viewport shell (orbit, views, display modes) | Done | All |
| Simulation playback (tool animation) | Done | All |
| Toolpath cache (SHA-256, persistence) | Done | All |
| Progress events | Done | All |
| G-code parser | Spec exists (`gcode-parser.md`) | 1 (primary), all (viewer) |
| Dexel material removal engine | Spec exists (`dexel-material-removal.md`) | 1 (primary), all (sim) |
| Tool geometry model (revolution profile) | Spec exists (`tool-geometry-model.md`) | All (via dexel) |
| Project format: mode field | Not started | All |
| SVG/DXF input parser | Not started | 2, 3, 5, 6 |
| Heightmap input loader | Not started | 4, 5 |
| STL/OBJ mesh parser | Not started | 4, 5, 7 |

### Project format updates

The `.jcam` `project.json` needs a `mode` field set at project creation.
The mode determines which file formats, operations, and UI panels are
available. Mode cannot be changed after creation (avoids invalid state).
One-way upgrade from simpler to more complex modes is permitted.

### Viewport shell enhancements

The viewport currently assumes a 3D scene with an OCCT-tessellated mesh.
Modes 2 and 3 need a 2D canvas as the primary workspace (pan/zoom on XY).
Mode 4 needs a heightmap displacement mesh. The viewport needs an
abstraction that supports these different presentation modes.

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
model. All three have specs and compose as a pipeline (see
`shared-engine-design-choices.md`).

**Suggested order within mode:**
1. G-code parser with unit tests
2. Tool geometry revolution profile
3. Dexel engine with unit tests
4. Wire parser output into dexel engine; integration tests
5. Viewport rendering of dexel mesh
6. UI shell (file open, playback, tool selector)

---

### Mode 2: 2D (SVG/DXF, fixed-depth operations)

2D vector artwork input. Profile, pocket, drill, island pocket, tab
retention. Each operation has a fixed Z depth. Primary workspace is a 2D
canvas.

**Reuses:** pocket clearing algorithm, profile contouring algorithm, drill
algorithm, Clipper2 integration, linking, arc fitting, post-processor,
toolpath cache, progress events.

**Needs new:**
- SVG parser (Rust, using `usvg` crate)
- DXF parser (Rust, using `dxf` crate)
- 2D geometry pipeline: parsed artwork to `Vec<Polyline>` / `Vec<ClosedRegion>`
  that the existing pocket/profile algorithms can consume (currently they
  require OCCT face boundary polygons -- see "What Existing Code Needs to
  Change" below)
- Island pocket algorithm (pocket with interior keep-out regions)
- Tab retention (bridges left at intervals during profile cuts)
- 2D canvas viewport mode (pan/zoom on XY, colored operation overlays)
- Simple 3D preview (extruded mesh from 2D paths, no OCCT)
- Mode-specific operation editor forms

**Dependencies on shared infra:** Clipper2 (done), post-processor (done),
project format mode field.

**Suggested order within mode:**
1. SVG parser with path extraction and unit tests
2. DXF parser with entity extraction and unit tests
3. 2D geometry pipeline adapter (parsed paths to boundary polygons)
4. Wire existing pocket/profile/drill through 2D pipeline; golden tests
5. Island pocket algorithm
6. Tab retention algorithm
7. 2D canvas viewport
8. Operation editor forms and UI shell

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
deep cavity work. Covered extensively in `toolpath-engine.md` and
`gcode-postprocessor.md`.

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

**Mode 2 (2D) second.** This is the largest user base -- flat panel work,
signs, PCB cutouts. It reuses the existing pocket, profile, and drill
algorithms but requires a new SVG/DXF input pipeline. The key work is
building the 2D geometry pipeline that bypasses OCCT (see "What Existing
Code Needs to Change").

**Mode 3 (2.5D) third.** It shares the vector artwork pipeline with Mode 2
and adds the V-carve algorithm. Natural extension after Mode 2 ships.

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

The most significant refactoring task is decoupling the toolpath planner from
OCCT-based geometry input.

**Current state:** The planner (`planner.rs`) resolves operation geometry by
calling `enumerate_faces` on an `OcctShape`, matching face fingerprints, and
extracting boundary polygons via OCCT. This works for solid models but
cannot serve modes that use non-OCCT geometry sources (heightmaps, STL
meshes, SVG/DXF paths).

**Required change:** The planner needs an alternative geometry input path.
Operations in Modes 2-3 receive their boundary polygons directly from the
SVG/DXF parser (via Clipper2 processing). Mode 4 receives a heightmap,
mesh, or OCCT surface via the `SurfaceModel` trait. Mode 5 may receive
any of these plus SVG/DXF paths. Mode 1 has no planner at all (it only
views G-code).

Concretely:
- `pocket_passes` and `profile_passes` already accept `boundary: &[(f64, f64)]`
  as a parameter. The OCCT dependency is in the boundary *resolution* step,
  not in the algorithms themselves.
- The planner's `resolve_geometry_boundary()` function needs to be generalized.
  For 2D modes, the boundary comes from parsed artwork paths. For solid modes,
  it comes from OCCT face analysis. The algorithms downstream are unchanged.
- A `GeometrySource` enum or trait could abstract over these input paths:
  `GeometrySource::OcctFaces(shape, fingerprints)`,
  `GeometrySource::Polygon(boundary)`,
  `GeometrySource::Heightmap(grid)`,
  `GeometrySource::Mesh(mesh)`.

This refactoring is the gate for Mode 2. It should be done early.

---

*Document status: Draft*
*Related documents: `modes-overview.md`, `implementation-status.md`, `shared-engine-design-choices.md`, `gcode-parser.md`, `dexel-material-removal.md`, `tool-geometry-model.md`*
