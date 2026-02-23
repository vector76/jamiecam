# JamieCam Development Roadmap

## Guiding Principles

**Runnable at every phase.** Each phase ends with a working application that
can do something useful. No phase ends with a half-assembled system.

**Validate the architecture early.** The riskiest parts of the stack — the OCCT
build system, the Rust FFI layer, and the IPC bridge — are tackled in Phase 0
before any CAM logic is written. Discovering build failures after writing three
phases of algorithms is expensive.

**Defer complexity, not correctness.** Simple operations should produce correct
output from day one. Better algorithms (adaptive clearing, 5-axis tilt) are
added later. A wrong toolpath is worse than a slow one.

**User value at every milestone.** Each phase unlocks a category of real
machining work that a user could take to a machine.

---

## Phase Overview

```
Phase 0 ── Foundation
   │        App shell, OCCT build, basic model display, project file I/O
   │        Validates: architecture, IPC bridge, OCCT on all 3 platforms
   │
Phase 1 ── 2D Operations (MVP)
   │        Profile, pocket, drill, basic linking, Fanuc + LinuxCNC output
   │        User value: machine flat 2D parts and panels
   │
Phase 2 ── 2.5D Operations
   │        Z-level roughing, adaptive clearing, step-down, better linking
   │        User value: rough and finish prismatic 3D parts
   │
Phase 3 ── 3D Surface Machining
   │        Parallel, scallop, flowline, pencil, gouge detection
   │        User value: finish freeform surfaces, molds, organic shapes
   │
Phase 4 ── 5-Axis
   │        Tool orientation, point milling, swarf, 5-axis kinematics
   │        User value: undercuts, blades, complex multi-axis parts
   │
Phase 5 ── Production Polish
            Simulation, performance, packaging, extended post-processors
            User value: confidence before cutting, distribution to end users
```

---

## Phase 0: Foundation

**Goal:** A working Tauri application that opens a 3D model, displays it in a
Three.js viewport, and saves/loads a project file. No CAM operations yet.
This phase exists to validate every architectural seam before building on top of it.

### Infrastructure Deliverables

- [ ] Tauri 2.x project scaffolded with React + TypeScript + Vite frontend
- [ ] Rust backend module structure (`commands/`, `geometry/`, `project/`, etc.)
- [ ] OCCT build system working on all three target platforms
  - cmake builds OCCT as static libraries
  - `build.rs` compiles the C++ wrapper (`cam_geometry.cpp`)
  - `bindgen` generates Rust FFI bindings from `cam_geometry.h`
  - Verified: Ubuntu 22.04, macOS 13+, Windows 11
- [ ] C wrapper: `cg_load_stl`, `cg_load_step`, `cg_shape_tessellate`,
  `cg_mesh_copy_*`, `cg_shape_free`, `cg_last_error_message`
- [ ] Rust safe wrappers: `OcctShape`, `OcctMesh` with `Drop` implementations
- [ ] Three.js viewport: orbit camera (Z-up), axis triad, grid
- [ ] IPC round-trip: frontend calls `open_model`, Rust tessellates, returns
  `MeshData` as typed arrays, frontend builds `THREE.BufferGeometry`
- [ ] `AppState` with `RwLock<Project>` — empty project structure
- [ ] `.jcam` save/load: project metadata + model reference only (no operations yet)
- [ ] Native file open/save dialogs via Tauri `dialog` plugin
- [ ] `tracing` log output to file
- [ ] GitHub Actions CI: build + test on all 3 platforms on every PR

### Acceptance Criteria

- Open a STEP file → shaded model appears in viewport, orbit works
- Open an STL file → same result
- Save project → `.jcam` file created; reopen it → model reference restored
- All CI checks pass on Linux, macOS, and Windows

### Key Risks in This Phase

The OCCT build on Windows is the highest-risk item in the entire project.
OCCT on Windows requires either MSVC or a MinGW toolchain, and linking from
Rust (which defaults to MSVC on Windows) requires careful attention to the
C runtime. Plan for this to take longer than expected. Use `vcpkg` as the
primary Windows OCCT source to avoid building from scratch.

---

## Phase 1: 2D Operations (MVP)

**Goal:** A user can load a 2D or flat 3D model, define a set of 2D machining
operations, and export G-code that can run on a real machine.

