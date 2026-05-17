# Phase 4 Design Decisions

Decisions made on 2026-05-17 to drive Phase 4 (Mode 2 introduction and
the cross-cutting machinery it requires). Recorded here so future work
does not relitigate them.

This document supersedes any conflicting guidance in `roadmap.md` or
`web-port-handoff.md`.

---

## 1. Phase 3 hardening: deferred

The two open Phase 3 items — bundle analysis / Three.js lazy-load and
SharedArrayBuffer / Rayon threading — are pure optimizations and block
nothing. They are explicitly deferred. Revisit only when a real
user-visible problem (startup time, simulation speed) motivates them.

---

## 2. Clipper2 delivery: `clipper2-rust` (pure-Rust port)

We adopt [`clipper2-rust`](https://crates.io/crates/clipper2-rust) — the
MatterHackers pure-Rust port of Angus Johnson's Clipper2 — as the polygon
clipping and offsetting primitive for Modes 2+.

**Why:** Compiles to `wasm32-unknown-unknown` through our existing
`wasm-pack` pipeline with no Emscripten and no C++ toolchain. Geometry
stays Rust-side, so the planner is not bottlenecked on JS↔WASM boundary
serialization (pocket clearing performs hundreds of offsets per
operation). Port has 444 tests with exact behavioral parity against the
C++ original.

**Rejected alternatives:**
- `clipper2-wasm` (npm) — would force every offset to cross the
  Rust↔JS boundary; also stale (last release ~a year ago).
- Building Clipper2 C++ into the crate via Emscripten/cc — no benefit
  over the Rust port now that the port exists.
- `i_overlay` — different algorithm, different edge-case behavior,
  much less battle-tested on adversarial CAM input.

**Risk mitigation:** `clipper2-rust` is recent and single-maintainer.
If maintenance becomes a concern, vendor the crate — it is pure Rust
with no dependencies.

---

## 3. 2D input parsers: SVG and DXF, both required

Mode 2 must accept both SVG and DXF on day one. SVG via the `usvg`
crate; DXF via the `dxf` crate. Both are pure Rust and WASM-compatible.
Implementation order between them is not constrained — they can land in
parallel or sequentially as convenient.

---

## 4. Planner geometry abstraction: stay 2D-only for now

The roadmap previously suggested a `GeometrySource` enum
(`Polygon | Heightmap | Mesh | OcctFaces`) as a forward-compatible
planner input type. **We are not doing that yet.**

The planner re-introduction takes `&[Polyline]` (or equivalent
2D-polygon-shaped types) directly. Generalization happens when Mode 4
actually needs a non-polygon input — at that point the abstraction can
be designed against a real second consumer rather than a hypothetical
one.

**Why:** Speculative abstractions cannot be tested. Our TDD convention
requires that every shape exists because at least one test drives it.

---

## 5. Mode 2 MVP scope: profile-only

The first Mode 2 ship is **profile cuts only**. End-to-end slice:

1. Import SVG or DXF.
2. Select paths to cut from the imported artwork.
3. Choose tool (see §6) and cut parameters.
4. Generate profile toolpath (offset by tool radius).
5. Simulate via existing dexel engine.
6. Export G-code (see §7).

Explicitly **out of scope** for the first Mode 2 ship:
- Pocket clearing.
- Drilling.
- Island pockets.
- Tab retention.

Each excluded operation will be added as a follow-up after profile-only
is shipped and stable.

---

## 6. Working environment: machine setups and tools

The **working environment** — workspace dimensions, machine kinematics,
post-processor selection, available tools — is saved **separately from
`.jcam` project files**, because it describes the user's CNC hardware
rather than the work being done in any particular project.

It is structured as two collections: machine setups and tools.

### Machine setups

A **machine setup** is a per-mode bundle: workspace dimensions, machine
kinematics, post-processor selection, safety parameters, etc. The user
may have several — e.g. one per physical CNC, or experimental vs.
production. Multiple setups per mode are allowed.

The complete setup collection (every setup across every mode) is saved
as a single file. A project references a setup by id; opening a project
against a missing setup is a recoverable error (prompt the user to
select or create one).

### Tools

Tools (cutter geometry, materials, recommended feeds/speeds) are
conceptually a special case of machine-setup data, but are modeled as
a **separate collection** rather than nested inside individual setups
— the same tool often fits more than one setup.

The working assumption is:

- Tools live in their own collection, logically distinct from setups.
- An **availability matrix** indicates which tools are usable on which
  setups (a 1/16" bit fits the engraver but not the big mill, and
  vice versa).

This matrix design is *provisional*. Confirm or revise when real
multi-setup use cases emerge.

**Open sub-decision:** whether the tool collection and the setup
collection share a single working-environment save file or live in
separate files. Resolve when the first writer/loader is implemented;
the data model above is unaffected either way.

### Implications for Mode 2

Mode 2 needs:
- An active machine setup (workspace bounds, post-processor).
- A way to pick a tool and validate it against the active setup's
  availability matrix.

The first Mode 2 ship can use minimal editing UIs for setups and
tools. What matters is that the data model is correct.

---

## 7. Post-processor: hardcoded GRBL only

The first Mode 2 ship emits **GRBL G-code from a hardcoded emitter**.
No config files, no machine-specific dialect support.

**Why:** GRBL covers nearly all hobbyist CNC. A pluggable post-processor
adds zero user value for the first Mode 2 ship and is a large amount
of code. Per §4, no speculative abstraction layer either — if a second
dialect is ever needed, the right shape can be designed against two
real consumers rather than guessed at now.

---

## 8. `.jcam` format: add `mode` field

The `project.json` inside a `.jcam` archive gains a required `mode`
field set at project creation. Values are short kebab-case identifiers
— e.g. `gcode-viewer` for Mode 1, something like `2d-profile` for Mode
2 (exact strings to be settled at implementation time). The mode is
immutable after creation.

**Migration:** existing `.jcam` files written before this change default
to the Mode 1 identifier on load — they could only have been produced
by Mode 1.

**Why:** The mode determines which UI surfaces, file formats, and
operations apply. Encoding it in the project file avoids ambiguous
state and lets the loader pick the right mode shell on open.

Add this field **before** any Mode 2 `.jcam` file is written.

---

## 9. 2D viewport: separate Canvas2D component

Mode 2's primary workspace is pan/zoom on the XY plane. We will
introduce a **separate `Canvas2D` component** (HTML canvas) for 2D
modes rather than reusing the existing Three.js viewport with a locked
orthographic camera.

**Why:** A native 2D canvas is lighter, simpler to interact with
(pan/zoom math is trivial), and avoids dragging the full Three.js
scene graph into a fundamentally 2D presentation. The 3D viewport
remains as-is for Mode 1 and for the 3D preview within Mode 2.

The deleted Tauri build also followed this split — `Canvas2D` as its
own component — which is evidence the boundary is natural.

---

## 10. Implementation dependencies

The decisions above describe *what* to build but not in what order.
The dependency graph between the implementation items:

```
Independent (can land in any order):
  §8  .jcam mode field
  §2  clipper2-rust integration
  §3  SVG + DXF parsers
  §6  working-environment data model
  §9  Canvas2D component

Dependent:
  §4+§5  profile toolpath generator  →  needs §2 (clipper) and §3 (paths)
                                     →  needs a Tool type, sourced either
                                        from §6 or inlined in the operation
                                        for the first slice
  §7     GRBL emitter                →  consumes toolpaths from §4+§5
```

**Type-coupling.** §3 (parser output), §5 (planner input/output), §6
(`Tool` type), and §7 (G-code emitter input) all need to agree on
shared Rust types. Whoever lands first sets the shape; later items
conform. Landing them in roughly that order avoids retroactive type
churn.

**Hard ordering rule.** §8's `mode` field must be added *before* any
Mode 2 `.jcam` file is written (restated from §8 for visibility).

---

## Status

*Document status: Decisions accepted 2026-05-17. Implementation not yet
started. Update each section as design evolves during implementation —
do not let this doc drift from reality.*

*Related documents: `roadmap.md`, `web-port-handoff.md`, `modes-overview.md`.*
