# Work Scope: Phase 3 Completion — Remaining Infrastructure

**Estimated effort:** 8–10 hours (human)
**Branch context:** Phase 3 is algorithmically complete (parallel, scallop, flowline, pencil milling, gouge detection, basic simulation all done). Remaining work is Phase 3 infrastructure that wasn't needed for the core algorithms but is needed to call the phase done and deliver real user value.

See `docs/development-roadmap.md` (Phase 3 section) and `docs/implementation-status.md` (summary section) for full context.

---

## Items in Scope

### 1. Material / Feed Library (~3–4 hours)

**Roadmap item:** "Material / feed library — lookup table keyed by workpiece material + tool material + operation type; populates default feeds/speeds"

A data-driven lookup table that auto-populates spindle speed and feed rate when the user sets up an operation.

**What to build:**

- A TOML-based material/feed library bundled with the app. Key dimensions: workpiece material (e.g. `aluminum-6061`, `steel-mild`, `wood-mdf`, `plastic-abs`) × tool material (e.g. `hss`, `carbide`) × operation category (e.g. `roughing`, `finishing`, `drilling`). Each entry provides `spindle_speed_rpm` and `feed_rate_mmpm` (and optionally `doc_mm` — depth of cut).
- Rust data model structs: `Material`, `FeedEntry`, `FeedLibrary`; loader that reads from bundled TOML files at startup and stores in `AppState`.
- IPC commands:
  - `list_materials() → Vec<MaterialMeta>` (id + display name)
  - `lookup_feeds(material_id, tool_material, operation_category) → FeedEntry`
- Frontend integration: a `workpiece_material` field on each operation's params (stored in the project, defaulting to `None`). When a material is selected in the operation editor, call `lookup_feeds` and pre-fill the spindle speed and feed rate fields (which remain user-editable). The material selector appears in the common section of each operation editor form (above the existing feed/speed override fields).

**Test requirements (per TDD policy):**
- Unit tests for the TOML loader and lookup function (error on missing material, correct values returned, default fallback).
- Frontend component tests for the material selector (selection triggers pre-fill, existing override values respected).
- No golden test changes needed — the feeds/speeds affect G-code output only when the user edits them, and they are already covered by existing golden files.

---

### 2. Viewport: Measurement Overlays (~2–3 hours)

**Roadmap item:** "Viewport: measurement overlays — CSS2DRenderer distance and angle labels"

Interactive measurement tool in the Three.js viewport so the user can measure distances and angles on the model or toolpath.

**What to build:**

- Add Three.js `CSS2DRenderer` alongside the existing `WebGLRenderer`. The CSS2DRenderer renders HTML labels in the same viewport coordinate space, positioned by 3D world coordinates.
- A `measurementStore` Zustand slice (or extension of `viewportStore`) managing:
  - `measurementMode: 'off' | 'distance' | 'angle'`
  - `measurementPoints: [x, y, z][]` (accumulates clicks)
  - `measurements: Measurement[]` (completed measurements with labels)
- Clicking the model while in measurement mode raycasts against the loaded geometry and adds a point. After 2 points: display a distance label at the midpoint. After 3 points: display an angle label at the middle point.
- A small toolbar button group to activate/deactivate measurement mode (icons: ruler for distance, protractor for angle; Clear All button).
- Labels are CSS2DObject instances anchored to the world-space midpoint/vertex.

**Test requirements:**
- Unit tests for measurement math (distance between two points, angle at vertex).
- Component tests for the toolbar button state changes.
- No backend changes needed.

---

### 3. Viewport: Toolpath LOD (~2 hours)

**Roadmap item:** "Viewport: toolpath LOD — decimated display path at low zoom"

At low zoom levels, large toolpaths are decimated for display so the GPU isn't overwhelmed. This is a display-only optimization; the full toolpath remains in memory for G-code generation.

**What to build:**

- A `decimateToolpath(points: Float32Array, maxPoints: number): Float32Array` utility function using a simple nth-sample approach (not Ramer–Douglas–Peucker, which is too slow for real-time zoom changes). `maxPoints` is a configurable constant (suggested default: 50 000 points for the combined scene).
- In the viewport's toolpath rendering code (`src/viewport/` or wherever `LineSegments` are built from toolpath data), detect the current camera distance from the model bounding sphere. Derive a LOD level (full / half / quarter / eighth) and use the appropriate decimated buffer.
- LOD switches smoothly when the user zooms in/out — no visual popping guard needed at this stage; just switch thresholds.
- The camera's `change` event (from OrbitControls) triggers a LOD recalculation.

**Test requirements:**
- Unit tests for `decimateToolpath` (empty input, single point, exact divisor, non-exact divisor, preserves first and last point).
- No backend changes needed.

---

## Items Explicitly Out of Scope

These Phase 3 roadmap items are deferred to a future bite:

- **Tessellation LOD** (`cg_face_tessellate` at multiple chord tolerances) — requires OCCT-side work and is lower priority than the above.
- **5-axis tool orientation indicators** (instanced cylinder meshes along the path) — infrastructure for Phase 4; not yet useful.
- **Planar face detection** (`cg_shape_find_planar_faces`) — useful but not blocking any current workflow.
- **Simulation Phase 1** (material database with Kt/Kr/Ka, machine model, dexel tracker, force model) — a full bite on its own.

---

## Completion Criteria

This scope is done when:

1. A user can select a workpiece material in any operation editor and see feed/speed fields auto-populate with reasonable values.
2. A user can activate distance measurement in the viewport, click two points on the model, and see a labeled distance in mm.
3. A user can activate angle measurement, click three points, and see the angle in degrees.
4. Zooming out on a toolpath with >50 000 display points does not cause frame rate drops (decimated buffer is used automatically).
5. All new Rust code passes `cargo test` and `cargo clippy --deny warnings`.
6. All new frontend code passes `pnpm test` and `pnpm typecheck`.
7. No existing golden tests are broken.
