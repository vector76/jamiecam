# JamieCam Cutting Simulation Engine

## The Core Idea

Traditional CAM is **open-loop**. A toolpath is generated from geometry and a
set of heuristic strategies ("adaptive", "scallop", "trochoidal"), then sent to
the machine. The strategies are informed by general machining wisdom but they do
not simulate what will actually happen when metal meets cutter. The user finds
out at the machine — through broken tools, chatter marks, dimensional errors,
or scrapped parts.

JamieCam's cutting simulation engine closes this loop. Every toolpath point is
evaluated against a physics model of the cutting process. The results feed back
into toolpath generation: feed rates, operation ordering, pass depths, and entry
strategies are adjusted to stay within predicted physical limits. The output is
not just a geometrically valid toolpath but a *physically validated* one.

```
                    TRADITIONAL CAM

  Geometry ──► Strategy ──► Toolpath ──► Machine ──► Part
                                                        ↑
                                                    (hope for the best)


                    JAMIECAM

  Geometry ──► Strategy ──► Toolpath ──► Simulation ──► Violations?
                                 ▲                            │
                                 └──── Optimizer ◄────────────┘
                                          │
                                          ▼
                                    Validated Toolpath ──► Machine ──► Part
```

The simulation engine is not a post-processing step — it is an integral part
of toolpath generation, running iteratively until the toolpath satisfies all
physical constraints.

---

## Physics Layers

The simulation is structured in layers of increasing complexity and computational
cost. Simpler layers run on every toolpath point; more expensive layers run only
where the simpler model identifies risk.

```
Layer 1: Chip Load & MRR            ← always on, cheap
Layer 2: Cutting Force              ← always on, cheap
Layer 3: Tool Deflection            ← always on, moderate
Layer 4: Thermal Model              ← configurable, moderate
Layer 5: Workpiece Structural FEA   ← on-demand or risk-triggered, expensive
Layer 6: Chatter / Stability        ← on-demand, moderate
```

---

### Layer 1: Chip Load and Material Removal Rate

The foundation of all other calculations. For each toolpath point:

**Instantaneous chip thickness:**
```
h = fz × sin(θ)
```
where `fz` = feed per tooth (mm/tooth), `θ` = instantaneous engagement angle.

**Radial engagement:**
Computed from the as-machined geometry tracker (see below) — the angle over
which the cutter is actually in contact with material at this point.

**Material removal rate:**
```
MRR = ae × ap × Vf
```
where `ae` = radial depth of cut (mm), `ap` = axial depth (mm), `Vf` = feed
rate (mm/min).

These values are computed per point and are the inputs to all subsequent layers.

---

### Layer 2: Cutting Force Model

**Mechanistic cutting force model** — the industry-standard approach for
predicting milling forces from first principles.

For each tooth in contact with material:

```
dFt = Kt × h × dz     (tangential — in direction of cut)
dFr = Kr × dFt        (radial — into the workpiece)
dFa = Ka × dFt        (axial — along spindle axis)
```

where `Kt`, `Kr`, `Ka` are **specific cutting force coefficients** — material
constants determined by experiment. These are stored in the material database.

Forces are integrated over the engaged arc (all teeth currently in cut) to
give the total force vector at each toolpath point:

```rust
pub struct CuttingForces {
    pub tangential_n: f64,
    pub radial_n:     f64,
    pub axial_n:      f64,
    pub resultant_n:  f64,
    pub torque_nm:    f64,
    pub power_w:      f64,
}
```

**Force direction** is also tracked — not just magnitude. The direction of the
resultant force determines whether it pushes the tool into or away from the
finished surface, which determines the sign of the dimensional error.

**Spindle torque and power** are checked against the machine model's limits.
Approaching the spindle's power envelope is a violation even if forces are
otherwise acceptable.

---

### Layer 3: Tool Deflection and Surface Error

Under lateral cutting forces, the tool deflects. This deflection is the
primary source of dimensional inaccuracy in milling.

**Beam model:** The tool is treated as a cantilever beam, fixed at the holder
and loaded at the center of the engaged flute length.

```
              holder (fixed)
              │
              │← shank
              │
              │← flute (loaded region)
              │    ← F (resultant lateral force)
              ●← tool tip (where deflection matters)
```

