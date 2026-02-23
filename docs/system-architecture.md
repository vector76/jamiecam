# JamieCam System Architecture

## Process Model

JamieCam runs as two operating-system processes connected by Tauri's IPC bridge.
The WebView process owns all UI and rendering. The Rust Core process owns all data
and computation. This separation is strict — the frontend holds no canonical state.

```
┌─────────────────────────────────────────────────────────────────────┐
│  OS PROCESS 1: WebView                                              │
│                                                                     │
│  ┌─────────────────────────┐   ┌─────────────────────────────────┐ │
│  │   React UI              │   │   Three.js Viewport             │ │
│  │   - Panels / dialogs    │   │   - Model mesh                  │ │
│  │   - Operation editor    │   │   - Toolpath lines              │ │
│  │   - Tool library        │   │   - Stock / fixture             │ │
│  │   - G-code viewer       │   │   - Tool animation              │ │
│  └────────────┬────────────┘   └──────────────┬──────────────────┘ │
│               │  Zustand shared state          │                    │
│               └───────────────┬────────────────┘                   │
│                    src/api/   │  typed invoke() wrappers           │
└───────────────────────────────┼─────────────────────────────────────┘
                                │
                    ╔═══════════╧═══════════╗
                    ║   Tauri IPC Bridge    ║
                    ║  invoke()  /  emit()  ║
                    ╚═══════════╤═══════════╝
                                │
┌───────────────────────────────┼─────────────────────────────────────┐
│  OS PROCESS 2: Rust Core      │                                     │
│                               │                                     │
│            ┌──────────────────▼──────────────────┐                 │
│            │  Command Dispatcher (Tauri handlers) │                 │
│            └──────────────────┬──────────────────┘                 │
│                               │                                     │
│   ┌───────────┬───────────────┼───────────────┬──────────────┐     │
│   │           │               │               │              │     │
│   ▼           ▼               ▼               ▼              ▼     │
│ File I/O  Project          Geometry       Toolpath      Post-      │
│ & Parse   State (RwLock)   Kernel FFI     Engine        Processor  │
│           (truth)          (OCCT)         (Rayon)       (TOML)     │
│                                                                     │
│            ┌────────────────────────────────────────────────┐      │
│            │  Tokio async runtime  /  Rayon thread pool     │      │
│            └────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## State Ownership

### Rust: Canonical State

All persistent, computable state lives in Rust inside a `tauri::State`-managed
`AppState`. Access is guarded by an `RwLock` so multiple read commands can run
concurrently while mutations are exclusive.

```
AppState
├── project: Project
│   ├── source_model: Option<LoadedModel>
│   │   ├── path, checksum
│   │   ├── b_rep (OCCT handle)
│   │   └── tessellation (mesh for display)
│   ├── stock: StockDefinition
│   ├── wcs: WorkCoordinateSystem
│   ├── tool_library: Vec<Tool>
│   ├── operations: Vec<Operation>   ← ordered list
│   └── toolpaths: HashMap<OperationId, Toolpath>
└── preferences: UserPreferences
    ├── recent_files
    ├── default_post_processor
    └── ui_settings
