# Tool Geometry Model

## Purpose

The existing `Tool` struct (`src-tauri/src/models/tool.rs`) stores machining parameters:
type, diameter, flute count, and default feeds/speeds. This is sufficient for toolpath
generation, where the cutter radius is the primary geometric input. But several
downstream features require the tool's **physical shape**:

- **Cut simulation** — computing the swept volume as the tool moves along a path
  requires knowing the tool's profile at every radial distance from the axis.
- **Viewport rendering** — displaying a realistic tool mesh in 3D (the current
  `toolMesh.ts` approximates all tools as a cylinder + shank cylinder).
- **Collision checking** — detecting whether the non-cutting portions of the tool
  (shank, holder) collide with remaining workpiece material.

This feature extends `Tool` with the geometric data needed to derive the tool's
physical shape, and provides a computed **revolution profile** — the 2D outline
that, when revolved around the tool axis, produces the tool solid.

---

## Coordinate Convention

The tool profile uses a local coordinate system:

- **Z = 0** at the tool tip (lowest cutting point)
- **Z positive** toward the spindle (upward along the tool axis)
- **R positive** outward from the tool axis (radial distance)
- The profile describes the **right-side outline** only; the full tool is this
  profile revolved 360° around the Z axis

```
        Z ↑
          │     shank
          │   ┌───────┐
          │   │       │
          │   │       │  ← shank_diameter / 2
  cutting │   ├───┐   │
  length  │   │   │   │  ← diameter / 2
          │   │   │
          │   │   │
        0 ┼───┴───┘
          │
          └──────────→ R
```

---

## New Fields on Tool

All new fields use `#[serde(default)]` with sensible defaults so that existing
`.jcam` project files (which lack these fields) continue to deserialize correctly.

### Universal Fields (all tool types)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cutting_length` | `f64` | `diameter × 3.0` | Length of the fluted cutting portion (mm) |
| `shank_diameter` | `f64` | `diameter` | Diameter of the non-cutting shank (mm) |
| `overall_length` | `f64` | `cutting_length × 3.0` | Total length from tip to end of shank (mm) |

### Type-Specific Fields

These fields are only meaningful for certain tool types. They live on a new
`ToolGeometry` enum (or within `ToolType` if the implementer prefers) that
parallels the existing `ToolType` discriminant.

| Tool Type | Field | Type | Default | Description |
|-----------|-------|------|---------|-------------|
| BullNose | `corner_radius` | `f64` | `diameter × 0.1` | Radius of the corner rounding (mm) |
| VBit | `included_angle` | `f64` | `90.0` | Full angle between cutting edges (degrees) |
| Drill | `point_angle` | `f64` | `118.0` | Full cone angle at the drill tip (degrees) |
| CenterDrill | `point_angle` | `f64` | `60.0` | Tip angle (degrees) |
| CenterDrill | `pilot_diameter` | `f64` | `diameter × 0.3` | Diameter of the pilot section (mm) |
| CenterDrill | `pilot_length` | `f64` | `cutting_length / 3` | Total length of the pilot portion including cone (mm) |
| Tap | `thread_pitch` | `f64` | `1.0` | Distance between threads (mm) |
| ThreadMill | `thread_pitch` | `f64` | `1.0` | Distance between threads (mm) |
| BoringBar | `min_bore_diameter` | `f64` | `diameter × 1.5` | Minimum bore the bar fits into (mm) |

**Taper support** (cross-cutting — applies to FlatEndmill, BullNose):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `taper_half_angle` | `Option<f64>` | `None` | Half-angle of taper from tip to shoulder (degrees). `None` = straight (no taper). |

When present, the cutting portion's radius increases linearly from the tip
radius to `tip_radius + cutting_length × tan(taper_half_angle)` at the
top of the cutting length.

---

## Revolution Profile

The core output is a method on `Tool`:

```rust
/// Returns the tool's revolution profile as an ordered sequence of (radius, z)
/// points, from tip (z=0) upward toward the shank.
///
/// The profile is a polyline — straight segments between consecutive points.
/// Curved regions (ball nose hemisphere, bull nose torus) are approximated
/// with enough points for the intended use (swept volume computation,
/// mesh generation). A `segments_per_quarter` parameter controls the
/// tessellation density of curved sections.
pub fn profile(&self, segments_per_quarter: u32) -> Vec<(f64, f64)>
```

### Profile Shapes by Tool Type

