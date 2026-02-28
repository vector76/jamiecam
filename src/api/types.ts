/**
 * TypeScript interfaces mirroring the Rust IPC types.
 *
 * All field names match the camelCase serde output produced by the Rust
 * backend (#[serde(rename_all = "camelCase")]).
 */

/** Tessellated triangle mesh returned by open_model. */
export interface MeshData {
  /** XYZ interleaved vertex positions — 3 values per vertex. */
  vertices: number[]
  /** XYZ interleaved normals — 3 values per vertex. */
  normals: number[]
  /** Triangle indices — 3 values per triangle. */
  indices: number[]
}

/** Reference to the source geometry model stored in a .jcam file. */
export interface SourceModelRef {
  /** Absolute path to the model file at last save. */
  path: string
  /** SHA-256 hex digest of the model file at last load. */
  checksum: string
  /** True when the model is embedded inside the .jcam ZIP. */
  embedded: boolean
}

// ── Shared geometry ───────────────────────────────────────────────────────────

/** 3-component vector used for positions and axis directions. */
export interface Vec3 {
  x: number
  y: number
  z: number
}

// ── Tool types ────────────────────────────────────────────────────────────────

/**
 * A cutting tool in the project tool library.
 *
 * Mirrors the Rust `Tool` struct. The tool geometry type is serialized under
 * the key `"type"` (not `"toolType"`) due to `#[serde(rename = "type")]` in
 * the Rust struct.
 */
export interface Tool {
  id: string
  name: string
  /** Snake_case tool geometry type (e.g. `"flat_endmill"`, `"ball_nose"`). */
  type: string
  material: string
  diameter: number
  fluteCount: number
  defaultSpindleSpeed?: number
  defaultFeedRate?: number
}

/**
 * Input for creating or replacing a tool (ID is excluded; generated server-side
 * on add, or provided separately on edit).
 */
export interface ToolInput {
  name: string
  /** Snake_case tool type string (e.g. `"flat_endmill"`). */
  type: string
  material: string
  diameter: number
  fluteCount: number
  defaultSpindleSpeed?: number
  defaultFeedRate?: number
}

/** A compact tool summary included in ProjectSnapshot. */
export interface ToolSummary {
  id: string
  name: string
  /** Snake_case tool type string (e.g. `"flat_endmill"`). */
  toolType: string
}

// ── Stock types ───────────────────────────────────────────────────────────────

/** A box-shaped stock solid. */
export interface BoxStock {
  type: 'box'
  /** Minimum-XYZ corner in WCS coordinates. */
  origin: Vec3
  /** Width along the X axis. */
  width: number
  /** Depth along the Y axis. */
  depth: number
  /** Height along the Z axis. */
  height: number
}

/**
 * Stock material block for the project.
 *
 * Internally-tagged enum matching `StockDefinition` in Rust.
 */
export type StockDefinition = BoxStock

// ── WCS types ─────────────────────────────────────────────────────────────────

/**
 * A named coordinate frame for positioning machining operations.
 *
 * Mirrors the Rust `WorkCoordinateSystem` struct.
 */
export interface WorkCoordinateSystem {
  id: string
  name: string
  origin: Vec3
  xAxis: Vec3
  zAxis: Vec3
}

// ── Operation types ───────────────────────────────────────────────────────────

/** Parameters for a Profile (contour) operation. */
export interface ProfileParams {
  depth: number
  stepdown: number
  compensationSide: 'left' | 'right' | 'center'
}

/** Parameters for a Pocket operation. */
export interface PocketParams {
  depth: number
  stepdown: number
  stepoverPercent: number
}

/** Parameters for a Drill operation. */
export interface DrillParams {
  depth: number
  peckDepth?: number
}

/**
 * A machining operation returned by the backend.
 *
 * The `type` discriminant and `params` object appear at the top level of the
 * JSON (flattened from `OperationParams` in Rust).
 */
export interface Operation {
  id: string
  name: string
  enabled: boolean
  toolId: string
  type: 'profile' | 'pocket' | 'drill'
  params: ProfileParams | PocketParams | DrillParams
}

/**
 * Input for creating or replacing an operation (ID excluded).
 *
 * The `type` and `params` fields correspond to the flattened `OperationParams`
 * in the Rust `OperationInput` struct.
 */
export interface OperationInput {
  name: string
  enabled?: boolean
  toolId: string
  type: 'profile' | 'pocket' | 'drill'
  params: ProfileParams | PocketParams | DrillParams
}

/** A compact operation summary included in ProjectSnapshot. */
export interface OperationSummary {
  id: string
  name: string
  operationType: 'profile' | 'pocket' | 'drill'
  enabled: boolean
  needsRecalculate: boolean
}

// ── ProjectSnapshot ───────────────────────────────────────────────────────────

/**
 * Lightweight snapshot of the active project returned by project commands.
 *
 * Mirrors the Rust `ProjectSnapshot` struct (camelCase via serde).
 */
export interface ProjectSnapshot {
  /** Absolute path to the loaded model file, or null if none. */
  modelPath: string | null
  /** SHA-256 hex digest of the loaded model file, or null if none. */
  modelChecksum: string | null
  /** Human-readable project name. */
  projectName: string
  /** ISO-8601 last-modified timestamp (empty string when not yet saved). */
  modifiedAt: string
  /** Tool library summaries. */
  tools: ToolSummary[]
  /** Stock solid definition, or absent/null if not set. */
  stock?: StockDefinition | null
  /** Work coordinate systems. */
  wcs: WorkCoordinateSystem[]
  /** Machining operation summaries, in program order. */
  operations: OperationSummary[]
}

/**
 * Error payload produced by all Rust command handlers.
 *
 * The Rust backend uses adjacently-tagged serde serialization:
 * `{ kind: string; message: string }`.
 * Unit variants (e.g. FileNotFound) omit the `message` field.
 */
export interface AppError {
  kind: string
  message?: string
}
