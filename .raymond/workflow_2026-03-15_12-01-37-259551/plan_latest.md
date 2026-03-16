# Implementation Plan: Phase 3 Completion — Remaining Infrastructure

**Derived from:** `fullspec_latest.md`

---

## Overview

This plan covers the three remaining Phase 3 infrastructure items:

1. **Material/Feed Library** — a TOML-backed data-driven lookup table that auto-populates feed/speed fields when a user selects a workpiece material on any operation.
2. **Viewport: Measurement Overlays** — interactive distance and angle measurement tool using `CSS2DRenderer` HTML labels anchored to world-space 3D points.
3. **Viewport: Toolpath LOD** — automatic decimation of toolpath display geometry at low zoom levels to maintain frame rate on large toolpaths.

The features are independent of each other and can be tackled in any order, but the recommended sequence is top-to-bottom: the Rust backend features first (Material/Feed Library), then the two pure-frontend viewport features. Within each feature, the Rust side (if any) should be completed before the frontend side to avoid blocked work.

---

## Phase 1: Material/Feed Library — Rust Backend

### Goal

Define and load a bundled TOML feed library into `AppState` at startup, and expose two IPC commands for the frontend to query it.

### Dependencies

None. This is the first phase.

### What must be accomplished

**TOML schema and bundled data files**

Define a TOML schema for the feed library. The key dimensions are:
- Workpiece material (e.g. `aluminum-6061`, `steel-mild`, `wood-mdf`, `plastic-abs`)
- Tool material (e.g. `hss`, `carbide`)
- Operation category (e.g. `roughing`, `finishing`, `drilling`)

Each entry provides `spindle_speed_rpm` and `feed_rate_mmpm`. Including `doc_mm` (depth of cut) as an optional field is desirable. The TOML can be structured as one file per workpiece material or as a single flat file with compound keys — choose whichever makes lookup code simplest while keeping the data files readable.

Create bundled data files covering at minimum: `aluminum-6061`, `steel-mild`, `wood-mdf`, and `plastic-abs` as workpiece materials; `hss` and `carbide` as tool materials; `roughing`, `finishing`, and `drilling` as operation categories.

Embed the files in the binary at compile time, following the same approach the postprocessor module uses for its bundled TOML configs.

**Rust data model**

Define structs: a `MaterialMeta` carrying the material's machine-readable ID and a human-readable display name; a `FeedEntry` carrying `spindle_speed_rpm`, `feed_rate_mmpm`, and optionally `doc_mm`; a `FeedLibrary` holding the full parsed data in a structure that allows efficient lookup by the three key dimensions.

All types must serialize to camelCase JSON across the IPC bridge, consistent with every other IPC return type in the codebase. The lookup key is a triple (workpiece material ID, tool material, operation category).

**Loader**

Write a loader that parses the embedded TOML at startup. The loader is called during `AppState` initialization. On parse failure, the application should log the error and surface it as a startup error rather than silently continuing with an empty library. Use structured error types consistent with the rest of the codebase — no panics at the parse boundary.

**AppState extension**

Extend `AppState` to hold the loaded feed library. Since the library is read-only after startup, it does not need lock protection — it can be stored directly on `AppState`, not behind a `RwLock`.

**IPC commands**

Implement two commands following the established inner-function wrapper pattern used by all other IPC commands:

- `list_materials() → Vec<MaterialMeta>` — returns all known workpiece materials with their display names.
- `lookup_feeds(material_id, tool_material, operation_category) → FeedEntry` — performs the three-key lookup and returns the matching entry. Returns a not-found error consistent with how other IPC commands handle missing resources if no entry exists for the requested combination.

Register both commands in `main.rs` alongside the existing command inventory.

**Unit tests**

Write Rust unit tests covering:
- The TOML loader: correctly parses a valid feed library.
- Lookup returns correct values for a known material/tool/operation combination.
- Lookup with an unrecognized material ID returns an error.
- Lookup with a valid material ID but an unrecognized tool material or operation category exercises the "default fallback" path. The implementer must decide whether this returns a generic default entry or a structured error — either is acceptable, but the behavior must be defined before writing the test, and the test must verify whichever behavior is chosen.

These tests belong in the `commands/` or a dedicated `feed_library/` module alongside the implementation, not in a separate test file that must be maintained separately.

### Delivers for Phase 2

A working Rust backend with two registered IPC commands that the frontend can call.

---

## Phase 2: Material/Feed Library — Frontend Integration

### Goal

Add a workpiece material selector to every operation editor form, wired to the `lookup_feeds` IPC command to auto-populate feed and spindle speed fields on selection. The populated fields remain user-editable.

