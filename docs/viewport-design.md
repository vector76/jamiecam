# JamieCam Viewport Design

## Overview

The 3D viewport is a Three.js scene rendered in a WebGL canvas inside the Tauri
webview. It is responsible for displaying the workpiece model, stock, fixtures,
toolpaths, and tool simulation. It is a pure display layer — all geometry data
originates in Rust and is pushed to the viewport via the IPC bridge.

---

## Coordinate System Convention

**JamieCam uses a Z-up right-handed coordinate system throughout.**

```
         Z (up — spindle direction)
         │
         │
         │
         └──────── X (right — table X axis)
        ╱
       ╱
      Y (into scene — table Y axis)
```

This matches the convention of virtually all CNC machine tools and the majority
of CAM software. All geometry data exported from Rust uses Z-up coordinates.

### Three.js Alignment

Three.js defaults to Y-up. Rather than transforming incoming data, the camera's
up vector and OrbitControls are configured for Z-up at initialization:

```typescript
camera.up.set(0, 0, 1)
controls.object.up.set(0, 0, 1)
controls.update()
```

All scene objects are placed directly in Z-up space. No scene-level rotation
transform is applied — this avoids confusion when reading object positions.

---

## Scene Graph

```
THREE.Scene
│
├── Lights
│   ├── AmbientLight          (soft fill, intensity 0.4)
│   ├── DirectionalLight      (key light, upper-right, intensity 0.8)
│   └── DirectionalLight      (rim light, lower-left, intensity 0.3)
│
├── GridHelper                (XY plane at Z=0, 10mm divisions, fades at distance)
│
├── WorldGroup                (Three.js Group, coordinate system root)
│   │
│   ├── WcsIndicator          (RGB axis arrows: X=red, Y=green, Z=blue)
│   │   └── positioned at WCS origin, always visible
│   │
│   ├── StockGroup
│   │   └── StockMesh         (box or cylinder, transparent overlay)
│   │
│   ├── FixtureGroup          (optional, imported fixture geometry)
│   │   └── FixtureMesh[]
│   │
│   ├── ModelGroup
│   │   ├── SolidMesh         (shaded model, MeshStandardMaterial)
│   │   ├── WireframeMesh     (optional edge overlay, LineSegments)
│   │   └── SelectionGroup    (highlighted faces/edges for active selection)
│   │
│   ├── ToolpathGroup
│   │   ├── OperationPath[0]  (LineSegments — rapids + cuts combined)
│   │   ├── OperationPath[1]
│   │   └── ...  (one entry per operation, toggled by visibility)
│   │
│   ├── SimulationGroup
│   │   ├── ToolMesh          (procedural endmill/ballnose geometry)
│   │   ├── ToolHolderMesh    (optional, for gouge checking visualization)
│   │   └── ToolAxisLine      (line showing current tool axis for 5-axis)
│   │
│   └── SimulationOverlay     (physics simulation visualization; hidden until sim run)
│       ├── HeatmapColorLayer (toolpath lines recolored by selected physics metric)
│       ├── ViolationMarkers  (InstancedMesh spheres at violation point locations)
│       └── ViolationPanel    (CSS2DObject — detail label for the selected violation)
│
└── OverlayGroup              (rendered last, no depth test)
    ├── MeasurementLabels     (CSS2DObject — HTML labels for dimensions)
    └── SelectionOutline      (post-process or additive highlight)
```

---

## Camera System

### Projection Modes

Two camera modes, switchable at any time without losing view direction:

| Mode | Class | Use case |
|---|---|---|
| Perspective | `THREE.PerspectiveCamera` | Default — natural depth perception |
| Orthographic | `THREE.OrthographicCamera` | Precision checking, 2D profile setup |

Switching between modes preserves the camera's look-at target and approximate
zoom level. The orthographic frustum is sized to match the perspective view's
apparent scale at the current orbit distance.

### Standard Views

Keyboard shortcuts snap to standard views. All views preserve the current
projection mode (perspective or ortho):

| Key | View | Camera position |
|---|---|---|
| `Numpad 7` | Top | +Z looking down (−Z) |
| `Numpad 1` | Front | +Y looking toward −Y |
| `Numpad 3` | Right | +X looking toward −X |
| `Numpad 5` | Toggle ortho/perspective | — |
| `Numpad 0` | Isometric | (+1, −1, +1) normalized |
| `F` | Frame selection | Fit camera to selected / all |

