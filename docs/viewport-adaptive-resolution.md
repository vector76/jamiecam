# Viewport-Adaptive Resolution for Cut Simulation

## Purpose

The dexel material removal engine (see `dexel-material-removal.md`) processes
the entire workpiece at a single fixed resolution. This creates a tension:
fine resolution (0.05mm) gives excellent visual quality but is expensive for
large workpieces, while coarse resolution (0.5mm) is fast but produces visible
staircase artifacts when the user zooms in.

This feature resolves that tension by **decoupling the simulation resolution
from the viewport resolution**. The full workpiece is simulated at a coarse
base resolution (fast, low memory). When the user zooms into a region, only
that region is re-simulated at higher resolution, using only the segments
that are relevant to that region.

The key insight: during the base simulation pass, the engine already knows
which motion segments touch which areas of the workpiece (this is the bounding
box culling step). By recording those associations, the engine builds a
**spatial index** as a side effect of the simulation. This index answers the
question "which segments affected this region?" without re-scanning the full
segment list.

---

## Concepts

### Base Grid

The full-workpiece dexel grid, processed at coarse resolution (e.g., 0.5mm).
This is the standard `DexelGrid` from the dexel material removal engine. It
provides the overview visualization and serves as the starting point for any
view of the workpiece.

### Tile

The base grid is logically divided into fixed-size **tiles** — rectangular
groups of cells (e.g., 32×32 cells per tile). Tiles are the unit of spatial
indexing. Each tile stores the list of segment indices that touched any cell
within that tile during the base simulation pass.

```
┌──────────┬──────────┬──────────┐
│ Tile 0,0 │ Tile 1,0 │ Tile 2,0 │
│ segs: [0,│ segs: [0,│ segs: [5,│
│  1, 2, 3]│  1, 4, 5]│  6, 7]   │   ← each tile records which
├──────────┼──────────┼──────────┤      motion segments touched it
│ Tile 0,1 │ Tile 1,1 │ Tile 2,1 │
│ segs: [2,│ segs: [4,│ segs: [7,│
│  8, 9]   │  5, 8]   │  10, 11] │
└──────────┴──────────┴──────────┘
```

The tile-segment association is an **over-approximation**: a segment is recorded
for a tile if the segment's tool bounding box overlapped the tile, even if
the tool only clipped its corner. This is acceptable — the per-cell bounding
box culling in the dexel engine filters out false positives cheaply during
the high-resolution replay.

### Viewport Region

The rectangular area of the workpiece visible in the user's current viewport,
expressed as an axis-aligned bounding box (AABB) in workpiece XY coordinates.
This is derived from the camera frustum intersected with the workpiece XY
plane.

### Detail Grid

A temporary, high-resolution dexel grid covering only the viewport region.
It is created on demand when the user zooms in beyond the base resolution's
visual quality threshold, simulated using only the segments relevant to that
region, and discarded when the viewport moves away.

---

## Workflow

### 1. Base Simulation

The full segment list is processed against the base grid at coarse resolution,
exactly as described in `dexel-material-removal.md`. The only addition: during
`apply_segment`, the engine also records the segment's index in every tile
whose bounding box overlaps the segment's tool bounding box.

This produces:
- The base dexel grid (coarse full-workpiece representation)
- The tile-segment index (which segments touched which tiles)

### 2. Overview Rendering

When the user views the full workpiece (or any view where the base resolution
is visually adequate), the base grid's mesh is rendered directly. No detail
grid is needed.

The threshold for "visually adequate" is when a single dexel cell maps to
roughly one pixel or smaller on screen. Above that zoom level, the staircase
artifacts become visible and the detail grid should be used.

### 3. Detail Rendering on Zoom

When the user zooms into a region that exceeds the base resolution's visual
quality:

1. **Determine the viewport AABB** in workpiece XY coordinates.
2. **Find overlapping tiles** — which tiles intersect the viewport AABB.
3. **Collect relevant segments** — the union of segment indices from those
   tiles. Deduplicate (a segment may appear in multiple tiles).
4. **Initialize a detail grid** covering the viewport AABB at the desired
   high resolution (e.g., 0.05mm). Initialize it from the stock definition
   (same as the base grid, but smaller extent and finer resolution).
5. **Replay relevant segments** against the detail grid. The standard
   `apply_segment` is used; its bounding box culling naturally skips segments
   that were recorded in the tile index but don't actually reach the detail
   grid's extent.
6. **Extract mesh** from the detail grid and render it for the viewport region.

The detail grid covers a small area (whatever fits on screen at high zoom),
so both the cell count and the relevant segment count are small. This makes
the replay fast even at very high resolution.

### 4. Pan and Re-Zoom

When the viewport moves (pan or zoom change):
- If the new viewport is still within the detail grid's extent (the user panned
  slightly), the existing detail grid may be reusable or extendable.
- If the viewport has moved significantly or the zoom level changed, discard
  the detail grid and repeat step 3.

Debouncing or throttling the re-render (e.g., 50–200ms after the viewport
stops moving) prevents wasted computation during continuous pan/zoom.

---

## Tile-Segment Index

### Structure

```
TileIndex {
    tile_size: usize                    // cells per tile edge (e.g., 32)
    tiles_x: usize                     // tile count in X
    tiles_y: usize                     // tile count in Y
    segments_per_tile: Vec<Vec<u32>>   // tile index → segment indices
}
```

The tile grid dimensions are derived from the base dexel grid dimensions and
the tile size: `tiles_x = ceil(nx / tile_size)`, `tiles_y = ceil(ny / tile_size)`.

### Population

