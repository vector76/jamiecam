# Beads: Phase 3 Completion — Remaining Infrastructure

## Codebase Notes

### Integration Points

**Rust — AppState and state module**
- `src-tauri/src/state.rs`: `AppState` struct (lines 96–101) has two fields: `project: RwLock<Project>` and `preferences: RwLock<UserPreferences>`. `Default` impl at lines 103–110. New `feed_library: FeedLibrary` field can be added directly (no `RwLock` needed; read-only after startup).
- `src-tauri/src/lib.rs`: `AppState::default()` is used to construct managed state (line ~30); `tauri::generate_handler![]` macro at lines 58–86 lists all IPC commands.

**Rust — IPC command pattern**
- All commands follow `fn foo_inner(args, &RwLock<Project>) -> Result<T, AppError>` + `#[tauri::command] async fn foo(args, state: tauri::State<'_, AppState>) -> Result<T, AppError>`.
- Examples: `src-tauri/src/commands/tools.rs` (add_tool_inner line 41, wrapper line 130); `src-tauri/src/commands/operations.rs` (add_operation_inner line 58, wrapper line ~116).
- Commands module: `src-tauri/src/commands/mod.rs` — new submodule `pub mod feeds;` must be added here; also contains shared helpers `parse_entity_id`, `write_project`, `read_project`.

**Rust — TOML embedding pattern**
- `src-tauri/src/postprocessor/mod.rs` lines 26–29: uses `include_str!("builtins/fanuc-0i.toml")` etc. Feed library files should follow the same approach: `pub const FEEDS_TOML: &str = include_str!("data/feeds.toml");` in the feed_library module (must be `pub` so `state.rs` can reference it as `crate::feed_library::FEEDS_TOML`).

**Rust — Error types**
- `src-tauri/src/error.rs` lines 17–56: `AppError` enum with `#[serde(tag = "kind", content = "message")]`. Use `AppError::NotFound(String)` for missing feed entries; `AppError::ProjectLoad(String)` for TOML parse failures.

**Rust — Operation model**
- `src-tauri/src/models/operation.rs`: `Operation` struct (lines 341–364) has `spindle_speed_override: Option<u32>` and `feed_rate_override: Option<f64>` at the top level (common to all variants). `OperationParams` enum (lines 316–333) has 10 variants: Profile, Pocket, Drill, ZLevelRoughing, ZLevelFinishing, AdaptiveClearing, ParallelFinishing, ScallopFinishing, FlowlineFinishing, PencilMilling. New `workpiece_material: Option<String>` field goes on `Operation` (same level as `spindle_speed_override`), not inside each `OperationParams` variant.
- `#[serde(default, skip_serializing_if = "Option::is_none")]` is the pattern for optional fields (consistent with `spindle_speed_override`).
- `OperationInput` is defined in `src-tauri/src/commands/operations.rs` alongside the inner functions. It needs the same `workpiece_material: Option<String>` field.

**Frontend — API layer**
- `src/api/errors.ts`: `typedInvoke<T>(cmd, args?)` is the canonical IPC wrapper; `toAppError(e)` converts raw errors.
- `src/api/types.ts`: Add `MaterialMeta`, `FeedEntry` interfaces; extend `Operation` and `OperationInput` with optional `workpieceMaterial?: string`. `LineGeometryData` (lines 417–424): `positions: number[]`, `colours: number[]` (British spelling), `types: number[]`.
- New API file: `src/api/feeds.ts` for `listMaterials()` and `lookupFeeds()`.

**Frontend — Zustand stores**
- `src/store/viewportStore.ts` (117 lines): holds `meshData`, `toolpathGeometry`, `selectionMode`, simulation state, etc. Measurement state should extend this store.
- `src/store/projectStore.ts`: `snapshot: ProjectSnapshot | null`, notification queue, `selectedOperationId`.

**Frontend — Operation editor**
- `src/components/operations/OperationEditorForm.tsx` (755 lines): renders per-operation-type subforms. Common fields (tool selector, spindle/feed overrides) appear directly in the file; sub-type editors delegated to `ParallelFinishingEditor`, `ScallopFinishingEditor`, `FlowlineFinishingEditor`, `PencilMillingEditor`. Material selector goes above spindle/feed override fields.
- The form uses **uncontrolled inputs** (`defaultValue`, not `value`) for number fields including spindle/feed override. All mutations go through the `save(patch: Partial<OperationInput>)` helper at lines 88–107, which calls `editOperation`, refreshes the snapshot, and calls `listOperations()` to re-fetch the operation state. There are no individual `useState` setters for spindle/feed — pre-filling is done by calling `save({spindleSpeedOverride: ..., feedRateOverride: ...})` which triggers a re-render with new `defaultValue`.

