# JamieCam G-code Post-Processor

## Overview

The post-processor is the final stage of the CAM pipeline. It translates an
abstract `Toolpath` (positions, orientations, feed types) into the specific
G-code dialect required by a target CNC controller.

Every controller has its own quirks: different syntax for the same concept,
different modal group rules, different handling of arcs, different 5-axis
conventions. The post-processor is **data-driven** — each controller is
described by a TOML configuration file. No Rust code changes are needed to
add or modify a controller target.

```
Toolpath (abstract)
        │
        ▼
┌───────────────────────────────────────────┐
│  Post-Processor Engine (Rust)             │
│                                           │
│  ┌──────────────┐   ┌──────────────────┐  │
│  │ Config       │   │ Modal State      │  │
│  │ (TOML)       │   │ Tracker          │  │
│  └──────┬───────┘   └────────┬─────────┘  │
│         │                   │             │
│  ┌──────▼───────────────────▼─────────┐  │
│  │  Block Formatter                   │  │
│  │  • word emission with suppression  │  │
│  │  • number formatting               │  │
│  │  • template substitution           │  │
│  └──────────────────┬─────────────────┘  │
│                     │                    │
│  ┌──────────────────▼─────────────────┐  │
│  │  Kinematics Solver (5-axis only)   │  │
│  │  • tool vector → A/B/C angles      │  │
│  │  • singularity handling            │  │
│  └──────────────────┬─────────────────┘  │
│                     │                    │
│  ┌──────────────────▼─────────────────┐  │
│  │  Program Assembler                 │  │
│  │  • header / footer                 │  │
│  │  • tool change sequences           │  │
│  │  • block ordering                  │  │
│  └────────────────────────────────────┘  │
└───────────────────────────────────────────┘
        │
        ▼
G-code text (written to .nc / .ngc / .tap file)
```

> **Pipeline position note:** The post-processor receives the *physics-optimized*
> `Toolpath` produced by the simulation optimizer — not the raw geometric toolpath
> from the toolpath engine. Feed rates at this point are final; the post-processor
> must not re-scale or re-compute them. Spring passes (`PassKind::SpringPass`) appear
> in the toolpath as ordinary finishing passes and are annotated in the output with
> a descriptive comment.

---

## Post-Processor TOML Configuration

Each controller is one `.toml` file. Built-in files are embedded in the binary
via `include_str!()`. User-defined files live in:

- Linux:   `~/.config/jamiecam/postprocessors/`
- macOS:   `~/Library/Application Support/jamiecam/postprocessors/`
- Windows: `%APPDATA%\jamiecam\postprocessors\`

### Full Schema with Annotations

```toml
# ── Identity ──────────────────────────────────────────────────────────────
[meta]
id          = "fanuc-0i"          # unique identifier, used internally
name        = "Fanuc 0i-MD"       # display name shown in UI
description = "Generic Fanuc 0i Mill-Turn, metric"
version     = "1.0"
author      = "JamieCam"

# ── Machine capabilities ───────────────────────────────────────────────────
[machine]
units       = "metric"     # "metric" | "imperial"
max_axes    = 3            # 3 | 4 | 5
# 5-axis machine type (only relevant when max_axes = 5):
#   "head_head"   — both rotary axes in the spindle head  (e.g. A+C head)
#   "head_table"  — one rotary in head, one in table       (e.g. B head + C table)
#   "table_table" — both rotary axes in the table          (e.g. A+C table)
five_axis_type = "head_table"

# ── Output formatting ──────────────────────────────────────────────────────
[format]
line_numbers          = true   # emit N-words (N10, N20, ...)
line_number_start     = 10
line_number_increment = 10
line_number_max       = 9999   # wrap around when exceeded (0 = never wrap)
decimal_places        = 3      # 10.000
trailing_zeros        = false  # true: 10.000   false: 10.
leading_zero_suppression = false  # true: .5   false: 0.5
word_separator        = " "    # character between words in a block
eol                   = "\r\n" # "\n" for Linux/Mac controllers, "\r\n" for Windows/Fanuc
percent_delimiters    = true   # emit % at start and end of file (Fanuc standard)
block_delete_char     = "/"    # optional: prefix for block-delete lines (empty = unused)

