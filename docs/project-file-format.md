# JamieCam Project File Format

## Design Goals

| Goal | Decision |
|---|---|
| Human-inspectable | JSON envelope for all project structure |
| Compact computed data | Binary format for toolpath point arrays |
| Single portable file | ZIP archive containing both |
| Stale cache detection | Content-hash cache key per operation |
| Forward compatibility | Schema version + migration chain |
| Portable paths | Relative + absolute paths for model references |
| Safe saves | Atomic write via temp-file rename |
| Debuggable | ZIP can be inspected with any archive tool |

---

## File Format: ZIP Archive

A `.jcam` file is a standard ZIP archive. Its extension is registered as the
JamieCam project format. Any ZIP tool can open it for inspection or manual recovery.

```
project.jcam  (ZIP archive)
│
├── project.json              human-readable project definition
├── model/                    optional: embedded copy of source model
│   └── source.step
├── toolpaths/                computed toolpath cache (binary)
│   ├── <operation-uuid>.bin
│   ├── <operation-uuid>.bin
│   └── ...
└── simdata/                  computed physics simulation cache (binary)
    ├── <operation-uuid>.sim.bin
    └── ...
```

`project.json` is always present. The `model/` directory is present only when
the user has chosen to embed the model. The `toolpaths/` directory is present
only when at least one toolpath has been computed and cached.

### ZIP Storage Policy

| File | Compression |
|---|---|
| `project.json` | Deflate |
| `model/source.step` | Deflate (STEP files are text-based, compress well) |
| `toolpaths/*.bin` | Deflate (position deltas compress well) |
| `simdata/*.sim.bin` | Deflate (repetitive float data compresses well) |

---

## project.json Schema

### Top Level

```json
{
  "schema_version": 1,
  "app_version": "0.1.0",
  "created_at": "2025-01-15T10:30:00Z",
  "modified_at": "2025-01-15T14:22:00Z",
  "project": { ... },
  "source_model": { ... },
  "stock": { ... },
  "wcs": [ ... ],
  "active_wcs": 0,
  "tools": [ ... ],
  "operations": [ ... ],
  "post_processor": { ... },
  "machine": { ... },
  "physics_limits": { ... }
}
```

| Field | Type | Description |
|---|---|---|
| `schema_version` | integer | Incremented on breaking format changes |
| `app_version` | string | JamieCam version that last saved this file |
| `created_at` | ISO 8601 | Creation timestamp (UTC) |
| `modified_at` | ISO 8601 | Last save timestamp (UTC) |

---

### `project`

```json
"project": {
  "name": "Bracket v3",
  "description": "Aluminum mounting bracket, 6061-T6",
  "units": "metric"
}
```

`units`: `"metric"` (mm) or `"imperial"` (inches). All values in `project.json`
are stored in the project's units. The binary toolpath format always stores in mm
regardless of project units; conversion is applied at export time.

---

### `source_model`

The source model file is referenced, not embedded by default. The user may
choose to embed it (for portability when sharing the project).

```json
"source_model": {
  "filename": "bracket_v3.step",
  "format": "step",
  "absolute_path": "/home/jamie/cad/bracket_v3.step",
  "relative_path": "../cad/bracket_v3.step",
  "sha256": "a3f8c27d91b4e56f...",
  "embedded": false,
  "embedded_path": null
}
```

| Field | Description |
|---|---|
| `filename` | Bare filename, for display |
| `format` | `"step"`, `"iges"`, `"stl"`, `"obj"` |
| `absolute_path` | Full path at last save; may not exist on another machine |
| `relative_path` | Path relative to the `.jcam` file; preferred for portability |
| `sha256` | Hash of the model file contents at last load |
| `embedded` | When `true`, the file is inside the ZIP at `model/source.<ext>` |
| `embedded_path` | When embedded, the ZIP-internal path |

**Model resolution on load:**

```
1. Try relative_path (relative to the .jcam file's directory)
   → found: load it, verify sha256
     → matches: proceed
     → mismatch: warn "model has changed since last save" — user decides
   → not found: continue

2. Try absolute_path
   → found: load it, verify sha256 (same as above)
   → not found: continue

3. If embedded: extract from zip and load

4. None found: prompt user to locate file manually
```