**Frontend — Viewport**
- `src/viewport/scene.ts`: `SceneManager` class; private `renderer: THREE.WebGLRenderer`, `toolpathGroup: THREE.Group`. `setToolpathLines()` at line 377. Render loop at line 127 calls `this.renderer.render(this.scene, this._activeCamera())`. CSS2DRenderer must be added alongside `renderer` and called in `_animate()`. Private `_modelMesh: THREE.Mesh | null` at line ~29 — needs a public getter for raycasting.
- `src/viewport/toolpathLines.ts` (37 lines): `buildToolpathLines(data)` creates `THREE.LineSegments` from `data.positions` and `data.colours`. In `Viewport.tsx` lines 152–154, `buildToolpathLines(toolpathGeometry)` is called and the result is passed to `mgr.setToolpathLines(lines)`. LOD integration replaces this call.
- `src/viewport/Viewport.tsx`: the viewport display mode and projection toggle buttons are **inline in `Viewport.tsx` around line 301** (not a separate toolbar component). Measurement toolbar buttons go in the same section. Existing raycasting for face selection is in `Viewport.tsx`. Test file: `src/viewport/Viewport.test.tsx` (exists).
- `docs/viewport-design.md` line 89–90: scene graph reserves `OverlayGroup > MeasurementLabels` slot for `CSS2DObject` instances.

**Test conventions**
- `src/components/operations/OperationEditorForm.test.tsx`: uses `@testing-library/react`, `vi.mock()` for API modules, sets store state via `useProjectStore.setState(...)`, uses UUID fixtures.
- `src/viewport/scene.test.ts`: mocks `requestAnimationFrame`, `ResizeObserver`, `three/WebGLRenderer`; uses `priv<T>()` helper for private fields.

### Patterns to Follow
- All IPC return types use `#[serde(rename_all = "camelCase")]`.
- Optional Rust fields: `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- New Rust modules go in their own directory (`src-tauri/src/feed_library/mod.rs`) with `pub mod feed_library;` in `lib.rs` and a `pub mod feeds;` entry in `commands/mod.rs`.
- Errors surface as `AppError::NotFound(msg)` for missing resources.
- `cargo clippy --deny warnings` and `pnpm typecheck` must pass.

---

## Beads

### bead-1 (bd-0mvo): Feed Library TOML data files and Rust data model

**Work:** Create the `feed_library` Rust module at `src-tauri/src/feed_library/mod.rs`. Define the following structs with the derives shown:
- `MaterialMeta { id: String, display_name: String }` — `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]` with `#[serde(rename_all = "camelCase")]`. (Clone needed for `.to_vec()` in IPC layer; Serialize needed for IPC return.)
- `FeedEntry { spindle_speed_rpm: u32, feed_rate_mmpm: f64, doc_mm: Option<f64> }` — same four derives and `#[serde(rename_all = "camelCase")]`. (Clone needed for `.cloned()` in IPC layer; Serialize needed for IPC return.)
- `FeedLibrary` — `#[derive(Debug)]` only. No Clone, no Serialize, no Deserialize: it is constructed programmatically from parsed TOML (not deserialized directly) and is never sent over IPC. It holds a `HashMap<(String, String, String), FeedEntry>` (key = workpiece_material_id, tool_material, operation_category) plus a `Vec<MaterialMeta>` for listing. Provide `FeedLibrary::lookup(&self, material_id, tool_material, op_category) -> Result<&FeedEntry, AppError>` returning `AppError::NotFound` for any unrecognized key combination. Provide `FeedLibrary::materials(&self) -> &[MaterialMeta]`.

Create the TOML data file at `src-tauri/src/feed_library/data/feeds.toml`. Use an array-of-tables layout where each entry has fields `material_id`, `tool_material`, `operation_category`, `spindle_speed_rpm`, `feed_rate_mmpm`, `doc_mm` (optional). Populate entries covering all combinations of: materials `aluminum-6061`, `steel-mild`, `wood-mdf`, `plastic-abs`; tool materials `hss`, `carbide`; operation categories `roughing`, `finishing`, `drilling` (24 entries minimum). Use reasonable real-world values (e.g., aluminum + carbide + roughing ≈ 10000 RPM, 1500 mm/min).

Embed the file: `pub const FEEDS_TOML: &str = include_str!("data/feeds.toml");` — must be `pub` so `state.rs` can reference it as `crate::feed_library::FEEDS_TOML`.

Write a `FeedLibrary::from_toml(toml_str: &str) -> Result<Self, AppError>` that parses the TOML (using the `toml` crate, which is already in `Cargo.toml`) and builds the HashMap. Define an intermediate serde struct for the flat array-of-tables entry to deserialize into before converting to the HashMap structure. On parse failure return `AppError::ProjectLoad(msg)`.

Declare the module in `src-tauri/src/lib.rs` with `pub mod feed_library;`.

Write unit tests inside `mod.rs` (in a `#[cfg(test)] mod tests` block):
- `from_toml` correctly parses the embedded `FEEDS_TOML` and succeeds.
- `lookup` returns correct `spindle_speed_rpm` for `("aluminum-6061", "carbide", "roughing")`.
- `lookup` returns `AppError::NotFound` for an unknown `material_id`.
- `lookup` returns `AppError::NotFound` for a valid material + valid tool material but invalid `operation_category`.
- `lookup` returns `AppError::NotFound` for a valid material + valid operation category but invalid `tool_material`.
- `materials()` returns a slice of length 4 (one per material in the bundled data).

Run `cargo test -p jamiecam_lib` and `cargo clippy --deny warnings` before considering this bead complete.

**Estimate:** ~45 min
**Dependencies:** none

---