**User value:** Machine flat 2D parts: brackets, panels, pockets, drilled patterns.

### Operations Implemented

- [ ] **Profile / Contour** — single depth, computer compensation, left/right/on
- [ ] **Pocket clearing** — offset strategy (conventional and climb)
- [ ] **Drilling** — drill and peck cycles, manual hole placement

### Infrastructure Deliverables

- [ ] **Tool library** — add/edit/remove tools, persistent per-project
- [ ] **Stock definition** — box stock with position offset from model origin
- [ ] **WCS setup** — single WCS, origin placement
- [ ] **Operation list panel** — add, reorder, enable/disable, delete operations
- [ ] **Operation editor** — form UI for each operation type's parameters
- [ ] **Geometry selection** — click faces/edges in viewport to define regions;
  face fingerprinting for stable re-identification
- [ ] **Clipper2 integration** — 2D polygon offset (tool radius compensation);
  C wrapper `cg_poly_offset`, `cg_poly_boolean`
- [ ] **Basic linking** — fixed-Z retract, linear lead-in/lead-out
- [ ] **Post-processor engine** — TOML config loader, modal state tracker,
  block formatter, number formatting, word suppression
- [ ] **Built-in post-processors**: `fanuc-0i.toml`, `linuxcnc.toml`
- [ ] **G-code export** — write `.nc` file via native save dialog
- [ ] **G-code preview panel** — read-only text view of generated G-code
- [ ] **Toolpath visualization** — `LineSegments` in Three.js, color per segment type
- [ ] **Progress events** — progress bar during toolpath calculation
- [ ] **Cache invalidation** — SHA-256 cache key, stale detection on load
- [ ] **Toolpath binary format** — read/write `toolpaths/*.bin` in `.jcam`
- [ ] **Error notifications** — toast/snackbar for IPC errors
- [ ] **Feeds/speeds** — tool default values + per-operation override

### Acceptance Criteria

- Load a 2D DXF or flat-faced STEP file
- Define a pocket operation: select boundary face, set depth/stepdown/tool
- Click Calculate: toolpath appears in viewport within a reasonable time
- Export G-code for Fanuc 0i: output loads cleanly in a G-code simulator
  (e.g. CAMotics or NCViewer) and produces the correct pocket shape
- Save and reopen project: toolpath cache is restored, no recalculation needed
- Modify an operation parameter: cache is invalidated, UI shows "recalculate"

---

## Phase 2: 2.5D Operations

**Goal:** Handle parts that require multiple Z depths and aggressive material removal.

**User value:** Rough and semi-finish prismatic 3D parts: pockets with floors,
stepped parts, parts with varying depth features.

### Operations Implemented

- [ ] **Multi-level profile** — step-down over multiple Z depths
- [ ] **Z-level roughing** — horizontal slice strategy with pocket fill per level
- [ ] **Adaptive (trochoidal) clearing** — constant engagement angle, high-speed machining
- [ ] **3D contour / Z-level finishing** — wall finishing via OCCT `BRepAlgoAPI_Section`

### Infrastructure Deliverables

- [ ] **OCCT section at Z** — `cg_shape_section_at_z` implemented and tested
- [ ] **Arc lead-in / lead-out** — circular arc approach/departure motions
- [ ] **Helical entry** — spiral descent for pocket entry
- [ ] **Ramp entry** — linear angled descent for slot entry
- [ ] **Arc fitting** — detect chord sequences → emit G2/G3 in output
- [ ] **Hole auto-detection** — `cg_shape_find_holes` from OCCT cylindrical face analysis
- [ ] **Drill sorting** — nearest-neighbor hole ordering
- [ ] **Canned cycle emission** — G81/G83/G73/G84/G85 blocks
- [ ] **Canned cycle expansion** — for controllers that don't support them (GRBL)
- [ ] **Built-in post-processors**: `mach4.toml`, `grbl.toml`
- [ ] **Rest machining (basic)** — compute stock remaining after roughing pass,
  clip finishing paths to un-machined regions only
- [ ] **Viewport: standard views** — Top/Front/Right/Iso keyboard shortcuts,
  smooth animated transitions