**Tip deflection:**
```
δ = F × (L³/3 + L²×a/2) / (E × I)
```
where:
- `F` = lateral force (radial component)
- `L` = total flute length
- `a` = distance from tip to load centroid
- `E` = tool material Young's modulus (carbide: ~600 GPa, HSS: ~210 GPa)
- `I` = second moment of area = π × d⁴ / 64

**Surface error:**
```
ε = δ × cos(φ)
```
where `φ` = angle between force direction and the surface normal at the cut
point. Only the component of deflection perpendicular to the finished surface
produces a dimensional error.

**Sign convention:** Deflection *away* from the surface leaves material
(undercut — part is oversized). Deflection *into* the surface removes extra
material (overcut — part is undersized). Both are violations but have different
consequences.

```rust
pub struct DeflectionResult {
    pub tip_deflection_mm:  f64,
    pub surface_error_mm:   f64,   // signed: positive = oversize, negative = undersize
    pub error_direction:    Vec3,
}
```

**Practical implication:** On a finishing pass, even small deflections (0.02mm)
can push the part outside tolerance. The optimizer responds by:
- Reducing feed rate to reduce forces and deflection
- Adding a *spring pass* — a second pass at the same position with no programmed
  offset. The tool is under essentially zero load, springs back to its undeflected
  position, and removes the thin layer left by deflection on the first pass.

---

### Layer 4: Thermal Model

Heat is generated at three zones: the primary shear zone (chip formation),
the tool-chip interface, and the tool-workpiece rubbing zone. For practical
simulation, a simplified model is used.

**Heat generation rate:**
```
Q_total = Ft × Vc
```
where `Vc` = cutting velocity (m/min). This is the mechanical power consumed,
essentially all of which becomes heat.

**Heat partition** — what fraction goes into the tool vs. chip vs. workpiece:
```
Rtool = 1 / (1 + (ρw × cw × kw) / (ρt × ct × kt) × √(Vc))
```
where `ρ`, `c`, `k` are density, specific heat, and thermal conductivity of
workpiece (w) and tool (t). At higher cutting speeds, more heat goes into the
chip (beneficial); at lower speeds, more goes into the tool and workpiece.

**Cumulative tool temperature** is tracked over the toolpath. It does not reset
instantly between cuts — heat soaks into the tool between passes.

**Thermal violations:**
- Tool temperature exceeding the coating softening point → accelerated wear
- Workpiece surface temperature exceeding material phase-change or oxidation
  threshold → surface integrity risk (residual stress, white layer in steel)
- Coolant effectiveness: thermal model adjusts partition ratios based on
  declared coolant type (flood, mist, through-spindle, dry)

---

### Layer 5: Workpiece Structural Analysis

This is the layer that enables the most sophisticated insights — predicting
whether the *workpiece itself* will deflect or fracture under cutting loads.

As material is removed during machining, the structural stiffness of the
remaining workpiece changes. A feature that starts as a small protrusion on a
large block eventually becomes a thin, compliant structure. The cutting forces
that were insignificant at the beginning can cause deflection or fracture at
the end.

**As-machined geometry tracking:**

The simulation maintains a volumetric representation of the workpiece that
is updated as each pass is simulated. This can use:
- **Dexel model** (z-map): fast, suitable for 3-axis machining
- **Voxel model**: accurate for multi-axis but memory-intensive
- **Exact B-rep**: most accurate but slow to update

For Phase 1, a dexel model is used. The voxel model is introduced in Phase 2
of simulation development.

**Thin feature detection:**

At each toolpath point, the remaining workpiece geometry near the cut is
analyzed for thin sections — walls, fins, cantilevered features. A feature
is flagged when:
```
min_wall_thickness < critical_thickness(material, height, load)
```

For a cantilevered wall under a point load F at height H, the maximum stress:
```
σ_max = F × H × (t/2) / I_wall
```
If `σ_max > σ_yield`, plastic deformation or fracture is predicted.

**Workpiece deflection at cut point:**

When the current cut is on or near a thin feature, the feature's stiffness
is estimated from its geometry and material properties. The cutting force is
applied as a point load. The resulting deflection is:
```
δ_workpiece = F × H³ / (3 × E × I_feature)
```

This deflection adds to the tool deflection to give the total dimensional error.
More importantly, it determines whether the feature will spring back (elastic —
acceptable) or yield (plastic — permanent deformation or fracture).

**Material anisotropy (wood and composites):**