### bead-2 (bd-4dpl): AppState extension and Feed Library IPC commands

**Work:** Extend `src-tauri/src/state.rs`: add `pub feed_library: FeedLibrary` field to `AppState` (not behind `RwLock`). Update `Default` impl to initialize it: `feed_library: FeedLibrary::from_toml(crate::feed_library::FEEDS_TOML).expect("bundled feed library must parse")`. Add the import `use crate::feed_library::FeedLibrary;`.

Create `src-tauri/src/commands/feeds.rs`. Add at the top of the file:
```rust
use crate::feed_library::{FeedEntry, FeedLibrary, MaterialMeta};
use crate::error::AppError;
use crate::state::AppState;
```

Implement:
- `fn list_materials_inner(feed_library: &FeedLibrary) -> Vec<MaterialMeta>` — returns `feed_library.materials().to_vec()` (Clone is available per bead-1).
- `#[tauri::command] pub async fn list_materials(state: tauri::State<'_, AppState>) -> Result<Vec<MaterialMeta>, AppError>` — body: `Ok(list_materials_inner(&state.feed_library))`.
- `fn lookup_feeds_inner(material_id: &str, tool_material: &str, operation_category: &str, feed_library: &FeedLibrary) -> Result<FeedEntry, AppError>` — delegates to `feed_library.lookup(material_id, tool_material, operation_category).cloned()`.
- `#[tauri::command] pub async fn lookup_feeds(material_id: String, tool_material: String, operation_category: String, state: tauri::State<'_, AppState>) -> Result<FeedEntry, AppError>` — body: `lookup_feeds_inner(&material_id, &tool_material, &operation_category, &state.feed_library)`.

Add `pub mod feeds;` to `src-tauri/src/commands/mod.rs`.

Register both commands in `src-tauri/src/lib.rs`'s `tauri::generate_handler![]` alongside existing commands: `commands::feeds::list_materials, commands::feeds::lookup_feeds`.

Write unit tests in `feeds.rs`:
- `list_materials_inner` returns at least 4 materials (use `FeedLibrary::from_toml(crate::feed_library::FEEDS_TOML).unwrap()` to construct the library in tests).
- `lookup_feeds_inner` returns the correct entry for a known triple.
- `lookup_feeds_inner` returns `NotFound` error for an unknown material.

Run `cargo test -p jamiecam_lib` and `cargo clippy --deny warnings` before considering this bead complete.

**Estimate:** ~40 min
**Dependencies:** bead-1

---

### bead-3 (bd-pnz2): TypeScript types, API wrappers, and Rust Operation model field

**Work:** This bead makes the `workpieceMaterial` field exist throughout the full stack. It has two independent halves (TypeScript and Rust) that can be done in either order.

**TypeScript half** — in `src/api/types.ts`, add after the existing interfaces:
```typescript
export interface MaterialMeta {
  id: string
  displayName: string
}

export interface FeedEntry {
  spindleSpeedRpm: number
  feedRateMmpm: number
  docMm?: number
}
```
Also add `workpieceMaterial?: string` to both the `Operation` interface and the `OperationInput` interface (the optional field represents the stored material ID preference).

Create `src/api/feeds.ts`:
```typescript
import { typedInvoke } from './errors'
import type { MaterialMeta, FeedEntry } from './types'

export async function listMaterials(): Promise<MaterialMeta[]> {
  return typedInvoke<MaterialMeta[]>('list_materials')
}

export async function lookupFeeds(
  materialId: string,
  toolMaterial: string,
  operationCategory: string,
): Promise<FeedEntry> {
  return typedInvoke<FeedEntry>('lookup_feeds', { materialId, toolMaterial, operationCategory })
}
```

**Rust half** — in `src-tauri/src/models/operation.rs`, add to the `Operation` struct alongside `spindle_speed_override` and `feed_rate_override`:
```rust
/// Optional workpiece material ID for feed/speed auto-population.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub workpiece_material: Option<String>,
```
Add the identical field to `OperationInput` in `src-tauri/src/commands/operations.rs`. In `add_operation_inner`, pass `input.workpiece_material` through when constructing the `Operation`. In `edit_operation_inner`, set `entry.workpiece_material = input.workpiece_material`. No schema migration needed — `#[serde(default)]` deserializes the field as `None` when absent in existing `.jcam` files.

Run `pnpm typecheck` and `cargo test -p jamiecam_lib` (including golden tests) and `cargo clippy --deny warnings` before considering this bead complete.

**Estimate:** ~40 min
**Dependencies:** none

---

### bead-4 (bd-i0ps): Material selector React component

**Work:** Create `src/components/operations/MaterialSelectorField.tsx`. This is a self-contained component that:
- Props: `currentMaterialId: string | null | undefined`, `toolMaterial: string | null | undefined`, `operationCategory: string`, `onMaterialChange: (id: string | null) => void`, `onFeedsFetched: (entry: FeedEntry) => void`, `onFeedsNotFound: () => void`.
- On mount, calls `listMaterials()` from `src/api/feeds.ts` to populate a `<select>` dropdown. First option is `"-- Select material --"` (value `""`).
- When the user selects a material (non-empty value): if `toolMaterial` is non-null, calls `lookupFeeds(materialId, toolMaterial, operationCategory)`. On success calls `onFeedsFetched(entry)`. On `NotFound` error (`err.kind === 'NotFound'`) calls `onFeedsNotFound()`. Calls `onMaterialChange(materialId)` regardless.
- When `toolMaterial` prop changes and `currentMaterialId` is already set, re-runs the lookup automatically with the new tool material.
- If `toolMaterial` is null when material is selected, calls `onMaterialChange(materialId)` but skips lookup (stores preference without pre-filling).
- On not-found: shows a non-blocking inline notice (e.g. a `<span className="material-not-found-notice">`) that disappears when the selection changes.

