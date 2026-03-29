# G-code Parser

## Purpose

The existing post-processor (`src-tauri/src/postprocessor/`) generates G-code text
from internal `Toolpath` data. This feature builds the **inverse**: a parser that
reads G-code text and produces structured motion data.

This enables:

- **G-code visualization** — render tool motion in the viewport from any G-code
  source, not just internally generated toolpaths. Users can load G-code from
  other CAM software, hand-written programs, or post-processor output and see
  exactly what the machine will do.
- **Cut simulation input** — a material removal engine needs a sequence of
  positioned tool motions. The parser provides this from G-code regardless of
  how the G-code was produced.
- **Round-trip verification** — post-process an internal toolpath to G-code,
  parse it back, and compare the resolved positions against the original. This
  catches post-processor bugs that would be invisible in text-only golden tests.

The parser lives in a new `src-tauri/src/gcode_parser/` module (or similar —
the implementer chooses the location). It has no dependency on the post-processor
module and no dependency on OCCT or the geometry kernel.

---

## Input and Output

### Input

A `&str` containing a complete G-code program. The parser handles:
- Programs with or without `%` delimiters
- Programs with or without program numbers (`O1234`)
- Any line ending (`\n`, `\r\n`, `\r`)
- Mixed case (`G01` and `g01` are equivalent)

### Output: `ParsedProgram`

```
ParsedProgram
├── metadata: ProgramMetadata
│   ├── program_number: Option<u32>
│   ├── source_units: Units (Metric | Imperial)  // what the program declared
│   └── header_comments: Vec<String>
├── segments: Vec<MotionSegment>
├── tool_changes: Vec<ToolChange>
└── warnings: Vec<ParseWarning>
```

**`MotionSegment`** — one resolved, fully-positioned tool motion:

| Variant | Fields | Description |
|---------|--------|-------------|
| `Rapid` | `start: Vec3, end: Vec3` | G0 — traverse at machine max rate |
| `Linear` | `start: Vec3, end: Vec3, feed_rate: f64` | G1 — feed move |
| `Arc` | `start: Vec3, end: Vec3, center: Vec3, clockwise: bool, plane: Plane, feed_rate: f64` | G2/G3 — circular interpolation |
| `Dwell` | `position: Vec3, seconds: f64` | G4 — timed pause |

Every segment carries metadata:

| Field | Type | Description |
|-------|------|-------------|
| `source_line` | `usize` | Line number in the original G-code (1-based) |
| `tool_number` | `u32` | Active tool at the time of this motion |
| `spindle_speed` | `f64` | Active spindle RPM |
| `spindle_dir` | `SpindleDir` | `Cw` (M3), `Ccw` (M4), or `Off` (M5) |

**`ToolChange`** — marks where tool changes occur in the segment stream:

| Field | Type | Description |
|-------|------|-------------|
| `segment_index` | `usize` | Index into `segments` where this tool starts |
| `tool_number` | `u32` | The new tool number |

**`ParseWarning`** — non-fatal issues:

| Field | Type | Description |
|-------|------|-------------|
| `line` | `usize` | Source line number |
| `message` | `String` | Human-readable description |

**`Plane`** enum: `Xy` (G17), `Xz` (G18), `Yz` (G19).

All positions in `MotionSegment` are **absolute machine coordinates in
millimeters**. If the program uses inches (G20), the parser converts
coordinates to mm (× 25.4) as they are parsed. This ensures all output
segments use a single consistent unit regardless of what the G-code declares.
Feed rates are similarly normalized to mm/min. If the program switches units
mid-stream (G21 → G20 or vice versa), the conversion tracks the current mode.

The parser does not apply work offsets (G54–G59) or tool length compensation
(G43) — these are machine-specific transformations that the consumer can
apply if needed.

---

## G-code Support Matrix

### Motion Codes (Group 1 — modal)

