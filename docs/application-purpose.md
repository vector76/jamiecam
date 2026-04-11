# JamieCam Application Purpose

## What Is JamieCam?

JamieCam is a cross-platform CAM (Computer-Aided Manufacturing) application
that generates G-code toolpaths for CNC machines. It takes input geometry --
vector artwork, heightmaps, or solid models -- and produces machine-ready
cutting programs through an interactive graphical interface with real-time
3D visualization and material removal simulation.

The application is built with Tauri (Rust backend, React/Three.js frontend)
and runs natively on Linux, macOS, and Windows.

---

## Mode-Based Architecture

JamieCam is organized around **seven independent modes**, each addressing a
distinct class of CNC work. A mode determines what input the user provides,
which operations are available, and how the workspace looks and behaves.

Modes are almost like separate applications that happen to share
infrastructure. This design reflects two realities:

1. **CNC work is not one problem.** Cutting a 2D sign from SVG artwork has
   almost nothing in common with 5-axis machining of an impeller from a STEP
   file. Forcing both into a single linear workflow creates complexity that
   serves neither user.

2. **Complexity should match the task.** A hobbyist cutting wooden signs
   should never encounter OCCT import dialogs or 5-axis tilt options. Each
   mode exposes only what its users need.

The modes form a complexity gradient from simple (Mode 1: G-code viewer) to
advanced (Mode 7: full 5-axis), matching the distribution of the CNC user
base -- most hobbyist and small-shop work is 2D or 2.5D.

---

## The Seven Modes

| # | Mode | Summary |
|---|------|---------|
| 1 | **G-code Viewer / Simulation** | Load G-code from any source and visualize tool motion with material removal simulation. No CAM generation -- purely a verification tool. |
| 2 | **2D** | 2D vector artwork (SVG/DXF) as input; fixed-depth operations like profile, pocket, and drill. Primary workspace is a 2D pan/zoom canvas. |
| 3 | **2.5D** | Same 2D artwork input but uses V-bits and variable-depth Z movements to carve shapes whose depth varies with local feature width. For signs, lettering, and inlay work. |
| 4 | **3D** | Heightmaps, STL meshes, or STEP solid models as input. Parallel raster and scallop passes carve a 3D surface reachable from the top only (3-axis). For topographic maps, lithophanes, relief carving, and 3D surface machining. |
| 5 | **2+Rotary** | Tool moves along X and Z while the workpiece rotates around the X axis. Accepts SVG/DXF, heightmaps, STL, or STEP. Lathe-like operations for cylindrical objects such as table legs, balusters, and fluted columns. |
| 6 | **3+Rotary** | XYZ linear motion plus one rotary axis moving simultaneously. Enables both lathe-style and multi-sided milling on a 4-axis machine. |
| 7 | **5-Axis** | Full arbitrary position and orientation with two rotary axes. STEP/IGES solid models as input. The complete OCCT geometry kernel is required. |

Each mode is implemented and shipped independently. Simpler modes do not
wait for complex modes to be ready.

---

## Target Users

JamieCam targets a range from hobbyist CNC operators to professional
machinists:

- **Hobbyists and makers** using benchtop CNC routers for signs, artwork,
  and small parts (Modes 1-4)
- **Small-shop machinists** doing production 2D/2.5D work and multi-setup
  3-axis milling (Modes 2-4, 6)
- **Professional machinists** running 4-axis and 5-axis machines on solid
  models (Modes 5-7)

The mode architecture means each user sees an interface matched to their
work. A sign maker using Mode 2 has a focused 2D workspace. A 5-axis
machinist using Mode 7 has the full solid-model pipeline.

---

## Shared Infrastructure

Six subsystems are shared across all modes, forming a narrow common core:

| Subsystem | Purpose |
|-----------|---------|
| **Tool library** | Define and manage cutting tools with physical geometry (diameter, flute length, profile shape). Every mode uses cutting tools. |
| **Post-processor engine** | Convert internal toolpaths to controller-specific G-code. Data-driven via TOML configuration files. Built-in GRBL configuration; additional controller configs planned for future releases. |
| **G-code parser** | Read G-code from any source and produce structured motion data. Powers Mode 1 (viewer) and round-trip verification for all other modes. |
| **Dexel material removal** | Track workpiece shape as cuts are applied. Powers simulation visualization and rest-machining calculations across all modes. |
| **Viewport** | Three.js-based 3D visualization with toolpath display, tool animation, simulation playback, and measurement overlays. Adapts to each mode's UI paradigm (2D canvas, 3D scene, or both). |
| **Project format** | `.jcam` ZIP archive containing a JSON project definition, optional embedded model, and cached toolpath/simulation data. |

Each mode plugs into these shared subsystems but has its own geometry
pipeline, operation set, and UI layout. The shared infrastructure is
designed from the start to serve all seven modes.

---

## Technology Stack

| Layer | Technology |
|-------|------------|
| Application shell | Tauri 2.x (native window, IPC bridge, file dialogs, packaging) |
| Backend | Rust (Tokio async runtime, Rayon for parallel computation) |
| Frontend framework | React 19 with TypeScript |
| 3D rendering | Three.js via WebGL |
| State management | Zustand (frontend); RwLock-guarded AppState (backend) |
| Geometry kernel | OpenCASCADE (OCCT) via C wrapper + Rust FFI (Modes 5-7 only) |
| 2D geometry | Clipper2 for polygon offsetting and boolean operations |
| Build system | Cargo + npm/pnpm + Vite + CMake (for OCCT/C++ components) |

The architecture maintains a strict separation: the Rust backend owns all
data and computation; the frontend is a view and input layer only. All
communication crosses the Tauri IPC bridge through typed wrapper functions.

---

## What Exists Today

The application has a working foundation including: the Tauri app shell,
OCCT build system and Rust FFI, Three.js viewport with toolpath
visualization and simulation playback, `.jcam` file I/O, tool library,
stock and WCS management, operations framework, post-processor engine
(GRBL configuration), G-code export, and a comprehensive set of CAM
algorithms (pocket, profile, drill, Z-level roughing, adaptive clearing,
Z-level finishing, parallel finishing, scallop finishing, flowline finishing,
pencil milling). Supporting features include gouge detection, arc fitting,
measurement overlays, and toolpath LOD.

---

*Document status: Draft*
*Related documents: `modes-overview.md`, `system-architecture.md`, `technology-stack.md`, `shared-engine-design-choices.md`*