---

### `stock`

```json
"stock": {
  "type": "box",
  "material": "aluminum-6061",
  "origin": { "x": -5.0, "y": -5.0, "z": -2.0 },
  "box": {
    "width":  120.0,
    "depth":   80.0,
    "height":  30.0
  }
}
```

The `origin` is the stock's minimum-XYZ corner position in WCS coordinates.

**Stock types:**

```json
{ "type": "box",      "box":      { "width": ..., "depth": ..., "height": ... } }
{ "type": "cylinder", "cylinder": { "diameter": ..., "height": ... } }
{ "type": "mesh",     "mesh":     { "zip_path": "model/stock.stl" } }
```

**Material identifiers** (used to look up feed/speed defaults):

```
"aluminum-6061", "aluminum-7075",
"steel-mild", "steel-4140", "steel-stainless-304",
"brass-360", "copper",
"titanium-grade5",
"plastic-abs", "plastic-delrin", "plastic-nylon",
"wood-mdf", "wood-hardwood",
"foam-tooling"
```

---

### `wcs`

An array of work coordinate systems. Most projects have one; multi-setup jobs may have several.

```json
"wcs": [
  {
    "id": "3f8a2b...",
    "name": "G54 — Top Setup",
    "origin": { "x": 0.0, "y": 0.0, "z": 0.0 },
    "x_axis": { "x": 1.0, "y": 0.0, "z": 0.0 },
    "z_axis": { "x": 0.0, "y": 0.0, "z": 1.0 }
  }
],
"active_wcs": 0
```

`x_axis` and `z_axis` are unit vectors defining the WCS orientation. The Y axis
is derived as `z_axis × x_axis`. This representation handles any orientation
including tilted setups.

---

### `tools`

Project-local tool library. A subset of (or override of) the global tool library.

```json
"tools": [
  {
    "id": "7f3c1a...",
    "name": "10mm 4F Flat Endmill",
    "type": "flat_endmill",
    "material": "carbide",
    "diameter": 10.0,
    "corner_radius": 0.0,
    "flute_length": 30.0,
    "overall_length": 75.0,
    "shank_diameter": 10.0,
    "flute_count": 4,
    "helix_angle": 35.0,
    "tip_angle": null,
    "cutting_data": {
      "spindle_rpm": 15000,
      "feed_per_tooth_mm": 0.04,
      "axial_doc_mm": 10.0,
      "radial_doc_mm": 5.0,
      "plunge_multiplier": 0.3,
      "ramp_multiplier": 0.5,
      "lead_multiplier": 0.5
    },
    "holder": {
      "type": "er32_collet",
      "body_diameter": 40.0,
      "body_length":   60.0,
      "taper_angle":   8.0
    }
  }
]
```

**Tool types:** `"flat_endmill"`, `"ball_nose"`, `"bull_nose"`, `"v_bit"`,
`"drill"`, `"center_drill"`, `"tap"`, `"reamer"`, `"boring_bar"`, `"thread_mill"`.

For `"bull_nose"`: `corner_radius` is the fillet radius.
For `"v_bit"` and `"drill"`: `tip_angle` is the included angle in degrees.
For `"ball_nose"`: `corner_radius` == `diameter / 2`; redundant but explicit.

---

### `operations`

An ordered array of machining operations. Program order follows array order.

```json
"operations": [
  {
    "id": "9a2f4c...",
    "name": "Adaptive Rough",
    "type": "pocket",
    "enabled": true,
    "wcs_id": "3f8a2b...",
    "tool_id": "7f3c1a...",
    "geometry": { ... },
    "params": { ... },
    "linking": { ... },
    "feeds_speeds": { ... },
    "cache": { ... }
  }
]
```

#### `geometry` — Geometry Selection

Faces and edges are identified by their index in OCCT's topology traversal order,
plus a **fingerprint** for change detection. If the fingerprint no longer matches
after a model reload, the geometry selection is flagged as invalid.

