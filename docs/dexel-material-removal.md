# Dexel Material Removal Engine

> **Post-pivot note:** This spec predates the Tauri→web pivot. The
> `#[tauri::command]` IPC example is obsolete; the engine now lives at
> `src-rust/src/dexel/` and is exposed via wasm + a Web Worker. The
> algorithmic specification (dexel grid, swept-volume update, meshing)
> remains accurate and ships today.

## Purpose

When a cutting tool moves along a path, it removes material from the workpiece.
The dexel material removal engine tracks the evolving shape of the workpiece by
maintaining a volumetric representation that is updated as each motion is
applied. It answers the question: **"what does the workpiece look like after
this sequence of cuts?"**

This enables:

- **Cut simulation visualization** — render the workpiece mesh at any point
  during the cutting process, showing material that has been removed and
  material that remains.
- **Material removal rate** — compute how much material each motion segment
  removes, which feeds into physics simulation (cutting forces, thermal load).
- **Rest machining** — determine what material remains after one tool/operation,
  so a subsequent operation can target only the leftover stock.
- **Collision detection** — detect whether the non-cutting portion of a tool
  (shank, holder) contacts remaining workpiece material.

The engine is pure computation — no UI, no OCCT dependency. It takes stock
geometry and a sequence of tool motions as input and produces an updated
volumetric model and optional triangle mesh as output.

---

## The Dexel Model

A **dexel** (depth element) model represents a 3D solid as a regular 2D grid
in XY, where each grid cell stores material occupancy along the Z axis. This
is a natural fit for CNC machining where the primary tool axis is Z and
material is removed from above.

```
    Y ↑
      │  ┌──┬──┬──┬──┬──┐
      │  │  │  │  │  │  │  Each cell stores Z-interval(s)
      │  ├──┼──┼──┼──┼──┤  describing where material exists
      │  │  │  │  │  │  │  along the vertical column.
      │  ├──┼──┼──┼──┼──┤
      │  │  │  │  │  │  │
      │  └──┴──┴──┴──┴──┘
      └──────────────────→ X

    For one cell:
    Z ↑
      │     ┌─┐     ← material present: [z_bottom, z_top]
      │     │█│
      │     │█│
      │     └─┘
      └──────→
```

### Grid Structure

```
DexelGrid {
    origin: (f64, f64)       // XY position of grid corner (0,0) cell
    resolution: f64          // cell size in mm (same in X and Y)
    nx: usize                // number of cells in X
    ny: usize                // number of cells in Y
    columns: Vec<DexelColumn> // nx × ny columns, row-major order
}
```

**`DexelColumn`** stores the material state of one vertical column:

```
DexelColumn {
    spans: Vec<ZSpan>   // sorted, non-overlapping vertical intervals
}

ZSpan {
    z_min: f64    // bottom of material
    z_max: f64    // top of material
}
```

A column with one span `[0.0, 50.0]` means solid material from Z=0 to Z=50.
A column with two spans `[0.0, 10.0], [30.0, 50.0]` means material from 0–10
and 30–50, with a void (through-slot or pocket) from 10–30.

### Initialization from Stock

Given a `StockDefinition::Box(dims)` where `dims` has origin, width, depth, height:

```
origin = (dims.origin.x, dims.origin.y)
nx = ceil(dims.width / resolution)
ny = ceil(dims.depth / resolution)
```

Each column starts with a single span: `[dims.origin.z, dims.origin.z + dims.height]`.

Cells outside the stock boundary but inside the grid bounding box are initialized
empty (no spans). This handles stocks whose dimensions aren't exact multiples of
the resolution.

---

## Tool Profile as Radial Function

To compute material removal, the engine needs to know the tool's cutting shape.
For 3-axis machining (tool always vertical), the tool's geometry can be described
as a **radial clearance function**:

```
z_clearance(r: f64) -> Option<f64>
```

Given a horizontal distance `r` from the tool axis, this returns how far **above
the tool tip** the cutting envelope sits at that radius. Returns `None` if `r`
is beyond the tool's cutting radius.

The engine receives this function as a trait object or closure — it does not
need to know about tool types or the `Tool` struct. This keeps the engine
decoupled from the tool model.

**Examples** (R = tool radius):

