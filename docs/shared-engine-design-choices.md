# Shared Engine Design Choices

This document records the assumptions, choices, and rationale behind the
feature specifications:

- [Tool Geometry Model](tool-geometry-model.md)
- [G-code Parser](gcode-parser.md)
- [Dexel Material Removal Engine](dexel-material-removal.md)
- [Viewport-Adaptive Resolution](viewport-adaptive-resolution.md)

These features are the computational foundation for cut simulation —
visualizing the evolving workpiece shape as tools move through G-code motions.
A planner or implementer working on any one of these features should read this
document for context on cross-cutting decisions.

---

## Why These Three Features

The project is moving toward mode-based workflows (2D, 2.5D, 3D orthogonal,
3D rotary, 4D rotary, 5D) where each mode is essentially a separate application
with its own operation set and UI. Three things are shared across all modes:

1. **Tool definition** — every mode uses cutting tools with physical geometry.
2. **G-code visualization** — every mode produces G-code, and the user needs
   to see what the machine will do, in workpiece-relative coordinates.
3. **Cut simulation** — a more advanced form of G-code visualization that shows
   material being removed, revealing the workpiece shape at each step.

These three features provide the computational core for items 2 and 3, while
item 1 is the geometric foundation both depend on.

They compose as a pipeline:

```
Tool geometry profile
        │
        ▼
G-code parser           ←── G-code from any source
        │
        ▼
Dexel material removal  ←── stock definition
        │
        ▼
Evolving workpiece mesh ──→ viewport rendering
```

However, **each feature is independently useful and independently testable**.
They can be implemented in any order or in parallel. The interfaces between
them are simple value types (tool radius + closure, motion segments, mesh data).

---

## Choice: Revolution Profile for Tool Geometry

**Decision**: Describe tool shapes as 2D polyline profiles revolved around the
Z axis, rather than parametric surfaces, triangle meshes, or CSG trees.

**Why**:
- Every standard milling tool (endmill, ball nose, V-bit, drill, etc.) is a
  body of revolution. This is a fundamental property of rotary cutting tools.
- A polyline profile is trivially convertible to a rendering mesh (lathe
  geometry) and to a radial clearance function (which the dexel engine needs).
- It's simple to define, serialize, and test — just a list of `(r, z)` points.
- Parametric surfaces (NURBS) would be more precise for the ball nose hemisphere
  but add complexity with no practical benefit — the polyline can approximate
  any curve to arbitrary precision by adding points.
- Triangle meshes are view-ready but harder to derive the radial clearance
  function from.

**Trade-off accepted**: The profile is an approximation for curved tools. At
`segments_per_quarter = 8` (32 segments for a full circle), the chord error
for a 10mm ball nose is ~0.024mm — well below machining tolerance.

---

## Choice: Radial Clearance Function as the Bridge Between Tool and Dexel

**Decision**: The dexel engine does not consume the `Tool` struct or the polyline
profile. It receives a `z_clearance(r) -> Option<f64>` function and a cutting
radius.

**Why**:
- Decouples the engine from the tool model. The dexel engine can be tested with
  trivial closures (`|r| if r <= 5.0 { Some(0.0) } else { None }`) without
  constructing `Tool` objects.
- The radial clearance function is the exact mathematical object the dexel
  algorithm needs — it maps radial distance to cut depth offset. Anything else
  would require internal conversion.
- For standard tool types, this function is a closed-form equation (not derived
  from the polyline), which is both faster and more precise than interpolating
  the profile.
- The tool geometry feature specifies both the polyline profile (for rendering)
  and the radial clearance function (for simulation). They're independent
  outputs from the same tool parameters.

**Trade-off accepted**: For exotic tool shapes that don't fit the standard
types, deriving `z_clearance` from the polyline profile (by interpolation) is
a future extension. The closed-form approach covers all current tool types.

---

## Choice: Dexel Over Voxel for Initial Implementation

**Decision**: Use a dexel (Z-height column) model for material tracking rather
than a full voxel grid.

**Why**:
- For 3-axis machining (tool always vertical), the dexel model is exact —
  material is always removed from above, and the tool profile is symmetric
  around the Z axis. A voxel model adds a third dimension of storage for no
  accuracy benefit.
- Memory: a 1000×1000 dexel grid at 0.1mm resolution is ~16 MB. An equivalent
  voxel grid at 0.1mm resolution would be 1000×1000×500 = 500M voxels, which
  at 1 bit per voxel is 62 MB, and at 1 byte per voxel is 500 MB. The dexel
  model is an order of magnitude lighter.
- Speed: updating a dexel column is O(spans), which is typically O(1) for
  3-axis work. Updating a voxel column requires scanning all Z voxels in the
  affected range.
- The existing `cutting-simulation.md` design document specifies dexels for
  3-axis work (modes 1-4) and voxels for multi-axis (modes 5-7), consistent
  with this choice.

