# JamieCam Toolpath Engine

## Overview

The toolpath engine transforms an operation definition (strategy, tool, feeds,
geometry source) into an ordered sequence of annotated 3D points -- the toolpath --
that can then be post-processed into G-code. It lives entirely in Rust and runs
on the Rayon thread pool for parallelism.

The engine is structured as a pipeline. Each stage has a clean input/output contract
so stages can be tested in isolation. The pipeline is shared across all modes;
what varies is the **geometry source** feeding into it and which **operations**
are available.

```
Mode-Specific Geometry Source
        |
        v
+-------------------+
| Feature Extraction |  interpret geometry per mode
+--------+----------+
         |
         v
+-------------------+
| Region Computation |  cutting boundaries + rest material
+--------+----------+
         |
         v
+-------------------+
|  Pass Generation  |  ordered cut point sequences
+--------+----------+
         |
         v
+-------------------+
|     Linking       |  rapids, lead-in/out, retracts
+--------+----------+
         |
         v
+-------------------+
|Collision Detection|  gouge check, holder clearance (modes 5-7 primarily)
+--------+----------+
         |
         v
+-------------------+
| Feed/Speed Assign |  feed rate and spindle speed per point
+--------+----------+
         |
         v
   Toolpath (geometric)
         |
         +->  Simulation Engine (see cutting-simulation.md)
              dexel removal, physics prediction, feed scaling
                        |
                        v
              Toolpath (physics-optimized)
```

---

## Core Data Types

These types are shared across all modes. Every operation produces a `Toolpath`
composed of `Pass` and `CutPoint` values.

```rust
pub struct CutPoint {
    pub position: Vector3<f64>,       // tool tip in WCS, Z-up (mm)
    pub orientation: Vector3<f64>,    // tool axis; (0,0,1) for 3-axis
    pub feed_type: FeedType,
    pub feed_rate: f64,               // mm/min; 0.0 = rapid
    pub spindle_speed: Option<f64>,   // RPM; None = unchanged
}

pub enum FeedType {
    Rapid, Cutting, Plunge, Ramp, Helix,
    LeadIn, LeadOut, Dwell(f64),
}

pub struct Pass {
    pub points: Vec<CutPoint>,
    pub kind: PassKind,
    pub z_depth: Option<f64>,
}

pub enum PassKind {
    Roughing, SemiFinishing, Finishing, SpringPass,
    LeadIn, LeadOut, Link,
}

pub struct Toolpath {
    pub operation_id: OperationId,
    pub tool_id: ToolId,
    pub passes: Vec<Pass>,
    pub stats: ToolpathStats,
}

pub struct ToolpathStats {
    pub total_length_mm: f64,
    pub cutting_length_mm: f64,
    pub rapid_length_mm: f64,
    pub estimated_duration: Duration,
    pub point_count: usize,
    pub max_scallop_height_mm: f64,
}
```

---

## Operations by Mode

### Mode 1 -- G-code Viewer

Mode 1 has **no toolpath generation**. It consumes G-code produced externally.
The G-code parser converts the file into `MotionSegment` values that feed
directly into the viewport for visualization and into the dexel engine for
material removal simulation.

---

### Mode 2 -- 2D Operations

Input is 2D vector artwork (SVG or DXF). Operations cut at fixed Z depths.
**Geometry source:** `usvg` (SVG) or `dxf` crate parses artwork into
`Vec<Polyline>` and `Vec<ClosedRegion>`. No OCCT involved.

#### Profile / Contour

Follows a 2D boundary at one or more Z depths. Parameters: cutting side
(left/right/on), total depth, step-down, roughing + finishing offsets, tabs for
through-cuts. Offset algorithm: Clipper2 via Rust FFI.

#### Pocket Clearing

Removes material inside a closed boundary down to a floor depth.

| Strategy | Description |
|---|---|
| Offset (conventional) | Inward-spiraling offset contours |
| Offset (climb) | Outward-spiraling contours |
| Parallel (zig/zig-zag) | Uni- or bi-directional raster |
| Adaptive (trochoidal) | Constant engagement arc for hard materials |