| Tool Shape | z_clearance(r) | Domain |
|------------|---------------|--------|
| Flat endmill | `0.0` | `r ≤ R` |
| Ball nose | `R - sqrt(R² - r²)` | `r ≤ R` |
| Bull nose (corner radius `cr`) | `0.0` for `r ≤ R-cr`; `cr - sqrt(cr² - (r-R+cr)²)` for `r > R-cr` | `r ≤ R` |
| V-bit (half angle `α`) | `r / tan(α)` | `r ≤ R` |
| Drill (half angle `α`) | `r / tan(α)` | `r ≤ R` |

The engine also needs the tool's **cutting radius** (maximum `r` for which
`z_clearance` returns `Some`) to bound the affected grid area.

---

## Material Removal Algorithm

### Per-Segment Operation

For each motion segment, the engine:

1. Computes the **XY bounding box** of the tool's swept region along the segment
   (segment XY extent expanded by tool radius on all sides).
2. Iterates over all grid cells within that bounding box.
3. For each cell, computes the **minimum Z the tool reaches** at that cell's
   XY position across the entire motion.
4. Subtracts material above that Z from the cell's span list.

### Linear Segments (Rapid and Feed moves)

A linear motion from point **A** to point **B**. The tool axis moves along the
line segment AB in 3D space. For a grid cell at position **(cx, cy)**:

1. **Project** to XY: find the parameter `t ∈ [0, 1]` of the closest point on
   the XY projection of AB to (cx, cy).
2. **Horizontal distance**: `d = ||(cx, cy) - lerp(A.xy, B.xy, t)||`
3. If `d > tool_radius`: this cell is outside the tool's reach — skip.
4. **Tool tip Z at closest approach**: `tip_z = lerp(A.z, B.z, t)`
5. **Cut floor Z**: `floor_z = tip_z + z_clearance(d)`
6. **Remove material**: for each span in the cell, truncate or split any
   portion above `floor_z`.

**Important nuance on non-horizontal moves**: the algorithm above uses the
closest XY point's `t` to compute `tip_z`. This is exact for horizontal moves
(constant Z) but incorrect for sloped or vertical ones. In the worst case — a
pure plunge (A.xy == B.xy) — the XY projection is a point, `t` is arbitrary,
and the algorithm may use the Z at the *top* of the plunge instead of the
bottom, missing the cut entirely.

The correct general approach: for each cell, find the range of `t` values
where the cell is within tool reach (`d(t) ≤ tool_radius`), then minimize
`lerp(A.z, B.z, t) + z_clearance(d(t))` over that range. Since `lerp` is
linear and `d(t)` is piecewise-smooth, this is a bounded 1D optimization.
For flat endmills (`z_clearance = 0`) it simplifies to taking the minimum
tip Z at the endpoints of the reachable `t` range. The implementer should
handle at least the vertical/steep case correctly; the shallow-slope
approximation (closest XY point) is acceptable as an optimization for
segments with small Z change relative to XY displacement.

### Arc Segments

Arc motions (G2/G3) in 3D. The recommended approach is to **discretize** the arc
into short linear sub-segments and apply the linear algorithm to each. A chord
tolerance of `resolution / 2` ensures the discretization error is smaller than
the grid resolution.

### Rapid Moves

Rapid moves (G0) also remove material if the tool passes through stock. The
engine treats them identically to feed moves for material removal purposes.
In practice, rapids are usually above the stock, but the engine should not
assume this.

### Span Operations

The core data operation is **subtracting a half-space** from a span list:
"remove all material above Z = floor_z."

Given `spans = [(z0, z1), (z2, z3), ...]` and a cut at `floor_z`:

- Spans entirely below `floor_z`: unchanged.
- Spans entirely above `floor_z`: removed.
- Spans that straddle `floor_z`: truncated to `(z_min, floor_z)`.

This is a simple linear scan of the sorted span list. For the initial
implementation (material removed only from above by a vertical tool), this
is the only span operation needed.

For future multi-axis work where tools approach from non-vertical angles,
the span operations become more complex (removing material from the middle
of a span, creating voids). The span list representation supports this
inherently — it just requires a more general "subtract interval" operation.

---

## Key Operations

### Core API

```
DexelGrid::new(stock: &StockDefinition, resolution: f64) -> DexelGrid
```
Initialize from stock geometry.

