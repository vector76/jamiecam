/**
 * TypeScript types mirroring the Rust wasm boundary.
 *
 * Field names match the camelCase serde output produced by Rust
 * (`#[serde(rename_all = "camelCase")]`).
 */

/** Tessellated triangle mesh. */
export interface MeshData {
  /** XYZ interleaved vertex positions — 3 values per vertex. */
  vertices: number[]
  /** XYZ interleaved normals — 3 values per vertex. */
  normals: number[]
  /** Triangle indices — 3 values per triangle. */
  indices: number[]
  /** Per-face triangle group boundaries. Empty for dexel meshes. */
  faceGroups: FaceGroup[]
}

export interface FaceGroup {
  startTriangle: number
  triangleCount: number
}

/** Flat-array line geometry for Three.js rendering. */
export interface LineGeometryData {
  /** Per segment: 6 floats (start XYZ + end XYZ). */
  positions: number[]
  /** Per segment: 6 floats (RGB × 2 vertices). */
  colours: number[]
  /** Per segment: 1 byte. 0 = linking/rapid, 1 = cutting. */
  types: number[]
}

/** Stock metadata parsed from a `; @STOCK` comment. */
export interface GcodeStockMetadata {
  stockType: string
  width: number
  depth: number
  height: number
  origin: { x: number; y: number; z: number }
}

/** Tool metadata parsed from a `; @TOOL` comment. */
export interface GcodeToolMetadata {
  number: number
  toolType: string
  diameter: number
  flutes: number | null
  material: string | null
}

export interface ParseWarning {
  line: number | null
  message: string
}

/** Composite result of loading a G-code file for the viewer. */
export interface GcodeViewerLoadResult {
  stock: GcodeStockMetadata | null
  tools: GcodeToolMetadata[]
  lineGeometry: LineGeometryData
  warnings: ParseWarning[]
}

/** Axis-aligned box stock dimensions. Mirrors Rust `BoxDimensions`. */
export interface BoxDimensions {
  origin: { x: number; y: number; z: number }
  width: number
  depth: number
  height: number
}

/** Parameters for the dexel material-removal simulation. */
export interface SimulateGcodeViewerParams {
  stock: BoxDimensions
  toolDiameter: number
  resolution: number
}

/**
 * Detail payload for the `ParseFailure` AppError variant. Used when a parser
 * cannot produce any structured output (recoverable issues are reported as
 * `ParseWarning` instead).
 */
export interface ParseFailureDetail {
  source: string
  message: string
  line: number | null
}

/**
 * Error shape returned by wasm entry points. Mirrors `AppError` in
 * `src-rust/src/error.rs`, serialized as `{ "kind": "<variant>", "message": <content> }`.
 * The `Unknown` / `WorkerError` / `Disposed` arms are synthesised by the TS
 * bridge for transport-layer failures.
 */
export type AppError =
  | { kind: 'Io'; message: string }
  | { kind: 'InvalidInput'; message: string }
  | { kind: 'ParseFailure'; message: ParseFailureDetail }
  | { kind: 'MissingSetup'; message: { id: string } }
  | { kind: 'MissingTool'; message: { id: string } }
  | { kind: 'UnknownProjectMode'; message: { mode: string } }
  | { kind: 'Unknown'; message: string }
  | { kind: 'WorkerError'; message: string }
  | { kind: 'Disposed'; message: string }

/**
 * Working environment (machine setups, tools, and their availability matrix).
 * Mirrors `src-rust/src/working_env/mod.rs`. Saved separately from `.jcam`
 * project files because it describes the user's CNC hardware rather than any
 * particular project. Per phase-4 design §6, tools are NOT nested inside
 * setups — the same tool often fits multiple setups, and the compatibility
 * relation is recorded in `AvailabilityMatrix`.
 */

/** Stable id for a `MachineSetup`. Intended source: UUID string. */
export type SetupId = string

/** Stable id for a `Tool`. Intended source: UUID string. */
export type ToolId = string

export interface SafetyParams {
  safeZ: number
  rapidFeedRate: number
}

export interface MachineSetup {
  id: SetupId
  name: string
  workspace: BoxDimensions
  kinematics: string
  postProcessor: string
  safety: SafetyParams
}

export interface FeedsAndSpeeds {
  spindleRpm: number
  feedRate: number
  plungeRate: number
}

export interface Tool {
  id: ToolId
  name: string
  diameter: number
  fluteCount: number
  length: number
  material: string
  recommended: FeedsAndSpeeds
}

export interface AvailabilityPair {
  setupId: SetupId
  toolId: ToolId
}

/** Serialized as a JSON array of `AvailabilityPair`s (sorted on the Rust side). */
export type AvailabilityMatrix = AvailabilityPair[]

export interface WorkingEnvironment {
  setups: MachineSetup[]
  tools: Tool[]
  availability: AvailabilityMatrix
}

/**
 * Shared 2D path representation used across the Mode 2 pipeline. Mirrors
 * `src-rust/src/geometry2d/mod.rs`.
 *
 * **Unit contract:** all coordinates and lengths are millimetres, `f64`
 * (number) throughout. Importers convert source units to mm before
 * constructing these values; the planner and emitter consume mm directly.
 *
 * Closed polylines do *not* duplicate the first point at the end of
 * `points` — the closing edge is implicit. Regions follow the same
 * convention for both exterior and holes.
 */
export interface Point2 {
  x: number
  y: number
}

export interface Polyline {
  points: Point2[]
  closed: boolean
}

export interface Region {
  exterior: Point2[]
  holes: Point2[][]
}

/**
 * Result of parsing an SVG document into 2D polylines. Mirrors
 * `ParseSvgResult` in `src-rust/src/wasm.rs`.
 */
export interface ParseSvgResult {
  paths: Polyline[]
  warnings: ParseWarning[]
}

/**
 * Result of parsing a DXF document into 2D polylines. Mirrors
 * `ParseDxfResult` in `src-rust/src/wasm.rs`.
 */
export interface ParseDxfResult {
  paths: Polyline[]
  warnings: ParseWarning[]
}

/**
 * Profile-cut operation: input descriptor and toolpath output.
 *
 * Mirrors `src-rust/src/profile/mod.rs`. Per phase-4 design §5 the first
 * Mode 2 ship is a single profile operation per project — these types
 * bracket the planner (input from the operation editor, output to the
 * GRBL emitter / dexel preview). Distances are millimetres, feeds are
 * mm/min.
 */

/** Which side of each boundary the cutter travels on. */
export type CutSide = 'outside' | 'inside' | 'onLine'

export interface ProfileOperationInput {
  boundaries: Polyline[]
  tool: Tool
  cutSide: CutSide
  depthTotal: number
  depthPerPass: number
  safeZ: number
  plungeFeed: number
  cutFeed: number
  spindleRpm: number
}

/**
 * A single toolpath move. `to` is `[x, y, z]` in mm. No arcs in first
 * ship — every cutting move is linear (G1); every traverse is a rapid
 * (G0).
 */
export type ToolpathMotion =
  | { kind: 'rapid'; to: [number, number, number] }
  | { kind: 'linear'; to: [number, number, number]; feed: number }

/** Ordered toolpath: the first move is normally a rapid to the approach
 * point at `safeZ`; the last is normally a rapid retract. */
export type ToolpathOutput = ToolpathMotion[]

/**
 * B-rep face descriptor. Mode 1 doesn't import models with face groups,
 * but the viewport store still carries the field for future modes.
 */
export interface FaceDescriptor {
  fingerprint: string
  faceIdx: number
  centroid: [number, number, number]
  normal: [number, number, number]
  area: number
}
