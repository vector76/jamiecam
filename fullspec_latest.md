# Full Spec: Phase 2 — Entry Motions + Multi-Level Profile

_Scope estimate: ~8–10 hours of human developer time_
_Prerequisite: Phase 1 complete, Phase 2 partial (Z-level roughing, viewport
standard views/projection/display mode all done)_

---

## What This Scope Covers

This bite completes the **linking and entry motion infrastructure** for Phase 2,
and extends the **Profile operation to multi-level (step-down) cutting**. These
are naturally grouped: entry motions are shared linking infrastructure used by
Profile, Pocket, and Z-Level Roughing alike.

Concretely, the deliverables are:

1. **Multi-level profile** — extend the existing Profile operation to step down
   through multiple Z depths rather than cutting at a single depth.
2. **Arc lead-in / lead-out** — circular arc approach and departure motions at
   the start and end of each cutting pass.
3. **Helical entry** — spiral descent into closed pockets (replaces the current
   fixed-Z plunge).
4. **Ramp entry** — linear angled descent for open slots and profiles.

These four features together cover all the Phase 2 "linking / entry" deliverables
from `docs/development-roadmap.md` (see the Phase 2 Infrastructure section). The
remaining Phase 2 items (arc fitting, hole auto-detection, drill sort, canned
cycles, adaptive clearing, 3D contour finishing, rest machining) are explicitly
**out of scope** for this bite.

---

## Motivation

The current Phase 1 linking module (`src-tauri/src/toolpath/linking.rs`) only
supports a fixed-Z retract with a straight vertical plunge. This is fine for
demonstration but produces poor results on real parts:

- Plunging straight down into material is hard on tooling.
- Profile operations cut at a single depth only — no way to machine a wall to
  full depth in steps.
- No smooth arc transitions at the start/end of passes increases corner loads.

This scope makes the operations usable for actual 2.5D machining on real material.

---

## Detailed Scope

### 1. Multi-Level Profile

**What:** Add a `stepdown: f64` parameter to `ProfileParams`. When `stepdown > 0`,
the profile pass is repeated at `Z = top, top - stepdown, top - 2*stepdown, ...`
down to `depth`. The final pass is always at the full depth. When `stepdown == 0`
(or omitted / backward-compat default), behavior is identical to Phase 1.

**Where:**
- `src-tauri/src/models/operation.rs` — add `stepdown: Option<f64>` to
  `ProfileParams`; default `None` = single-level (backward compatible).
- `src-tauri/src/toolpath/operations/profile.rs` — loop over Z levels using the
  same integer-step pattern established in `zlevel_roughing.rs`.
- `src/api/types.ts` — add `stepdown?: number` to `ProfileParams` interface.
- `src/components/OperationEditorForm.tsx` — add stepdown input to Profile form
  (disabled / hidden = single level, enabled = multi-level).

**Tests:**
- Rust unit: single-level path (stepdown absent) is unchanged.
- Rust unit: multi-level produces correct number of passes and correct Z values.
- Rust unit: floor depth always machined even when depth not a multiple of stepdown.
- Golden test update: existing `profile_golden.rs` fixture is unchanged (stepdown
  absent → single level).

---

### 2. Arc Lead-In / Lead-Out

**What:** At the start of each cutting pass, approach the material with a circular
arc tangent to the cut direction rather than a direct linear plunge. Symmetrically,
depart with an arc at the end of the pass.

**Parameters** (added to `LinkingParams` or a new `EntryExitParams` struct):
- `arc_lead_in_radius: Option<f64>` — radius of the lead-in arc. `None` = no arc
  (current behavior).
- `arc_lead_out_radius: Option<f64>` — radius of the lead-out arc.

**Geometry:** A quarter-circle arc is generated in the XY plane, tangent to the
first (or last) cutting segment, at the cutting Z depth. The arc starts from a
point outside the material boundary and sweeps into the start of the cut.

**Where:**
- `src-tauri/src/toolpath/linking.rs` — new helper
  `arc_approach(start_pt, direction, radius, z) -> Vec<Move>` generating arc
  move points; called when `arc_lead_in_radius` is set.
- The arc is represented as a sequence of linear moves (chord approximation at
  ≤0.01 mm chordal error) for now — arc fitting to G2/G3 is a later scope item.
- `LinkingParams` struct updated; `wrap_pass()` calls arc helper when radius
  is set.

**Tests:**
- Arc moves appear in linked output when radius is set.
- No arc moves when radius is `None`.
- Arc start point is outside the stock boundary.
- Arc end point equals the first (last) cut point.

---

### 3. Helical Entry

**What:** Instead of plunging straight down before a pocket pass, descend on a
helix to the cutting depth. Avoids full-width engagement at entry depth.

**Parameters** (added to `LinkingParams`):
- `helical_entry_radius: Option<f64>` — radius of the helix. `None` = plunge
  (current behavior).