Create `src/components/operations/MaterialSelectorField.test.tsx` following the mock/fixture pattern in `OperationEditorForm.test.tsx`:
- Mock `src/api/feeds` — `listMaterials` resolves with `[{id:'carbide-test',displayName:'Test'}]`, `lookupFeeds` resolves with `{spindleSpeedRpm:8000,feedRateMmpm:1200}`.
- Test: selecting a material calls `onFeedsFetched` with the lookup result.
- Test: selecting a material when `toolMaterial` is null calls `onMaterialChange` but does NOT call `lookupFeeds`.
- Test: when `lookupFeeds` rejects with `{ kind: 'NotFound', message: '...' }`, `onFeedsNotFound` is called, a notice element appears in the DOM, and `onFeedsFetched` is NOT called.
- Test: changing `toolMaterial` prop from null to a real value while `currentMaterialId` is set triggers a lookup and calls `onFeedsFetched`.
- Test: changing `toolMaterial` prop from one non-null value to a different non-null value while `currentMaterialId` is set re-triggers the lookup; if that lookup returns NotFound, `onFeedsNotFound` is called and `onFeedsFetched` is NOT called.
- Test: selecting a material updates the visible `<select>` value.

**Estimate:** ~55 min
**Dependencies:** bead-3 (types and API wrappers)

---

### bead-5 (bd-16a6): Wire material selector into OperationEditorForm

**Work:** In `src/components/operations/OperationEditorForm.tsx`:

Define an operation-type-to-category mapping near the top of the file:
```typescript
const OPERATION_CATEGORY: Record<string, string> = {
  pocket: 'roughing',
  adaptiveClearing: 'roughing',
  zLevelRoughing: 'roughing',
  profile: 'roughing',
  drill: 'drilling',
  zLevelFinishing: 'finishing',
  parallelFinishing: 'finishing',
  scallopFinishing: 'finishing',
  flowlineFinishing: 'finishing',
  pencilMilling: 'finishing',
}
```

Add state: `const [workpieceMaterial, setWorkpieceMaterial] = useState<string | null>(operation?.workpieceMaterial ?? null)`. Sync it with `operation` in the existing `useEffect` that loads the operation (so switching operations resets the local value).

Determine the current tool's material: the form already reads a tools list from the project snapshot — find the existing `useProjectStore` selector near the top of the component that provides the tools array (it is used by the existing tool selector dropdown). Derive `const currentTool = tools.find(t => t.id === operation?.toolId) ?? null`. Pass `currentTool?.material ?? null` as the `toolMaterial` prop.

The form uses uncontrolled inputs with `defaultValue` and a `save(patch: Partial<OperationInput>)` helper at lines 88–107 that persists changes and refreshes the operation from the backend. All mutations in the callbacks must go through this helper — there are no standalone state setters for spindle/feed override values.

In the form JSX, in the common fields section above the spindle speed override field (spindle/feed overrides are common fields rendered directly in `OperationEditorForm.tsx`, not inside the type-specific sub-editors), mount:
```tsx
<MaterialSelectorField
  currentMaterialId={workpieceMaterial}
  toolMaterial={currentTool?.material ?? null}
  operationCategory={OPERATION_CATEGORY[operation.type] ?? 'roughing'}
  onMaterialChange={(id) => {
    setWorkpieceMaterial(id)
    void save({ workpieceMaterial: id || undefined })
  }}
  onFeedsFetched={(entry) => {
    // Pre-fill override fields by persisting through save(); the uncontrolled
    // inputs will re-mount with the new defaultValue from the refreshed operation.
    void save({
      spindleSpeedOverride: entry.spindleSpeedRpm,
      feedRateOverride: entry.feedRateMmpm,
    })
  }}
  onFeedsNotFound={() => pushNotification('No feed data found for this material/tool/category combination')}
/>
```
(`pushNotification` is a placeholder name — check how other error or warning cases in `OperationEditorForm.tsx` surface user-visible notifications and use the same call pattern.)

Write component tests in `OperationEditorForm.test.tsx` (add new `describe` block), mocking `src/api/feeds`:
- Selecting a material pre-fills the spindle speed and feed rate override fields with values from the lookup response.
- After pre-fill, user can still edit those fields — blur on the spindle field triggers a save with the user's edited value, not the pre-filled one.
- Not-found response triggers a notification (`pushNotification` called, or the notification text appears in the DOM).

**Estimate:** ~50 min
**Dependencies:** bead-4, bead-3

---

### bead-6 (bd-am7i): Measurement math utilities and unit tests