For anisotropic materials, the elastic moduli differ by direction. Wood has:
- Along grain: E_L ≈ 10–15 GPa
- Across grain (radial): E_R ≈ 1–2 GPa
- Across grain (tangential): E_T ≈ 0.5–1 GPa

The material model includes a `grain_direction: Vec3` field. When computing
feature stiffness, the modulus used depends on the angle between the feature's
primary axis and the grain direction.

**The root-to-tip vs. tip-to-root insight:**

Consider a thin fin being profiled by a side-cutting pass. Two sequences:

```
Sequence A (root → tip):            Sequence B (tip → root):

  Pass 1: cut at base               Pass 1: cut at tip
          ↓                                  ↓
  Pass 2: cut at mid                Pass 2: cut at mid
          ↓                                  ↓
  Pass 3: cut at tip                Pass 3: cut at base
          ↑
    At pass 3, the fin is             At pass 3, the fin is
    unsupported — cutting             fully constrained by
    forces push the tip.              its base — rigid.
    HIGH BREAKAGE RISK.               LOW BREAKAGE RISK.
```

The structural simulation detects Sequence A as high-risk and the optimizer
automatically inverts the pass order for that operation.

---

### Layer 6: Chatter and Vibration

**Regenerative chatter** occurs when vibration in one pass produces a wavy
surface that interacts with the next pass to amplify vibration. Whether chatter
occurs depends on the spindle speed and depth of cut — their safe combinations
form the *stability lobe diagram*.

**Simplified stability model (Altintas, 1995):**

For a given number of flutes `N`, tool natural frequency `ωn`, damping ratio
`ζ`, and specific cutting coefficient `Kt`:

The critical axial depth at which chatter onset occurs:
```
blim = -1 / (2 × Kt × N × Re(Φ(iωc)))
```
where `Φ` is the frequency response function of the tool-holder-spindle system.

Stability lobes occur at spindle speeds:
```
n = 60 × ωc / (N × (k + ε/π))    for k = 0, 1, 2, ...
```

The optimizer can recommend a spindle speed adjustment to move into the center
of a stable lobe — often allowing a *higher* depth of cut safely than the
current setting.

**The tab mark problem (zero-load oscillation at tab transitions):**

When cutting an outer profile in slotting passes, holding tabs are formed by
raising the tool in Z at the tab boundary, traversing over the tab, and
plunging on the far side. During slotting, the cutting engagement provides
damping that suppresses tool oscillation. As the tool ascends in Z at the tab
boundary, cutting engagement drops to zero and that damping disappears. The
tool is briefly free to oscillate at or near the tool-spindle natural
frequency. This oscillation temporarily enlarges the effective cutting radius
beyond the nominal tool diameter, leaving a blemish in the finished workpiece
wall at the elevation of the tab transition.

The simulation predicts this effect from the tool-spindle frequency response
function and the engagement conditions at the transition point. The primary
mitigation is to offset the tool outward (away from the finished surface) in
XY during the Z ascent and descent at each tab transition, so any oscillation-
enlarged cutting affects only the tab region rather than the finished wall.
Feed rate reduction in the approach to each tab is a secondary measure that
reduces the abruptness of the load drop.

---

## Material Database

Every physics model requires material properties. The material database is
stored as TOML files (one per material family), user-extensible.

```toml
[material]
id          = "aluminum-6061-t6"
name        = "Aluminum 6061-T6"
category    = "aluminum"
isotropic   = true

[mechanical]
youngs_modulus_gpa    = 68.9
poissons_ratio        = 0.33
yield_strength_mpa    = 276.0
tensile_strength_mpa  = 310.0
hardness_brinell      = 95.0

[thermal]
conductivity_w_mk     = 167.0
specific_heat_j_kgk   = 896.0
density_kg_m3         = 2700.0
melting_point_c       = 660.0

[cutting]
# Specific cutting force coefficients (from calibrated experiments)
# These vary with tool geometry; values here are for typical carbide geometry
Kt_n_mm2    = 700.0    # tangential
Kr           = 0.30     # radial ratio
Ka           = 0.10     # axial ratio
# Chip thinning exponent
mc           = 0.25
# Built-up edge temperature threshold (above this, BUE forms)
bue_temp_c   = 150.0
```