- `helical_entry_pitch: f64` — Z descent per full revolution (default: tool
  diameter / 3 is reasonable).

**Geometry:** Starting at the retract height above the pocket center (or
boundary centroid), generate a helical spiral descending to cutting depth.
Each revolution is approximated as N linear segments (chord error ≤ 0.01 mm).
At the bottom, a full-diameter arc cleans up the helix floor before the first
pocket pass begins.

**Where:**
- `src-tauri/src/toolpath/linking.rs` — new helper
  `helical_descent(center, radius, pitch, z_start, z_end) -> Vec<Move>`.
- `wrap_pass()` calls helical descent when `helical_entry_radius` is set, replacing
  the current `RapidZ` + `PlungeZ` sequence.
- `LinkingParams` updated.

**Tests:**
- Helical descent moves span from retract height to cut depth.
- Z decreases monotonically.
- XY positions trace a circle of the specified radius.
- Falls back to straight plunge when radius is `None`.
- Helical pitch parameter controls number of revolutions.

---

### 4. Ramp Entry

**What:** Descent along a linear angled path (ramp) for open slots and profile
operations. The tool descends from retract height to cut depth while traversing
forward along the first cutting segment.

**Parameters** (added to `LinkingParams`):
- `ramp_entry_angle_deg: Option<f64>` — ramp angle in degrees from horizontal
  (e.g. 3°). `None` = plunge (current behavior).

**Geometry:** The ramp starts at the retract height above the start of the first
cutting segment and descends at the specified angle until reaching cut depth, at
which point normal cutting continues. If the first segment is too short to
accommodate the full ramp descent at the given angle, the ramp is shortened (the
entry angle becomes steeper) and a warning is logged.

**Where:**
- `src-tauri/src/toolpath/linking.rs` — new helper
  `ramp_descent(start_xy, end_xy, z_start, z_end, angle_deg) -> Vec<Move>`.
- `wrap_pass()` calls ramp descent when `ramp_entry_angle_deg` is set for profile
  passes (ramp does not apply to closed pockets; helical is used there).
- `LinkingParams` updated.

**Tests:**
- Ramp moves span from retract height to cut depth.
- Z decreases along the ramp linearly.
- Ramp length is consistent with angle and depth.
- Falls back to plunge when angle is `None`.
- Short-segment warning path covered.

---

## What Is Not In Scope

The following Phase 2 items are explicitly deferred to future bites:

- Arc fitting (chord detection → G2/G3 emission in post-processor)
- Hole auto-detection (`cg_shape_find_holes`)
- Drill sorting (nearest-neighbor)
- Canned cycle emission and expansion
- Adaptive (trochoidal) clearing
- 3D contour / Z-level finishing
- Rest machining

---

## Acceptance Criteria

By the end of this scope, the following must all pass:

1. **Multi-level profile:** Load a flat STEP part; create a Profile operation with
   `stepdown=2` and `depth=8`; Calculate — toolpath has 4 passes at Z=-2, -4, -6,
   -8. G-code exported to LinuxCNC matches expected step-down structure.

2. **Arc lead-in/out:** Enable `arc_lead_in_radius=3` on a Profile operation;
   Calculate — the first move of each pass is a curved approach, not a plunge.
   The linked toolpath in the viewport shows the arc approach.

3. **Helical entry:** Enable `helical_entry_radius=2` on a Pocket operation;
   Calculate — entry into the pocket is a helix, not a straight plunge. Verify in
   viewport: the lead-in path spirals downward.

4. **Ramp entry:** Enable `ramp_entry_angle_deg=3` on a Profile operation;
   Calculate — entry is a ramp along the first cut segment, not a plunge. Z starts
   at retract height and reaches cut depth at the ramp end point.

5. **All tests pass:** `cargo test` (Rust, full suite) and `pnpm test` (frontend)
   pass with no regressions. The golden file for the single-level profile is
   unchanged.

---

## Key Files

See `docs/system-architecture.md`, `docs/toolpath-engine.md`, and
`docs/viewport-design.md` for broader context.

Rust:
- `src-tauri/src/toolpath/linking.rs` — primary change surface for entry motions
- `src-tauri/src/toolpath/operations/profile.rs` — multi-level profile
- `src-tauri/src/models/operation.rs` — param structs
- `src-tauri/src/toolpath/types.rs` — `LinkingParams` struct

Frontend:
- `src/api/types.ts` — TypeScript interface updates
- `src/components/OperationEditorForm.tsx` — UI for new params

Tests:
- `src-tauri/src/toolpath/linking.rs` (inline unit tests)
- `src-tauri/src/toolpath/operations/profile.rs` (inline unit tests)
- `src-tauri/tests/profile_golden.rs` (golden — should be unchanged)
