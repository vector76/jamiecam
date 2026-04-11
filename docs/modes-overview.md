# JamieCam Modes Overview

## Introduction

JamieCam is organized around seven independent modes, each addressing a
distinct class of CNC machining work. This document defines each mode in
detail: what it does, what input it accepts, which operations it provides,
how the UI is structured, and what geometry dependencies it requires.

For the high-level rationale behind the mode-based architecture, see
`application-purpose.md`.

---

## Mode Capability Matrix

| Capability | 1: G-code Viewer | 2: 2D | 3: 2.5D | 4: 3D | 5: 2+Rotary | 6: 3+Rotary | 7: 5-Axis |
|---|---|---|---|---|---|---|---|
| **Primary input** | G-code text | SVG / DXF | SVG / DXF | Heightmap / STL / STEP | SVG / DXF / Heightmap / STL / STEP | STEP / SVG / Heightmap | STEP / IGES |
| **Generates toolpaths** | No | Yes | Yes | Yes | Yes | Yes | Yes |
| **OCCT required** | No | No | No | Optional | Optional | Optional | Yes |
| **Clipper2 required** | No | Yes | Yes | Optional | Optional | Optional | Yes |
| **Linear axes** | n/a | 3 (XYZ) | 3 (XYZ) | 3 (XYZ) | 2 (XZ) | 3 (XYZ) | 3 (XYZ) |
| **Rotary axes** | n/a | 0 | 0 | 0 | 1 (A) | 1 (A or B) | 2 (A+C, B+C, or A+B) |
| **Primary UI** | 3D viewport | 2D canvas | 2D canvas | 3D viewport | 3D + unwrap | 3D viewport | 3D viewport |
| **Z depth model** | n/a | Fixed per op | Variable (medial axis) | Height field | Radial profile | Per-surface | Per-surface |

---

## Shared vs. Mode-Specific Components

**Shared across all modes:** tool library, post-processor engine, G-code
parser, dexel material removal, viewport infrastructure, project format
(`.jcam`), toolpath linking, and simulation playback.

**Mode-specific components:**

| Component | Modes |
|-----------|-------|
| SVG/DXF parser (`usvg`, `dxf` crate) | 2, 3, 5, 6 |
| Clipper2 (polygon offset/boolean) | 2, 3, 4 (optional), 5 (optional), 6 (optional), 7 |
| Medial axis / straight skeleton | 3 |
| Heightmap sampler | 4, 5, 6 |
| STL/OBJ mesh parser | 4, 5, 7 |
| OCCT geometry kernel | 4 (optional), 5 (optional), 6 (optional), 7 |
| Rotary coordinate transform | 5, 6 |
| 5-axis kinematics solver | 7 |

## Mode Relationships

- **Modes 2 and 3** share SVG/DXF parsing and Clipper2. Mode 3 extends
  Mode 2 by adding variable-depth Z from the medial axis.
- **Modes 4 and 5** both consume heightmap input through the same
  `SurfaceModel` trait. Mode 5 applies it to a cylindrical surface.
- **Modes 5 and 6** share rotary coordinate transforms and post-processor
  rotary axis configuration. Mode 6 adds a Y axis for 4-axis simultaneous.
- **Modes 6 and 7** share `CutPoint` with tool orientation vectors and can
  both consume STEP/IGES models. Mode 6 is a kinematic subset of Mode 7.

---

## Mode 1: G-code Viewer / Simulation

**What it is:** A read-only visualization mode. The user loads G-code from
any source and sees tool motion in the viewport. With a stock definition,
the dexel engine shows material removal. No toolpaths are generated.

**Use cases:** Checking programs from other CAM software, verifying
hand-edited G-code, round-trip verification, training and education.

**Input:** `.nc`, `.ngc`, `.tap`, `.gcode` files, or pasted G-code text.

**Operations:** None. Simulation controls only (play, pause, step, scrub).

**UI:** 3D viewport with G-code text panel (current line highlighted). The
user defines stock and tool geometry to enable material removal simulation.

**Dependencies:** G-code parser (ISO 6983 subset: G0/G1/G2/G3, work planes,
canned cycles expanded to motions, common M-codes), dexel engine, tool
geometry model. No OCCT, no Clipper2.

---

## Mode 2: 2D

**What it is:** 2D vector artwork (SVG/DXF) as input. The user assigns
fixed-depth cutting operations to paths or regions.

**Use cases:** Flat panel parts, signs, gaskets, PCB isolation routing,
decorative profiles, nested sheet cutting.

**Input:** SVG (parsed via `usvg`) or DXF (parsed via `dxf` crate).

**Operations:**

| Operation | Description |
|-----------|-------------|
| Profile (outside/inside/on-line) | Tool follows path boundary or center line |
| Pocket / island pocket | Clear material inside a boundary, optionally leaving islands |
| Drill | Point operations at auto-detected circle centers |
| Tab retention | Bridges to prevent parts falling free during profiling |