```json
"geometry": {
  "selections": [
    {
      "type": "face",
      "index": 5,
      "fingerprint": {
        "surface_type": "Plane",
        "centroid":     [10.000, 20.000, 0.000],
        "normal":       [0.000,  0.000,  1.000],
        "area":         800.000
      }
    },
    {
      "type": "edge",
      "index": 12,
      "fingerprint": {
        "curve_type":   "Line",
        "start":        [0.000,  0.000,  0.000],
        "end":          [100.000, 0.000, 0.000],
        "length":       100.000
      }
    }
  ]
}
```

Fingerprint matching tolerance: centroid and vertices within 0.01mm, area within 0.1%.

#### `params` — Operation-Specific Parameters

Each operation type has its own `params` object. The `type` field at the operation
level is the discriminant.

**`type: "contour"`**
```json
"params": {
  "side":              "left",
  "depth_mm":          10.0,
  "step_down_mm":      2.5,
  "roughing_offset_mm": 0.3,
  "finishing_passes":  1,
  "compensation":      "computer",
  "tabs": {
    "enabled":  false,
    "width_mm": 3.0,
    "height_mm": 1.0,
    "count":    4
  }
}
```

**`type: "pocket"`**
```json
"params": {
  "depth_mm":           15.0,
  "step_down_mm":       3.0,
  "strategy":           "adaptive",
  "stepover_percent":   45.0,
  "direction":          "climb",
  "entry":              "helical",
  "helix_diameter_mm":  8.0,
  "helix_pitch_mm":     1.5,
  "min_radial_doc_percent": 5.0,
  "floor_finish_pass":  true,
  "wall_finish_passes": 1,
  "wall_finish_offset_mm": 0.1
}
```

**`type: "drill"`**
```json
"params": {
  "cycle":              "peck",
  "depth_mm":           20.0,
  "peck_depth_mm":      5.0,
  "retract_mm":         2.0,
  "dwell_s":            0.0,
  "auto_detect":        true,
  "detect_min_dia_mm":  5.9,
  "detect_max_dia_mm":  6.1,
  "sort_strategy":      "nearest_neighbor"
}
```

**`type: "parallel"`** (3D raster finishing)
```json
"params": {
  "angle_deg":          0.0,
  "step_over_mm":       0.5,
  "tolerance_mm":       0.01,
  "direction":          "zig_zag",
  "boundary":           "silhouette"
}
```

**`type: "scallop"`**
```json
"params": {
  "scallop_height_mm":  0.01,
  "tolerance_mm":       0.005,
  "start_from":         "boundary"
}
```

**`type: "flowline"`**
```json
"params": {
  "direction":          "u",
  "step_over_mm":       0.5,
  "tolerance_mm":       0.01,
  "reverse":            false
}
```

**`type: "five_axis_point"`**
```json
"params": {
  "base_strategy":          "scallop",
  "base_params":            { ... },
  "orientation_strategy":   "smoothed_normal",
  "lead_angle_deg":         5.0,
  "lag_angle_deg":          0.0,
  "max_tilt_deg":           30.0,
  "smoothing_distance_mm":  10.0
}
```

**`type: "swarf"`**
```json
"params": {
  "tolerance_mm":   0.01,
  "direction":      "along_u"
}
```

#### `linking`

Common to all operation types:

```json
"linking": {
  "retract_strategy":     "clearance_plane",
  "retract_clearance_mm": 5.0,
  "safe_z_mm":            50.0,
  "lead_in_style":        "arc",
  "lead_in_radius_mm":    4.0,
  "lead_in_angle_deg":    90.0,
  "lead_out_style":       "arc",
  "lead_out_radius_mm":   4.0,
  "path_smoothing":       true,
  "smoothing_tolerance_mm": 0.02,
  "arc_fitting":          true,
  "arc_fit_tolerance_mm": 0.001
}
```

#### `feeds_speeds`

Overrides the tool's default cutting data. `null` means "use tool default".

```json
"feeds_speeds": {
  "spindle_rpm":      null,
  "feed_mmpm":        null,
  "plunge_mmpm":      null,
  "ramp_mmpm":        null,
  "lead_mmpm":        null
}
```