- [ ] **Viewport: perspective / ortho toggle**
- [ ] **Viewport: display mode selector** — Shaded, Shaded+Edges, Wireframe, Transparent

### Acceptance Criteria

- Load a multi-depth STEP part; define a Z-level roughing operation
- Toolpath shows correct horizontal slices in viewport
- Export G-code: passes NCViewer simulation without gouges
- Adaptive clearing produces trochoidal loops at high engagement areas
- Arc moves appear as G2/G3 in Fanuc output (not thousands of tiny linears)
- GRBL output: all drilling operations expanded to explicit moves (no G83)

---

## Phase 3: 3D Surface Machining

**Goal:** Handle genuinely 3D freeform surfaces — the kind found in molds,
dies, and organic shapes.

**User value:** Finish freeform surfaces to a consistent scallop height;
reach corners and fillets that roughing tools miss.

### Operations Implemented

- [ ] **Parallel (raster) finishing** — constant-direction scan across surface
- [ ] **Scallop finishing** — variable stepover for constant scallop height
- [ ] **Flowline finishing** — follows UV parameter lines of NURBS surfaces
- [ ] **Pencil milling** — traces concave corners and fillets

### Infrastructure Deliverables

- [ ] **OCCT surface evaluation** — `cg_face_eval_normal`, `cg_face_eval_point`,
  `cg_face_project_point` fully implemented and tested
- [ ] **OCCT surface type query** — `cg_face_surface_type`, `cg_face_plane`,
  `cg_face_cylinder`
- [ ] **3-axis gouge detection** — verify each toolpath point is at or above
  surface; report violations with location
- [ ] **Auto-lift on gouge** — adjust Z to clear detected gouges
- [ ] **Material / feed library** — lookup table keyed by workpiece material +
  tool material + operation type; populates default feeds/speeds
- [ ] **Planar face detection** — `cg_shape_find_planar_faces` (feature recognition)
- [ ] **Tessellation LOD** — multiple resolution levels; switch on viewport zoom
- [ ] **5-axis tool orientation indicators** — instanced cylinder meshes showing
  tool axis at intervals along the path (infrastructure for Phase 4)
- [ ] **Viewport: simulation mode (basic)** — tool mesh moving along path,
  play/pause/scrub controls, per-segment feed type color coding
- [ ] **Viewport: measurement overlays** — CSS2DRenderer distance and angle labels
- [ ] **Viewport: toolpath LOD** — decimated display path at low zoom

### Acceptance Criteria

- Load an organic STEP surface (e.g. a mold cavity)
- Define parallel finishing: toolpath covers entire surface, no missed regions
- Define scallop finishing: scallop height measured in NCViewer ≤ specified target
- Pencil milling: toolpath traces all concave corners of a fillet
- Simulation: tool mesh moves smoothly along path at 10× speed without stutter
- Gouge check: intentionally bad toolpath is flagged with violation locations

---

## Phase 4: 5-Axis

**Goal:** Simultaneous 5-axis control — tool position and orientation moving
together. Unlocks undercuts, turbine blades, impellers, and deep cavity work.

**User value:** Parts that 3-axis cannot reach; better surface finish by keeping
the tool normal to the surface; swarf milling of ruled surfaces in one pass.

### Operations Implemented

- [ ] **5-axis point milling** — ball nose on surface, tool axis varies per point
- [ ] **Swarf milling** — tool side follows ruled surface (blades, walls)
- [ ] **Multi-axis contour** — 5-axis wall finishing

### Infrastructure Deliverables

- [ ] **Tool orientation strategies** — fixed tilt, surface normal, smoothed normal,
  auto-tilt; configurable per operation
- [ ] **Singularity detection and handling** — freeze rotary axis near gimbal lock
- [ ] **5-axis gouge detection** — discretized disc model of tool body; check each
  disc against part + fixtures
- [ ] **Holder collision detection** — simplified cylinder+cone holder geometry;
  report minimum stick-out required
- [ ] **Auto-tilt** — minimum rotation to clear gouge while maintaining surface contact
- [ ] **Kinematics solver** — table-table (A-C), head-head, head-table (B-C)
  configurations; tool axis vector → machine A/B/C angles