```toml
[material]
id        = "oak-red"
name      = "Red Oak (hardwood)"
isotropic = false

[mechanical]
# Along-grain
youngs_modulus_longitudinal_gpa = 12.5
# Radial (across grain, toward center)
youngs_modulus_radial_gpa       = 1.8
# Tangential (across grain, along rings)
youngs_modulus_tangential_gpa   = 1.1
modulus_of_rupture_mpa          = 97.0   # bending strength (along grain)
# Perpendicular-to-grain rupture (for cross-grain features)
modulus_of_rupture_perpendicular_mpa = 8.5

[thermal]
conductivity_w_mk   = 0.18
specific_heat_j_kgk = 1700.0
density_kg_m3       = 740.0

[cutting]
# Wood-specific: fiber direction sensitivity
# 0° = with grain, 90° = against grain (worst for tear-out)
# These scale the required cutting force by grain angle
force_angle_multiplier = [
  [0,   1.0],
  [30,  1.2],
  [60,  1.8],
  [90,  2.5],
  [120, 2.0],
  [150, 1.4],
  [180, 1.0],
]
tear_out_risk_angle_deg = 90.0   # perpendicular cuts have highest tear-out risk
```

---

## Machine Model

Physical limits of the machine tool are required to validate that operations
don't exceed what the machine can provide.

```toml
[machine]
id   = "my_router"
name = "3-axis CNC Router, 1.5kW"

[spindle]
max_rpm        = 24000
min_rpm        = 6000
max_power_w    = 1500
max_torque_nm  = 0.6
# Frequency response function parameters (for chatter model)
natural_freq_hz = 800
damping_ratio   = 0.03

[axes]
max_feed_mmpm   = 10000
max_rapid_mmpm  = 15000

[stiffness]
# Machine structural stiffness at the tool tip (affects surface error)
# Lower stiffness machines deflect more under cutting forces
x_stiffness_n_mm = 50.0
y_stiffness_n_mm = 50.0
z_stiffness_n_mm = 200.0
```

---

## Simulation Pipeline

```
Toolpath (ordered CutPoints)
        │
        ▼
┌─────────────────────────────────────────────────────────┐
│  As-Machined Geometry Tracker                           │
│  Updates dexel/voxel model as each point is processed  │
│  Provides: engagement angle, radial depth at each point │
└────────────────────────┬────────────────────────────────┘
                         │
        ┌────────────────▼────────────────────┐
        │  Per-Point Physics Evaluator         │
        │  (runs on Rayon thread pool)         │
        │                                      │
        │  L1: chip load, MRR                  │
        │  L2: cutting forces (mechanistic)    │
        │  L3: tool deflection, surface error  │
        │  L4: thermal (if enabled)            │
        └──────────────────┬───────────────────┘
                           │
        ┌──────────────────▼───────────────────┐
        │  Structural Analysis (risk-triggered) │
        │                                       │
        │  Identify thin features near cut      │
        │  Compute feature stiffness            │
        │  Predict workpiece deflection         │
        │  Grain-angle breakage risk (wood)     │
        │  Sequence risk assessment             │
        └──────────────────┬────────────────────┘
                           │
        ┌──────────────────▼───────────────────┐
        │  Chatter Predictor (on-demand)        │
        │  Stability lobe at current conditions │
        └──────────────────┬────────────────────┘
                           │
                           ▼
              SimulationResult (per-point data + violation list)
```

### Simulation Result Types

```rust
pub struct PointSimData {
    pub mrr_mm3_min:            f64,
    pub chip_thickness_mm:      f64,
    pub cutting_forces:         CuttingForces,
    pub tool_deflection_mm:     f64,
    pub surface_error_mm:       f64,   // signed
    pub tool_temp_c:            Option<f64>,
    pub workpiece_deflection_mm: f64,
    pub breakage_risk:          f64,   // 0.0 – 1.0
    pub chatter_risk:           Option<f64>,
    pub severity:               SimSeverity,
}

pub struct SimViolation {
    pub point_range:      Range<usize>,   // first..last affected point
    pub kind:             ViolationKind,
    pub severity:         SimSeverity,
    pub actual_value:     f64,
    pub limit_value:      f64,
    pub message:          String,
    pub suggested_action: Vec<SuggestedAction>,
}

pub enum ViolationKind {
    ExcessiveCuttingForce,
    SpindlePowerExceeded,
    ToolDeflectionHigh,
    SurfaceAccuracyRisk,    // deflection exceeds tolerance
    ThermalDamageRisk,
    WorkpieceDeflectionHigh,
    BreakageRisk,
    ChatterRisk,
    CrossGrainTearOut,      // wood-specific
    TabMark,                // exit/re-entry deflection step
}

pub enum SuggestedAction {
    ReduceFeedRate        { range: Range<usize>, to_mmpm: f64 },
    ReduceDepthOfCut      { to_mm: f64 },
    AdjustSpindleSpeed    { to_rpm: f64, reason: String },
    AddSpringPass         { after_pass_index: usize },
    ReversePassOrder      { operation_id: OperationId },
    SplitAtDepth          { depth_mm: f64 },
    AddSupportStock       { region: BoundingBox },
    OffsetTabTransition   { outward_offset_mm: f64 },         // XY offset during Z ascent/descent at tabs
    ReduceTabFeedRate     { approach_mm: f64, to_mmpm: f64 },
}
```