```
DexelGrid::apply_segment(
    &mut self,
    segment: &MotionSegment,
    tool_radius: f64,
    z_clearance: &dyn Fn(f64) -> Option<f64>,
)
```
Remove material for one motion segment.

```
DexelGrid::apply_segments(
    &mut self,
    segments: &[MotionSegment],
    tool_radius: f64,
    z_clearance: &dyn Fn(f64) -> Option<f64>,
)
```
Remove material for a sequence of segments (convenience wrapper).

### Query Operations

```
DexelGrid::volume(&self) -> f64
```
Total remaining material volume: `sum over cells of (sum of span heights) × resolution²`.

```
DexelGrid::height_at(&self, x: f64, y: f64) -> Option<f64>
```
Top surface Z at an XY point (interpolated or nearest-cell). Returns `None`
if outside the grid or if the cell is empty.

```
DexelGrid::max_height(&self) -> f64
```
Maximum Z across all cells (useful for visualization bounds).

```
DexelGrid::removed_volume_since(&self, previous: &DexelGrid) -> f64
```
Material removed between two states. Useful for computing per-operation or
per-segment material removal.

### Snapshot and Clone

```
DexelGrid::snapshot(&self) -> DexelGrid
```
Deep copy of the current state. Needed for before/after comparisons and for
"undo" in interactive simulation scrubbing.

---

## Mesh Extraction

For visualization, the dexel grid must be convertible to a triangle mesh that
can be rendered in Three.js (via the same `MeshData` pipeline used for model
geometry).

### Approach

The most straightforward mesh extraction:

1. **Top surface**: For each non-empty cell, emit a quad (2 triangles) at the
   cell's top Z height. The quad covers the cell's XY extent.