| Code | Meaning | Notes |
|------|---------|-------|
| G0 / G00 | Rapid positioning | No F-word; uses machine max rate |
| G1 / G01 | Linear interpolation | Requires active F-word |
| G2 / G02 | Circular CW | IJK or R format |
| G3 / G03 | Circular CCW | IJK or R format |

### Plane Selection (Group 2 — modal)

| Code | Plane | Arc axes | Linear axis |
|------|-------|----------|-------------|
| G17 | XY | I, J | K (helical) |
| G18 | XZ | I, K | J (helical) |
| G19 | YZ | J, K | I (helical) |

### Distance Mode (Group 3 — modal)

| Code | Meaning |
|------|---------|
| G90 | Absolute coordinates |
| G91 | Incremental coordinates |

### Units (Group 6 — modal)

| Code | Meaning |
|------|---------|
| G20 | Inches |
| G21 | Millimeters |

### Canned Cycles (Group 9 — modal)

| Code | Cycle | Behavior |
|------|-------|----------|
| G81 | Simple drill | Rapid to R, feed to Z, rapid retract |
| G82 | Drill + dwell | Like G81 but dwells at bottom |
| G83 | Peck drill (full retract) | Peck to depth, retract to R between pecks |
| G73 | Peck drill (chip break) | Peck with partial retract |
| G80 | Cancel canned cycle | |

Canned cycle parameters: `R` (retract plane Z), `Z` (final depth), `Q` (peck
increment), `P` (dwell time in seconds — same convention as G4), `F` (feed
rate), `L` (repeat count, default 1).

When a canned cycle is active, each subsequent block containing X and/or Y
coordinates triggers the full cycle motion at that position. The parser
**expands** canned cycles into explicit `MotionSegment` sequences (Rapid to R,
Linear to depth, Rapid retract, etc.) so consumers never see canned cycle
abstractions — only resolved motions.

### Feed Mode (Group 5 — modal)

| Code | Meaning |
|------|---------|
| G93 | Inverse time feed |
| G94 | Feed per minute (default) |
| G95 | Feed per revolution |

The parser records the active feed mode but always stores `feed_rate` as the
raw F-word value. Consumers that need mm/min must convert G93/G95 values
themselves (they need spindle speed and segment length, which the parser
provides).

### Other G-codes (recognized, not geometrically interpreted)

| Code | Meaning | Parser behavior |
|------|---------|-----------------|
| G4 | Dwell | Emits `Dwell` segment with P-word as seconds |
| G28 | Return to reference | Treated as rapid to intermediate then home position. Since home position is machine-specific, emit a warning and treat as rapid to the intermediate point if specified. |
| G43 / G49 | Tool length comp on/off | Noted in state; not applied to coordinates |
| G54–G59 | Work offsets | Noted in state; not applied to coordinates |
| G40–G42 | Cutter compensation | Noted; not applied. Emit warning — external cutter comp means the G-code coordinates are not the true tool path. |
| G98 / G99 | Canned cycle retract mode | Controls whether retract goes to initial Z (G98) or R-plane (G99) |

### M-codes

| Code | Meaning | Parser behavior |
|------|---------|-----------------|
| M0 / M1 | Program stop / optional stop | No motion effect |
| M2 / M30 | Program end | Terminates parsing |
| M3 | Spindle CW | Updates `spindle_dir` to `Cw` |
| M4 | Spindle CCW | Updates `spindle_dir` to `Ccw` |
| M5 | Spindle stop | Updates `spindle_dir` to `Off` |
| M6 | Tool change | Emits `ToolChange` record |
| M8 / M9 | Coolant on/off | No motion effect |

### Word Parsing

A G-code word is a letter followed by a number: `G01`, `X-12.5`, `F1500.0`.