#### `cache`

Records the cached toolpath state for this operation.

```json
"cache": {
  "key":           "sha256:f4a9c21...",
  "valid":         true,
  "computed_at":   "2025-01-15T14:00:00Z",
  "engine_version": "0.1.0",
  "binary_file":   "toolpaths/9a2f4c.bin",
  "stats": {
    "point_count":         15420,
    "pass_count":          12,
    "total_length_mm":     2341.5,
    "cutting_length_mm":   1890.2,
    "rapid_length_mm":     451.3,
    "estimated_duration_s": 284,
    "max_scallop_mm":      0.009
  },
  "simulation_key":  "sha256:b2f71c3...",
  "sim_valid":       true,
  "sim_binary_file": "simdata/9a2f4c.sim.bin",
  "sim_optimized":   true,
  "violations_summary": {
    "total":      3,
    "force":      1,
    "deflection": 2,
    "chatter":    0
  }
}
```

When `valid` is `false`, the `binary_file` may or may not be present but will
not be used. `key` is stored to detect when re-computation produces a matching
result (indicating the cache can be trusted again without running the algorithm).

---

### `post_processor`

```json
"post_processor": {
  "id":           "fanuc-0i",
  "custom_path":  null,
  "overrides": {
    "program.number": 1050,
    "format.decimal_places": 4
  }
}
```

`custom_path` is set when using a user-defined post-processor file.
`overrides` is a flat map of `section.field` → value for post-processor
TOML fields that are overridden for this project without modifying the file.

---

### `machine`

Optional reference to a machine model file describing the physical characteristics
of the CNC machine used in this project. Used by the simulation engine. `null`
disables machine-aware simulation features (chatter prediction, travel limit checks).

```json
"machine": {
  "id":          "haas-vf2",
  "custom_path": null
}
```

The machine model file (format described in `cutting-simulation.md`) specifies spindle
power, feed rate limits, structural stiffness, and axis travel envelopes.

---

### `physics_limits`

Project-level thresholds for the physics simulation. Operations may define per-operation
overrides in their own `physics_limits` block.

```json
"physics_limits": {
  "max_cutting_force_n":         200.0,
  "max_spindle_power_fraction":  0.80,
  "max_surface_error_mm":        0.05,
  "max_tool_deflection_mm":      0.02,
  "max_tool_temp_c":             600.0,
  "max_workpiece_temp_c":        150.0,
  "max_workpiece_deflection_mm": 0.03,
  "min_feature_safety_factor":   2.0,
  "max_chatter_risk":            0.3,
  "max_tab_mark_height_mm":      0.02,
  "enabled_layers":              [1, 2, 3]
}
```

| Field | Description |
|---|---|
| `max_cutting_force_n` | Total resultant cutting force threshold (Newtons) |
| `max_spindle_power_fraction` | Max fraction of rated spindle power (0–1) |
| `max_surface_error_mm` | Total allowable surface deviation — tool + workpiece deflection |
| `max_tool_deflection_mm` | Tool-only deflection limit (stricter, for finishing passes) |
| `max_tool_temp_c` | Tool temperature limit (°C); lower for uncoated HSS |
| `max_workpiece_temp_c` | Workpiece surface temperature limit (°C) |
| `max_workpiece_deflection_mm` | Workpiece deflection limit for thin-feature analysis |
| `min_feature_safety_factor` | Ratio of yield strength to predicted stress; < 2.0 triggers breakage risk |
| `max_chatter_risk` | Chatter risk index threshold (0–1 scale) |
| `max_tab_mark_height_mm` | Maximum acceptable step height at tab exit/entry points |
| `enabled_layers` | Which simulation layers to run: 1=chip load, 2=forces, 3=deflection, 4=thermal, 5=structural FEA, 6=chatter |

---

## Cache Invalidation

A toolpath cache entry is valid when its stored `key` matches the computed cache
key of the current inputs. The key is:

```
key = "sha256:" + hex(SHA-256(canonical_cache_input))
```