2. **Vertical walls**: Where adjacent cells have different top Z heights (or
   one is empty and the other isn't), emit vertical quad(s) connecting them.
3. **Bottom surface**: For most machining visualizations, the bottom of the
   stock is not visible. Optionally emit bottom quads for cells at the stock
   floor.

This produces a **staircase mesh** — axis-aligned steps whose visual quality
depends on grid resolution. At 0.1mm resolution, the steps are imperceptible
at normal viewport zoom levels.

### Output Format

The mesh should be returned as vertex positions + normals + triangle indices,
matching the structure expected by the frontend's `MeshData` type
(`src/api/types.ts`):

```typescript
interface MeshData {
  vertices: number[]    // [x,y,z, x,y,z, ...]
  normals: number[]     // [nx,ny,nz, ...]
  indices: number[]     // [i0,i1,i2, ...]
  faceGroups: Array<{ startTriangle: number; triangleCount: number }>
}
```

For dexel meshes, `faceGroups` can be a single group covering all triangles
(the dexel mesh has no meaningful face decomposition like a CAD model does).

### Mesh Optimization (optional)

- **Greedy meshing**: Merge adjacent cells with the same top Z into larger
  quads. Dramatically reduces triangle count for flat regions (common after
  pocket clearing). This is a well-known algorithm from voxel rendering
  (Minecraft-style).
- **Normal smoothing**: Not recommended for machining visualization — the
  sharp staircase edges accurately represent the discretized surface.

---

## Performance Considerations

### Grid Size

For a 100mm × 100mm stock at 0.1mm resolution: 1000 × 1000 = 1M cells.
Each cell is a `Vec<ZSpan>` — for unmachined stock, each has exactly one
span (16 bytes). Total: ~16 MB baseline + Vec overhead.

At 0.05mm: 4M cells, ~64 MB. At 0.5mm: 40K cells, <1 MB.

### Parallelism

The per-cell computation within `apply_segment` is independent across cells.
This is naturally parallelizable with Rayon's `par_iter_mut`. The bounding box
culling (step 1) is serial but trivial; the per-cell work (steps 2–4) is the
hot loop and can be split across cores.

### Incremental Updates

The engine updates in-place. Consumers that need animation (showing the
workpiece at frame N of M) have two options:

1. **Snapshot-per-step**: Apply segments one at a time, snapshotting after each.
   Memory-intensive but allows random access to any frame.
2. **Replay from start**: Store only the initial grid + segment list. To show
   frame N, apply the first N segments. Fast for sequential playback; slow for
   random access. Can be mitigated by periodic keyframe snapshots.

The implementer should start with approach 2 (replay) and add keyframe
snapshots if scrubbing performance requires it.

---

## Integration Points

### Input: Motion Segments

The engine consumes motion segments — a sequence of positioned tool motions.
These come from two sources:

1. **Internal toolpaths**: The existing `Toolpath` type (`toolpath/types.rs`)
   contains `Pass` → `CutPoint` sequences. These can be converted to motion
   segments by pairing consecutive CutPoints. The conversion is straightforward
   but is the consumer's responsibility, not the engine's.

2. **Parsed G-code**: A G-code parser (if one exists) produces motion segments
   directly. The engine consumes these as-is.

The engine defines its own `MotionSegment` type (or accepts a trait) so it
doesn't depend on either source's types.

### Input: Tool Profile

The engine receives the tool's radial clearance function and cutting radius.
It does not depend on the `Tool` struct — any callable that maps `r → Option<f64>`
works. This allows the engine to be tested with synthetic tool profiles
(e.g., `|r| if r <= 5.0 { Some(0.0) } else { None }` for a 10mm flat endmill).

### Input: Stock

The engine takes `StockDefinition` (from `models/stock.rs`) for initialization.
Only the `Box` variant is relevant initially.

### Output: To Frontend

An IPC command exposes the engine's mesh to the frontend:

```rust
#[tauri::command]
async fn get_simulation_mesh(
    resolution: f64,
    operation_ids: Option<Vec<Uuid>>,
    up_to_segment: Option<usize>,
) -> Result<MeshData, AppError>
```

- **resolution**: Grid cell size in mm.
- **operation_ids**: Which operations to simulate, in order. `None` means all
  enabled operations in project order. Each operation's tool profile is resolved
  from its `tool_id`.
- **up_to_segment**: Stop after this many total motion segments (across all
  requested operations). `None` means apply all.

The frontend renders this mesh alongside (or instead of) the source model mesh.
The existing `MeshData` → `BufferGeometry` pipeline in `modelMesh.ts` can be
reused directly.

### Existing State

The dexel grid is computed on-demand inside the IPC command — it is not
persisted in `AppState`. The `snapshot()` and `removed_volume_since()` methods
exist on `DexelGrid` for future interactive scrubbing but are not exposed via
IPC initially. The dexel engine does not replace the
existing `rest.rs` module (which uses 2D Clipper boolean ops per Z-layer for
rest machining during toolpath generation) — the dexel engine is a separate,
finer-grained model for visualization and physics simulation.

---

## Test Strategy

### Unit Tests: Span Operations

- Remove above Z from a single span → truncated span.
- Remove above Z below the span → no change.
- Remove above Z above the span → span removed entirely.
- Remove above Z in the middle of multiple spans → correct spans remain.
- Remove above Z at exactly a span boundary → exact truncation.

### Unit Tests: Grid Initialization

- Box stock → correct grid dimensions, all cells have one span.
- Volume of initialized grid matches stock volume (within resolution error).

### Unit Tests: Material Removal — Flat Endmill

- **Straight slot**: 10mm flat endmill moves linearly across a block from
  X=0 to X=100 at Z=-5. Verify: cells under the tool path have top Z reduced
  to stock_top - 5. Cells outside the 10mm corridor are unchanged.
- **Pocket floor**: Tool at fixed Z rasters back and forth. Verify uniform
  floor Z in the pocket area.
- **Plunge cut**: Vertical move from Z=50 down to Z=10. Verify a cylindrical
  hole in the dexel grid.

### Unit Tests: Material Removal — Ball Nose

- Straight slot with ball nose → curved floor profile. Sample several cells
  at known radial distances from the tool path centerline, verify Z matches
  the analytical hemisphere equation.

### Unit Tests: Volume Accounting

- After removing a known rectangular pocket (width W, depth D, height H),
  verify `removed_volume ≈ W × D × H` within resolution-dependent tolerance.

### Integration Tests

- Apply all segments from a golden `.nc` fixture (if a G-code parser is
  available) or from an internal toolpath. Verify the resulting volume is
  less than the initial stock volume by a plausible amount.
- Mesh extraction produces valid triangle data (no degenerate triangles,
  normals point outward).

### Resolution Convergence

- Run the same toolpath at 0.5mm, 0.1mm, and 0.05mm resolution. Verify the
  computed removed volume converges (tighter resolution → closer to analytical
  value).