**Work:** Create `src/viewport/measurementMath.ts` as a pure, side-effect-free module with no Three.js dependency:
```typescript
export type Point3 = [number, number, number]

/** Returns the Euclidean distance between two 3D points in mm. */
export function distanceBetweenPoints(a: Point3, b: Point3): number

/** Returns the interior angle in degrees at the vertex (middle point). */
export function angleBetweenThreePoints(a: Point3, vertex: Point3, b: Point3): number
```

Implement `distanceBetweenPoints` as `Math.sqrt((b[0]-a[0])**2 + (b[1]-a[1])**2 + (b[2]-a[2])**2)`. Implement `angleBetweenThreePoints` using dot product: vectors `u = a - vertex`, `v = b - vertex`, angle = `Math.acos(clamp(dot(u,v) / (|u| * |v|), -1, 1)) * (180 / Math.PI)`. The clamp to `[-1, 1]` before `acos` prevents NaN on degenerate inputs (coincident points).

Create `src/viewport/measurementMath.test.ts`:
- Distance between identical points = 0.
- Distance between `[0,0,0]` and `[3,4,0]` = 5.
- Distance between `[1,2,3]` and `[4,6,3]` = 5.
- Angle at right-angle vertex: `a=[1,0,0]`, vertex `[0,0,0]`, `b=[0,1,0]` → 90°.
- Angle at straight line: `a=[-1,0,0]`, vertex `[0,0,0]`, `b=[1,0,0]` → 180°.
- Angle at acute vertex: equilateral triangle points gives 60°.

**Estimate:** ~30 min
**Dependencies:** none

---

### bead-7 (bd-4qk6): Zustand measurement state slice

**Work:** Extend `src/store/viewportStore.ts`. Define the `Measurement` type at the top of the file (before the interface):
```typescript
export interface Measurement {
  points: [number, number, number][]
  value: number          // distance in mm or angle in degrees
  label: string          // e.g. "42.3 mm" or "90.0°"
  anchor: [number, number, number]  // world-space label position
}
```

Add these to the `ViewportState` interface:
```typescript
measurementMode: 'off' | 'distance' | 'angle'
measurementPoints: [number, number, number][]
measurements: Measurement[]
setMeasurementMode: (mode: 'off' | 'distance' | 'angle') => void
addMeasurementPoint: (point: [number, number, number]) => void
clearMeasurements: () => void
removeMeasurement: (index: number) => void
```

Implement `addMeasurementPoint` with completion logic inside the Zustand `set` call:
- In `distance` mode: when adding the 2nd point, compute distance using `distanceBetweenPoints` from `measurementMath.ts`, compute midpoint as anchor, push `{ points: [p1, p2], value: distance, label: \`${distance.toFixed(1)} mm\`, anchor: midpoint }` to `measurements`, reset `measurementPoints` to `[]`.
- In `angle` mode: when adding the 3rd point, compute angle using `angleBetweenThreePoints` (vertex = second point in `measurementPoints`), push `{ points: [p1, vertex, p3], value: angle, label: \`${angle.toFixed(1)}°\`, anchor: vertex }` to `measurements`, reset.
- Otherwise: append the new point to `measurementPoints`.

Implement `setMeasurementMode`: sets mode and resets `measurementPoints` to `[]` (changes mode but keeps completed `measurements`).

Initial state: `measurementMode: 'off'`, `measurementPoints: []`, `measurements: []`.

Write tests in `src/store/viewportStore.test.ts` (create if absent, following Zustand test patterns — call actions directly via `useViewportStore.getState()`):
- `addMeasurementPoint` in distance mode: first point does not trigger completion; second point completes, correct distance stored, `measurementPoints` reset to `[]`.
- Completed measurement label for distance is formatted as e.g. `"5.0 mm"`.
- `addMeasurementPoint` in angle mode: first two points don't trigger; third triggers completion with correct angle value.
- `clearMeasurements` resets both `measurements` and `measurementPoints` to `[]`.
- `removeMeasurement(0)` removes the first entry; remaining entries preserved.
- `setMeasurementMode` resets `measurementPoints` to `[]` but keeps `measurements`.

**Estimate:** ~40 min
**Dependencies:** bead-6

---

### bead-8 (bd-0sl7): CSS2DRenderer and measurement overlay group in SceneManager

**Work:** In `src/viewport/scene.ts`:

1. Import `CSS2DRenderer, CSS2DObject` from `three/examples/jsm/renderers/CSS2DRenderer.js`.

2. Add private fields:
   - `private css2dRenderer: CSS2DRenderer`
   - `private measurementGroup: THREE.Group` — holds CSS2DObject labels for completed measurements
   - `private measurementMarkersGroup: THREE.Group` — child of `measurementGroup`; holds sphere meshes for in-progress click points

3. In the constructor, after creating `this.renderer`:
   - Create `this.css2dRenderer = new CSS2DRenderer()`.
   - Size it: `this.css2dRenderer.setSize(container.clientWidth, container.clientHeight)`.
   - Style its DOM element: `position: absolute; top: 0; left: 0; pointer-events: none`.
   - Append it to the same container as the WebGL canvas.
   - Create `this.measurementGroup = new THREE.Group()` with `name = 'MeasurementGroup'`, add to `this.scene`.
   - Create `this.measurementMarkersGroup = new THREE.Group()` with `name = 'MeasurementMarkersGroup'`, add as child of `this.measurementGroup`.