---

## The Optimization Loop

After the simulation produces violations, the optimizer acts on them. This
is iterative — each round of optimization may resolve some violations while
creating new ones (e.g., reducing feed rate resolves a force violation but
may change thermal conditions).

```rust
pub fn optimize_toolpath(
    toolpath: Toolpath,
    material: &MaterialModel,
    machine:  &MachineModel,
    tool:     &Tool,
    limits:   &PhysicsLimits,
    options:  &OptimizationOptions,
) -> OptimizationResult {

    let mut current = toolpath;
    let mut iteration = 0;

    loop {
        let sim = simulate(&current, material, machine, tool);

        if sim.violations.is_empty() || iteration >= options.max_iterations {
            return OptimizationResult { toolpath: current, simulation: sim, iterations: iteration };
        }

        current = apply_optimizations(&current, &sim.violations, options);
        iteration += 1;
    }
}
```

### Optimization Actions by Violation Type

**ExcessiveCuttingForce / SpindlePowerExceeded:**
Per-point feed rate scaling:
```
F_new[i] = F_old[i] × (F_limit / F_actual[i])
```
Applied with a smoothing window to avoid abrupt feed changes (which cause
their own dynamic problems). Minimum feed rate is clamped to 10% of nominal
to prevent near-zero moves that stall the machine.

**SurfaceAccuracyRisk (finishing passes):**
Add a spring pass. A spring pass is a copy of the finishing pass with
zero programmed offset — the tool is essentially re-tracing the finished surface.
Because the tool is under near-zero load (only removing the thin deflection
error), it cuts close to its true centerline.

**BreakageRisk (thin feature, wrong sequence):**
Reorder passes within the operation so free ends are machined before roots.
For a wall being profiled top-to-bottom:
- Original: start at top (root of cantilever), end at bottom (tip)
- Optimized: start at bottom (tip), end at top (root)

This is only valid when the geometry allows — the optimizer checks that
the reordering doesn't create a new gouge or collision.

**ChatterRisk:**
Compute the stability lobe diagram for the current conditions. Find the
nearest stable spindle speed that:
a) falls within a stability lobe (avoids chatter)
b) is within the machine's spindle range
c) does not increase forces beyond limits

If no stable speed can be found within constraints, the optimizer recommends
reducing axial depth of cut instead.

**CrossGrainTearOut (wood):**
Flag the cut direction relative to grain angle. Recommend:
- Reversing cut direction (conventional → climb milling, or vice versa)
- Finishing the at-risk feature with a down-cut spiral bit (if currently using up-cut)
- Reducing feed rate at crossing points

**TabMark:**
Offset the tool outward from the finished surface in XY during each Z ascent
and descent at tab transitions. The outward offset moves the tool's effective
cutting circle into the tab region so that any zero-load oscillation enlargement
does not blemish the finished wall. The offset magnitude is derived from the
predicted oscillation amplitude (from the tool-spindle FRF). As a secondary
measure, reduce feed rate in the `approach_mm` before each tab to soften
the load transition.

---

## Visualization

Simulation results are overlaid on the toolpath in the viewport. The user
can switch between overlay modes in the viewport toolbar.

### Heatmap Overlays

The toolpath `LineSegments` geometry has a per-vertex color attribute. The
simulation engine produces a scalar value per toolpath point; the color is
mapped through a diverging colormap.

| Overlay | Low (cool) | Mid | High (warm/red) |
|---|---|---|---|
| Cutting Force | Blue | Yellow | Red |
| Surface Error | Blue (undersize) | Green (nominal) | Red (oversize) |
| Tool Temperature | Blue | Orange | Red |
| Breakage Risk | Green | Yellow | Red |
| Chatter Risk | Green | Yellow | Red |
| MRR | Blue | Cyan | White |