Entry methods: helical ramp, linear ramp, pre-drilled hole, open side.

#### Drilling

Point operations at discrete X/Y locations (auto-detected from circles in
artwork, or manually placed). Cycle types: spot, drill, peck, chip-break,
boring, reaming, tapping. Hole sorting: nearest-neighbor TSP approximation.

#### Key Refactoring Point: Geometry-Source Independence

The existing codebase implements profile, pocket, and drill algorithms through
OCCT face selection. In the mode-centric architecture, mode 2 needs these
**same algorithms** driven by SVG/DXF paths instead.

The core algorithms (Clipper2 offset, boolean + fill, hole sorting) are
**geometry-source-agnostic** -- they operate on 2D polylines regardless of
origin. The refactoring is in the **input pipeline**, not the algorithms:

```
Current:   OCCT face --> extract boundary --> Clipper2 --> toolpath
Mode 2:    SVG/DXF   --> parse paths      --> Clipper2 --> toolpath
                                               ^
                                     Same algorithm from here on
```

The `planner.rs` dispatcher should accept `Vec<Polyline>` rather than an OCCT
face handle. Both OCCT face extraction and SVG/DXF parsing produce this type.

---

### Mode 3 -- 2.5D Operations (V-Carve)

Same 2D vector input as mode 2, but the toolpath is 3D -- a V-bit descends to
a depth that varies with the local width of each shape.

**Standard V-carve:** The tool tip traces the **medial axis** at varying depths.
For V-bit included angle alpha and local half-width w/2:
`depth = (w/2) / tan(alpha/2)`. Initial implementation: progressive inward
offset via Clipper2. Future: exact medial axis via Straight Skeleton.

**Flat-bottom V-carve:** Flat endmill clears the center area first when width
exceeds V-bit reach; V-bit carves the edges to full profile depth.

**Inlay:** Two pieces cut from the same artwork -- female pocket (V-carve) and
male inlay (mirror, offset inward by glue gap).

**Paint-fill:** Same V-carve geometry with a metadata flag for filled rendering.

---

### Mode 4 -- 3D Operations

3-axis surface machining reachable from the top only (Z+ access, no
undercuts). Input is a heightmap, STL mesh, or STEP solid model. Heightmaps
are loaded into a `HeightmapGrid`; STL meshes are parsed into a triangle
mesh for ray-cast Z sampling; STEP models are imported via OCCT (optional).

**Parallel (raster) finishing:** Constant-spacing scan lines, Z sampled from
height field at each point.

**Scallop finishing:** Variable spacing for constant scallop height regardless
of local slope.

**Roughing passes:** Coarse Z-level passes for deep reliefs.

#### The SurfaceModel Trait

Mode 4's parallel and scallop algorithms are the **same math** as mode 7's 3D
surface operations, but on a height field instead of OCCT surfaces. A trait
abstracts over both:

```rust
pub trait SurfaceModel: Send + Sync {
    fn sample_z(&self, x: f64, y: f64) -> f64;
    fn normal_at(&self, x: f64, y: f64) -> Vector3<f64>;
    fn bounds(&self) -> BoundingBox2D;
}

struct HeightmapSurface { grid: HeightmapGrid }  // bilinear interpolation
struct MeshSurface      { mesh: TriangleMesh }   // ray-cast Z sampling
struct OcctFaceSurface  { face: OcctFaceHandle }  // OCCT BRepAdaptor_Surface
```

The `surface.rs` operations module is parameterized over `impl SurfaceModel`.
Mode 4 passes `HeightmapSurface`, `MeshSurface`, or `OcctFaceSurface`
depending on input; mode 7 passes `OcctFaceSurface`. Algorithms are
identical -- only surface evaluation changes. Heightmap evaluation is
extremely fast (array lookup); mesh ray casting is moderate; OCCT surface
evaluation is slowest but most precise.

---

### Modes 5-6 -- Rotary Operations