Depth varies per operation but is constant within each operation.

**UI:** 2D canvas (pan/zoom on XY). Operations as colored overlays. 3D
preview on demand via simple extrusion (no OCCT).

**Dependencies:** Clipper2 (tool offset, island booleans), SVG/DXF parsers.
No OCCT. Geometry pipeline: `SVG/DXF -> 2D paths -> Clipper2 offset ->
2D toolpath with Z per pass -> post-processor`.

---

## Mode 3: 2.5D

**What it is:** Same 2D vector artwork input as Mode 2, but toolpaths are
3D. A V-bit descends to a depth that varies with local shape width. Narrow
features are shallow; wide features are deep. The V-bit walls meet the
surface exactly at the artwork boundary.

**Use cases:** Sign lettering, decorative carving, inlay work, relief
carving, paint-fill/epoxy-fill decorative work.

**Input:** SVG, DXF, or traced raster images (bitmap-to-vector pre-process).

**Operations:**

| Operation | Description |
|-----------|-------------|
| Standard V-carve | V-bit traces medial axis. `depth = (w/2) / tan(alpha/2)` |
| Flat-bottom V-carve | Flat endmill clears wide centers; V-bit carves edges |
| Inlay V-carve | Female pocket + male inlay piece from same artwork |
| Paint-fill V-carve | Standard V-carve with metadata flag for filled preview |
| Relief carve | V-carve outlines, then pocket-clear the background |
| All Mode 2 operations | Profile, pocket, drill, tabs also available |

**V-carve geometry:** The tool traces the **medial axis** (skeleton) of
each shape -- the locus of centers of all maximal inscribed circles. Depth
at each point is `(w/2) / tan(alpha/2)` where `w` is local width and
`alpha` is the V-bit included angle.

**UI:** 2D canvas (same as Mode 2) with 3D preview showing the V-carved
surface, generated analytically from the swept V-bit profile.

**Dependencies:** Clipper2 (progressive inward offset for medial axis
approximation, plus Mode 2 polygon ops), SVG/DXF parsers. No OCCT.

**Key algorithm:** Medial axis via progressive Clipper2 inward offset
(initial). Straight skeleton for exact results (future enhancement).

---

## Mode 4: 3D

**What it is:** 3-axis surface machining where all tool access is from the
top (Z+ direction). Input can be a heightmap, an STL mesh, or a STEP solid
model. The key constraint is 3-axis only -- no undercuts.

**Use cases:** Topographic maps, lithophanes, artistic relief from photos,
texture carving, portrait carving, 3D surface machining of solid models.

**Input:** Heightmap (PNG/TIFF grayscale, 16-bit/32-bit RAW), STL mesh, or
STEP solid model. For heightmaps, user specifies physical size, Z depth
range, and invert flag. For STL/STEP, the model is used directly. OCCT is
required only for STEP import.

**Operations:**

| Operation | Description |
|-----------|-------------|
| Parallel (raster) | Constant-direction scan. Most common. |
| Scallop | Variable step-over for uniform surface finish |
| Roughing passes | Coarse passes before finishing for deep reliefs |

All operations use ball-nose or tapered ball-nose tools, sampling Z at each
XY position via bilinear interpolation (heightmap) or ray casting (mesh).

**UI:** 3D viewport. Heightmaps rendered as `THREE.PlaneGeometry` with Z
displacement. STL/STEP models rendered as triangle meshes.

**Dependencies:** Heightmap sampler and/or mesh sampler via `SurfaceModel`
trait (`sample_z`, `normal_at`, `bounds`). OCCT optional (STEP import only).
Clipper2 optional (Z-level roughing on mesh/STEP models). The `SurfaceModel`
trait abstracts over heightmaps, meshes, and OCCT faces, allowing shared
algorithms.

---

## Mode 5: 2+Rotary

**What it is:** Machine has X (along stock), Z (radial depth), and A
(rotation around X). No independent Y -- rotation replaces it. For turning
cylindrical objects on a router/mill with rotary attachment.

**Use cases:** Table legs, balusters, rolling pins, fluted columns,
cylindrical signs, cam profiles.

**Coordinate model:** G-code axes X, Z, A. Feed rate expressed as mm/min
(surface speed), deg/min (angular), or rev/min x feed/rev (lathe-style).

**Input:** SVG/DXF artwork (unwrapped onto cylinder surface), 2D profile
curve (revolved), cylindrical heightmap, STL mesh, or STEP solid model.
OCCT is required only for STEP import.

**Operations:** Roughing (profile), finishing (profile), fluting
(longitudinal/helical channels), cylindrical relief (heightmap wrapped on
cylinder), indexing (standard 2D ops on unwrapped surface).