# ── Axis naming ────────────────────────────────────────────────────────────
[axes]
x = "X"
y = "Y"
z = "Z"
a = "A"    # rotary around X axis
b = "B"    # rotary around Y axis
c = "C"    # rotary around Z axis

[axes.limits]              # software limits (used by kinematics solver)
a_min = -120.0
a_max =  120.0
b_min =  -90.0
b_max =  120.0
c_min = -9999.0            # effectively unlimited (continuous rotation)
c_max =  9999.0

# ── Program structure ──────────────────────────────────────────────────────
[program]
number_prefix = "O"        # Fanuc: "O1000",  Siemens: "%_N_",  empty = omit
number        = 1000
number_format = "%04d"     # printf-style format for the number part
comment_open  = "("        # Fanuc/LinuxCNC: "(",   Siemens: ";"
comment_close = ")"        # empty for line-comment style

# Lines emitted after the program number, before any operations
header = [
  "G90 G94 G17",           # absolute coords, feed/min, XY plane
  "G21",                   # metric (G20 for imperial)
  "G28 G91 Z0.",           # incremental home Z
  "G90",                   # back to absolute
]

# Lines emitted after all operations
footer = [
  "M05",                   # spindle off
  "G28 G91 Z0.",           # home Z
  "G90",
  "G28 X0. Y0.",           # home XY
  "M30",                   # program end + rewind
]

# ── Tool change ────────────────────────────────────────────────────────────
[tool_change]
# Lines before the T-word (retract, spindle off)
pre = [
  "G28 G91 Z0.",
  "G90",
  "M05",
]
# The tool-change block. Template variables: {tool_number}, {tool_diameter},
# {tool_description}
command = "T{tool_number:02} M06"

# Lines after the T-word (TLO, spindle on, approach)
# Template variables: {tool_number}, {spindle_speed}, {coolant}
post = [
  "G43 H{tool_number:02}",
  "M03 S{spindle_speed}",
]

# Whether to suppress the tool change block when the first operation uses T1
# and no prior tool is loaded (machine powers up with no tool)
suppress_first_if_t1 = false

# ── Motion commands ────────────────────────────────────────────────────────
[motion]
rapid        = "G00"
linear       = "G01"
arc_cw       = "G02"
arc_ccw      = "G03"

# Arc format: "ijk" (center offsets from start) or "r" (radius word)
# "ijk" is preferred — R-format cannot represent 180° arcs
arc_format   = "ijk"

# Plane selection codes (for arc interpretation)
plane_xy     = "G17"
plane_xz     = "G18"
plane_yz     = "G19"

# ── Feed and speed words ───────────────────────────────────────────────────
[words]
feed         = "F"
spindle      = "S"
tool         = "T"
tool_offset  = "H"
dwell        = "P"    # dwell time parameter word

# Feed rate mode
feed_per_min   = "G94"
feed_per_rev   = "G95"
inverse_time   = "G93"    # used for 5-axis simultaneous moves on some controllers

# Distance mode
absolute      = "G90"
incremental   = "G91"

# ── Spindle ────────────────────────────────────────────────────────────────
[spindle]
on_cw        = "M03"
on_ccw       = "M04"
off          = "M05"
orient       = "M19"     # optional: orient spindle for tool change
max_rpm      = 15000     # clamp spindle speed (0 = no limit)

# ── Coolant ────────────────────────────────────────────────────────────────
[coolant]
flood        = "M08"
mist         = "M07"
air          = "M07"
off          = "M09"
through_tool = "M88"    # optional: through-spindle coolant

# ── Canned drilling cycles ─────────────────────────────────────────────────
[cycles]
supported    = true     # false: expand all cycles to explicit point moves
drill        = "G81"    # spot / through drill
peck         = "G83"    # peck drilling (full retract between pecks)
chip_break   = "G73"    # chip-breaking (partial retract)
boring_feed  = "G85"    # bore in, bore out at feed
boring_dwell = "G86"    # bore in, dwell, rapid out
reaming      = "G85"
tapping      = "G84"    # right-hand tapping
tapping_ccw  = "G74"    # left-hand tapping
cycle_cancel = "G80"
r_plane_abs  = "G98"    # return to initial Z after cycle
r_plane_r    = "G99"    # return to R-plane after cycle (between holes)
# Q-word: peck depth.  R-word: R-plane height.  P-word: dwell time (ms).