Standard views use smooth animated transitions (400ms ease-in-out) rather than
snapping instantly.

### Orbit Controls

`THREE.OrbitControls` with Z-up configuration:

| Input | Action |
|---|---|
| Left drag | Orbit |
| Right drag | Pan |
| Scroll wheel | Zoom |
| Middle drag | Pan |
| Double-click face | Set orbit target to clicked point |

Constraints:
- Prevent flipping through the Z pole (polar angle clamped to [1°, 179°])
- Minimum zoom: 1mm from target
- Maximum zoom: 10m from target (configurable per scene size)

---

## Model Display

### Materials

```typescript
// Primary solid material
const solidMaterial = new THREE.MeshStandardMaterial({
  color: 0x8899aa,        // cool metallic gray
  metalness: 0.3,
  roughness: 0.6,
  side: THREE.FrontSide,
})

// Transparent stock overlay
const stockMaterial = new THREE.MeshStandardMaterial({
  color: 0xddaa44,        // amber
  metalness: 0.0,
  roughness: 0.9,
  transparent: true,
  opacity: 0.15,
  side: THREE.DoubleSide,
  depthWrite: false,      // avoid z-fighting with model
})

// Wireframe overlay
const wireframeMaterial = new THREE.LineBasicMaterial({
  color: 0x334455,
  transparent: true,
  opacity: 0.4,
})
```

### Display Modes

Toggled from the viewport toolbar:

| Mode | Solid | Wireframe | Description |
|---|---|---|---|
| Shaded | ✓ | — | Default — solid shaded |
| Shaded + Edges | ✓ | ✓ | Solid with edge overlay |
| Wireframe | — | ✓ | Edges only |
| Transparent | ✓ (0.4) | — | See-through model for deep features |

### Geometry Format (from Rust)

Model mesh data is transferred as flat typed arrays to avoid JSON overhead:

```typescript
interface MeshData {
  vertices: Float32Array   // [x,y,z, x,y,z, ...]  3 floats per vertex
  normals:  Float32Array   // [nx,ny,nz, ...]       3 floats per vertex
  indices:  Uint32Array    // [i0,i1,i2, ...]       3 indices per triangle
}
```

Rust generates this via OCCT tessellation. The frontend constructs a
`THREE.BufferGeometry` directly from these buffers with no intermediate
conversion:

```typescript
function buildGeometry(mesh: MeshData): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry()
  geo.setAttribute('position', new THREE.BufferAttribute(mesh.vertices, 3))
  geo.setAttribute('normal',   new THREE.BufferAttribute(mesh.normals, 3))
  geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1))
  return geo
}
```

---

## Toolpath Visualization

### Geometry Format (from Rust)

```typescript
interface LineGeometryData {
  // Interleaved position data for all segments
  // Each pair of points is one line segment (start, end, start, end, ...)
  positions: Float32Array

  // Per-segment color (r,g,b per vertex, so 6 floats per segment)
  colors: Float32Array

  // Segment type index (used for filtering/visibility)
  // 0 = rapid, 1 = cutting, 2 = lead-in, 3 = lead-out, 4 = plunge
  types: Uint8Array
}
```

### Color Coding

Each segment type has a fixed color. Each operation additionally gets a
unique hue from a palette so overlapping operations remain distinguishable.

| Segment type | Color | Style |
|---|---|---|
| Rapid traverse | `#888888` gray | Dashed (`LineDashedMaterial`) |
| Cutting move | Operation palette color | Solid |
| Lead-in | Operation color, 60% brightness | Solid |
| Lead-out | Operation color, 60% brightness | Solid |
| Plunge / ramp | `#ff6644` orange | Solid |

**Operation color palette** (cycles for additional operations):
`#4e9af1`, `#f1c94e`, `#6dbf6d`, `#c46df1`, `#f16d6d`, `#6df1e8`

### Visibility Controls

- Each operation's `OperationPath` Group has a `.visible` flag
- Viewport toolbar: show all / show selected only / hide all
- Rapid moves: global toggle (rapids are often visual noise)
- Individual operation visibility toggled from the operations panel

### 5-Axis Tool Orientation Indicators

For 5-axis operations, tool axis vectors are drawn at regular intervals
along the path to visualize how the tool tilts:

```
toolpath line
   ●────────────────────────────────●
   │↑          │↑         │↑        │↑
   │           │          │         │
  (tool axis vectors, every N mm of path length)
```

Implemented as `THREE.InstancedMesh` of thin cylinders. Density is
configurable (default: every 10mm of path length). Hidden for 3-axis operations.

---

## Selection Model

### Selectable Entities

| Entity | How selected | Used for |
|---|---|---|
| Model face | Left-click on shaded face | Defining machining regions |
| Model edge | Left-click near edge (edge detection) | Contour selection |
| Toolpath / operation | Left-click on a toolpath line | Selects operation in panel |
| WCS origin | Click + drag on WCS indicator | Repositioning WCS |

### Selection Highlight

Selected faces receive an emissive highlight overlay. A separate
`SelectionMesh` is built from only the selected face indices and rendered
on top with an additive blending material:

```typescript
const selectionMaterial = new THREE.MeshBasicMaterial({
  color: 0x4488ff,
  transparent: true,
  opacity: 0.35,
  depthTest: true,
  depthWrite: false,
})
```

Edges near the cursor are highlighted on hover using a screen-space distance
calculation in the vertex shader (avoids per-frame CPU raycasting for edges).

### Multi-Selection

`Shift+click` adds to selection. `Ctrl+click` toggles. A rubber-band
rectangle select (drag on empty space) selects all faces within the screen-space
rectangle.

---

## Simulation Mode

Simulation animates the tool moving along a computed toolpath.

### Tool Geometry

Procedurally generated from the tool definition stored in Rust.
Simple geometry is sufficient — not a manufacturing-accurate model.

| Tool type | Geometry |
|---|---|
| Flat endmill | Cylinder + flat disc cap |
| Ball nose | Cylinder + hemisphere cap |
| Bull nose | Cylinder + rounded-edge disc |
| V-bit | Inverted cone |
| Drill | Cylinder + conical tip |

Tool geometry is rebuilt when the simulated operation's tool changes.
For performance it is generated once and reused across simulation frames.

### Simulation Playback

```typescript
interface SimFrame {
  position: [number, number, number]    // tool tip position (Z-up)
  orientation: [number, number, number] // tool axis unit vector (Z-up)
  feedType: 'rapid' | 'cutting' | 'plunge'
}
```

Frames are fetched from Rust via `get_simulation_frames(operationId)` as a
pre-computed array. Interpolation between frames is handled in the frontend
using `THREE.Vector3.lerp` and `THREE.Quaternion.slerp`.

**Playback controls:**
- Play / Pause
- Speed: 0.25×, 0.5×, 1×, 2×, 10×, 100× (realtime relative to feed rate)
- Scrub bar: drag to any point in the path; when physics simulation data is available,
  violation tick marks appear as colored dots above the scrub timeline
- Single-step forward / backward
- Jump to next / previous violation (when simulation data is present)

During simulation, the toolpath line behind the tool is rendered in a
"completed" color (desaturated) while the remaining path retains its
operation color. When a heatmap overlay is active, the toolpath retains
heatmap coloring regardless of playback position.

### Physics Heatmap Overlays

When a physics simulation result is available, the toolpath lines can be recolored
by a selected physics metric. Six heatmap modes are available, toggled from the
viewport toolbar:

| Mode | What it shows | Color scale |
|---|---|---|
| Off | Standard operation color coding | — |
| Force | Predicted cutting force magnitude (N) | Blue → yellow → red |
| Surface Error | Predicted surface error from deflection (mm, signed) | Blue (undersize) → green (nominal) → red (oversize) |
| Temperature | Estimated cutting zone temperature (°C) | Blue → orange → red |
| Breakage Risk | Workpiece feature breakage risk index (0–1) | Green → yellow → red |
| Chatter Risk | Chatter onset proximity (stability margin) | Green → yellow → red |
| MRR | Material removal rate (cm³/min) | Blue → cyan → white |

Heatmap colors are computed per-point from `PointSimData` values and uploaded to
the GPU as a vertex color buffer. The `HeatmapColorLayer` shares geometry positions
with `ToolpathGroup` but renders on top with a custom shader that substitutes heatmap
colors when the overlay mode is active.

Violation locations are marked with small spheres (`ViolationMarkers`) rendered as
`THREE.InstancedMesh`. Spheres are yellow for `Warning` severity and red for `Error`
or `Critical`. Clicking a violation sphere opens the `ViolationPanel` CSS2D label
showing the violation type, actual vs. limit values, and suggested actions as
clickable buttons that immediately apply the optimization to the toolpath.