**Trade-off accepted**: Dexels cannot represent material that overhangs in Z
(e.g., undercuts from 4/5-axis machining). This is acceptable because the
initial implementation targets 3-axis work. When multi-axis modes are added,
the engine will need a voxel or tri-dexel extension. The per-cell span list
representation is forward-compatible — a tri-dexel model uses three orthogonal
dexel grids.

---

## Choice: Multi-Span Columns

**Decision**: Each dexel column stores a `Vec<ZSpan>` (multiple intervals)
rather than a single Z height value.

**Why**:
- A single-height dexel (just the top surface Z) cannot represent through-holes,
  internal pockets, or any geometry where material exists below a void. A drill
  operation creates exactly this situation — material above the hole, a void
  where the drill passed, and material below (if the drill didn't go all the
  way through).
- Multi-span columns handle these cases correctly with minimal additional
  complexity. The span operations (truncate above Z, split at Z) are simple
  and fast.
- For most of the workpiece, columns will have exactly one span. The Vec
  overhead is minimal, and the common case (one span) is the fast path.

**Trade-off accepted**: More complex span arithmetic than single-height. But
single-height would require a second pass (or a separate model) to handle
drill holes, which is worse overall.

---

## Choice: ISO 6983 Subset for the Parser

**Decision**: The G-code parser targets standard ISO 6983 G-code (the subset
covering 3-axis mill work) rather than attempting to handle every dialect.

**Supported**: G0/G1/G2/G3, G17/G18/G19, G20/G21, G28, G4, G43/G49,
G54–G59, G73/G80/G81/G82/G83, G90/G91, G93/G94/G95, G98/G99, and common
M-codes (M0–M6, M8/M9, M30).