- [ ] **RTCP/TCP support** — emit G43.4 (Fanuc) or TRAORI (Siemens) when supported
- [ ] **Inverse time feed mode** — G93 for controllers that require it for 5-axis
- [ ] **Built-in post-processors**: `fanuc-30i.toml` (5-axis, RTCP), `siemens-840d.toml`
- [ ] **Viewport: 5-axis simulation** — tool tilts correctly during animation;
  `Quaternion.slerp` between orientation frames
- [ ] **Viewport: holder visualization** — cylinder+cone mesh above tool
- [ ] **Fixture definition** — declare fixture geometry for collision avoidance

### Acceptance Criteria

- Load a turbine blade STEP model; define swarf milling operation
- Toolpath shows correct tool tilt along blade surface in viewport
- Gouge check passes (no tool body intersection with part)
- Export Fanuc 30i G-code: A/B/C words present, RTCP active
- Load a mold cavity; define 5-axis scallop with auto-tilt
- Auto-tilt eliminates gouges that 3-axis path would produce
- Simulation: tool tilts smoothly along path, holder visible, no clipping

---

## Phase 5: Production Polish

**Goal:** The application is ready for real-world daily use. Performance is solid,
distribution is streamlined, and the UI is refined.

**User value:** Confidence before cutting (simulation); predictable performance
on large parts; installable on customer machines.

### Items

- [ ] **Material removal simulation** — render stock being progressively removed
  as tool moves (dexel or mesh-boolean approach — significant engineering work)
- [ ] **Machine envelope simulation** — define machine travel limits;
  visualize and flag out-of-travel moves
- [ ] **Inverse kinematics per machine config** — A/C, B/C, A/B table configs;
  machine-specific offset geometry
- [ ] **Minimal retract linking** — compute minimum clearance per rapid move
  (replaces fixed clearance plane for cycle time optimization)
- [ ] **Extended post-processors**: `centroid.toml`, `okuma-osp.toml`
- [ ] **Heidenhain TNC 640** — extended template engine for conversational format
- [ ] **Tool library: global** — persistent across projects, importable from `.csv`
- [ ] **Post-processor editor** — syntax-highlighted TOML editor in-app
- [ ] **Undo / redo** — command history for all project mutations
- [ ] **Keyboard shortcut system** — fully configurable
- [ ] **Performance: large toolpaths** — GPU buffer streaming for paths > 1M points
- [ ] **Performance: OCCT import** — background thread + progress events for slow STEP files
- [ ] **Installer / updater** — Tauri bundler targets: `.msi` (Windows),
  `.dmg` (macOS), `.AppImage` / `.deb` (Linux); auto-update via Tauri updater plugin
- [ ] **Crash reporting** — opt-in telemetry / crash dumps
- [ ] **Onboarding** — welcome screen, sample projects, first-run walkthrough

---

## Cross-Cutting Concerns

These tracks run in parallel with the phased feature work.

### Testing

```
tests/
├── unit/
│   ├── geometry/          C wrapper unit tests (C++ catch2)
│   ├── toolpath/          Rust: algorithm correctness per operation type
│   ├── postprocessor/     Rust: block formatting, modal suppression, arc math
│   └── project/           Rust: serialization round-trips, migration chain
│
├── integration/
│   ├── golden_toolpaths/  fixed input → compare binary output byte-for-byte
│   ├── golden_gcode/      fixed toolpath → compare G-code output to checked-in .nc
│   └── round_trip/        generate G-code → simulate → compare positions to toolpath
│
└── e2e/
    └── tauri_driver/      Tauri's WebDriver integration: UI flows end-to-end
```

**Golden file policy:** When an algorithm change intentionally alters output,
golden files are updated deliberately and the diff is reviewed in the PR.
Unintentional golden file changes fail CI.

### Continuous Integration

GitHub Actions matrix:

| Job | OS | Trigger |
|---|---|---|
| `build-and-test` | Ubuntu 22.04, macOS 13, Windows 11 | Every PR |
| `clippy` | Ubuntu | Every PR |
| `frontend-typecheck` | Ubuntu | Every PR |
| `golden-tests` | Ubuntu | Every PR |
| `release-build` | All three | Tag push (`v*`) |