### Dependencies

Phase 1 must be complete: `list_materials` and `lookup_feeds` must be registered and callable.

### What must be accomplished

**TypeScript types**

Add `MaterialMeta` and `FeedEntry` TypeScript interfaces to `src/api/types.ts`, matching the camelCase fields serialized by the Rust structs.

Add an optional workpiece material ID field to the operation input/params type in `types.ts`. This field is stored in the project and defaults to absent/null. It is included in the operation input payload when adding or updating an operation.

**API wrappers**

Add wrapper functions for `list_materials` and `lookup_feeds` to the appropriate file in `src/api/`, following the same typed IPC invoke pattern used by all other API wrappers.

**Project persistence**

The `workpieceMaterial` field must flow through the full round-trip: it is included in the operation form state, sent to Rust on save (via `update_operation`), stored in `AppState`, and restored from `.jcam` on load. The Rust `OperationParams` variants must be updated to carry this optional field. Because this is a new optional field with a `null` default, existing saved projects remain compatible without migration.

**Operation editor UI**

Add a material selector dropdown to the common section of the operation editor form — above the existing feed rate and spindle speed override fields, where it will be visible for every operation type.

On mount, the selector calls the `list_materials` API wrapper to populate its options. When the user picks a material, the component calls the `lookup_feeds` API wrapper and pre-fills the spindle speed and feed rate fields with the returned values. The user can then edit those fields freely — the material selection is advisory, not locked.

The tool material argument to the lookup comes from the tool currently selected on the operation. The operation category is derived from the operation type. Before implementing the UI, define a complete mapping from every operation type in the codebase to one of the three library categories (`roughing`, `finishing`, `drilling`) — for example, pocket and adaptive operations map to `roughing`, surface finishing passes map to `finishing`, and drill operations map to `drilling`. Profile/contour and Z-level operations should be similarly mapped. This mapping must cover all existing operation types; any gaps would cause the lookup to silently fail to auto-populate.

**Edge case — no tool selected**: if the operation has no tool assigned at the time a material is selected, the auto-fill cannot proceed because the tool material is unknown. The material selector should still allow the user to choose and save a material (the preference is stored), but the feed/speed pre-fill should be deferred until a tool is assigned. When a tool is subsequently assigned to the operation, the component should re-attempt the lookup if a workpiece material is already selected.

**Edge case — tool changes**: if the user changes the tool on an operation that already has a workpiece material selected, the pre-fill should re-run using the new tool's material. The lookup result depends on the tool material, so a tool change is equivalent to a fresh selection trigger. If the new tool material produces a not-found result, apply the same non-blocking notice behavior as any other not-found case.

If the lookup command returns a not-found error (e.g. the selected tool material has no entry for that operation category), display a non-blocking notice rather than an error — the fields simply don't auto-populate.

**Frontend tests**

Write component tests for the material selector covering:
- Selecting a material triggers pre-fill of spindle speed and feed rate fields with values from the lookup response.
- After pre-fill, those fields remain editable — a subsequent user edit is accepted and takes precedence over the pre-filled value (this is what the fullspec means by "existing override values respected": the override mechanism works correctly after pre-fill, not just before it).
- A "not found" response from the lookup command shows the non-blocking notice and does not modify the existing field values.
- When no tool is assigned, selecting a material saves the material preference but does not attempt a lookup or modify the feed/speed fields. When a tool is subsequently assigned while a material is already selected, the lookup is triggered automatically.
- When the tool changes on an operation that already has a material selected, the pre-fill re-runs using the new tool's material; if the new combination produces a not-found result, the notice is shown and existing field values are left unchanged.

---

## Phase 3: Viewport Measurement Overlays — Math and State

### Goal

Lay the pure-logic and state foundation for the measurement overlay: measurement math utilities and a Zustand measurement state slice. This work has no visual output on its own but is required before the viewport integration.

### Dependencies

Phase 1 and 2 (Material/Feed Library) are independent and can be in progress simultaneously. This phase has no dependency on them.

### What must be accomplished

**Measurement math utilities**

Write a pure-function utility module (no Three.js dependency) with:
- A distance function: given two 3D points, return the Euclidean distance in mm.
- An angle function: given three 3D points where the middle point is the vertex, return the interior angle in degrees at that vertex.

Keep the utilities framework-free so they can be tested without a DOM or Three.js environment.

**Zustand measurement state**

Add measurement state to the appropriate Zustand store (either extend `viewportStore` or create a focused `measurementStore`). The state must hold:
- `measurementMode: 'off' | 'distance' | 'angle'`
- `measurementPoints: [number, number, number][]` — world-space 3D points accumulated by clicking in the viewport
- `measurements: Measurement[]` — completed measurements, each carrying the points used, the computed value, a display label string, and the world-space anchor point for the label

