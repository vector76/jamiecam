# JamieCam System Architecture

## Process Model

JamieCam runs as two operating-system processes connected by Tauri's IPC bridge.
The WebView process owns all UI and rendering. The Rust Core process owns all data
and computation. This separation is strict -- the frontend holds no canonical state.

The application is organized as a **shared core** with **mode-specific extensions**.
Seven modes exist (G-code viewer, 2D, 2.5D, 3D, 2+rotary, 3+rotary, 5-axis),
each behaving almost as a separate application. The shared core provides the tool
library, post-processor, G-code parser, dexel simulation engine, viewport shell,
project format, and IPC bridge. Each mode supplies its own geometry pipeline,
operation set, and UI panels.

OCCT is an **optional** dependency. Mode 7 (5-axis) requires it. Modes 4-6
use it optionally for STEP file import. Modes 1-3 have no OCCT dependency.
The application can be built and run for modes 1-3 without OCCT.

---

## State Ownership

### Rust: Canonical State

All persistent, computable state lives in Rust inside a `tauri::State`-managed
`AppState`. Fields are guarded by `RwLock` so multiple read commands can run
concurrently while mutations are exclusive. The actual struct lives in `state.rs`
-- read it for the current shape.

The frontend never derives machining data from its own logic; it holds display
copies received from Rust plus UI-only state (camera, selection, panel visibility).

---

## Mode Selection and UI Adaptation

When the user creates a new project, a mode selection dialog presents the seven
modes. The selected mode is stored in the project and cannot be changed after
creation. The mode determines:

- **File formats** in the import dialog (G-code for mode 1, SVG/DXF for modes 2-3,
  heightmap/STL/STEP for mode 4, SVG/DXF/heightmap/STL/STEP for mode 5,
  STEP/SVG/DXF/heightmap/STL for mode 6, STEP/IGES for mode 7)
- **Available operations** in the operation picker
- **UI panels**: mode 2 shows a 2D canvas primary; modes 4-7 show 3D viewport;
  mode 1 shows G-code text alongside 3D visualization
- **Post-processor settings**: rotary axis configuration for modes 5-6
- **Viewport behavior**: 2D-primary vs 3D-primary, cylindrical unwrap for mode 5

Projects can optionally be **upgraded** from a simpler mode to a more complex one
(e.g., 2D to 5-axis after adding a solid model). This is one-way and explicit.

---

## IPC Pattern

All frontend-to-Rust calls go through the typed wrapper module `src/api/`.
Commands return `Promise<T>` on the frontend; `Result<T, AppError>` in Rust.
Commands follow the `_inner` + thin `#[tauri::command]` wrapper pattern.

The actual command set is defined in `src-tauri/src/commands/` -- read the
source for the current inventory. Events emitted from Rust to the frontend
are defined inline in the command handlers (search for `.emit(`).

---

## Geometry Pipelines by Mode

```
Mode 1 (G-code viewer):    .nc --> G-code parser --> MotionSegments --> viewport
                            No toolpath generation. Feeds directly to simulation.

Modes 2-3 (2D / 2.5D):    SVG/DXF --> usvg/dxf parser --> Vec<Polyline>
                            --> Clipper2 (offset, boolean) --> 2D toolpath
                            Mode 3 adds medial axis for V-carve depth

Mode 4 (3D):               PNG/TIFF --> HeightmapGrid | STL --> Mesh
                            | STEP --> OCCT B-rep (optional)
                            --> SurfaceModel trait --> parallel/scallop passes

Mode 5 (2+Rotary):         SVG/DXF --> usvg/dxf parser | Heightmap | STL
                            | STEP --> OCCT B-rep (optional)
                            --> rotary coordinate transform --> toolpath

Modes 6-7 (3+Rotary/5ax):  STEP/IGES --> OCCT B-rep --> face selection
                            --> SurfaceModel trait --> 3D/multi-axis operations
```

Data is transferred to the frontend as flat typed arrays (Float32Array for
vertices/normals, Uint32Array for indices) to avoid JSON serialization overhead
and map directly to WebGL buffer uploads.

---

## Threading Model

```
Main thread (Tauri event loop)
|
+-- invoke("calculate_toolpath") received
|     +-- Tokio task spawned
|     invoke() returns JobId immediately (frontend not blocked)
|
|     Tokio async task:
|       1. Acquire RwLock read, clone inputs, release lock
|       2. Rayon parallel computation (CPU-bound, all cores)
|       3. Emit progress events
|       4. Acquire RwLock write, store result, release lock
|       5. Emit complete event
```

**Rules:**
- IPC handlers are always `async fn` -- never block the dispatch thread
- CPU-bound work goes to Rayon via `tokio::task::spawn_blocking`
- Inputs are cloned before lock release; computation runs lock-free
- Only the result write requires a write lock, and it is brief

---

## Error Handling

All Rust commands return `Result<T, AppError>`. `AppError` is adjacently-tagged
(`#[serde(tag = "kind", content = "message")]`). On the frontend, `src/api/`
uses the `toAppError()` pattern. Internal errors are logged via `tracing`;
critical errors are surfaced to the user via toast.

**Rule:** No `unwrap()` or `expect()` in command handlers. All fallible paths
return `Result`.

---

## Cross-Cutting Concerns

**Logging:** Rust uses `tracing` (file + stderr); frontend uses `console.*`.
IPC logged at DEBUG in dev, suppressed in release.

**Configuration:** User preferences in TOML (OS config dir). Post-processor
configs embedded via `include_str!()`, with user-addable custom configs.

**Conditional compilation:** OCCT is behind a Cargo feature flag. Builds for
modes 1-3 can exclude OCCT entirely, simplifying CI and reducing binary size.

---

*Document status: Draft*
*Related documents: `technology-stack.md`, `viewport-design.md`, `toolpath-engine.md`, `modes-overview.md`*