# ── Miscellaneous ──────────────────────────────────────────────────────────
[misc]
optional_stop  = "M01"
program_stop   = "M00"
```

---

## Template Variables

Template strings (used in `tool_change.command`, `tool_change.post`, etc.)
support the following variables. The format specifier after `:` follows
`printf` conventions.

| Variable | Type | Description |
|---|---|---|
| `{tool_number}` | int | Tool number (1-indexed) |
| `{tool_number:02}` | int | Tool number, zero-padded to 2 digits |
| `{tool_diameter}` | float | Tool diameter in current units |
| `{tool_description}` | string | Tool name from library |
| `{spindle_speed}` | int | Spindle speed (RPM), clamped to max |
| `{feed_rate}` | float | Current feed rate |
| `{program_number}` | int | The program number |
| `{date}` | string | Date at output time (ISO 8601) |
| `{filename}` | string | Output filename without extension |

---

## Rust Data Structures

```rust
/// Loaded from a TOML file. Fully describes one controller.
#[derive(Debug, Deserialize)]
pub struct PostProcessorConfig {
    pub meta:         MetaConfig,
    pub machine:      MachineConfig,
    pub format:       FormatConfig,
    pub axes:         AxesConfig,
    pub program:      ProgramConfig,
    pub tool_change:  ToolChangeConfig,
    pub motion:       MotionConfig,
    pub words:        WordsConfig,
    pub spindle:      SpindleConfig,
    pub coolant:      CoolantConfig,
    pub cycles:       CycleConfig,
    pub misc:         MiscConfig,
}

/// Tracks what G-code modes are currently active.
/// Suppresses output of words that haven't changed.
#[derive(Debug, Default)]
struct ModalState {
    motion_mode:   Option<String>,   // G00 / G01 / G02 / G03 / G8x
    feed_rate:     Option<f64>,
    spindle_speed: Option<f64>,
    tool_number:   Option<u32>,
    coolant:       CoolantMode,
    coord_system:  String,           // G54 / G55 / ...
    distance_mode: DistanceMode,     // Absolute / Incremental
    feed_mode:     FeedMode,         // PerMin / PerRev / InverseTime
    plane:         Plane,            // XY / XZ / YZ
}

/// A single G-code output block (one line).
#[derive(Debug, Default)]
struct Block {
    line_number:  Option<u32>,
    words:        Vec<Word>,
    comment:      Option<String>,
}

#[derive(Debug)]
struct Word {
    letter: char,
    value:  WordValue,
}

#[derive(Debug)]
enum WordValue {
    Code(String),        // G01, M03  — emitted as-is
    Float(f64),          // X, Y, Z, F values — formatted per config
    Int(i64),            // S, T, H, P values
}
```

---

## Block Formatter

The block formatter builds one output line from a set of word emissions.
It enforces standard word ordering within a block and applies modal suppression.

### Word Order Within a Block

```
N___  /  G___(motion)  G___(other)  X___  Y___  Z___  A___  B___  C___
      I___  J___  K___  R___  F___  S___  T___  M___(coolant)  M___(spindle)