```

### Frontend: UI and Display State

The frontend (Zustand store) holds only:
- Which operation is selected / being edited
- Viewport camera position and mode
- Active panel / dialog state
- Display copies of data received from Rust (Three.js geometry, etc.)

The frontend never derives machining data from its own logic. Every piece of
machining information is fetched from or confirmed by Rust.

---

## IPC Command Inventory

All frontend→Rust calls go through the typed wrapper module `src/api/`.
Commands return `Promise<T>` on the frontend; `Result<T, AppError>` in Rust.

### File & Project Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `open_model` | `path: string` | `MeshData` | Loads + tessellates model |
| `new_project` | — | `ProjectSummary` | Clears AppState |
| `save_project` | `path: string` | — | Serializes to `.jcam` |
| `load_project` | `path: string` | `ProjectSnapshot` | Restores full project |
| `export_gcode` | `ExportParams` | — | Writes `.nc` file to disk |

### Stock & Setup Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `set_stock` | `StockParams` | `MeshData` | Returns stock mesh for display |
| `set_wcs` | `WcsParams` | — | Coordinate system origin/orientation |
| `get_project_snapshot` | — | `ProjectSnapshot` | Full project read for UI sync |

### Tool Library Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `add_tool` | `ToolDefinition` | `ToolId` | — |
| `update_tool` | `ToolId, ToolDefinition` | — | — |
| `remove_tool` | `ToolId` | — | Errors if tool in use |
| `list_tools` | — | `Tool[]` | — |

### Operation Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `add_operation` | `OperationParams` | `OperationId` | — |
| `update_operation` | `OperationId, OperationParams` | — | Invalidates cached toolpath |
| `remove_operation` | `OperationId` | — | — |
| `reorder_operations` | `OperationId[]` | — | Sets program order |

### Toolpath Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `calculate_toolpath` | `OperationId` | `JobId` | Returns immediately; async |
| `calculate_all_toolpaths` | — | `JobId` | Queued sequential computation |
| `cancel_job` | `JobId` | — | Signals cancellation token |
| `get_toolpath_geometry` | `OperationId` | `LineGeometryData` | For viewport display |
| `get_gcode_preview` | `OperationId, PostProcessorId` | `string` | Raw G-code text |
| `list_post_processors` | — | `PostProcessor[]` | — |

### Display Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `get_mesh_data` | — | `MeshData` | Re-fetch model mesh |
| `get_toolpath_geometry` | `OperationId` | `LineGeometryData` | — |
| `get_simulation_frames` | `OperationId` | `SimFrame[]` | Tool position per step |

---

## Event Inventory (Rust → Frontend)

Events are pushed by Rust without the frontend requesting them.
The frontend registers listeners at app startup and never removes them.

| Event | Payload | When emitted |
|---|---|---|
| `toolpath:progress` | `{ job_id, operation_id, percent, message }` | During computation |
| `toolpath:complete` | `{ job_id, operation_id }` | Computation finished |
| `toolpath:error` | `{ job_id, operation_id, error }` | Computation failed |
| `project:modified` | `{ change_type }` | Any backend mutation |
| `job:cancelled` | `{ job_id }` | Cancellation confirmed |

---

## Threading Model

```
Main thread (Tauri event loop)
│
├── invoke("calculate_toolpath") received
│     │
│     └── Tokio task spawned ──────────────────────────────────────┐
│                                                                   │
│     invoke() returns JobId immediately                            │
│     (frontend is not blocked)                                     │
│                                                                   │
│                            ┌──────────────────────────────────┐  │
│                            │  Tokio async task                │  │
│                            │                                  │  │
│                            │  1. Acquire RwLock read on state │  │
│                            │  2. Clone required input data    │  │
│                            │  3. Release lock                 │  │
│                            │                                  │  │
│                            │  4. Rayon parallel computation   │◄─┘
│                            │     (CPU-bound, uses all cores)  │
│                            │                                  │
│                            │  5. Emit progress events         │
│                            │     window.emit("toolpath:...")  │
│                            │                                  │
│                            │  6. Acquire RwLock write         │
│                            │  7. Store result in AppState     │
│                            │  8. Release lock                 │
│                            │                                  │
│                            │  9. Emit complete event          │
│                            └──────────────────────────────────┘
```

**Rules:**
- IPC command handlers are always `async fn` — they never block the dispatch thread
- CPU-bound work (toolpath math) is handed off to Rayon from within a Tokio task
  via `tokio::task::spawn_blocking`
- AppState is cloned (inputs only) before releasing the lock; computation runs
  lock-free
- Only the result write requires a write lock, and it is brief

---

## Data Flow: Key Scenarios

### Scenario 1 — Opening a STEP File

```
Frontend                         Rust Core
────────                         ─────────
File dialog (native Tauri)
User selects file
invoke("open_model", path) ──►  read file from disk
                                parse via OCCT FFI
                                tessellate B-rep → triangle mesh
                                store LoadedModel in AppState
                    ◄────────── return MeshData
                                  { vertices: Float32Array,
                                    normals:  Float32Array,
                                    indices:  Uint32Array }