Where `canonical_cache_input` is the UTF-8 JSON serialization of:

```json
{
  "engine_version": "0.1.0",
  "model_sha256":   "<model file hash>",
  "stock":          { ... },
  "tool":           { ... },
  "operation_type": "pocket",
  "params":         { ... },
  "linking":        { ... },
  "feeds_speeds":   { ... },
  "geometry":       { ... }
}
```

Keys in this JSON are sorted alphabetically (canonical form) before hashing
to ensure key stability regardless of serialization order.

**What invalidates the cache:**

| Change | Invalidates toolpath? | Invalidates simulation? |
|---|---|---|
| Model file content | Yes (model SHA-256 changes) | Yes |
| Stock dimensions or position | Yes | Yes |
| Tool geometry (diameter, length) | Yes | Yes |
| Tool cutting data | No — feeds/speeds are post-computation | Yes (changes chip load) |
| Operation strategy params | Yes | Yes |
| Linking params | Yes | Yes |
| Feeds/speeds overrides | No | Yes |
| Operation name or color | No | No |
| Post-processor selection | No | No |
| Engine version | Yes (algorithm may have changed) | Yes |
| Physics limits (thresholds) | No | Yes — limits are part of the sim cache key; different limits produce different violations and a different optimized toolpath |
| Material version (cutting-simulation.md) | No | Yes |
| Machine model | No | Yes (affects chatter calculation) |
| Simulation layer toggle | No | Yes (layer set changes output) |

**On invalidation:**

- `cache.valid` is set to `false`
- The binary file is retained in the ZIP (allows undo of accidental changes)
- The UI shows the operation as "needs recalculation"
- The stale toolpath geometry is shown in the viewport with a distinct color
  (desaturated, striped overlay) so the user can see what will change

---

## Binary Toolpath Format

Each `toolpaths/<uuid>.bin` file stores the computed toolpath for one operation.

### File Layout

```
Offset   Size   Field
──────────────────────────────────────────────────────────
0        8      magic:          b"JCAMPATH"
8        2      version:        u16 = 1
10       2      flags:          u16  (see below)
12       16     operation_id:   UUID bytes (big-endian)
28       4      point_count:    u32
32       4      pass_count:     u32
36       8      total_len_mm:   f64
44       8      cutting_len_mm: f64
52       8      rapid_len_mm:   f64
60       8      duration_s:     f64
68       8      max_scallop_mm: f64 (NaN if not applicable)
76       4      reserved:       [u8; 4]
──────────────────────────────────────────────────────────
80       N×32   point array     (see Point layout below)
──────────────────────────────────────────────────────────
```

Header is 80 bytes. Points follow immediately.

**Flags (u16 bitfield):**

| Bit | Meaning |
|---|---|
| 0 | `HAS_5AXIS` — orientation vectors are meaningful (not always (0,0,1)) |
| 1 | `HAS_SPINDLE_CHANGES` — spindle field is used |
| 2 | `HAS_COOLANT_CHANGES` — coolant changes encoded in flags byte |
| 3–15 | Reserved, must be zero |

### Point Layout (32 bytes)

```
Offset   Size   Field
──────────────────────────────────────────────────────────
0        4      x:          f32   tool tip X (mm, Z-up WCS)
4        4      y:          f32   tool tip Y
8        4      z:          f32   tool tip Z
12       4      ix:         f32   tool axis X component
16       4      iy:         f32   tool axis Y component
20       4      iz:         f32   tool axis Z component
24       4      feed_rate:  f32   mm/min; 0.0 = rapid (no feed word)
28       1      feed_type:  u8    (see enum below)
29       1      point_flags:u8    (see below)
30       2      spindle:    u16   RPM / 10; 0 = unchanged
──────────────────────────────────────────────────────────
```

**`feed_type` enum (u8):**

```
0 = Rapid
1 = Cutting
2 = Plunge
3 = Ramp
4 = Helix
5 = LeadIn
6 = LeadOut
7 = Dwell  (feed_rate field holds dwell time in seconds × 100)
```

**`point_flags` bitfield (u8):**