```

Standard word order matters for some controllers even if the spec says it
shouldn't. The formatter always emits in the order above.

### Modal Suppression Rules

| Word | Suppressed when |
|---|---|
| Motion code (G00/G01/etc.) | Same as last emitted motion code |
| F (feed rate) | Same value as last emitted, and motion code hasn't changed |
| S (spindle speed) | Same value as last emitted |
| G90/G91 | Same mode as currently active |
| G17/G18/G19 | Same plane as currently active |
| G94/G95 | Same feed mode as currently active |

Coordinate words (X, Y, Z, A, B, C) are suppressed when their value is
identical to the last emitted value for that axis. A tolerance of 1e-6 mm
is used for floating-point equality.

### Number Formatting

```rust
fn format_coordinate(value: f64, cfg: &FormatConfig) -> String {
    let places = cfg.decimal_places;
    let s = format!("{:.prec$}", value, prec = places);

    // Strip trailing zeros if configured
    let s = if !cfg.trailing_zeros {
        s.trim_end_matches('0').trim_end_matches('.')
    } else { &s };

    // Leading zero suppression: ".5" instead of "0.5"
    if cfg.leading_zero_suppression && s.starts_with("0.") {
        s[1..].to_string()
    } else {
        s.to_string()
    }
}
```

---

## Arc Output

Arc moves in the toolpath are stored as `ArcMove { center, end, clockwise }`.
The post-processor converts to either IJK or R format depending on config.

### IJK Format

I, J, K are the vector from the **arc start point** to the **arc center**,
in the current plane. This handles arcs of any angle including 360°.

```rust
fn format_arc_ijk(
    start: Vec3, center: Vec3, end: Vec3,
    clockwise: bool, cfg: &PostProcessorConfig,
) -> Block {
    let i = center.x - start.x;
    let j = center.y - start.y;
    let k = center.z - start.z;

    let motion = if clockwise {
        &cfg.motion.arc_cw
    } else {
        &cfg.motion.arc_ccw
    };

    // Build block: G02/G03 X__ Y__ Z__ I__ J__ K__ F__
    // ...
}
```

### R Format

R is the signed radius. Positive R: minor arc (< 180°). Negative R: major arc (> 180°).
R-format cannot represent exactly 180° arcs — these must be split into two 90° arcs
or output as IJK.

```rust
fn format_arc_r(
    start: Vec3, center: Vec3, end: Vec3,
    clockwise: bool,
) -> Block {
    let radius = (center - start).magnitude();
    // Determine sign: if arc sweeps > 180°, use negative R
    let angle = arc_sweep_angle(start, center, end, clockwise);
    let r = if angle > std::f64::consts::PI { -radius } else { radius };
    // ...
}
```

---

## 5-Axis Kinematics

For 5-axis toolpaths, each `CutPoint` has an `orientation: Vec3` (the tool axis
direction). The post-processor must convert this to machine rotary axis angles
(A, B, C) using the machine's kinematic model.

This is machine-specific. Three kinematic families are supported:

### Table-Table (A-C configuration)

Both rotary axes are in the table. The tool always points straight down (+Z in
machine coordinates), but the **part** rotates under it. Most common on vertical
machining centers with a tilting rotary table.

```
Given tool axis vector (i, j, k) in workpiece coordinates (Z-up):

C = atan2(-j, -i)           (rotation of table around Z)
A = atan2(sqrt(i² + j²), k) (tilt of table around X)
```

Machine X, Y, Z positions must be transformed by the inverse of the table
rotation to produce the correct commanded positions.

### Head-Head (B-C configuration)

Both rotary axes are in the spindle head. The part stays fixed.

```
Given tool axis vector (i, j, k):

B = atan2(-i, k)             (head tilt around Y)
C = atan2(j, sqrt(i² + k²)) (head rotation around Z)
```

### Head-Table (B head + C table — common on 5-axis mills)

Head tilts (B axis) and table rotates (C axis). Partial transform required.

```
C = atan2(-j, -i)
B = atan2(sqrt(i² + j²), k)
```

Position compensation depends on the pivot length from spindle nose to B-axis
center (the RTCP/TCPM offset), which is a machine parameter.

### RTCP / TCP

**RTCP (Rotational Tool Center Point):** The controller compensates for the
pivot distance automatically, requiring only tool tip position + orientation.
Many modern controllers support this (Fanuc: G43.4/G43.5, Heidenhain: M128,
Siemens: TRAORI).

```toml
[five_axis]
rtcp_supported = true
rtcp_on  = "G43.4 H{tool_number}"   # Fanuc RTCP on
rtcp_off = "G49"
pivot_length = 150.0   # mm, spindle nose to rotary center
                       # (only needed when rtcp_supported = false)