Each platform build:
1. Install OCCT via platform package manager (apt / brew / vcpkg)
2. `cargo build --release`
3. `pnpm install && pnpm build`
4. `cargo test`
5. `tauri build` (produces installer artifact)

### Documentation

Architecture documents (this set) are kept alongside code in `docs/`. They are
updated when decisions change — treated as living documents, not snapshots.

A separate `docs/user/` directory (created before Phase 1 ships) will contain
user-facing guides. Architecture docs are for contributors.

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| OCCT Windows build failures | High | High | Tackle in Phase 0; use vcpkg; dedicate time to this specifically |
| OCCT → Rust FFI memory unsafety | Medium | High | C wrapper catches all exceptions; Rust `Drop` on all handles; fuzz the C boundary |
| WebView2 inconsistencies on older Windows | Medium | Low | Minimum Windows 10 1803 (WebView2 ships with 10+); test on VM |
| Large toolpath performance (millions of points) | Medium | Medium | GPU buffer streaming deferred to Phase 5; use LOD display from Phase 3 |
| OCCT surface evaluation accuracy | Low | High | Cross-check against OCCT unit tests; validate scallop height in simulation |
| 5-axis kinematics correctness | Medium | High | Validate against known machine programs; implement one config at a time; dry-run on machine before real cut |
| Heidenhain format complexity | High | Low | Scoped to Phase 5; flag as requiring engine extension; not blocking |
| `.jcam` format migration burden | Low | Medium | Schema version from day one; migration tests in CI |
| Tauri WebView CSS inconsistency | Low | Low | Use a component library with cross-platform testing; avoid cutting-edge CSS |

---

## Architecture Decision Log

Decisions made and recorded in the architecture documents. Listed here as a
navigable cross-reference.

| Decision | Rationale | Document |
|---|---|---|
| Tauri (not Electron) | Rust backend ideal for compute; smaller binary; no bundled Chromium | `technology-stack.md` |
| React + TypeScript | Mature ecosystem, strong typing, broad hiring pool | `technology-stack.md` |
| Three.js (not Babylon.js) | Simpler API for CAM viewport needs; larger ecosystem | `technology-stack.md` |
| OCCT (not truck) | truck not mature enough for STEP/5-axis; OCCT proven at scale | `technology-stack.md` |
| C wrapper over direct C++ FFI | Avoids C++ ABI exposure; `bindgen` from plain C; exception boundary | `geometry-kernel.md` |
| Z-up coordinate system | Matches CNC machine convention; configured via `camera.up` not scene rotation | `viewport-design.md` |
| Handle registry (not raw pointers) | No C++ type exposure across FFI; controlled lifetime via Rust `Drop` | `geometry-kernel.md` |
| Rust is authoritative state owner | Frontend is pure view; no derived CAM state in TypeScript | `system-architecture.md` |
| ZIP archive for `.jcam` | Human-inspectable; separates JSON metadata from binary toolpath blobs | `project-file-format.md` |
| SHA-256 cache key | Content-addressed; detects model changes and param changes uniformly | `project-file-format.md` |
| Data-driven post-processors (TOML) | No code change to add/modify controllers; user-extensible | `gcode-postprocessor.md` |
| f64 compute / f32 display | Algorithmic precision in Rust; GPU-friendly transfer to Three.js | `geometry-kernel.md` |
| Rayon for toolpath parallelism | CPU-bound work; natural data-parallel structure; Tokio for async shell | `system-architecture.md` |
| IJK arc format over R | R-format cannot represent 180° arcs; IJK is unambiguous | `gcode-postprocessor.md` |
| Clipper2 (not custom) | Robust handling of self-intersecting offsets; battle-tested | `geometry-kernel.md` |

---

## Phase Completion Checklist

Before closing a phase and beginning the next:

- [ ] All acceptance criteria for the phase pass on all 3 platforms
- [ ] All new code has unit tests; golden files are checked in
- [ ] CI is green (build, test, clippy, typecheck)
- [ ] Architecture documents updated to reflect any decisions that changed
- [ ] No `unwrap()` or `expect()` in command handlers or anything they call
- [ ] No `TODO` comments left in merged code (converted to tracked issues)
- [ ] A release build artifact is produced and manually tested on all 3 platforms

---

*Document status: Draft*
*Related documents: all documents in `docs/`*
