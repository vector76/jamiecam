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
  | { kind: 'Unknown'; message: string }
  | { kind: 'WorkerError'; message: string }
  | { kind: 'Disposed'; message: string }

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