The parser must handle:
- Integer and decimal values: `G0`, `G00`, `G0.0`
- Negative values: `X-5.25`
- Leading/trailing zeros: `X.5` (= 0.5), `X5.` (= 5.0), `X005.250`
- Suppressed leading zeros: `.5` as a coordinate value
- Multiple G-words on one line: `G90 G17 G21` (common in setup lines)
- Multiple M-words on one line: `M3 M8`
- Line numbers: `N10 G01 X5 Y3` — the N-word is metadata, not motion
- Comments: `(text)` inline or `;text` to end of line
- Blank lines and whitespace-only lines: skip silently

---

## Modal State Machine

The parser maintains a modal state that persists across lines:

```
ModalState {
    motion_mode:   Option<MotionMode>   // G0, G1, G2, G3
    plane:         Plane                // G17 (default)
    distance_mode: DistanceMode         // G90 (default)
    feed_mode:     FeedMode             // G94 (default)
    units:         Units                // G21 (default — see design choices)
    feed_rate:     f64                  // mm/min (or in/min)
    spindle_speed: f64                  // RPM
    spindle_dir:   SpindleDir           // Off
    tool_number:   u32                  // 0
    position:      Vec3                 // (0, 0, 0)

    // Canned cycle state
    cycle_active:  Option<CycleMode>    // None
    cycle_r:       f64                  // R-plane Z
    cycle_q:       f64                  // peck increment
    cycle_p:       f64                  // dwell seconds
    retract_mode:  RetractMode          // G98 (initial point)
    initial_z:     f64                  // Z before cycle activated
}
```

**Key modal behaviors:**

1. **Motion mode persists.** A line with only `X10 Y20` (no G-word) uses the
   last active motion mode. If no motion mode has been set, emit a warning
   and treat as G1.

2. **Coordinates persist.** A `G1 X10` line moves only X; Y and Z remain at
   their current values. In incremental mode (G91), missing axes have zero
   increment.

3. **Feed rate persists.** Once set by an F-word, it remains active until
   changed. A G1 move with no F-word and no prior F-word is a warning.

4. **Tool change resets nothing.** Unlike the post-processor (which re-emits
   all modal codes after tool change for safety), the parser does not reset
   modal state on M6. The G-code is the authority — if it re-emits G90 after
   a tool change, the parser accepts it; if it doesn't, the prior mode persists.

---

## Arc Resolution

Arcs (G2/G3) in G-code come in two formats. The parser must handle both and
normalize to center-point representation.

### IJK Format (center offsets)

```gcode
G02 X10 Y0 I0 J-5 F200
```

- I, J, K are **incremental offsets** from the arc start point to the arc center
  (this is the most common convention and the one used by the project's
  post-processor; some controllers use absolute IJK — see design choices).
- Center = start + (I, J, K)
- The parser computes the center, then stores it as an absolute position.

### R Format (radius)

```gcode
G02 X10 Y0 R5 F200
```

- Positive R = minor arc (sweep ≤ 180°)
- Negative R = major arc (sweep > 180°)
- The parser computes the center from start, end, and R, then stores it.
- A full circle (start == end) cannot be represented in R format — emit warning.

### Validation

After computing the center, the parser should verify that `|center - start|`
and `|center - end|` are approximately equal (within a tolerance — suggest
0.01mm). If they diverge significantly, emit a warning (the G-code may have
rounding errors or be malformed) but still produce the segment using the
computed center.

### Helical Arcs

When an arc block includes a linear axis (e.g., G17 arc with a Z-word), the
motion is helical — circular in the arc plane with simultaneous linear motion.
The parser stores the full 3D start and end positions; the `plane` field tells
consumers which two axes form the circular component.

---

## Error Handling Philosophy

The parser is **lenient by default**. The goal is to extract as much useful
motion data as possible from real-world G-code, which is often imperfect:

- **Unrecognized G-codes** (e.g., G43.1, G68): skip with warning.
- **Unrecognized M-codes** (e.g., M98 subprogram call): skip with warning.
- **Malformed lines** (no parseable words): skip with warning.
- **Missing feed rate on G1**: use last known feed rate; warn if none.
- **Missing coordinates**: use current position (modal behavior).
- **Arc radius mismatch**: warn but emit segment with best-effort center.
- **Subprogram calls (M98/M99, O-word labels)**: not expanded. Warn that
  subprograms are not supported; parse only the main program.
- **Expressions and variables** (`#100 = 5.0`, `[#100 + 1]`): not evaluated.
  Warn and skip lines containing brackets or `#`.

The parser should **never panic** and should **never return an error that
prevents all output**. Even if the first 50 lines are garbage, if line 51
starts making sense, the parser should produce segments from line 51 onward.

---

## Integration Points

### Existing Golden Fixtures

The `tests/fixtures/` directory contains `.nc` files generated by the
post-processor:

- `adaptive_clearing_golden.nc`
- `parallel_finishing_golden.nc`
- `scallop_finishing_golden.nc`
- `flowline_finishing_golden.nc`
- `pencil_milling_golden.nc`

These are ideal parser test inputs. They exercise: program structure (`%`, O-word,
header), rapids (G0/G00), linear feeds (G1/G01), arcs (G2/G3 with IJK), tool
changes (T/M6), spindle control (M3/M5/S), and coordinate formatting (suppressed
zeros, variable decimal places).

### Post-Processor Config

The parser does **not** need a post-processor config to parse G-code. It handles
the standard ISO 6983 word set directly. This is intentional — the parser must
work on G-code from any source, not just G-code produced by our post-processor.

### Existing Types

The parser should use the existing `Vec3` from `models::stock` for positions.
The `Plane` enum is new to the parser (the post-processor has plane codes but
no `Plane` enum). `MotionSegment` is a new type — it is deliberately different
from the toolpath's `CutPoint`/`MoveKind` because it represents resolved
machine motion, not CAM-generated cutting strategy.

### IPC Command

A Tauri command should expose the parser to the frontend:

```rust
#[tauri::command]
async fn parse_gcode(gcode: String) -> Result<ParsedProgram, AppError>
```

This allows the frontend to load a `.nc` file (via the existing file dialog),
send its contents to the backend, and receive structured motion data for
visualization.

---

## Test Strategy

### Unit Tests

- **Word parsing**: Individual words (`G01`, `X-5.25`, `F1500`, `(comment)`,
  `N10`) → correct letter + value extraction.
- **Modal persistence**: Multi-line program where motion mode, feed rate, and
  position carry forward correctly.
- **Incremental mode**: G91 program → verify absolute positions are correctly
  accumulated.
- **Arc resolution**: IJK and R format arcs → correct center computation.
  Test CW and CCW. Test in all three planes (G17/G18/G19). Test helical arcs.
- **Canned cycle expansion**: G81 with three XY positions → 9 segments
  (3 holes × 3 motions each). G83 peck drill → correct peck/retract sequence.
- **Warning generation**: Unrecognized codes, missing feed rates, malformed
  lines → warnings with correct line numbers.

### Integration Tests

- **Golden file round-trip**: Parse each golden `.nc` fixture. Verify segment
  count is reasonable, first/last positions match expected values, all arcs
  have valid centers.
- **Post-processor round-trip**: Generate G-code from a known `Toolpath` via
  the post-processor, parse it back, compare resolved positions against the
  original `CutPoint` positions. Tolerance: 0.001mm (accounts for coordinate
  formatting precision loss).

### Edge Cases

- Empty program → empty segments, no errors.
- Program with only comments → empty segments, no errors.
- G-code with Windows line endings (`\r\n`) → parses correctly.
- G-code with no spaces between words (`G01X5Y3Z-1F200`) → parses correctly.
- Line with multiple G-words from different groups (`G90 G01 G17 X5`) →
  all modal states updated, one motion segment emitted.
- Tool change mid-program → segment metadata reflects new tool number.