**Mode 5 (2+rotary, X/Z/theta):** Machine has X, Z, and A (rotation around X).
No independent Y -- achieved by rotating the stock. Input: SVG/DXF artwork,
heightmaps, STL meshes, or STEP models. Operations: roughing/finishing from
revolved profile, fluting, cylindrical relief (wrapped heightmap), indexing
(2D ops on unwrapped surface). A coordinate transformer converts XZ+angle to
Cartesian; `CutPoint` stays Cartesian, post-processor converts to X/Z/A.

**Mode 6 (3+rotary, XYZ+A):** Four simultaneous axes. Standard 3D surface
operations extended with tool tilt around the rotary axis. Simpler kinematics
than 5-axis: `A = atan2(-tool_axis_x, tool_axis_z)`. Uses `CutPoint.orientation`
same as mode 7.

---

### Mode 7 -- 5-Axis Operations

OCCT geometry. All 3D surface operations and full 5-axis operations.

#### 3D Surface Operations (3-axis, vertical spindle)

Uses `SurfaceModel` trait with `OcctFaceSurface`.

**Parallel finishing:** Constant-spacing cutting planes intersect the surface.
Scallop: `h = R - sqrt(R^2 - (s/2)^2) / cos(theta)`. Best for shallow regions.

**Scallop finishing:** Variable spacing maintaining constant scallop height via
slope-corrected stepover: `s = 2 * sqrt(2Rh - h^2) / cos(theta)`.

**Flowline finishing:** Follows UV parameter directions of NURBS surfaces.

**Pencil milling:** Traces concave corner regions where larger tools cannot reach.

**Z-level roughing:** Horizontal slicing, each layer cleared via Clipper2 polygon
clipping of model cross-sections.

#### 5-Axis Operations

Tool orientation varies continuously. Strategies: fixed tilt, fixed world axis,
surface normal, smoothed normal, auto-tilt, swarf.

**Point milling:** Ball-nose contacts surface; orientation from surface normal +
tilt strategy. Singularity handling for flat-top surfaces.

**Swarf milling:** Tool flank follows a ruled surface. One pass covers full depth.
For impeller blades, turbine vanes, mold walls.

**Multi-axis contour:** 5-axis Z-level finishing with tilting engagement angle.

---

## Linking

Linking connects passes into a continuous program. Shared across modes 2-7.

### Retract Strategies

| Strategy | Description |
|---|---|
| Fixed Z | Retract to user-defined Z |
| Clearance plane | Z = stock top + clearance (default) |
| Safe sphere | Sphere of radius R around part center |
| Minimal | Computed obstacle clearance (deferred) |

### Lead-In / Lead-Out

| Style | Geometry |
|---|---|
| Linear | Tangent extension, straight line |
| Arc | Circular arc tangent to cut direction (default for profiles) |
| Helical | Arc + ramp descent |
| Ramp | Angled linear descent |

Lead radius defaults to 40% of tool diameter.

### Linking Algorithm

For each consecutive pass pair: generate lead-out, retract, rapid to next lead-in
start, descend, generate lead-in. Pass ordering optimized by nearest-neighbor
sort within each Z level to minimize rapid travel.

---

## Collision Detection

Applies primarily to **modes 5-7** where tool body and orientation create
collision risk. Modes 2-4 have fixed vertical orientation; collision is limited
to gouge detection.

**Gouge detection (3-axis):** Verify tool tip Z >= surface Z at each XY. For
height fields: direct grid lookup. For OCCT: `BRepExtrema_DistShapeShape`.
Violations resolved by lifting or reported as errors.

**Gouge detection (5-axis):** Full tool body check. Tool discretized into N
cylindrical discs; each disc's distance to surface verified. Auto-tilt adjusts
orientation by minimum rotation to clear.

**Holder collision (modes 5-7):** Simplified holder solid (cylinder + cone)
checked against part and fixtures. Reported with collision location and minimum
required flute length. No automatic resolution.

---

## Feed and Speed Assignment

