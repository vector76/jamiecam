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