4. In `_animate()`, after `this.renderer.render(...)`, add `this.css2dRenderer.render(this.scene, this._activeCamera())`.

5. In the existing resize handler, add `this.css2dRenderer.setSize(width, height)`.

6. In `dispose()`, remove `this.css2dRenderer.domElement` from the container.

7. Add public method `updateMeasurementLabels(measurements: Measurement[]): void` (import `Measurement` from `../store/viewportStore`):
   - Remove stale CSS2DObject labels from `this.measurementGroup` without touching `measurementMarkersGroup`: iterate `[...this.measurementGroup.children]` and call `this.measurementGroup.remove(child)` for each child where `child instanceof CSS2DObject`. (The only non-CSS2DObject child is `measurementMarkersGroup`, which is a `THREE.Group` and must be preserved.)
   - For each measurement, creates a `<div>` styled with white text, `rgba(0,0,0,0.6)` background, `4px` padding, `4px` border-radius, `pointer-events: none`, `font-size: 12px`.
   - Sets `div.textContent = measurement.label`.
   - Creates `new CSS2DObject(div)`, sets `.position.set(...measurement.anchor)`, adds to `this.measurementGroup`.

8. Add public method `updateMeasurementPoints(points: [number, number, number][]): void`:
   - Clears all children from `this.measurementMarkersGroup`.
   - For each point, creates a `new THREE.Mesh(new THREE.SphereGeometry(1, 8, 8), new THREE.MeshBasicMaterial({ color: 0x00ffff }))`, sets position, adds to `this.measurementMarkersGroup`.

9. In `src/viewport/Viewport.tsx`, subscribe to `measurements` via `useViewportStore` and in a `useEffect` that depends on `measurements`, call `mgr.updateMeasurementLabels(measurements)`. Subscribe to `measurementPoints` separately and call `mgr.updateMeasurementPoints(measurementPoints)`. (`mgr` is the existing `SceneManager` reference used throughout `Viewport.tsx`.)

Write tests in `src/viewport/scene.test.ts` (new describe block, using the existing `priv<T>()` pattern):
- Calling `updateMeasurementLabels([])` results in `measurementGroup` having only the `measurementMarkersGroup` child (no CSS2DObjects).
- Calling `updateMeasurementLabels` with one measurement adds one CSS2DObject child to `measurementGroup`.
- Calling `updateMeasurementPoints` with two points adds two meshes to `measurementMarkersGroup`.
- Calling `updateMeasurementPoints([])` clears `measurementMarkersGroup`.

**Estimate:** ~50 min
**Dependencies:** bead-7

---

### bead-9 (bd-md0n): Measurement raycasting, point markers, and keyboard shortcut

**Work:** In `src/viewport/Viewport.tsx`:

1. Read `measurementMode`, `addMeasurementPoint`, `setMeasurementMode`, and `setSelectionMode` from `useViewportStore`. (`setSelectionMode` is needed for mutual-exclusivity in step 3. The `measurementPoints` subscription was already added to `Viewport.tsx` in bead-8 step 9 — do not add it again.)

2. Add a pointer-down handler on the canvas element. When `measurementMode !== 'off'`:
   - Compute normalized device coordinates from the pointer event relative to the canvas bounding rect: `x = (clientX - rect.left) / rect.width * 2 - 1`, `y = -(clientY - rect.top) / rect.height * 2 + 1`.
   - Create a `THREE.Raycaster` and call `.setFromCamera({x, y}, mgr.getActiveCamera())`. Add a `public getActiveCamera(): THREE.Camera` getter to `SceneManager` that returns `this._activeCamera()`.
   - Raycast against `[mgr.getModelMesh()].filter(Boolean)`. Add a `public getModelMesh(): THREE.Mesh | null` getter to `SceneManager` that returns `this._modelMesh`.
   - If an intersection is found, call `addMeasurementPoint([hit.point.x, hit.point.y, hit.point.z])` with the first intersection point.
   - Call `e.stopPropagation()` to prevent triggering face selection.

3. Mutual exclusivity: add a `useEffect` that watches `measurementMode`. When it becomes non-`'off'`, call `setSelectionMode(false)`. Add a second `useEffect` that watches `selectionMode`. When it becomes `true`, call `setMeasurementMode('off')` (which also resets `measurementPoints`).

4. Add a `keydown` listener (on `window`) for `Escape`: when pressed, read the current mode via `useViewportStore.getState().measurementMode` (not the React hook value, to avoid stale closure); if it is not `'off'`, call `setMeasurementMode('off')` (which resets `measurementPoints` and mode but keeps completed `measurements`). Register and clean up in a `useEffect` with an empty dependency array — reading store state inside the handler via `getState()` means no dependencies are needed.

Write tests in `src/viewport/Viewport.test.tsx`:
- Pressing Escape while `measurementMode === 'distance'` sets mode to `'off'`.
- Setting `measurementMode` to `'distance'` while `selectionMode` is true sets `selectionMode` to false.
- Setting `selectionMode` to `true` while `measurementMode` is `'distance'` sets `measurementMode` to `'off'`.