build Three.js BufferGeometry
add mesh to scene
render
```

Geometry is transferred as flat typed arrays. This avoids JSON serialization
overhead on large meshes and maps directly to WebGL buffer uploads.

### Scenario 2 — Defining and Computing an Operation

```
Frontend                         Rust Core
────────                         ─────────
User fills operation form
invoke("add_operation", params) ──►  validate params
                                     append to operations list
                        ◄──────────  return OperationId

User clicks "Calculate"
invoke("calculate_toolpath",
        operation_id) ──────────►  spawn Tokio task
              ◄─────────────────── return JobId  (immediate)

                     [async task running]
                                    acquire read lock
                                    clone model + operation
                                    release lock
                                    run algorithm (Rayon)
emit("toolpath:progress") ◄──────── every N passes
update progress bar

emit("toolpath:complete") ◄──────── result stored in AppState
invoke("get_toolpath_geometry",
        operation_id) ──────────►  serialize toolpath to LineGeometryData
              ◄─────────────────── return LineGeometryData
render toolpath in viewport
```

### Scenario 3 — Exporting G-code

```
Frontend                         Rust Core
────────                         ─────────
User picks post-processor,
output path (native dialog)
invoke("export_gcode", params) ──►  fetch toolpath from AppState
                                    run post-processor engine
                                    apply TOML config for controller
                                    write file to disk
                    ◄──────────── return (success / AppError)
show success notification
```

---

## Error Handling

All Rust commands return `Result<T, AppError>`. `AppError` is a serializable enum:

```rust
#[derive(thiserror::Error, Serialize, Debug)]
pub enum AppError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("STEP import failed: {0}")]
    GeometryImport(String),

    #[error("Toolpath calculation failed: {0}")]
    ToolpathError(String),

    #[error("Operation references unknown tool: {0}")]
    UnknownTool(ToolId),

    #[error("IO error: {0}")]
    Io(String),
    // ...
}
```

On the frontend, the `src/api/` wrapper converts all rejected promises into typed
`AppError` objects. UI components handle errors via a notification/toast system.
Internal errors are logged via `tracing` at the appropriate level; critical errors
are also surfaced to the user.

**Rule:** No `unwrap()` or `expect()` in command handlers or anything they call.
Panics in async Tauri commands produce confusing behavior; all fallible paths
return `Result`.

---

## Rust Module Structure

```
src-tauri/src/
│
├── main.rs                  # Tauri builder, plugin registration,
│                            # command registration, AppState setup
│
├── state.rs                 # AppState, Project, and all domain structs
│
├── error.rs                 # AppError enum
│
├── commands/                # Tauri command handlers (thin — delegate to modules)
│   ├── file.rs              # open_model, save_project, load_project, export_gcode
│   ├── project.rs           # set_stock, set_wcs, get_project_snapshot
│   ├── tools.rs             # add_tool, update_tool, remove_tool, list_tools
│   ├── operations.rs        # add_operation, update_operation, remove_operation
│   ├── toolpath.rs          # calculate_toolpath, cancel_job, get_toolpath_geometry
│   └── display.rs           # get_mesh_data, get_simulation_frames
│
├── geometry/                # Geometry kernel integration
│   ├── mod.rs
│   ├── ffi.rs               # Raw bindgen output + safe wrappers
│   ├── importer.rs          # Format dispatch: STEP/IGES/STL/OBJ/DXF/SVG
│   └── tessellator.rs       # B-rep → triangle mesh, LOD strategy
│
├── toolpath/                # CAM algorithm engine
│   ├── mod.rs
│   ├── types.rs             # Toolpath, Pass, CutPoint, ToolOrientation
│   ├── planner.rs           # Top-level: operation → toolpath
│   ├── linking.rs           # Rapids, lead-in, lead-out, retract
│   ├── collision.rs         # Gouge detection, holder clearance
│   └── operations/
│       ├── contour.rs       # 2D/3D profile contouring
│       ├── pocket.rs        # Pocket clearing (parallel, spiral, adaptive)
│       ├── drill.rs         # Drilling cycles
│       ├── surface.rs       # 3D surface machining (scallop, flowline)
│       └── five_axis.rs     # 5-axis swarf, flank, and point milling
│
├── postprocessor/
│   ├── mod.rs
│   ├── engine.rs            # Template engine: Toolpath → G-code string
│   ├── types.rs             # PostProcessor config struct
│   └── builtins/            # Embedded TOML configs
│       ├── fanuc.toml
│       ├── heidenhain.toml
│       ├── siemens_840d.toml
│       ├── mach4.toml
│       └── linuxcnc.toml
│
└── project/
    ├── mod.rs
    ├── types.rs             # All domain value types (Tool, Stock, WCS, etc.)
    └── serialization.rs     # .jcam read/write