### Violation Markers

Violations are shown as icons anchored to the toolpath at the violation location
via `CSS2DObject` (the same mechanism as measurement labels):
- ⚠ Warning — yellow diamond
- ✖ Critical — red circle

Clicking a violation marker opens a panel showing:
- Violation type and description
- Actual vs. limit values
- Suggested actions (as clickable buttons that apply the optimization)

### Side Panel: Simulation Charts

A simulation panel (collapsible, alongside the G-code preview panel) shows
time-series charts for the full toolpath:
- Cutting force vs. toolpath distance
- Surface error vs. toolpath distance
- MRR vs. toolpath distance
- Temperature vs. toolpath distance (if thermal layer enabled)

Scrubbing the chart updates the viewport to show the tool position at that
point. Violations appear as red bands on the chart background.

### 3D Deflection Preview

For the tool deflection visualization, the viewport can show an exaggerated
(e.g., 10×) rendering of the deflected tool path alongside the nominal path.
This makes the physical deviation visible even when it is sub-millimeter.

---

## Computational Strategy

### Per-Point Cost

Layers 1–3 are fast: a few microseconds per point. For a 100,000-point
toolpath, layers 1–3 complete in under a second on a modern CPU with Rayon
parallelism.

Layer 4 (thermal) adds ~10× cost due to the heat accumulation tracking
(which is inherently sequential along the path). It runs on a single thread
with state carried forward, then Rayon is used for the final aggregation.

Layer 5 (structural FEA) is only triggered at points where the structural
risk heuristic fires. For most toolpaths this is a small fraction of points.

### Parallelism Approach

```
Toolpath points [0 .. N]
        │
        ├── Rayon par_iter() for Layers 1–3 (independent per point)
        │   Results: Vec<PartialSimData>
        │
        └── Sequential pass for Layer 4 (thermal — state carries forward)
            Results: Vec<ThermalData> merged into PartialSimData
        │
        └── Risk-gated sequential pass for Layer 5
            Only triggered where breakage_risk_heuristic() > threshold
```

### Simulation Resolution

Not every toolpath point needs full simulation. The engine supports configurable
resolution:

| Mode | Points evaluated | Use case |
|---|---|---|
| Full | Every point | Final validation before cutting |
| Sampled | Every Nth point (default N=10) | Interactive feedback while editing |
| Summary | One value per pass | Fast overview when first loading project |

Resolution is configured per-operation and can be overridden globally.

### Caching

Simulation results are cached using the same key mechanism as toolpaths:
```
sim_key = sha256(toolpath_key + material_id + machine_id + physics_limits)
```

Cached simulation results are stored alongside toolpath binary files in the
`.jcam` archive:
```
toolpaths/<operation-uuid>.bin       (toolpath points)
simdata/<operation-uuid>.sim.bin     (per-point simulation data)
```

The `.sim.bin` format is a flat binary of `PointSimData` structs (one per
toolpath point), plus a header with the simulation configuration and violation
summary.

---

## Physics Limits Configuration

The user defines acceptable limits per project or per operation. These determine
when violations are raised.

```toml
[physics_limits]
# Cutting force
max_cutting_force_n        = 200.0    # total resultant force
max_spindle_power_fraction = 0.80     # don't use more than 80% of rated power

# Surface accuracy
max_surface_error_mm       = 0.05     # total allowable deviation (tool + workpiece deflection)
max_tool_deflection_mm     = 0.02     # tool deflection limit for finishing passes

# Thermal
max_tool_temp_c            = 600.0    # for uncoated carbide (lower for HSS)
max_workpiece_temp_c       = 150.0    # for aluminum (lower for heat-sensitive materials)

# Structural
max_workpiece_deflection_mm = 0.03
min_feature_safety_factor   = 2.0     # ratio of yield strength to predicted stress

# Chatter
max_chatter_risk            = 0.3     # 0.0 – 1.0 scale

# Tab
max_tab_mark_height_mm      = 0.02
```

Limits can be set at three levels (most specific wins):
1. Global defaults (from user preferences)
2. Project-level overrides
3. Per-operation overrides

---

## Rust Module Structure