Actions needed: set mode, add a point (and trigger completion logic when enough points are present), clear all measurements, remove a single measurement.

Completion logic: after 2 points in distance mode, compute and store a distance measurement with its midpoint as label anchor; after 3 points in angle mode, compute and store an angle measurement with the vertex as label anchor; then reset `measurementPoints` to empty so the user can start the next measurement.

**Unit tests**

Write tests for the measurement math utilities covering:
- Distance between two identical points (zero).
- Distance between known points with expected values.
- Angle at a right-angle vertex (90°).
- Angle at a straight line (180°).
- Angle at an acute vertex with known expected value.

---

## Phase 4: Viewport Measurement Overlays — Three.js and UI

### Goal

Integrate the measurement state and math into the Three.js viewport using `CSS2DRenderer`, and add the measurement toolbar button group.

### Dependencies

Phase 3 must be complete (state slice and math utilities exist). The existing `SceneManager` in `src/viewport/scene.ts` is the integration point.

### What must be accomplished

**CSS2DRenderer setup**

Add a `CSS2DRenderer` to `SceneManager` alongside the existing `WebGLRenderer`. The CSS2DRenderer shares the same camera. It must be sized to match the canvas and updated in the render loop after the WebGL render call.

Position the CSS2DRenderer's DOM element over the canvas using the same positioning approach used for the existing overlay elements. The overlay element itself must not intercept pointer events destined for the WebGL canvas beneath it — only individual label elements should be interactive. Ensure the element is included in the DOM cleanup on scene teardown.

**Scene graph: measurement overlay group**

Add a measurement group to the `OverlayGroup` in the scene (consistent with the scene graph documented in `viewport-design.md`, which already reserves `MeasurementLabels` here). This group holds `CSS2DObject` instances — one per completed measurement.

Each `CSS2DObject` wraps a styled HTML element: white text, dark semi-transparent background, rounded corners. Distance labels show the value in mm; angle labels show the value in degrees. Choose a precision appropriate to each measurement type.

The measurement group is rebuilt from the Zustand `measurements` array whenever that array changes. Subscribing to store changes follows the same pattern used elsewhere in the viewport (direct store subscription in `useEffect` or equivalent scene lifecycle hook).

**Raycasting for measurement points**

Measurement clicking must be handled separately from the existing face-selection raycasting in `Viewport.tsx`. When `measurementMode` is not `'off'`, a pointer-down event on the canvas raycasts against the loaded model mesh and stock mesh and adds the intersection point (in world space) to `measurementPoints` via the store action.

The existing face-selection behavior must remain unchanged when `measurementMode` is `'off'`. Mutual exclusivity: measurement mode and face-selection mode should not be active simultaneously. Activating measurement mode should implicitly deactivate selection mode and vice versa.

**Point markers**

Render small markers at each accumulated click point so the user can see where they've placed measurement anchors. These markers live in the 3D scene (not as CSS2D labels) within the measurement group, so they are cleared along with measurements. Use a visually distinct color to differentiate them from model geometry.

**Toolbar button group**

Add a measurement section to the viewport toolbar with:
- A "ruler" (distance) button: activates distance measurement mode
- A "protractor" (angle) button: activates angle measurement mode
- A "Clear All" button: clears all measurements and returns to `'off'` mode

Buttons reflect active state from the Zustand store. The toolbar lives in the existing toolbar component above the canvas.

**Keyboard shortcut**

Pressing `Escape` while in measurement mode should cancel any in-progress (incomplete) measurement, clear `measurementPoints`, and return mode to `'off'`. Completed measurements remain visible until explicitly cleared.

**Component tests**

Test the toolbar state changes: clicking each button updates the store mode; clicking Clear All resets state.

---

## Phase 5: Viewport Toolpath LOD

### Goal

Automatically decimate the toolpath display geometry at low zoom levels so large toolpaths don't cause frame rate drops. This is entirely a frontend change; no backend involvement.

### Dependencies

No dependency on Phases 1–4. Can be implemented in parallel with any of the above. The only prerequisite is the existing toolpath rendering in `src/viewport/toolpathLines.ts` and `SceneManager`.

### What must be accomplished

**Decimation utility**

Write a pure-function decimation utility (the fullspec names it `decimateToolpath`) using an nth-sample approach: if the input has fewer or equal points than the maximum, return it unchanged; otherwise, compute a uniform step size and sample every nth point, always preserving the first and last point. The result must remain structurally valid line-segment geometry — the output must preserve the segment-pair alignment required by the toolpath renderer, not just reduce point count arbitrarily.