During `apply_segment` on the base grid, the segment's XY bounding box (tool
path extent ± tool radius) is computed. The tiles overlapping that bounding
box are identified and the segment index is appended to each tile's list.

This is cheap: a typical segment touches 1–4 tiles (a 10mm tool at 0.5mm
resolution with 32-cell tiles means each tile covers 16mm × 16mm, and the
tool sweeps a ~10mm-wide corridor). The cost is a few Vec pushes per segment.

### Storage

The total storage is the sum of all tile segment lists. For a 50K-segment
toolpath where each segment touches ~2 tiles on average: 100K entries × 4
bytes = 400 KB. Negligible relative to the base grid itself. The actual tile
count depends on part size (a 100mm × 100mm part at 0.5mm base resolution
with 32-cell tiles has ~50 tiles; a 300mm × 300mm part has ~400 tiles), but
the storage is dominated by the entries-per-segment factor, not the tile count.

### Querying

Given a viewport AABB, find all tiles whose spatial extent intersects it
(simple grid math), then collect and deduplicate their segment indices. The
deduplication can use a bitset (one bit per segment) for O(1) membership
testing, or a sorted merge.

---

## Compositing

When the detail grid is active, the viewport shows two layers:

- **Base mesh** — the coarse full-workpiece mesh, always present.
- **Detail mesh** — the high-resolution mesh for the zoomed region, overlaid
  on (or replacing) the corresponding portion of the base mesh.

The simplest compositing strategy: render only the detail mesh for the
viewport region, and the base mesh for everything else. The detail grid's
spatial extent defines the boundary. Since the detail grid is initialized
from the same stock and processes (a superset of) the same segments, the
two meshes agree at their boundary to within the base resolution's
discretization error.

An alternative: render only the detail mesh when zoomed in (the base mesh
is off-screen or nearly so at high zoom levels). This avoids boundary
artifacts entirely.

---

## Interaction with Simulation Playback

The existing simulation playback (step through segments over time) composes
naturally:

- During playback, the base grid is updated incrementally (one segment at a
  time). The tile-segment index grows as segments are applied.
- If the user is zoomed in during playback, the detail grid is also updated
  incrementally — but only for segments that fall within the viewport tiles.
  Segments outside the viewport update the base grid's tile index but don't
  trigger detail grid work.
- Scrubbing backward requires replaying from the start (or from a keyframe
  snapshot). The tile-segment index for the target frame is a prefix of the
  full index (segments 0..N), so the detail grid replay uses only that prefix.

---

## Resolution Selection

The detail grid resolution should be chosen to give approximately one cell
per screen pixel in the viewport. Given the viewport's world-space width `W`
and pixel width `P`:

```
detail_resolution = W / P
```

For a 20mm-wide view in a 1000-pixel viewport: `0.02mm`. For a 100mm view
in the same viewport: `0.1mm`.

A minimum resolution floor (e.g., 0.01mm) prevents pathological cases at
extreme zoom. A maximum (the base resolution) prevents wasted work when the
overview is sufficient.

---

## What This Feature Does NOT Include

- **Automatic resolution selection** — the system computes a recommended
  resolution from viewport parameters, but the user can override it.
- **Persistent high-resolution cache** — detail grids are transient. They are
  recomputed on zoom, not saved to the project file. The base grid (optionally)
  persists.
- **Multi-axis extensions** — this feature assumes Z-axis dexel columns
  (3-axis machining). Multi-axis viewport-adaptive resolution would require
  viewport-aligned slicing, which is a different approach.
- **UI design** — this feature describes the computational mechanism. The
  viewport integration (when to trigger detail rendering, how to composite
  meshes, zoom threshold UX) is left to the frontend implementer.

---

## Dependencies

This feature builds on the dexel material removal engine
(`dexel-material-removal.md`). It requires:

- `DexelGrid` with `apply_segment` — the same simulation operation, used for
  both the base pass and the detail replay.
- `DexelGrid::new` with arbitrary origin, extent, and resolution — needed to
  create the detail grid for a viewport sub-region.
- Motion segments with stable indices — the segment list must be indexable by
  `u32` position, and the order must be deterministic.

It does not depend on the G-code parser or tool geometry model directly,
though in practice the motion segments and tool profiles come from those
features.

---

## Test Strategy

### Tile Index Tests

- Process N segments at base resolution. Verify each tile's segment list
  contains exactly the segments whose tool bounding box overlapped that tile.
- A segment entirely within one tile appears in only that tile's list.
- A segment spanning two tiles appears in both lists.
- A segment outside all tiles (entirely off the stock) appears in no lists.

### Detail Grid Correctness

- Process the full segment list at base resolution (coarse) and independently
  at high resolution (fine, full workpiece). Then create a detail grid for a
  sub-region, replay the tile-selected segments, and compare cell-by-cell
  against the corresponding region of the full high-resolution grid. They
  must match exactly — the detail grid sees all and only the segments that
  affect that region.

### Segment Subset Sufficiency

- The segments selected by tile lookup for a given viewport must be a
  **superset** of the segments that actually modify cells within that viewport
  at any resolution. Verify by processing the full segment list against a
  high-res grid covering the viewport, then comparing the modified cells
  against the detail grid result. They must be identical.

### Performance Scaling

- Verify that the number of segments selected for a viewport region scales
  with the region size, not the total segment count. A viewport covering 10%
  of the workpiece should select roughly 10% of the segments (for uniformly
  distributed toolpaths).

### Boundary Consistency

- The detail grid and base grid should agree at their spatial boundary to
  within the base resolution's discretization error. Sample heights along the
  boundary and verify they are consistent.
