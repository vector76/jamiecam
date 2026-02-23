# JamieCam Toolpath Engine

## Overview

The toolpath engine transforms an operation definition (strategy, tool, feeds, geometry
selection) into an ordered sequence of annotated 3D points — the toolpath — that can
then be post-processed into G-code. It lives entirely in Rust and runs on the Rayon
thread pool for parallelism.

The engine is structured as a pipeline. Each stage has a clean input/output contract
so stages can be tested in isolation and swapped independently.

```
Operation Definition
        │
        ▼
┌───────────────────┐
│ Feature Extraction │  identify machinable geometry from B-rep
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Region Computation │  compute 2D/3D cutting boundaries + rest material
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│  Pass Generation  │  produce ordered cut point sequences
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│     Linking       │  connect passes with rapids, lead-in/out, retracts
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│Collision Detection│  gouge check, holder clearance, tilt correction
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Feed/Speed Assign │  annotate each point with feed rate and spindle speed
└────────┬──────────┘
         │
         ▼
       Toolpath
```

---

## Core Data Types

```rust
/// A single commanded point in the toolpath.
pub struct CutPoint {
    /// Tool tip position in WCS, Z-up (mm).
    pub position: Vector3<f64>,

    /// Tool axis unit vector (Z-up). For 3-axis, always (0, 0, 1).
    /// For 5-axis, defines the orientation of the spindle.
    pub orientation: Vector3<f64>,

    /// The type of motion to this point.
    pub feed_type: FeedType,

    /// Feed rate to this point (mm/min). 0.0 = rapid.
    pub feed_rate: f64,

    /// Spindle speed (RPM). None = unchanged from previous.
    pub spindle_speed: Option<f64>,
}

pub enum FeedType {
    Rapid,
    Cutting,
    Plunge,       // vertical entry into material
    Ramp,         // angled entry
    Helix,        // circular ramp entry
    LeadIn,       // approach arc/line before engaging material
    LeadOut,      // departure arc/line after leaving material
    Dwell(f64),   // pause in seconds (for boring, chip clearing)
}

/// A connected sequence of CutPoints representing one pass.
pub struct Pass {
    pub points: Vec<CutPoint>,
    pub kind: PassKind,
    pub z_depth: Option<f64>,   // for Z-level passes
}

pub enum PassKind {
    Roughing,
    SemiFinishing,
    Finishing,
    LeadIn,
    LeadOut,
    Link,         // rapid move connecting passes
}

/// The complete computed toolpath for one operation.
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
    pub max_scallop_height_mm: f64,  // computed for finishing ops
}
```

---

## Operation Taxonomy

Operations are organized into four tiers reflecting increasing geometric complexity.
Development follows this order.

```
Tier 1: 2D          ── Z is fixed during each cut pass
Tier 2: 2.5D        ── Z steps between passes; constant during each pass
Tier 3: 3D Surface  ── 3 simultaneous axes, tool normal varies
Tier 4: 5-Axis      ── 5 simultaneous axes, tool can tilt arbitrarily
```

### Tier 1 — 2D Operations

#### Profile / Contour

Follows a 2D boundary at one or more Z depths. The dominant 2D operation.

```
Input:   closed or open 2D curve (from selected edge loop, DXF, or SVG)
         cutting side (left / right / on)
         depth parameters
Output:  offset curve at tool radius, duplicated per depth level
```

Parameters:
- Depth: total depth, step-down per level
- Tool compensation: computer (pre-compensated) or controller (G41/G42)
- Multiple passes: roughing offset + finishing offset
- Tabs: leave material bridges to hold part (for through-cuts)

Offset algorithm: Clipper2 (C++ library, called via Rust FFI). Clipper2 handles
self-intersecting offsets and island avoidance robustly.

#### Pocket Clearing

Removes all material inside a closed boundary down to a specified floor depth.

```
Input:   outer boundary polygon(s), island polygon(s)
         floor depth, step-down, tool
Output:  area-filling passes at each Z level
```

Strategies:

| Strategy | Description | Best for |
|---|---|---|
| Offset (conventional) | Inward-spiraling offset contours | General pocketing |
| Offset (climb) | Outward-spiraling contours | Finish walls |
| Parallel (zig) | Uni-directional raster | Wide open pockets |
| Parallel (zig-zag) | Bi-directional raster | Fast roughing |
| Adaptive (trochoidal) | Constant engagement arc | Hard materials, long tools |

Adaptive clearing maintains a constant tool engagement angle (default 30–60°)
by computing trochoidal loops wherever the engagement would exceed the target.
Algorithm: compute cutter contact region at each path position; insert circular
detour when engagement exceeds threshold.

Entry methods:
- Helical ramp (preferred: minimum axial load)
- Linear ramp (where helical diameter doesn't fit)
- Pre-drilled hole (no ramp, plunges to depth)
- Open side (direct lateral entry for open pockets)

#### Drilling

Point operations at discrete X/Y locations.

```
Input:   list of hole centers + diameters (auto-detected from model circles/arcs
         or manually placed), depth per hole
Output:  approach + cycle + retract sequence per hole
```

Cycle types:

| Cycle | Description | G-code |
|---|---|---|
| Spot | Partial depth, chamfer only | G81 |
| Drill | Full depth, single plunge | G81 |
| Peck | Full depth, pecking retract for chip clearing | G83 |
| Chip-break | Partial retract without full withdrawal | G73 |
| Boring | Single point boring, precision diameter | G85/G86 |
| Reaming | Multi-flute, finished hole | G85 |
| Tapping | Synchronized feed/speed for threads | G84 |

Hole sorting: nearest-neighbor (TSP approximation) by default to minimize
rapid travel. Alternative: sort by diameter, by Z depth.

Auto-detection: OCCT identifies cylindrical faces and extracts center axis
position, diameter, and depth. User confirms and assigns drill cycles.

---

### Tier 2 — 2.5D Operations

#### Z-Level Roughing

Horizontal slicing of the part volume into layers. Each layer is cleared
using pocket-filling passes. The primary roughing strategy for most parts.

```
Input:   part solid, stock solid, step-down, tool
Output:  one pocket-fill pass set per Z level
```

Remaining material tracking: each level clips the cutting boundary to the
remaining stock above and around. Implemented as 2D polygon clipping
(Clipper2) of the model's horizontal cross-section at each Z.

Helical entry is generated per level when the start point of the pass is
inside material.

#### Adaptive Z-Level Roughing

Combines Z-level slicing with per-level adaptive (trochoidal) clearing.
Preferred for hard materials or when using long-reach tooling.

#### 3D Contour (Z-Level Finishing)

Follows the surface at multiple Z depths — used to finish vertical and
near-vertical walls after roughing.

```
Input:   surface faces (from model), Z step-down, tolerance
Output:  contour pass at each Z level (intersection of Z plane with surface)
```

Each contour is the intersection of a horizontal plane with the part surface.
Computed via OCCT `BRepAlgoAPI_Section`. The resulting curves are offset by
tool radius normal to the surface.

---

### Tier 3 — 3D Surface Operations

All Tier 3 operations drive the ball-nose or tapered ball-nose tip along the
surface. Tool orientation is always (0, 0, 1) — vertical spindle.

#### Parallel (Raster) Finishing

Cutting planes at constant X or Y spacing intersect the surface. Intersection
curves become the cut passes.

```
Input:   surface faces, cutting angle (0° = X-parallel), stepover, tolerance
Output:  passes at even spacing in the chosen direction
```

Scallop height for a ball-nose tool on a slope of angle θ:
```
h = R - √(R² - (s/2)²) / cos(θ)
```
where `R` = ball radius, `s` = stepover. Scallop increases on steep faces.
Parallel finishing is most suited to shallow (< 30°) regions.

#### Scallop (Constant Scallop Height) Finishing

Each pass is offset from the previous by the amount that keeps scallop height
constant regardless of slope. Delivers uniform surface finish.

```
Input:   surface faces, target scallop height, tolerance
Output:  variable-spacing passes that maintain constant scallop
```

Algorithm:
1. Start from a seed curve (boundary or user-defined)
2. For each point on the current pass, compute the surface normal
3. Step perpendicular to the pass direction by the slope-corrected stepover:
   `s_local = 2 × √(2Rh - h²) / cos(θ)`
4. Project stepped point back onto surface
5. Repeat until surface is covered

Implemented using OCCT surface evaluation: `BRepAdaptor_Surface` for normals
and `GeomAPI_ProjectPointOnSurf` for projection.

#### Flowline Finishing

Follows the UV parameter directions of NURBS surfaces. Produces the most
natural-looking finish on aerodynamic and freeform surfaces.

```
Input:   NURBS surface face(s), U or V direction, stepover
Output:  passes along iso-parameter curves of the surface
```

Requires surfaces with well-defined parameterization. OCCT's
`BRepAdaptor_Surface::UParameter` / `VParameter` methods provide the curves.
Problematic for faces with singularities (trimmed, degenerate edges).

#### Pencil Milling

Detects and machines concave corner regions (fillets, junctions between faces)
where a larger tool could not reach. Typically run as a finishing pass after
scallop or parallel.

```
Input:   surface faces, tool radius
Output:  trace curves along all concave contact regions
```

Algorithm: compute the locus of tool center points where the tool simultaneously
contacts two faces. OCCT `BRepOffsetAPI_OffsetShape` and distance computations
identify these regions. The resulting curves are sorted and linked.

---

### Tier 4 — 5-Axis Operations

5-axis operations control the tool's orientation vector simultaneously with its
position. The tool's Z-axis aligns with the `orientation` field of each `CutPoint`.

#### Tool Orientation Strategies

These strategies apply across all 5-axis operations:

| Strategy | Description |
|---|---|
| Fixed tilt | Tool tilts at a fixed lead/lag angle relative to cut direction |
| Fixed world axis | Tool tilts toward a fixed world vector (e.g., tilted from Z) |
| Normal to surface | Tool axis = surface normal (true 5-axis contact) |
| Smoothed normal | Normal to surface, Gaussian-smoothed to reduce axis motion |
| Auto-tilt | Minimal tilt away from gouge while keeping surface contact |
| Swarf | Tool side follows a ruled surface (see below) |

#### 5-Axis Point Milling

The ball-nose tip contacts the surface at a point. Tool orientation varies
continuously to maintain a desired lead/lag angle or to avoid collision.

```
Input:   surface faces, orientation strategy, lead angle, lag angle, tolerance
Output:  toolpath with varying CutPoint.orientation per point
```

Base path is computed identically to Tier 3 (parallel, scallop, or flowline).
Then each path point's orientation is computed from the surface normal at that
point, modified by the chosen tilt strategy.

Singularity handling: when the surface normal is exactly aligned with Z (flat
top surface), a small fixed lead angle is applied to avoid gimbal lock issues
on machine tools with A/C or B/C axis configurations.

#### Swarf Milling (Ruled Surface)

The side (flank) of a cylindrical or tapered tool follows a ruled surface —
a surface generated by sweeping a straight line. One machining pass covers
the full depth of the ruled surface, eliminating scallop entirely.

```
Input:   ruled surface face(s), tool (cylindrical or tapered), tolerance
Output:  single-pass path with orientation = ruling direction at each point
```

Algorithm:
1. OCCT identifies the ruling direction at each parameter point on the surface
2. Tool center is positioned so the tool side is tangent to the surface
3. The offset accounts for tool taper angle
4. Over/undercut check: verify tool side doesn't gouge adjacent faces

Primary use: impeller blades, turbine vanes, mold side walls.

#### Multi-Axis Contour

5-axis equivalent of Z-level contour finishing. The tool axis tilts to maintain
a constant engagement angle with steep walls, allowing a side-cutting strategy
on walls that 3-axis cannot reach cleanly.

#### Future: Port Milling, Turbine Blade (5-axis specialty operations)

Specialized templates for common 5-axis part families. Planned for post-v1.

---

## Linking

Linking connects the cut passes into a continuous program by inserting rapids,
retracts, lead-in motions, and lead-out motions between passes.

### Retract Strategies

| Strategy | Description | When to use |
|---|---|---|
| Fixed Z | Retract to a user-defined Z height | Simple setups |
| Clearance plane | Retract to Z = stock top + clearance | Multi-level ops |
| Safe sphere | Retract to a sphere of radius R around part center | Complex 3D parts |
| Minimal | Just clear the next obstacle (computed) | Cycle-time optimization |

Default: Clearance plane. Minimal retract is deferred to a later version.

### Lead-In / Lead-Out Styles

Lead-in and lead-out motions smooth the tool's entry and exit from material,
reducing witness marks and entry shock.

| Style | Description | Geometry |
|---|---|---|
| None | Direct plunge to start | Not recommended |
| Linear | Tangent extension of the first cut move | Straight line |
| Arc | Circular arc, tangent to cut direction | G2/G3 |
| Helical | Arc + ramp descent simultaneously | 3D arc |
| Ramp | Angled linear descent | Straight line, angled in Z |

Arc lead-in is the default for profile and contour operations. Lead radius
defaults to 40% of tool diameter.

### Linking Algorithm

```
for each ordered pair of consecutive passes (prev, next):

  1. Generate lead-out from prev.last_point
     (arc or linear extending beyond the exit)

  2. Compute retract point above lead-out end
     (using chosen retract strategy)

  3. Rapid to point above lead-in start of next

  4. Feed down to lead-in start (plunge or ramp)

  5. Generate lead-in to next.first_point

  6. Append [lead-out, retract, rapid, descend, lead-in] as Link Pass
```

Pass ordering is optimized before linking to minimize total rapid travel.
Nearest-neighbor sort applied within each Z level.

---

## Collision Detection

### Gouge Detection (3-axis)

For 3-axis: the tool tip must never penetrate the part surface.

After pass generation, each `CutPoint.position` is verified:
- The point must lie on or above the surface at its XY location
- Implemented as: compute Z of surface at (X, Y) using OCCT
  `BRepExtrema_DistShapeShape` and verify tool tip Z ≥ surface Z

Violations are flagged with their location and severity. Options:
- Lift the path to clear (adjust Z)
- Report error and stop (for operations where lifting would change intent)

### Gouge Detection (5-axis)

For 5-axis the full tool body — not just the tip — must not intersect the part.

The tool is discretized into N cylindrical discs along its axis. Each disc
center's distance to the part surface is computed. If distance < disc radius,
a gouge is detected at that disc's axial position.

For each gouging point, the `auto-tilt` strategy adjusts the tool orientation
by the minimum rotation needed to bring the tool body clear.

### Holder Collision Detection

The tool holder body (above the flute) is represented as a simplified solid
(cylinder + truncated cone). Collision with the part and any declared fixtures
is checked after gouge detection.

Holder collision is reported with:
- Collision location
- Minimum required flute length (so the user can adjust stick-out)

Automatic resolution is not attempted for holder collisions — the user must
either increase stick-out, change tool, or adjust the operation.

---

## Feed and Speed Assignment

Each `CutPoint` is annotated with a feed rate and spindle speed. Sources
in priority order:

1. Per-point override (computed for adaptive clearing — feed varies by engagement)
2. Operation-level override (user sets explicit values in operation params)
3. Tool definition defaults (stored in tool library)
4. Material library lookup (tool material + workpiece material + operation type)

### Feed Scaling by Motion Type

Applied as multipliers on the base cutting feed rate:

| Motion type | Default multiplier |
|---|---|
| Cutting | 1.0× |
| Plunge | 0.3× |
| Ramp / helix entry | 0.5× |
| Lead-in / lead-out | 0.5× |
| Rapid | machine max (not annotated in feedrate) |

### Adaptive Feed (Engagement-Based)

For adaptive clearing operations, feed rate varies per-point based on the
instantaneous tool engagement angle θ_e:

```
feed = base_feed × (target_engagement / θ_e)
```

Clamped to [0.2×, 1.5×] of base feed. This maintains constant chip load
across changing path geometry.

---

## Chord Tolerance and Path Smoothing

### Chord Tolerance

All curves are linearized to straight `CutPoint` sequences. The chord tolerance
controls the maximum deviation from the true curve:

```
            true curve
           ╭──────────╮
          ╱      │     ╲
         │  chord│      │
          ╲      │ tol ╱
           ╰──────────╯
            linearized chord
```

Default: 0.01mm for finishing, 0.05mm for roughing.

### Arc Fitting

For controllers that support G2/G3 arc moves (most do), sequences of short
linear segments that approximate an arc are detected and replaced with a single
arc move. This reduces G-code file size dramatically for circular features and
improves surface finish (controller interpolates smoothly rather than executing
thousands of micro-moves).

Arc fitting tolerance: 10% of chord tolerance.

### Path Smoothing

Optional post-pass Gaussian smoothing of the point sequence to reduce
direction changes. Useful for high-speed machining where abrupt direction
changes cause dynamic errors. Implemented as a moving weighted average of
position vectors, applied along the path.

Not applied to 2D profile operations (would alter the intended geometry).
Applied by default to 3D surface and 5-axis operations.

---

## Development Roadmap

Operations will be implemented in tier order. Each tier is a releasable milestone.

### Phase 1 — Core 2D (MVP)

- [ ] Profile / Contour (single depth, fixed Z)
- [ ] Pocket clearing (offset strategy only)
- [ ] Drilling (drill and peck cycles)
- [ ] Basic linking (fixed-Z retract, linear lead-in/out)
- [ ] Fixed feeds (no adaptive, no material library)
- [ ] Gouge detection (3-axis, report only)

### Phase 2 — 2.5D and Roughing

- [ ] Multi-level profile (step-down)
- [ ] Z-level roughing
- [ ] Adaptive / trochoidal clearing
- [ ] Arc lead-in / lead-out
- [ ] Helical entry
- [ ] Hole sorting (nearest-neighbor)
- [ ] Arc fitting in output

### Phase 3 — 3D Surface

- [ ] Parallel (raster) finishing
- [ ] Scallop finishing
- [ ] Pencil milling
- [ ] Flowline finishing
- [ ] Gouge detection + auto-lift
- [ ] Material library for feeds/speeds
- [ ] Rest machining (identify what prior tool left)

### Phase 4 — 5-Axis

- [ ] Tool orientation strategies (fixed tilt, surface normal, smoothed normal)
- [ ] 5-axis point milling
- [ ] Swarf milling (ruled surface)
- [ ] Holder collision detection
- [ ] Auto-tilt for collision avoidance
- [ ] Multi-axis contour

### Phase 5 — Advanced (Post-v1)

- [ ] Machine kinematics model (A/C, B/C table configurations)
- [ ] Inverse kinematics for specific machine types
- [ ] Machine collision checking (full machine envelope)
- [ ] Barrel / oval-form tool support
- [ ] Minimum-time linking (minimal retract)
- [ ] Engagement-based adaptive feed (all operation types)

---

## Rust Module Responsibilities (recap)

```
toolpath/
├── planner.rs         dispatches to the correct operation module;
│                      owns the pipeline stages in order
├── types.rs           CutPoint, Pass, Toolpath, ToolpathStats, FeedType
├── linking.rs         retract strategy, lead-in/out generation, pass ordering
├── collision.rs       gouge check (3-axis + 5-axis), holder check, auto-tilt
├── feeds.rs           feed/speed annotation, engagement-based scaling
├── arc_fit.rs         detect and replace chord sequences with arc moves
├── smoothing.rs       Gaussian path smoothing
└── operations/
    ├── contour.rs     Profile, Z-level contour
    ├── pocket.rs      Pocket clearing (offset, parallel, adaptive)
    ├── drill.rs       Drilling cycle generation
    ├── surface.rs     Parallel, scallop, flowline, pencil
    └── five_axis.rs   Point milling, swarf, multi-axis contour
```

---

*Document status: Draft*
*Related documents: `system-architecture.md`, `geometry-kernel.md`, `gcode-postprocessor.md`*