---

## Performance Strategy

### Toolpath Line Rendering

Large toolpaths can contain millions of segments. Key rules:

- Always use `THREE.LineSegments` (not `THREE.Line`) — better GPU utilization
- Transfer from Rust as `Float32Array`; upload to GPU in one `BufferAttribute` call
- Never rebuild the geometry object — update the buffer in place
  (`attribute.needsUpdate = true`) when toolpath is recalculated
- Rapids and cut moves are in the same geometry; type-based visibility uses
  a custom shader that discards fragments based on the `type` attribute
  rather than splitting into separate geometry objects

### Level of Detail

For display zoom levels where individual moves are sub-pixel:
- Rust provides a decimated version of the toolpath (configurable chord tolerance)
- The full-resolution path is fetched only when the user zooms in past a threshold
- LOD switch is hysteresis-gated to prevent thrashing

### Model Tessellation Resolution

OCCT tessellation tolerance is chosen based on model bounding box size:

| Model size | Chord tolerance | Angular tolerance |
|---|---|---|
| < 50mm | 0.01mm | 5° |
| 50–500mm | 0.05mm | 10° |
| > 500mm | 0.1mm | 15° |

A higher-resolution tessellation can be requested explicitly (e.g., for
export or close-up rendering) without changing the interactive display mesh.

### Frame Budget

Target: 60fps on a mid-range laptop GPU.

- Lighting: maximum 3 light sources
- Shadow maps: disabled (not needed for CAM tool UI)
- Anti-aliasing: MSAA 4× (WebGL default via `antialias: true`)
- Post-processing: none in v1 (outline effects deferred)

---

## Viewport Toolbar Layout

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ [Shaded▼] [Edges] [Stock] [Rapids]  │  [Heatmap: Off▼] [Violations]             │
│ display modes          visibility   │  simulation overlay                        │
├──────────────────────────────────────────────────────────────────────────────────┤
│ [Top] [Front] [Right] [Iso]   [Fit]                                              │
│ standard views                frame                                              │
└──────────────────────────────────────────────────────────────────────────────────┘
```

The **[Heatmap: Off▼]** dropdown selects the active physics overlay mode (Off, Force,
Surface Error, Temperature, Breakage Risk, Chatter Risk, MRR). It is grayed out when
no simulation result is available for the selected operation. **[Violations]** toggles
the violation sphere markers and scrub-bar tick marks.

Toolbar lives above the canvas inside the React layout, not rendered in WebGL.
Icons are from the chosen UI component library's icon set.

---

## Viewport ↔ Operations Panel Synchronization

The viewport and the operations panel (list of machining operations) maintain
bidirectional selection sync:

- Clicking an operation in the panel highlights and frames its toolpath in the viewport
- Clicking a toolpath line in the viewport selects that operation in the panel
- Both use the Zustand `selectedOperationId` store atom as the shared source of truth
- The viewport listens to store changes and updates `OperationPath[n].visible`
  and material highlights accordingly

---

## Measurement Overlay

Basic distance and angle measurements displayed as HTML labels anchored to
3D points using `THREE.CSS2DRenderer` (renders HTML elements over the canvas):

- Point-to-point distance: click two points, shows distance label at midpoint
- Face normal angle: click a face, shows angle from Z axis (useful for 5-axis setup)
- Labels are cleared on Escape or when a new operation is started

`CSS2DRenderer` runs as a second renderer alongside the WebGL renderer,
sharing the same camera. Labels are standard `<div>` elements styled by CSS.

---

## HUD Elements (in-canvas)

Rendered as WebGL geometry overlaid on the scene (no depth test):

| Element | Description |
|---|---|
| Axis triad | Small RGB axis indicator, bottom-left corner, always visible |
| View label | Current standard view name ("TOP", "FRONT", etc.) fades in/out |
| Scale bar | Bottom of viewport, updates with zoom level |
| WCS indicator | Origin arrows at WCS location in world space |
| Simulation status | Top-right badge: "Sim: OK", "Sim: 3 violations", or "Sim: outdated" |

---

*Document status: Draft*
*Related documents: `technology-stack.md`, `system-architecture.md`, `toolpath-engine.md`*