```

---

## Frontend Module Structure

```
src/
│
├── main.tsx                 # React root, Tauri event listener setup
│
├── api/                     # Typed IPC wrappers (only place invoke() is called)
│   ├── file.ts
│   ├── project.ts
│   ├── tools.ts
│   ├── operations.ts
│   ├── toolpath.ts
│   └── types.ts             # Shared TypeScript types mirroring Rust structs
│
├── store/                   # Zustand state
│   ├── projectStore.ts      # Project summary, selection state
│   ├── viewportStore.ts     # Camera, display modes, visibility
│   └── jobStore.ts          # Active computation jobs and progress
│
├── viewport/                # Three.js integration
│   ├── Viewport.tsx         # React component hosting the canvas
│   ├── scene.ts             # Three.js scene, camera, renderer setup
│   ├── modelMesh.ts         # Building geometry from MeshData
│   ├── toolpathLines.ts     # Building geometry from LineGeometryData
│   ├── stockMesh.ts
│   ├── simulation.ts        # Tool animation along toolpath
│   └── controls.ts          # OrbitControls, selection raycasting
│
└── components/
    ├── layout/              # App shell, panel layout, splitters
    ├── toolbar/             # Top toolbar, view controls
    ├── operations/          # Operation list, operation editor forms
    ├── tools/               # Tool library panel
    ├── gcode/               # G-code preview panel
    └── common/              # Notifications, dialogs, progress indicators
```

---

## Startup Sequence

```
1. OS launches Rust process
2. Rust: initialize tracing (log to file + stderr)
3. Rust: load UserPreferences from OS config dir
4. Rust: initialize AppState with empty project
5. Rust: register all Tauri commands and plugins
6. Tauri: create window, load embedded frontend assets
7. Frontend: React renders — reads empty Zustand store → shows empty/welcome state
8. Frontend: registers event listeners (toolpath:progress, toolpath:complete, etc.)
9. Frontend: calls get_project_snapshot() to sync initial state
10. App is ready
```

On subsequent launches, step 9 may trigger `load_project(last_project_path)`
if a recent project is configured.

---

## Cross-Cutting Concerns

### Logging
- Rust: `tracing` crate, logs to `~/.local/share/jamiecam/logs/` (platform-appropriate path)
- Frontend: `console.*` methods, surfaced in Tauri dev tools during development
- IPC traffic is logged at `DEBUG` level in dev builds, suppressed in release

### Configuration
- User preferences: TOML file in OS config directory (`dirs` crate for path)
- Post-processor configs: embedded in binary as `include_str!()`, user can add custom ones

### Cancellation
- Long-running Rust tasks accept a `CancellationToken` (from `tokio-util`)
- Frontend sends `cancel_job(job_id)`; the token is signalled
- Tasks check the token at loop boundaries and return early with a `Cancelled` error
- The frontend receives a `job:cancelled` event to reset UI state

---

*Document status: Draft*
*Related documents: `technology-stack.md`, `viewport-design.md`, `toolpath-engine.md`*