**Not supported**: Subprogram calls (M98/M99, O-word labels), macro variables
and expressions (#100, bracket math), G68/G69 coordinate rotation, G43.1 RTCP,
G51 scaling, custom G-codes (Gxxx above G99), Mazak/Okuma/Heidenhain dialects.

**Why**:
- The supported subset covers what our post-processor emits and what 95%+ of
  3-axis G-code programs use. This gets us from zero to useful quickly.
- Subprograms and macros require an interpreter, not a parser — they involve
  loops, conditionals, and variable evaluation. This is a qualitatively
  different problem that can be added later.
- Controller-specific dialects (conversational Mazak, Heidenhain plaintext)
  are different languages that would need separate parsers. ISO 6983 is the
  common denominator.
- The parser is lenient — unrecognized codes produce warnings, not errors. So
  G-code files that contain unsupported codes will still parse; the unsupported
  lines are skipped.

**Trade-off accepted**: G-code from some machines (especially lathes, Swiss-type,
wire EDM) won't parse well. That's fine — this feature targets mill G-code.

---

## Choice: IJK Offsets Are Incremental

**Decision**: The parser interprets I, J, K words in arc commands (G2/G3) as
**incremental offsets from the start point to the arc center**, not as absolute
coordinates.

**Why**:
- This is the convention used by Fanuc, LinuxCNC, Mach3/4, GRBL, and the vast
  majority of CNC controllers. It's also what our post-processor emits.
- Absolute IJK (used by some Haas configurations and a few others) is the
  minority case. Supporting it would require either a user-specified flag or
  heuristic detection, adding complexity for little gain.
- The `cutting-simulation.md` design doc and post-processor config both assume
  incremental IJK.

**Trade-off accepted**: G-code from controllers configured for absolute IJK
will produce incorrect arc centers. A future extension could add an option to
the parser for this. The lenient error handling will catch gross radius
mismatches and warn.

---

## Choice: Parser Does Not Apply Work Offsets or Tool Length Comp

**Decision**: The parser resolves coordinates in the program's own coordinate
system. G54–G59 work offsets and G43 tool length compensation are noted in the
modal state but not applied to output positions.

**Why**:
- Work offsets and tool length comp are machine-specific values that aren't
  in the G-code — they're stored in the machine controller's offset tables.
  The parser has no access to these values.
- For visualization purposes, the G-code coordinates are the right frame of
  reference. The user wants to see tool motion relative to the workpiece,
  which is what the G-code coordinates represent (the programmer already
  accounted for the work offset when writing the program).
- If a consumer needs to apply offsets (e.g., for multi-fixture simulation),
  it can do so externally using the offset information the parser records in
  the modal state.

---

## Choice: Default Units Are Metric (G21)

**Decision**: If the G-code does not contain a G20 or G21 units declaration,
the parser defaults to metric (millimeters).

**Why**:
- The project uses mm internally for all geometry. Defaulting to mm avoids
  accidental 25.4× scaling errors when parsing programs that omit the units
  declaration.
- Most G-code programs include an explicit G20 or G21, so this default rarely
  matters in practice.
- The parser emits a warning when no units declaration is found, so the user
  is informed.

---

## Choice: Canned Cycles Are Expanded

**Decision**: The parser expands canned drill cycles (G81/G83/G73) into
explicit motion segments. Consumers never see canned cycle abstractions.

**Why**:
- The dexel engine and visualization layer work with motion segments. They
  would need to expand canned cycles themselves if the parser didn't do it.
  Centralizing expansion in the parser means every consumer gets fully resolved
  motions.
- The expansion is deterministic and well-defined by the ISO standard. There's
  no ambiguity about what a G83 cycle does — it's a sequence of rapid/feed/retract
  motions with specific Z values.
- The post-processor's `cycles.rs` already contains the logic for cycle
  detection (the inverse — recognizing drill patterns in toolpath data).
  The expansion logic mirrors this in reverse.

---

## Choice: Dexel Resolution Default Is 0.1mm

**Decision**: The default grid resolution is 0.1mm. The user can adjust this
for speed (0.5mm) or quality (0.05mm).

**Why**:
- 0.1mm is finer than typical machining tolerance (±0.01–0.05mm for precision
  work, ±0.1mm for general work). The dexel model's discretization error at
  0.1mm is on the order of ±0.05mm (half the cell diagonal projected to the
  surface normal), which is within machining tolerance.
- At 0.1mm for a typical small part (100mm × 100mm), the grid is 1M cells —
  fast enough for interactive use on modern hardware.
- Coarser resolution (0.5mm) is useful for quick previews of large parts.
  Finer (0.05mm) is useful for verifying fine detail (small pockets, thin
  walls) where the staircase effect matters.

**Planned extension**: Viewport-adaptive resolution
(`viewport-adaptive-resolution.md`) uses a coarse base grid with a tile-segment
index to enable on-demand high-resolution re-rendering of zoomed-in regions.
The dexel engine implementer should ensure that `DexelGrid` can be constructed
with an arbitrary origin and extent (not just full stock), and that
`apply_segment` works correctly on sub-region grids. These are natural
properties of the design but worth verifying in tests.

---

## Choice: No UI Specified

**Decision**: All three feature specifications describe computational engines
with IPC commands as the interface. No UI components are specified.

**Why**:
- The features are mode-independent. The UI for invoking cut simulation will
  differ by mode (2D mode might show a top-down material removal animation;
  3D mode shows a rotating 3D viewport). Specifying UI now would prematurely
  couple the engine to one mode's workflow.
- The existing frontend already has: a 3D viewport with mesh rendering, a
  simulation playback control panel, and tool animation infrastructure. The
  UI integration is a natural follow-on once the computational engines exist,
  and the existing patterns are clear enough that a separate specification is
  unnecessary.
- TDD is better served by pure computational features. Every function in these
  engines can be tested with deterministic inputs and exact expected outputs.
  UI testing is inherently more complex and less reliable.

---

## Coordinate Convention: Z-Up, Right-Handed

All three features use the project's existing coordinate convention:

- **X**: left-right
- **Y**: front-back
- **Z**: up-down (spindle axis, tool moves in -Z to cut)
- Right-handed coordinate system
- The existing `Vec3` struct (`models/stock.rs`) is used for all 3D positions

The tool geometry model uses a **local** coordinate system (Z=0 at tip,
Z positive toward spindle). This is converted to the machine coordinate system
by translating to the tool tip position — the consumer handles this.

---

## Independence and Ordering

The three features have **no build-time dependencies** on each other. Each can
be implemented and merged independently:

- **Tool geometry** depends on nothing new — it extends the existing `Tool` struct.
- **G-code parser** depends on nothing new — it reads text, produces new types.
- **Dexel engine** depends on nothing new — it takes simple numeric inputs.

They **compose at runtime** through simple interfaces:

- Tool geometry provides `z_clearance(r)` → dexel engine consumes it.
- G-code parser provides `Vec<MotionSegment>` → dexel engine consumes them.
- Dexel engine provides `MeshData` → frontend renders it.

A planner implementing one of these features does not need to wait for or
coordinate with the other two. Each feature's test suite is self-contained
using direct construction of the types it consumes.

---

## Backward Compatibility

The tool geometry feature adds new optional fields to `Tool`. All new fields
have defaults derived from existing fields (`diameter`, `tool_type`) so that:

- Existing `.jcam` project files deserialize without error.
- The frontend handles missing fields gracefully (`field?: number` in TypeScript).
- Existing tests continue to pass without modification (they construct Tools
  without the new fields; defaults apply).

The G-code parser and dexel engine are entirely new modules. They add no fields
to existing types and introduce no breaking changes.

The **serde default derivation** for tool geometry fields requires care: the
defaults for `cutting_length` and `overall_length` depend on `diameter`, which
isn't available in a `#[serde(default)]` function. The implementer has several
options (post-deserialization fixup, a `#[serde(deserialize_with)]` that reads
sibling fields, or storing the defaults as `0.0` and interpreting `0.0` as
"use heuristic" at call sites). This is an implementation detail left to the
planner.