**UI:** 3D viewport showing cylindrical workpiece + unwrapped 2D view for
artwork placement. Toolpaths shown on both views.

**Dependencies:** Rotary coordinate transform (XZ+angle to Cartesian),
heightmap sampler (same `SurfaceModel` as Mode 4), SVG/DXF parsers. OCCT
optional (STEP import only). Clipper2 optional (SVG/DXF offset ops).
Post-processor uses `[rotary]` config section (axis, feed mode).

---

## Mode 6: 3+Rotary

**What it is:** Four simultaneous axes: X, Y, Z + one rotary (A or B).
Unlike Mode 5 where rotation replaces Y, here it adds an orientation DOF.
The rotary axis moves continuously during cutting.

**Use cases:** Cam lobes, spiral flutes, impeller blades (simplified),
complex turned forms, port machining, multi-sided milling.

**How it differs from Mode 5:**

| Aspect | Mode 5 (X, Z, A) | Mode 6 (X, Y, Z, A) |
|--------|-------------------|----------------------|
| Linear axes | 2 | 3 |
| Rotary purpose | Replaces Y | Adds orientation DOF |
| Typical stock | Cylindrical | Any shape |
| G-code | X, Z, A | X, Y, Z, A |

**Input:** STEP/IGES (optional, requires OCCT), SVG/DXF (surface wrapping),
or heightmap.

**Operations:** Standard 3D surface operations (parallel, scallop, flowline)
extended with tool axis tilt around the rotary axis. A subset of 5-axis
capability with simpler kinematics: `A = atan2(-ix, iz)`.

**UI:** 3D viewport with rotary axis visualization and indexing mode for
multi-sided work.

**Dependencies:** Rotary coordinate transform (shared with Mode 5), OCCT
(optional), Clipper2 (optional for indexed faces), SVG/DXF parsers.

---

## Mode 7: 5-Axis

**What it is:** All five axes move simultaneously: X, Y, Z + two rotary
(A+C, B+C, or A+B). Tool can be positioned and oriented arbitrarily. The
most capable and complex mode.

**Use cases:** Aerospace components, impellers/blisks, complex molds/dies,
medical implants, parts requiring undercut access.

**Input:** STEP (.stp/.step), IGES (.igs/.iges), STL/OBJ (mesh only).

**Operations:** Z-level roughing, adaptive clearing, Z-level finishing,
parallel finishing, scallop finishing, flowline finishing, pencil milling,
swarf milling, multi-setup (multiple WCS for different part faces).

**Multi-setup:** Each setup is a group of operations sharing a WCS. Setup
types: flip (180 deg), 90-degree rotation, custom orientation, compound
angle. Post-processor generates one file per setup or combined with `M00`
stops. Alignment via reference features (datum holes/faces).

**UI:** 3D viewport only. Face selection assigns geometry to operations.
Multiple setups visible simultaneously or isolated.

**Dependencies:** OCCT (required: import, B-rep, tessellation, surface
queries), Clipper2 (Z-level pocket passes), 5-axis kinematics solver.

**Kinematics configurations:**

| Config | Axes | Examples |
|--------|------|----------|
| Table-table | A + C | DMG MORI, Haas UMC |
| Head-table | B + C | Hermle, Mazak |
| Head-head | A + C | Robot arms, gantry mills |

See `toolpath-engine.md` and `gcode-postprocessor.md` for full detail.

---

## Mode Selector and Project Behavior

When the user creates a new project, a mode selection dialog determines
which file formats, panels, operation types, and post-processor settings
are available. The mode is stored in `project.json` and **cannot be changed
after creation**. Projects in simpler modes can optionally be upgraded
one-way to a more complex mode.

---

## Operations by Mode

| Operation | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|-----------|---|---|---|---|---|---|---|
| Profile (outside/inside/on-line) | | x | x | | | | x |
| Pocket / island pocket | | x | x | | | x | x |
| Drill | | x | x | | | | x |
| Tab retention | | x | x | | | | |
| V-carve (standard/flat-bottom/inlay) | | | x | | | | |
| Relief carve | | | x | | | | |
| Parallel (raster) | | | | x | | x | x |
| Scallop finishing | | | | x | | x | x |
| Roughing (height field) | | | | x | | | |
| Roughing / finishing (profile) | | | | | x | | |
| Fluting | | | | | x | | |
| Cylindrical relief | | | | | x | | |
| Z-level roughing / finishing | | | | | | x | x |
| Adaptive clearing | | | | | | x | x |
| Parallel / flowline finishing | | | | | | x | x |
| Pencil milling | | | | | | x | x |
| Swarf milling | | | | | | | x |
| Multi-setup | | | | | | | x |

---

*Document status: Draft*
*Related documents: `application-purpose.md`, `toolpath-engine.md`, `gcode-postprocessor.md`, `project-file-format.md`, `shared-engine-design-choices.md`*
