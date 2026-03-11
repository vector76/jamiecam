# JamieCam — Work Scope: Phase 2 Kickoff

_Estimated human effort: 8–10 hours_
_Prerequisite: Phase 1 complete (see `docs/implementation-status.md`)_

---

## Context

Phase 0 (Foundation) and Phase 1 (2D Operations MVP) are complete. The full
2D toolpath pipeline — pocket, profile, drill, post-processor, G-code export,
toolpath visualization, geometry selection, and cache — is implemented and
tested. See `docs/implementation-status.md` for the detailed baseline.

This scope begins Phase 2 (2.5D Operations) from `docs/development-roadmap.md`.
It targets a coherent deliverable: viewport UX improvements that benefit all
future phases, plus the OCCT Z-section primitive and the Z-level roughing
algorithm that is the centrepiece of Phase 2.

---

## Deliverables

### 1. Viewport Standard Views (≈ 2 hours)

Keyboard shortcuts to snap the camera to standard orthographic-ish views:

| Key | View |
|-----|------|
| `T` | Top (looking down −Z) |
| `F` | Front (looking along +Y) |
| `R` | Right (looking along −X) |
| `I` | Isometric (standard 3-axis diagonal) |

Each transition **animates smoothly** using `@tweenjs/tween.js` (already a
project dependency) over ~300 ms. Camera `position`, `up`, and `target` are
all tweened. `OrbitControls` target is reset to scene origin on each snap.

The keyboard handler should be attached in `Viewport.tsx` (or a thin hook)
and must not fire when the user is typing in a form input.

Tests: one test per view confirming the SceneManager method is called with the
correct target position.

### 2. Viewport Perspective / Ortho Toggle (≈ 0.5 hours)

A toggle button (or keyboard shortcut `P`) switches between
`THREE.PerspectiveCamera` and `THREE.OrthographicCamera`. Both cameras share
the same orbit controls target and approximate the same view frustum.

`SceneManager` already owns the camera. Add `toggleProjection()` and
`getProjectionMode()` methods. The toolbar should show the current mode.

Tests: toggle cycles projection mode; scene manager exposes correct mode string.

### 3. Viewport Display Mode Selector (≈ 1.5 hours)

A dropdown or button group in the toolbar selects one of four display modes
for the model mesh:

| Mode | Behaviour |
|------|-----------|
| Shaded | Current default — phong/lambert shading, no edges |
| Shaded + Edges | Shaded fill + `THREE.EdgesGeometry` / `LineSegments` overlay |
| Wireframe | `material.wireframe = true`; no fill |
| Transparent | `material.transparent = true`, `opacity ≈ 0.3` |

`SceneManager` adds `setDisplayMode(mode: DisplayMode)` that mutates the model
mesh material(s) and edge overlay visibility. The edge overlay is built lazily
on first request and reused.

`viewportStore` stores `displayMode: DisplayMode` (default `'shaded'`).

Tests: each mode sets the correct material properties; edge overlay created
once and reused on second request.

### 4. `cg_shape_section_at_z` C++ Wrapper (≈ 2 hours)

Add the OCCT Z-section primitive described in `docs/geometry-kernel.md` and
`docs/development-roadmap.md` Phase 2.

**C API** (in `cam_geometry.h` / `cam_geometry.cpp`):

```c
// Section the shape with the plane Z = z_height.
// Returns a handle to a PolyHandle containing the resulting edge loop(s)
// as a flat array of (x, y) pairs in the XY plane.
// Returns CG_ERR_NO_RESULT on error.
CgPolyHandle cg_shape_section_at_z(CgShapeHandle shape, double z_height);
```

Implementation uses `BRepAlgoAPI_Section` with a `gp_Pln` at the given Z,
then walks the result edges to produce ordered 2D loops. The implementation
lives alongside the existing Clipper2 and OCCT wrappers in `cam_geometry.cpp`.

**Rust wrapper** in `src-tauri/src/geometry/`:

```rust
pub fn shape_section_at_z(shape: &OcctShape, z: f64) -> Result<Vec<Vec<(f64, f64)>>, GeometryError>
```

Returns multiple loops (outer boundary + any inner holes) as separate `Vec`s.
Dual-compiled: real FFI behind `#[cfg(cam_geometry_bindings)]`; stub returning
`GeometryError::NotImplemented` otherwise.