**Estimate:** ~45 min
**Dependencies:** bead-8

---

### bead-10 (bd-b62i): Measurement toolbar buttons and tests

**Work:** The viewport display mode and projection toggle buttons are inline in `src/viewport/Viewport.tsx` around line 301 (not a separate toolbar component). Add a measurement section there, after the existing control buttons, with three new buttons:
- "Ruler" button (`title="Distance measurement"`): on click calls `setMeasurementMode('distance')`. Add an active CSS class (e.g. `className={measurementMode === 'distance' ? 'active' : ''}`) matching the pattern of existing active-state buttons.
- "Protractor" button (`title="Angle measurement"`): on click calls `setMeasurementMode('angle')`. Active when `measurementMode === 'angle'`.
- "Clear" button (`title="Clear measurements"`): on click calls `clearMeasurements()` then `setMeasurementMode('off')`.

Read `measurementMode`, `setMeasurementMode`, and `clearMeasurements` from `useViewportStore` at the top of the component alongside the existing store reads.

Write tests in `src/viewport/Viewport.test.tsx` (add describe block):
- Clicking the Ruler button sets `measurementMode` to `'distance'`.
- Clicking the Protractor button sets `measurementMode` to `'angle'`.
- Clicking Clear calls `clearMeasurements` and sets mode to `'off'`.
- The Ruler button has an active CSS class when `measurementMode === 'distance'`.

**Estimate:** ~35 min
**Dependencies:** bead-7

---

### bead-11 (bd-53lh): Toolpath LOD decimation utility and unit tests

**Work:** Create `src/viewport/decimateToolpath.ts`. Add at the top:
```typescript
import type { LineGeometryData } from '../api/types'
```

Define LOD constants at the top:
```typescript
export const LOD_MAX_DISPLAY_POINTS = 50_000
export const LOD_THRESHOLDS = {
  FULL: 1.5,    // camera distance < 1.5× bounding radius → full res
  HALF: 3.0,    // < 3.0× → half
  QUARTER: 6.0, // < 6.0× → quarter
  // else: eighth
} as const
```

Implement:
```typescript
export function decimateToolpath(
  data: LineGeometryData,
  maxPoints: number,
): LineGeometryData
```
- Let `n = data.positions.length / 3` (number of points). If `n === 0` or `n <= maxPoints`, return `data` unchanged.
- `positions` encodes line segments as consecutive point pairs: segment `k` occupies point indices `2k` and `2k+1`. Decimation must always include both points of each selected segment or neither, or it produces invalid geometry.
- Compute the number of segments: `const nSegs = n / 2`. Compute `segStep = Math.ceil(nSegs / (maxPoints / 2))`. Iterate over segment indices: `for (let s = 0; s < nSegs; s += segStep)` and for each selected segment `s`, copy both point indices `2s` and `2s+1` (6 floats from `positions`, 6 from `colours`, 2 from `types`). Force-include the last segment (indices `n-2` and `n-1`) if it was not already sampled.
- Return a new `LineGeometryData` with the sampled arrays as plain `number[]`.

Note: `LineGeometryData.positions` and `colours` are `number[]` (not Float32Array); `types` is `number[]`.

Create `src/viewport/decimateToolpath.test.ts`:
- Empty input (`positions: [], colours: [], types: []`) returns unchanged.
- Input with exactly `maxPoints` points returns unchanged (identity check — same object reference).
- Input with fewer than `maxPoints` returns unchanged.
- Input with `2 × maxPoints` points: result has ≤ `maxPoints` points, first point preserved, last point preserved.
- Non-exact divisor case: result still satisfies first/last preservation and ≤ `maxPoints`.
- Input with 1 point: returns unchanged.
- After decimation, `positions.length / 3 === colours.length / 3` and equals `types.length` (alignment check).
- Segment-pair preservation: after decimation, `positions.length / 3` is always even (each segment contributes exactly 2 points); verify with an input of `4 × maxPoints` points (8 × maxSegments) that the output point count is even.

**Estimate:** ~40 min
**Dependencies:** none

---

### bead-12 (bd-7or0): Toolpath LOD integration in SceneManager

**Work:** In `src/viewport/scene.ts`:

Add these imports at the top of the file before making any other changes:
```typescript
import { decimateToolpath, LOD_MAX_DISPLAY_POINTS, LOD_THRESHOLDS } from './decimateToolpath'
import { buildToolpathLines } from './toolpathLines'
```
Also add `import type { LineGeometryData } from '../api/types'` if not already imported (needed for the new field types). (`decimateToolpath`/`LOD_*` are needed by both the field initializer and `_applyLOD`; `buildToolpathLines` is needed by `_applyLOD`.)

1. Add private fields:
   - `private _rawToolpathData: LineGeometryData | null = null`
   - `private _modelBoundingRadius: number = 100` (default until a model is loaded)
   - `private _activeLodMaxPoints: number = LOD_MAX_DISPLAY_POINTS` (cached LOD level; compare before rebuilding)