| Bit | Meaning |
|---|---|
| 0 | `NEW_PASS` — this point starts a new pass |
| 1 | `SPINDLE_CHANGE` — `spindle` field is a new value, not zero |
| 2 | `COOLANT_ON` — turn on coolant at this point |
| 3 | `COOLANT_OFF` — turn off coolant at this point |
| 4–7 | Reserved |

For 3-axis toolpaths (`HAS_5AXIS` not set), `ix`, `iy`, `iz` are always
`(0.0, 0.0, 1.0)`. They compress extremely well in this case.
f32 precision (≈7 significant digits) gives sub-micron resolution at 1000mm
coordinate values — adequate for all machining applications.

### Reading in Rust

```rust
#[repr(C, packed)]
struct ToolpathHeader {
    magic:          [u8; 8],
    version:        u16,
    flags:          u16,
    operation_id:   [u8; 16],
    point_count:    u32,
    pass_count:     u32,
    total_len_mm:   f64,
    cutting_len_mm: f64,
    rapid_len_mm:   f64,
    duration_s:     f64,
    max_scallop_mm: f64,
    reserved:       [u8; 4],
}

#[repr(C, packed)]
struct ToolpathPoint {
    x:           f32,
    y:           f32,
    z:           f32,
    ix:          f32,
    iy:          f32,
    iz:          f32,
    feed_rate:   f32,
    feed_type:   u8,
    point_flags: u8,
    spindle:     u16,
}

// Verify sizes at compile time
const _: () = assert!(std::mem::size_of::<ToolpathHeader>() == 80);
const _: () = assert!(std::mem::size_of::<ToolpathPoint>()  == 32);
```

---

## Binary Simulation Data Format

Each `simdata/<uuid>.sim.bin` file stores the per-point physics simulation results
for one operation, referenced by `cache.sim_binary_file` in `project.json`.

### File Layout

```
Offset   Size   Field
──────────────────────────────────────────────────────────
0        8      magic:           b"JCAMSIM\0"
8        2      version:         u16 = 1
10       2      flags:           u16  (reserved, must be zero)
12       16     operation_id:    UUID bytes (big-endian)
28       4      point_count:     u32
32       4      violation_count: u32
36       4      reserved:        [u8; 4]
──────────────────────────────────────────────────────────
40       V×32   violation array  (see Violation layout below)
next     N×32   point sim data   (see PointSimData layout below)
──────────────────────────────────────────────────────────
```

Header is 40 bytes, followed immediately by the violation array, then the
per-point simulation data array.

### PointSimData Layout (32 bytes)

```
Offset   Size   Field
──────────────────────────────────────────────────────────
0        4      force_n:        f32   total cutting force magnitude (N)
4        4      surface_err_mm: f32   predicted surface error from deflection (mm)
8        4      temperature_c:  f32   estimated cutting zone temperature (°C)
12       4      mrr_cm3min:     f32   material removal rate (cm³/min)
16       1      breakage_risk:  u8    0–255 mapped to 0.0–1.0
17       1      chatter_risk:   u8    0–255 mapped to 0.0–1.0
18       2      reserved:       u16
20       4      optimized_feed: f32   optimizer-adjusted feed (mm/min); 0 = unchanged
24       8      reserved:       [u8; 8]
──────────────────────────────────────────────────────────
```

### Violation Layout (32 bytes)

```
Offset   Size   Field
──────────────────────────────────────────────────────────
0        4      point_index:    u32   index into the PointSimData array
4        1      kind:           u8    0=ExcessiveCuttingForce
                                      1=SpindlePowerExceeded
                                      2=ToolDeflectionHigh
                                      3=SurfaceAccuracyRisk
                                      4=ThermalDamageRisk
                                      5=WorkpieceDeflectionHigh
                                      6=BreakageRisk
                                      7=ChatterRisk
                                      8=CrossGrainTearOut
                                      9=TabMark
5        1      severity:       u8    0=Warning, 1=Error, 2=Critical
6        2      reserved:       u16
8        4      value:          f32   measured value that triggered violation
12       4      threshold:      f32   threshold that was exceeded
16       1      action_taken:   u8    0=None, 1=FeedScaled, 2=SpringPassAdded,
                                      3=PassReordered, 4=SpindleShifted
17       15     reserved:       [u8; 15]
──────────────────────────────────────────────────────────
```