The utility must be pure and side-effect-free so it can be unit-tested without DOM, Three.js, or any async context.

**LOD constants**

Define LOD thresholds as named constants (not magic numbers) in the same module or in a shared viewport constants file:
- Maximum display points for the combined scene (suggested: 50,000)
- Distance thresholds for each LOD level: full resolution, half, quarter, eighth — expressed as multiples of the model's bounding sphere radius so thresholds scale with model size rather than being fixed in mm

**Integration with toolpath rendering**

In the toolpath rendering code (the module that calls `buildToolpathLines` / creates `LineSegments` from `LineGeometryData`), maintain a reference to the current raw `LineGeometryData` received from the backend.

There are three events that must trigger an LOD recalculation:

1. **Camera zoom change**: the `change` event from `OrbitControls` fires whenever the camera moves, including zoom. On each such event, measure the camera's distance from the scene's bounding sphere center, compare against the LOD thresholds to select the appropriate level (full / half / quarter / eighth), decimate all three parallel buffers together at that level, and update the GPU geometry in place rather than rebuilding the `LineSegments` object. No polling or per-frame recalculation is needed beyond this event.

2. **Toolpath data change**: when new toolpath geometry arrives from the backend (e.g., after the user recalculates an operation), the raw `LineGeometryData` reference changes. The LOD system must apply decimation immediately at the current LOD level rather than displaying the raw full-resolution data until the next camera movement.

3. **Model change**: when a new model is loaded, the bounding sphere changes and the LOD thresholds must be recomputed. The LOD system must subscribe to model load events (or recalculate on the next camera change after a model load) so that thresholds don't reference a stale bounding sphere from a previously loaded model.

**Buffer synchronization**

The `LineGeometryData` has three parallel arrays: `positions` (Float32Array), `colors` (Float32Array), and `types` (Uint8Array). When decimating, all three arrays must be decimated together with the same step size so per-segment data stays aligned. Implement a helper that decimates all three together.

**Hysteresis is not required** at this stage per the spec — LOD switches immediately at the threshold. This simplifies the implementation at the cost of potential LOD "flickering" at the boundary, which can be addressed in a future iteration.

**Unit tests**

Write tests for `decimateToolpath` covering:
- Empty input returns empty output.
- Input with fewer points than `maxPoints` is returned unchanged.
- Input with exactly `maxPoints` points is returned unchanged.
- Input with more points: result length is ≤ `maxPoints`, first point is preserved, last point is preserved.
- Non-exact divisor: result still satisfies the first/last preservation and ≤ maxPoints constraints.
- Input length of 1: returns unchanged.

---

## Cross-Cutting Concerns

### TDD policy compliance

Per project policy, tests are written alongside or before implementation. Each phase above has explicit test requirements. Before considering any phase complete, all described tests must exist and pass.

Run only tests relevant to the feature being developed during development. Run the full suite before committing.

### Clippy and typecheck

All new Rust code must pass `cargo clippy --deny warnings`. All new TypeScript code must pass `pnpm typecheck`. These gates are enforced by CI.

### No golden test regressions

None of the three features alter toolpath computation or G-code generation logic. Existing golden tests must remain green throughout. If any golden test breaks, it indicates an unintended side effect and should be investigated rather than updated.

### Project persistence: material field

The workpiece material field added to operation params in Phase 2 must be correctly serialized and deserialized in the `.jcam` format. Since it is an optional field that defaults to absent, existing projects without this field will deserialize it as null without requiring a schema migration version bump — confirm that the Rust deserialization path applies the appropriate default rather than failing on missing fields.

### Documentation updates

After all phases are complete, update `docs/implementation-status.md` and `docs/system-architecture.md` to reflect:
- The `FeedLibrary` field in `AppState`
- The two new IPC commands (`list_materials`, `lookup_feeds`)
- The `CSS2DRenderer` added to the viewport
- The measurement overlay scene group
- The toolpath LOD utility and integration

---

## Completion Checklist

Before considering this scope done:

- [ ] A user can select a workpiece material in any operation editor and see feed/speed fields auto-populate with reasonable values
- [ ] A user can activate distance measurement, click two points on the model, and see a labeled distance in mm
- [ ] A user can activate angle measurement, click three points, and see the angle in degrees
- [ ] Zooming out on a toolpath with >50,000 display points does not cause visible frame rate drops
- [ ] All new Rust code passes `cargo test` and `cargo clippy --deny warnings`
- [ ] All new TypeScript code passes `pnpm test` and `pnpm typecheck`
- [ ] No existing golden tests are broken
