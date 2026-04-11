# JamieCam G-code Post-Processor

## Overview

The post-processor is the final stage of the CAM pipeline. It translates an
abstract `Toolpath` (positions, orientations, feed types) into the specific
G-code dialect required by a target CNC controller.

The post-processor is **data-driven** — each controller is described by a TOML
configuration file. No Rust code changes are needed to add or modify a
controller target.

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
> must not re-scale or re-compute them.

---

## Current Scope

The built-in post-processor configuration is **GRBL 1.1** (`grbl.toml`).
This is a 3-axis controller with no canned cycle support and no line numbers.
All drilling operations are expanded to explicit linear moves.

Additional controller configurations (Fanuc, LinuxCNC, Mach4, Siemens, etc.)
are deferred to a future release. The engine and TOML schema already support
the features these controllers need (line numbers, canned cycles, percent
delimiters, etc.) — only the configuration files and testing are missing.

5-axis post-processing (kinematics solver, RTCP/TCP, inverse time feed) is
also deferred. The TOML schema has placeholder fields for these but the
engine does not implement them yet.

---

## Post-Processor TOML Configuration

Each controller is one `.toml` file. Built-in files are embedded in the binary
via `include_str!()`. User-defined files live in:

- Linux:   `~/.config/jamiecam/postprocessors/`
- macOS:   `~/Library/Application Support/jamiecam/postprocessors/`
- Windows: `%APPDATA%\jamiecam\postprocessors\`

### Schema (annotated with GRBL values)

Additional optional fields exist for deferred features (5-axis kinematics,
RTCP, canned cycle codes, rotary axis limits). See `PostProcessorConfig` in
the source for the complete set.

```toml
# ── Identity ──────────────────────────────────────────────────────────────
[meta]
id          = "grbl"
name        = "GRBL 1.1"
description = "GRBL 1.1, 3-axis, metric, no canned cycles"
version     = "1.0"
author      = "JamieCam"

# ── Machine capabilities ───────────────────────────────────────────────────
[machine]
units       = "metric"     # "metric" | "imperial"
max_axes    = 3            # 3 | 4 | 5

# ── Output formatting ──────────────────────────────────────────────────────
[format]
line_numbers          = false  # GRBL: no line numbers
line_number_start     = 1
line_number_increment = 1
line_number_max       = 0
decimal_places        = 3      # 10.000
trailing_zeros        = false  # 10. not 10.000
leading_zero_suppression = false
word_separator        = " "
eol                   = "\n"
percent_delimiters    = false  # GRBL: no % delimiters
block_delete_char     = ""

# ── Axis naming ────────────────────────────────────────────────────────────
[axes]
x = "X"
y = "Y"
z = "Z"

# ── Program structure ──────────────────────────────────────────────────────
[program]
number_prefix  = ""
number         = 0
number_format  = "%d"
comment_open   = "("
comment_close  = ")"
header = [
  "G90 G94 G17",
  "G21",
]
footer = [
  "M05",
  "M09",
  "M30",
]

# ── Tool change ────────────────────────────────────────────────────────────
[tool_change]
pre = ["M05", "M09"]
command = "T{tool_number} M06"
post = ["M03 S{spindle_speed}"]
suppress_first_if_t1 = false

# ── Motion commands ────────────────────────────────────────────────────────
[motion]
rapid      = "G00"
linear     = "G01"
arc_cw     = "G02"
arc_ccw    = "G03"
arc_format = "ijk"     # "ijk" (center offsets) or "r" (radius word)
plane_xy   = "G17"
plane_xz   = "G18"
plane_yz   = "G19"

# ── Feed and speed words ───────────────────────────────────────────────────
[words]
feed         = "F"
spindle      = "S"
tool         = "T"
tool_offset  = "H"
dwell        = "P"
feed_per_min = "G94"
feed_per_rev = "G95"
inverse_time = "G93"
absolute     = "G90"
incremental  = "G91"

# ── Spindle ────────────────────────────────────────────────────────────────
[spindle]
on_cw   = "M03"
on_ccw  = "M04"
off     = "M05"
max_rpm = 0            # 0 = no limit

# ── Coolant ────────────────────────────────────────────────────────────────
[coolant]
flood = "M08"
mist  = "M07"
air   = "M07"
off   = "M09"

# ── Canned drilling cycles ─────────────────────────────────────────────────
# GRBL does not support canned cycles; all drilling is expanded to linear moves.
# Controllers that support cycles would set supported = true and define
# drill, peck, chip_break, boring_feed, etc.
[cycles]
supported = false

# ── Miscellaneous ──────────────────────────────────────────────────────────
[misc]
optional_stop = "M01"
program_stop  = "M00"
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

## Block Formatter

The block formatter builds one output line from a set of word emissions.
It enforces standard word ordering within a block and applies modal suppression.

### Word Order Within a Block

```
G___(motion)  G___(other)  X___  Y___  Z___  A___  B___  C___
I___  J___  K___  R___  F___  S___  T___  M___(coolant)  M___(spindle)
```

When line numbers are enabled (not GRBL), `N___` precedes the block.

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

---

## Arc Output

Arc moves in the toolpath are stored as `ArcMove { center, end, clockwise }`.
The post-processor converts to either IJK or R format depending on config.

**IJK format** (used by GRBL): I, J, K are the vector from the arc start
point to the arc center, in the current plane. This handles arcs of any
angle including 360°.

**R format** (available for other controllers): R is the signed radius.
Positive R: minor arc (< 180°). Negative R: major arc (> 180°). R-format
cannot represent exactly 180° arcs — these must be split or output as IJK.

---

## Drilling Expansion

GRBL does not support canned drilling cycles. When `cycles.supported = false`,
each drilling move is expanded to explicit linear moves:

```gcode
(Drill at X10 Y10, peck cycle expanded)
G00 X10 Y10
G00 Z2
G01 Z-5 F80       (peck 1)
G00 Z2             (retract)
G01 Z-10 F80       (peck 2)
G00 Z2
G01 Z-20 F80       (final depth)
G00 Z5             (clear)
```

The TOML schema supports defining canned cycle codes (G81, G83, etc.) for
controllers that accept them. This is not currently used.

---

## GRBL Output Example

```gcode
(Generated by JamieCam — Pocket_1)
G90 G94 G17
G21
(--- Tool 1: 6mm Flat Endmill ---)
M05
M09
T1 M06
M03 S10000
G00 X15 Y15
Z5
G01 Z-3 F150
X85 F500
Y85
X15
Y15
G02 X20 Y15 I2.5 J0
(Lead-out arc)
G00 Z5
(Rapid to next pass)
G00 X10 Y10
G01 Z-3 F150
...
M05
M09
M30
```

---

## User-Defined Post-Processors

Users can create `.toml` files following the same schema. The file is validated
on load and any errors are shown in the UI. Custom post-processors appear in
the post-processor selector under a "Custom" group, separate from built-ins.

---

*Document status: Draft*
*Related documents: `toolpath-engine.md`, `system-architecture.md`, `project-file-format.md`*