```

When RTCP is supported, the post-processor emits raw (X, Y, Z, A, B/C) values —
the controller handles the pivot compensation internally.

When RTCP is **not** supported, the post-processor applies inverse kinematics
and pivot compensation itself to compute the corrected (X, Y, Z) positions.

### Singularity Handling

When the tool axis is exactly parallel to Z (vertical), the C (or A) axis angle
is undefined — this is a gimbal singularity. The post-processor:

1. Detects approach to singularity (within 0.5° of vertical)
2. Freezes the rotary axis at its last commanded value rather than
   jumping to an arbitrary angle
3. Emits a comment flagging the singularity for operator awareness

```
(SINGULARITY REGION: C axis frozen at last position)
```

---

## Program Structure Output

### Complete Fanuc Output Example

```gcode
%
O1000
(Generated by JamieCam — Pocket_1 + Contour_1)
(Simulation: max force 214 N | max deflection 0.012 mm | 1 spring pass added)
(Tool 1: 10mm 4-flute flat endmill)
N10 G90 G94 G17
N20 G21
N30 G28 G91 Z0.
N40 G90
(--- Tool 1: 10mm Flat Endmill ---)
N50 G28 G91 Z0.
N60 G90
N70 M05
N80 T01 M06
N90 G43 H01
N100 M03 S5000
N110 G00 X15. Y15.
N120 Z5.
N130 G01 Z-3. F150.
N140 X85. F500.
N150 Y85.
N160 X15.
N170 Y15.
N180 G02 X20. Y15. I2.5 J0.
(Lead-out arc)
N190 G00 Z5.
(Rapid to next pass)
N200 G00 X10. Y10.
N210 G01 Z-3. F150.
...
N9980 M05
N9990 G28 G91 Z0.
N9995 G90
N9996 G28 X0. Y0.
N9999 M30
%
```

### Comment Style by Controller

| Controller | Comment style | Example |
|---|---|---|
| Fanuc | Parentheses | `(this is a comment)` |
| LinuxCNC | Parentheses | `(this is a comment)` |
| Siemens 840D | Semicolon | `; this is a comment` |
| Mach4 | Parentheses | `(this is a comment)` |
| GRBL | Parentheses | `(this is a comment)` |
| Heidenhain | Semicolon | `; this is a comment` |

### Automatic Pass Annotations

The post-processor inserts comments at the start of certain pass types to aid
operator review:

```gcode
(Spring pass — removing deflection error)
N510 G01 X15. Y10. F600.
N520 X85.
```

These comments are suppressed when `include_comments = false` in `GenerateOptions`.

---

## Canned Cycle Expansion

When `cycles.supported = true`, drilling operations emit native canned cycles:

```gcode
(Drill 12 holes, 6mm dia, 20mm deep)
N100 G00 X10. Y10.
N110 G83 Z-20. R2. Q5. F80.   (peck cycle: Q = peck depth)
N120 X30.
N130 X50.
...
N200 G80                        (cancel cycle)
```

When `cycles.supported = false` (e.g., GRBL), each drilling move is expanded to
explicit linear moves:

```gcode
(Drill at X10 Y10, expanded)
N100 G00 X10. Y10.
N110 G00 Z2.
N120 G01 Z-5. F80.    (peck 1)
N130 G00 Z2.          (retract)
N140 G01 Z-10. F80.   (peck 2)
N150 G00 Z2.
N160 G01 Z-20. F80.   (final depth)
N170 G00 Z5.          (clear)
```

---

## Built-in Post-Processors

### Included at Launch (Phase 1)

| File | Controller | Notes |
|---|---|---|
| `fanuc-0i.toml` | Fanuc 0i-MD/MF | 3-axis, standard G-code, canned cycles |
| `linuxcnc.toml` | LinuxCNC 2.x | 3-axis, open-source controller |
| `mach4.toml` | Mach4 Mill | 3-axis, hobbyist/small-shop |
| `grbl.toml` | GRBL 1.1 | 3-axis, no canned cycles, no line numbers |

### Phase 2 (3D Surface Ops milestone)

| File | Controller | Notes |
|---|---|---|
| `fanuc-30i.toml` | Fanuc 30i/31i/32i | 5-axis, RTCP (G43.4) |
| `siemens-840d.toml` | Siemens 840D sl | 5-axis, TRAORI, different syntax |
| `centroid.toml` | Centroid Acorn | 3-axis, popular mid-range |

### Phase 3 (5-axis milestone)

| File | Controller | Notes |
|---|---|---|
| `heidenhain-tnc640.toml` | Heidenhain TNC 640 | Requires extended template engine; Heidenhain uses conversational + DIN/ISO hybrid |
| `okuma-osp.toml` | Okuma OSP-P300 | 5-axis |
| `mazak-mazatrol.toml` | Mazak Mazatrol | Conversational — significant engine extension needed |

> **Note on Heidenhain and Mazatrol:** These controllers use programming formats
> that deviate significantly from standard G-code (modal codes, coordinates, etc.).
> Supporting them requires extending the template engine beyond what the TOML config
> format covers. They are scoped to Phase 3 and may require a separate code path.

---

## Engine Module Structure

```
postprocessor/
├── mod.rs              public API: PostProcessor struct, run() method
├── config.rs           PostProcessorConfig deserialization from TOML
├── modal.rs            ModalState — tracks active G-code modes
├── block.rs            Block, Word, BlockBuilder — format one output line
├── formatter.rs        number formatting, template substitution
├── arcs.rs             IJK and R arc format computation
├── kinematics.rs       5-axis: tool vector → A/B/C axis angles
├── cycles.rs           canned cycle emission and expansion
├── program.rs          full program assembly: header, ops, footer
└── builtins/           embedded TOML files (include_str!)
    ├── fanuc-0i.toml
    ├── linuxcnc.toml
    ├── mach4.toml
    └── grbl.toml