### Reading in Rust

```rust
#[repr(C, packed)]
struct SimHeader {
    magic:           [u8; 8],
    version:         u16,
    flags:           u16,
    operation_id:    [u8; 16],
    point_count:     u32,
    violation_count: u32,
    reserved:        [u8; 4],
}

#[repr(C, packed)]
struct PointSimData {
    force_n:        f32,
    surface_err_mm: f32,
    temperature_c:  f32,
    mrr_cm3min:     f32,
    breakage_risk:  u8,
    chatter_risk:   u8,
    reserved_u16:   u16,
    optimized_feed: f32,
    reserved_tail:  [u8; 8],
}

#[repr(C, packed)]
struct SimViolationRecord {
    point_index:   u32,
    kind:          u8,
    severity:      u8,
    reserved_u16:  u16,
    value:         f32,
    threshold:     f32,
    action_taken:  u8,
    reserved_tail: [u8; 15],
}

const _: () = assert!(std::mem::size_of::<SimHeader>()          == 40);
const _: () = assert!(std::mem::size_of::<PointSimData>()       == 32);
const _: () = assert!(std::mem::size_of::<SimViolationRecord>() == 32);
```

---

## Schema Versioning and Migration

### Version History

| `schema_version` | Change |
|---|---|
| 1 | Initial format |

Versions are incremented only on **breaking changes** — fields removed or
semantics changed. Adding optional fields is non-breaking and does not
increment the version.

### Migration Chain

On load, if `schema_version < current`, migrations are applied in sequence:

```rust
pub fn migrate(mut raw: serde_json::Value, from_version: u32) -> Result<serde_json::Value, MigrationError> {
    let mut version = from_version;
    while version < CURRENT_SCHEMA_VERSION {
        raw = match version {
            1 => migrate_v1_to_v2(raw)?,
            2 => migrate_v2_to_v3(raw)?,
            v => return Err(MigrationError::UnknownVersion(v)),
        };
        version += 1;
    }
    Ok(raw)
}
```

Before migration, the original file is backed up as `project.jcam.v{N}.bak`
in the same directory. The user is notified that a migration occurred.

### Forward Compatibility

If `schema_version > current` (file was created by a newer version of JamieCam):
- The app attempts to load anyway (unknown optional fields are silently ignored)
- A warning is shown: "This project was created by a newer version of JamieCam.
  Some features may not be available."
- Saving is disabled by default to prevent overwriting data the current version
  doesn't understand. The user can explicitly override this.

---

## Atomic Save Procedure

```
1. Serialize project.json to memory (Vec<u8>)
2. Compute new cache keys for any invalidated operations
3. Open a temporary file: <project>.jcam.tmp (same directory as target)
4. Write the complete ZIP archive to the temp file:
   a. Write project.json
   b. Copy unchanged toolpath .bin files from the existing archive
   c. Write any newly computed toolpath .bin files
   d. Copy unchanged simulation .sim.bin files from the existing archive
   e. Write any newly computed simulation .sim.bin files
   f. Write embedded model if present
5. Flush and sync the temp file to disk (fsync)
6. Rename temp file over the target file
   (rename() is atomic on POSIX; MoveFileExW with MOVEFILE_REPLACE_EXISTING on Windows)
7. On failure at any step: delete temp file, report error, leave original intact
```

This ensures the project file is never in a partially written state. Either the
old complete file exists or the new complete file exists — never a corrupt hybrid.

---

## Auto-Save

JamieCam maintains an auto-save copy to protect against crashes.

```
Auto-save path:
  Linux/macOS: ~/.local/share/jamiecam/autosave/<project-name>.autosave.jcam
  Windows:     %APPDATA%\jamiecam\autosave\<project-name>.autosave.jcam
```

Auto-save triggers:
- Every 5 minutes while the project is modified
- Immediately after any toolpath computation completes
- On application focus loss (user switches to another window)