Tests:
- C++ doctest: section a simple box at mid-height → rectangular loop
- Rust unit test (gated): section a box shape → single loop, 4 points
- Rust unit test (stub): returns error as expected

### 5. Z-Level Roughing Algorithm (≈ 3 hours)

Add the `ZLevelRoughing` operation type end-to-end: data model, algorithm,
IPC, and UI.

#### Data model

New `OperationParams::ZLevelRoughing(ZLevelRoughingParams)` variant:

```rust
pub struct ZLevelRoughingParams {
    pub geometry: Option<Vec<String>>,  // face fingerprints (same pattern as Pocket/Profile)
    pub depth: f64,                     // total Z depth (positive value, plunges downward)
    pub stepdown: f64,                  // depth per Z level
    pub stepover: f64,                  // radial stepover as fraction of tool diameter (0–1)
}
```

Serialised with `#[serde(rename_all = "camelCase")]`. Follows the same
`add_operation` / `edit_operation` / IPC pattern as existing operations (see
`docs/implementation-status.md` — Operations section).

#### Algorithm (`src-tauri/src/toolpath/operations/zlevel_roughing.rs`)

```
fn zlevel_roughing_passes(stock, params, tool_diameter, shape) -> Result<Vec<Pass>>
```

For each Z level from `0` down to `-depth` (step = `stepdown`):
1. Call `shape_section_at_z(shape, z_level)` to get the boundary contour(s)
   at that Z. If the shape handle is `None` (no model loaded), use the stock
   bounding rectangle as the boundary (same fallback as Pocket).
2. Apply inward tool-radius offset via `poly_offset` (same as Pocket).
3. Generate concentric offset contours inward by `stepover * tool_diameter`
   until the polygon collapses — identical to the pocket clearing loop.
4. Collect passes for this Z level.

The resulting passes are linked by `link_passes` (existing function).

The algorithm is intentionally structurally similar to `pocket_passes`; reuse
the offset loop. The key difference is that the boundary comes from OCCT section
geometry at each Z rather than the static stock boundary.

Golden file test (`src-tauri/tests/zlevel_roughing_golden.rs`, gated on
`cam_geometry_bindings`) — exercises a simple box part at three Z levels.

#### Operation editor UI

Add `+ Z-Level Roughing` button to `OperationListPanel` (alongside existing
`+ Profile`, `+ Pocket`, `+ Drill`).

`OperationEditorForm` gains a `ZLevelRoughing` branch:
- Tool selector
- Depth / Stepdown / Stepover inputs (same style as Pocket)
- Geometry section (Select Faces / Done / Clear — same as Pocket)
- Feed/speed override inputs

Calculate button enabled when stock is defined (same gate as Pocket).

Tests: form renders; depth/stepdown/stepover inputs save on blur; geometry
section works; Calculate button gate; add button disabled when no tools.

---

## Out of Scope (deferred to later Phase 2 work)

The following Phase 2 items from `docs/development-roadmap.md` are **not**
included in this scope:

- Adaptive (trochoidal) clearing
- 3D contour / Z-level finishing
- Arc lead-in / lead-out, helical entry, ramp entry
- Arc fitting (G2/G3 chord detection)
- Hole auto-detection (`cg_shape_find_holes`)
- Drill sorting
- Canned cycle emission (G81/G83/G73/G84/G85)
- Rest machining
- Simulation track (Phase 2 parallel track)

---

## Acceptance Criteria

1. Pressing T/F/R/I in the viewport animates the camera to the corresponding
   standard view; pressing again snaps back (or has no effect if already there).
2. The projection toggle switches between perspective and orthographic; the model
   remains centred and approximately the same size.
3. Switching display mode to Shaded+Edges shows edge lines; Transparent makes
   the model semi-transparent; Wireframe shows only edges.
4. `cg_shape_section_at_z` doctest passes on Linux.
5. A Z-Level Roughing operation can be added, configured, and calculated on a
   loaded STEP model; the toolpath appears in the viewport as horizontal slice
   contours.
6. Full test suite passes (`cargo test`, `pnpm test`).

---

## Reference Documents

- `docs/development-roadmap.md` — Phase 2 plan and acceptance criteria
- `docs/implementation-status.md` — Phase 1 baseline
- `docs/geometry-kernel.md` — C wrapper conventions, handle registry
- `docs/viewport-design.md` — Three.js viewport architecture
- `docs/system-architecture.md` — IPC command patterns, `_inner` wrapper style