```

### Public API

```rust
pub struct PostProcessor {
    config: PostProcessorConfig,
}

impl PostProcessor {
    /// Load from an embedded built-in by ID (e.g. "fanuc-0i").
    pub fn builtin(id: &str) -> Result<Self, PostProcessorError>;

    /// Load from a user-provided TOML file path.
    pub fn from_file(path: &Path) -> Result<Self, PostProcessorError>;

    /// List all available built-in post-processors.
    pub fn list_builtins() -> Vec<PostProcessorMeta>;

    /// Generate G-code for one or more toolpaths in program order.
    /// Returns the complete G-code as a String.
    pub fn generate(
        &self,
        toolpaths: &[&Toolpath],
        tool_library: &ToolLibrary,
        options: &GenerateOptions,
    ) -> Result<String, PostProcessorError>;
}

pub struct GenerateOptions {
    pub program_number:  Option<u32>,     // override TOML default
    pub output_units:    Option<Units>,   // override TOML default
    pub split_by_tool:   bool,            // one file per tool change
    pub include_comments: bool,           // suppress all comments
}
```

---

## Validation

The post-processor validates the config on load and catches common errors:

| Check | Error |
|---|---|
| Template variable `{tool_number}` in `command` | Error if missing |
| `arc_format = "r"` with `max_axes = 5` | Warning: R-format unreliable for 5-axis arcs |
| `cycles.supported = true` but no `drill` code defined | Error |
| `five_axis_type` defined but `max_axes < 5` | Warning: ignored |
| `rtcp_supported = true` but no `rtcp_on` template | Error |

Validation errors and warnings are returned as structured data and displayed
in the UI before the user can use the post-processor.

---

## Testing Strategy

### Unit Tests (per module)

- `formatter.rs`: number formatting edge cases (negative, zero, very small)
- `modal.rs`: suppression logic for each modal group
- `arcs.rs`: IJK values for known circle geometry; sign of R for major/minor arcs
- `kinematics.rs`: known tool vectors → expected A/B/C values per kinematics type

### Integration Tests (per built-in)

Each built-in post-processor has a set of golden output tests:
a fixed input `Toolpath` is post-processed and the output is compared
byte-for-byte against a checked-in `.nc` file. Changes to the engine that
alter output require deliberate golden file updates.

```
tests/
└── postprocessor/
    ├── fanuc-0i/
    │   ├── simple_pocket.toolpath.json   (serialized input)
    │   └── simple_pocket.nc              (expected output)
    ├── linuxcnc/
    │   └── ...
    └── grbl/
        └── ...
```

### Round-Trip Simulation Test

For controllers with straightforward G-code (Fanuc, LinuxCNC), the generated
G-code is parsed back by a minimal G-code interpreter (in-process) that tracks
tool position. The resulting position sequence is compared against the original
toolpath points. Deviations > chord tolerance flag a failure.

This catches: wrong arc direction, missing modal context, incorrect IJK signs.

---

## User-Defined Post-Processors

Users can create `.toml` files following the same schema. The file is validated
on load and any errors are shown in the UI with line numbers.

Custom post-processors appear in the post-processor selector under a "Custom"
group, separate from built-ins. They can be edited from within the app
(opens the file in the system's default text editor via `tauri::api::shell::open`).

---

*Document status: Draft*
*Related documents: `toolpath-engine.md`, `system-architecture.md`, `project-file-format.md`*