```
simulation/
├── mod.rs              public API: run_simulation(), OptimizationResult
├── engine.rs           top-level simulation pipeline orchestration
├── geometry_tracker.rs as-machined geometry (dexel model, MRR, engagement)
├── cutting_force.rs    mechanistic force model (Kt, Kr, Ka)
├── deflection.rs       tool beam model, surface error calculation
├── thermal.rs          heat partition, temperature accumulation
├── structural.rs       thin feature detection, workpiece FEA (beam/plate)
├── chatter.rs          stability lobe diagram computation
├── optimizer.rs        violation → toolpath modification loop
├── material_db.rs      material property loading from TOML
├── machine_model.rs    machine property loading from TOML
├── limits.rs           PhysicsLimits struct, violation checking
├── cache.rs            .sim.bin read/write
└── visualization.rs    serialize per-point data for viewport heatmaps
```

---

## Implementation Phasing

### Phase 1: Force and Deflection (foundational)

- [ ] Material database (TOML format, ~10 common materials)
- [ ] Machine model (TOML format)
- [ ] As-machined geometry tracker (dexel model)
- [ ] Layer 1: chip load and MRR per point
- [ ] Layer 2: cutting force (mechanistic model, isotropic materials)
- [ ] Layer 3: tool deflection and surface error (beam model)
- [ ] Violation detection: force, power, deflection, surface accuracy
- [ ] Feed rate optimizer: per-point feed scaling
- [ ] Spring pass generator
- [ ] Viewport heatmap overlay (force, surface error, MRR)
- [ ] Violation markers in viewport
- [ ] Simulation panel with time-series charts
- [ ] `.sim.bin` cache format

**User value:** Finishing passes have predictable accuracy. Feed rates are
automatically reduced where forces spike (entry, corners, full-width cuts).
Spring passes are suggested or auto-added where deflection exceeds tolerance.

### Phase 2: Thermal and Structural

- [ ] Layer 4: thermal model (heat partition, temperature accumulation)
- [ ] Layer 5: thin feature detection from as-machined geometry
- [ ] Layer 5: workpiece deflection (beam/plate model for simple features)
- [ ] Pass sequence optimizer (tip-before-root ordering)
- [ ] Tab mark prediction and feed rate mitigation
- [ ] Material anisotropy (wood grain direction)
- [ ] Breakage risk assessment and grain-angle tear-out
- [ ] Extended material database (woods, plastics, steels)

**User value:** Thin features are machined safely. Wood grain direction affects
toolpath sequencing automatically. Tab faces are clean.

### Phase 3: Chatter and Advanced

- [ ] Layer 6: stability lobe computation
- [ ] Chatter risk heatmap
- [ ] Spindle speed optimizer (recommend stable lobe)
- [ ] Voxel as-machined geometry (replaces dexel for multi-axis)
- [ ] Full beam/plate FEA for complex thin-wall features
- [ ] Composite material model (fiber-reinforced)
- [ ] Tool wear accumulation model (Taylor's equation)
- [ ] Remaining tool life prediction

**User value:** Chatter is predicted and avoided without manual stability lobe
charts. Tool life is tracked and replacement is predicted before failure.

---

## Relationship to Existing Architecture

The simulation engine is a new top-level module in the Rust backend. Its
position in the overall pipeline:

```
Toolpath Engine  ──►  Simulation Engine  ──►  Optimizer  ──►  Post-Processor
                              ▲                    │
                              └────────────────────┘
                              (iteration until converged)
```

**Changes to `system-architecture.md`:** The IPC command inventory gains
`run_simulation(operation_id)`, `get_simulation_data(operation_id)`, and
`apply_optimization(operation_id, actions[])`. New events:
`simulation:progress`, `simulation:complete`, `simulation:violation`.

**Changes to `project-file-format.md`:** The `cache` block gains a
`simulation_key` and `sim_binary_file` alongside the existing toolpath key.
`CacheState` tracks simulation validity separately from toolpath validity —
a toolpath recompute always invalidates the simulation; a physics limits change
invalidates the simulation but not the toolpath geometry.

**Changes to `development-roadmap.md`:** Simulation Phase 1 runs in parallel
with toolpath Phase 2 (2.5D), since the force/deflection model does not depend
on 3D surface machining capabilities.

---

*Document status: Draft*
*Related documents: `toolpath-engine.md`, `system-architecture.md`, `project-file-format.md`, `development-roadmap.md`*