Auto-save uses the same atomic write procedure as regular saves.

On launch, if an auto-save file is newer than the project file (indicating a
crash), the user is offered: **Restore auto-save** / **Ignore**.

Auto-save files older than 30 days are deleted on launch.

---

## Rust Serialization Types

```rust
/// The complete deserialized project. Built by loading project.json.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub schema_version: u32,
    pub app_version:    String,
    pub created_at:     DateTime<Utc>,
    pub modified_at:    DateTime<Utc>,
    pub project:        ProjectMeta,
    pub source_model:   SourceModel,
    pub stock:          Stock,
    pub wcs:            Vec<Wcs>,
    pub active_wcs:     usize,
    pub tools:          Vec<Tool>,
    pub operations:     Vec<Operation>,
    pub post_processor: PostProcessorRef,
}

/// Operation uses an untagged enum for type-specific params.
#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id:          Uuid,
    pub name:        String,
    pub enabled:     bool,
    pub wcs_id:      Uuid,
    pub tool_id:     Uuid,
    pub geometry:    GeometrySelection,
    #[serde(flatten)]
    pub kind:        OperationKind,    // contains both type tag and params
    pub linking:     LinkingParams,
    pub feeds_speeds: FeedsSpeedsOverride,
    pub cache:       CacheState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "snake_case")]
pub enum OperationKind {
    Contour(ContourParams),
    Pocket(PocketParams),
    Drill(DrillParams),
    Parallel(ParallelParams),
    Scallop(ScallopParams),
    Flowline(FlowlineParams),
    FiveAxisPoint(FiveAxisPointParams),
    Swarf(SwarfParams),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheState {
    pub key:            Option<String>,   // None if never computed
    pub valid:          bool,
    pub computed_at:    Option<DateTime<Utc>>,
    pub engine_version: Option<String>,
    pub binary_file:    Option<String>,   // ZIP-internal path
    pub stats:          Option<ToolpathStats>,
    // Physics simulation cache
    pub simulation_key:  Option<String>,         // None if never simulated
    pub sim_valid:       bool,
    pub sim_binary_file: Option<String>,         // ZIP-internal path to .sim.bin
    pub sim_optimized:   bool,                   // true if optimizer modified the toolpath
    pub violations_summary: Option<ViolationsSummary>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ViolationsSummary {
    pub total:      u32,
    pub force:      u32,
    pub deflection: u32,
    pub chatter:    u32,
}
```

The `#[serde(tag = "type", content = "params")]` attribute produces the
`"type": "pocket", "params": { ... }` JSON structure shown in the schema above.

---

## Open/Save API (Rust Commands)

```rust
/// Load a .jcam file. Validates schema, runs migrations if needed,
/// resolves the source model path, populates AppState.
#[tauri::command]
pub async fn load_project(path: String, state: State<'_, AppState>) -> Result<ProjectSnapshot, AppError>;

/// Save the current AppState to a .jcam file using atomic write.
#[tauri::command]
pub async fn save_project(path: String, state: State<'_, AppState>) -> Result<(), AppError>;

/// Save a copy to a new path (Save As). Does not change the active project path.
#[tauri::command]
pub async fn save_project_copy(path: String, state: State<'_, AppState>) -> Result<(), AppError>;

/// Embed or un-embed the source model in the project file.
#[tauri::command]
pub async fn set_model_embedded(embed: bool, state: State<'_, AppState>) -> Result<(), AppError>;
```

`ProjectSnapshot` is the frontend-facing summary returned after load — the full
project state the UI needs to reconstruct its view, without the large binary data.

---

## File Association and Extension

| Extension | MIME type (proposed) |
|---|---|
| `.jcam` | `application/x-jamiecam-project` |

Platform file association is registered by the Tauri bundler via the
`fileAssociations` config in `tauri.conf.json`. Double-clicking a `.jcam`
file launches JamieCam and opens the project.

---

*Document status: Draft*
*Related documents: `system-architecture.md`, `toolpath-engine.md`, `gcode-postprocessor.md`*