2. Add public method `setToolpathData(data: LineGeometryData | null): void`:
   - Store `this._rawToolpathData = data`.
   - Reset `this._activeLodMaxPoints = -1` to invalidate the LOD cache before calling `_applyLOD()`. Without this, if new toolpath data arrives while the camera is at the same LOD tier, the early-exit guard in `_applyLOD` would silently skip the rebuild.
   - Call `this._applyLOD()`.
   Make the existing `setToolpathLines(lines: THREE.LineSegments | null): void` at line 377 **private** (rename to `private _setToolpathLines`). It will be called only by `_applyLOD` going forward. Its signature and body stay the same.

3. Add private method `_applyLOD(): void`:
   - If `_rawToolpathData` is null, clear `toolpathGroup` children and return.
   - Compute `cameraDistance = this.controls.target.distanceTo(this._activeCamera().position)`.
   - Select `maxPoints`: `ratio = cameraDistance / this._modelBoundingRadius`; if `ratio < LOD_THRESHOLDS.FULL` → `maxPoints = LOD_MAX_DISPLAY_POINTS`; else if `ratio < LOD_THRESHOLDS.HALF` → `maxPoints = LOD_MAX_DISPLAY_POINTS / 2`; else if `ratio < LOD_THRESHOLDS.QUARTER` → `maxPoints = LOD_MAX_DISPLAY_POINTS / 4`; else → `maxPoints = LOD_MAX_DISPLAY_POINTS / 8`.
   - **Early-exit if LOD tier is unchanged**: if `maxPoints === this._activeLodMaxPoints`, return without rebuilding. The OrbitControls `change` event fires on every pixel of camera movement; rebuilding on every fire would defeat the purpose of LOD. Only rebuild when the camera has crossed a threshold boundary.
   - Set `this._activeLodMaxPoints = maxPoints`.
   - Call `decimateToolpath(this._rawToolpathData, maxPoints)` to get decimated data.
   - Call `buildToolpathLines(decimated)` to get `THREE.LineSegments`.
   - Call `this._setToolpathLines(lines)` to update the group (the now-private method at line 377 handles clearing and adding).

4. In the constructor, subscribe to OrbitControls `change` event: `this.controls.addEventListener('change', () => this._applyLOD())`.

5. Update `setModelMesh`: add a branch that handles the bounding radius. If `mesh` is null (model cleared), set `this._modelBoundingRadius = 100` and reset `this._activeLodMaxPoints = -1` (so the next camera `change` event forces a rebuild), then fall through to the existing null-mesh handling (remove mesh from scene, etc. — do not skip it). If `mesh` is non-null, after storing it call `mesh.geometry.computeBoundingSphere()` and set `this._modelBoundingRadius = mesh.geometry.boundingSphere?.radius ?? 100`; reset `this._activeLodMaxPoints = -1` (using `-1` not `LOD_MAX_DISPLAY_POINTS` — if the camera happens to be at the full-resolution tier, `LOD_MAX_DISPLAY_POINTS` would equal the computed `maxPoints` and the early-exit in `_applyLOD` would skip the rebuild), then call `this._applyLOD()` immediately so the toolpath re-decimates for the new model's scale.

6. Update caller: in `src/viewport/Viewport.tsx` lines 152–154, replace the `buildToolpathLines(toolpathGeometry)` + `mgr.setToolpathLines(lines)` pair with `mgr.setToolpathData(toolpathGeometry)`. Remove the `buildToolpathLines` import from `Viewport.tsx` if it is no longer used.

Write tests in `src/viewport/scene.test.ts` (new describe block):
- After `setToolpathData(twoSegmentData)`, `toolpathGroup.children.length === 1`.
- After `setToolpathData(null)`, `toolpathGroup.children.length === 0`.

**Estimate:** ~50 min
**Dependencies:** bead-11

---

### bead-13 (bd-62cw): Documentation updates

**Work:** Update `docs/implementation-status.md` to mark Phase 3 remaining items as complete and add a summary of what was implemented.

Update `docs/system-architecture.md` to reflect:
- `AppState.feed_library: FeedLibrary` (read-only after startup, no RwLock; initialized from embedded `FEEDS_TOML`)
- Two new IPC commands: `list_materials` and `lookup_feeds` in `src-tauri/src/commands/feeds.rs`
- `Operation.workpiece_material: Option<String>` added to operation model (top-level field, not inside params variants; optional with `#[serde(default)]` for backward compatibility)
- `CSS2DRenderer` added to `SceneManager` alongside `WebGLRenderer`; sized to match canvas; rendered after WebGL in `_animate()`
- `measurementGroup` in the scene graph (under `OverlayGroup`; holds CSS2DObject labels and a `measurementMarkersGroup` sub-group for sphere markers)
- `measurementMath.ts`: pure `distanceBetweenPoints` and `angleBetweenThreePoints` utilities
- Measurement state in `viewportStore`: `measurementMode`, `measurementPoints`, `measurements`, and associated actions
- `decimateToolpath.ts`: LOD utility with `LOD_MAX_DISPLAY_POINTS = 50_000` and radius-relative thresholds (`LOD_THRESHOLDS`)
- LOD system in `SceneManager`: `setToolpathData` stores raw data; `_applyLOD` selects max-points based on camera distance / bounding radius; re-runs on OrbitControls `change`, toolpath data change, and model mesh change

**Estimate:** ~20 min
**Dependencies:** bead-1 through bead-12