Shared across all modes. Sources in priority order:
1. Per-point override (engagement-based, for adaptive clearing)
2. Operation-level override (user explicit values)
3. Tool definition defaults
4. Material library lookup

### Feed Scaling

| Motion type | Multiplier |
|---|---|
| Cutting | 1.0x |
| Plunge | 0.3x |
| Ramp / helix | 0.5x |
| Lead-in / lead-out | 0.5x |

**Adaptive feed:** For trochoidal clearing,
`feed = base_feed * (target_engagement / theta_e)`, clamped to [0.2x, 1.5x].

**Rotary feed (modes 5-6):** Post-processor converts surface speed to angular
rate based on current diameter.

> Feed/speed values are initial. The simulation optimizer may override per-point
> rates and insert `SpringPass` entries after physics simulation.

---

## Chord Tolerance and Arc Fitting

All curves linearized to `CutPoint` sequences. Chord tolerance: 0.01mm finishing,
0.05mm roughing. Arc fitting replaces chord sequences with G2/G3 arc moves
(tolerance: 10% of chord tolerance), reducing G-code size and improving finish.

Optional Gaussian path smoothing for 3D/5-axis operations (modes 4 and 7).
Not applied to 2D profiles (would alter geometry).

---

## Rust Module Responsibilities

```
toolpath/
+-- planner.rs         dispatches to operation module; accepts abstract
|                      geometry source (polylines, heightmap, or OCCT faces)
+-- types.rs           CutPoint, Pass, Toolpath, ToolpathStats, FeedType
+-- linking.rs         retract, lead-in/out, pass ordering
+-- collision.rs       gouge check (3-axis + 5-axis), holder, auto-tilt
+-- feeds.rs           feed/speed annotation, engagement scaling
+-- arc_fit.rs         chord-to-arc replacement
+-- smoothing.rs       Gaussian path smoothing
+-- surface_model.rs   SurfaceModel trait
+-- operations/
    +-- contour.rs     Profile (accepts Vec<Polyline>)
    +-- pocket.rs      Pocket clearing (offset, parallel, adaptive)
    +-- drill.rs       Drilling cycles
    +-- vcarve.rs      Medial axis V-carve, flat-bottom, inlay
    +-- surface.rs     Parallel, scallop, flowline, pencil (generic over SurfaceModel)
    +-- five_axis.rs   Point milling, swarf, multi-axis contour
    +-- rotary.rs      Rotary profile, fluting, cylindrical relief

modes/
+-- mode_2d/           SVG/DXF --> Vec<Polyline> for contour/pocket/drill
+-- mode_25d/          Same input + medial axis for vcarve.rs
+-- mode_3d/           Heightmap --> HeightmapSurface for surface.rs
+-- rotary/            Coordinate transforms, cylindrical unwrap
+-- five_axis/         OCCT face selection --> OcctFaceSurface
```

---

## Development Approach by Mode

Each mode is a releasable milestone:

1. **Mode 2 (2D):** Profile, pocket, drilling with SVG/DXF. Basic linking. No
   OCCT. Validates geometry-source-agnostic refactoring of Clipper2 algorithms.
2. **Mode 3 (2.5D):** V-carve variants. Builds on mode 2's SVG/DXF pipeline.
3. **Mode 4 (3D):** Parallel/scallop on heightmaps, STL meshes, and STEP
   models. Introduces `SurfaceModel` trait with `HeightmapSurface`,
   `MeshSurface`, and `OcctFaceSurface` implementations.
4. **Modes 5-6 (rotary):** Coordinate transforms, post-processor rotary support.
   Reuses mode 4's heightmap/mesh engine and mode 2's SVG/DXF pipeline.
5. **Mode 7 (5-axis):** Full 3D/5-axis via OCCT. `OcctFaceSurface` implementation.
   Holder collision detection.

---

*Document status: Draft*
*Related documents: `system-architecture.md`, `modes-overview.md`, `geometry-kernel.md`, `gcode-postprocessor.md`, `cutting-simulation.md`*