**FlatEndmill** (no taper):
```
(0, 0)  →  (R, 0)  →  (R, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `R = diameter / 2`, `shank_R = shank_diameter / 2`.

**FlatEndmill** (with taper):
```
(0, 0)  →  (R, 0)  →  (R_top, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `R_top = R + cutting_length × tan(taper_half_angle)`.

**BallNose**:
```
(0, 0)  →  (arc...)  →  (R, R)  →  (R, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
The arc from `(0, 0)` to `(R, R)` is a quarter-circle of radius R, approximated
with `segments_per_quarter` line segments.

**BullNose**:
```
(0, 0)  →  (R-cr, 0)  →  (arc...)  →  (R, cr)  →  (R, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
The arc from `(R-cr, 0)` to `(R, cr)` is a quarter-circle of the corner radius `cr`.

**BullNose** (with taper):
```
(0, 0)  →  (R-cr, 0)  →  (arc...)  →  (R, cr)  →  (R_top, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `R_top = R + (cutting_length - cr) × tan(taper_half_angle)`. The taper applies
to the straight wall above the corner radius. The corner arc itself is not tapered.

**VBit**:
```
(0, 0)  →  (R, R / tan(half_angle))  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `half_angle = included_angle / 2` in radians.

**Drill**:
```
(0, 0)  →  (R, R / tan(point_half_angle))  →  (R, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `point_half_angle = point_angle / 2` in radians.

**CenterDrill**:
```
(0, 0)  →  (pilot_R, pilot_R / tan(point_half_angle))  →  (pilot_R, pilot_length)  →  (R, pilot_length)  →  (R, cutting_length)  →  (shank_R, cutting_length)  →  (shank_R, overall_length)
```
where `pilot_R = pilot_diameter / 2`, `point_half_angle = point_angle / 2` in radians,
and `pilot_length` = height of the pilot cone + a short cylindrical pilot section.
The pilot-to-body transition is modeled as a vertical step (no countersink angle).
`pilot_length` defaults to `cutting_length / 3`.

**Tap, Reamer, BoringBar, ThreadMill**: Treat as cylindrical (same as FlatEndmill
profile) for geometric purposes. Thread form detail is not modeled in the
revolution profile — these tools' cutting action is axial/radial, not swept-volume
based.

---

## Radial Clearance Function

For material removal computation, the key derived quantity is a function that
answers: "at horizontal distance `r` from the tool axis, how far above the
tool tip Z=0 is the bottom of the cutting envelope?"

```rust
/// Returns the Z clearance at radial distance `r` from the tool axis.
/// Returns `None` if `r` is outside the tool's cutting radius.
///
/// For a flat endmill: z_clearance(r) = Some(0.0) for r ≤ R
/// For a ball nose:    z_clearance(r) = Some(R - sqrt(R² - r²)) for r ≤ R
pub fn z_clearance(&self, r: f64) -> Option<f64>
```

This is a closed-form function (not derived from the polyline profile) that
gives exact results for each tool type. It only describes the **cutting portion**
of the tool — not the shank.

| Tool Type | z_clearance(r) | Domain |
|-----------|---------------|--------|
| FlatEndmill (straight) | `0` | `r ≤ R` |
| FlatEndmill (tapered) | `0` for `r ≤ R`; `(r - R) / tan(taper_half_angle)` for `R < r` | `r ≤ R_top` |
| BallNose | `R - sqrt(R² - r²)` | `r ≤ R` |
| BullNose (straight) | `0` for `r ≤ R-cr`; toroidal section for `R-cr < r ≤ R` | `r ≤ R` |
| BullNose (tapered) | `0` for `r ≤ R-cr`; toroidal for `R-cr < r ≤ R`; `cr + (r - R) / tan(taper_half_angle)` for `R < r` | `r ≤ R_top` |
| VBit | `r / tan(half_angle)` | `r ≤ R` |
| Drill | `r / tan(point_half_angle)` | `r ≤ R` |
| CenterDrill | `r / tan(point_half_angle)` | `r ≤ pilot_R` (see note) |

**CenterDrill note**: The z_clearance function only covers the pilot cone portion.
For `r > pilot_R`, the center drill's geometry depends on the pilot-to-body
transition (a vertical step in the profile model), so `z_clearance` returns `None`
for `r > pilot_R`. In practice, center drills are used for shallow centering
operations where only the pilot tip engages.

For BullNose, the toroidal section:
```
z_clearance(r) = cr - sqrt(cr² - (r - (R - cr))²)    for R-cr < r ≤ R
```

---

## Serialization and Backward Compatibility

New fields must be added to `Tool` (or a nested struct within it) with
`#[serde(default)]` so that existing project files that lack these fields
still deserialize. The `Default` implementation applies the heuristic defaults
listed above (which depend on `diameter` — see design choices document for
how to handle this).

The TypeScript `Tool` type in `src/api/types.ts` must be updated to mirror
the new fields. Optional fields should use `fieldName?: number` so the
frontend gracefully handles old snapshots.

The existing `toolMesh.ts` can be updated to use the revolution profile for
rendering instead of its current hardcoded cylinder approximation, but this
is not required as part of this feature — it's a natural follow-on.

---

## Test Strategy

### Unit Tests (Rust)

- **Profile generation**: For each tool type with known parameters, call
  `profile()` and verify the returned points match expected coordinates.
  Test with `segments_per_quarter = 1` for easy verification of curved tools.
- **Radial clearance**: For each tool type, test `z_clearance(r)` at known
  radial distances (center, mid-radius, edge, outside) against analytically
  computed values.
- **Taper**: Verify tapered flat endmill profile has linearly increasing radius.
- **Serialization round-trip**: Serialize a `Tool` with geometry fields, deserialize,
  verify all fields preserved.
- **Backward compatibility**: Deserialize a `Tool` JSON string that lacks all
  new fields (the format from existing `.jcam` files). Verify defaults are
  applied and the tool is usable.

### Property-Based Tests (optional but valuable)

- For any tool, `z_clearance(0.0)` should return `Some(0.0)` for pointed tools
  (VBit, Drill) and `Some(0.0)` for flat tools, `Some(R)` for ball nose at r=0...
  actually, at r=0 for ball nose: `R - sqrt(R²) = 0`. So `z_clearance(0) = 0`
  for all types. This is a universal invariant.
- For any tool, `z_clearance(max_cutting_radius + ε)` should return `None`
  (where `max_cutting_radius` is `R` for most tools, `R_top` for tapered).
- The profile's first point should have `z = 0`.
- The profile's last point should have `z = overall_length`.
- Profile R values should never be negative.
