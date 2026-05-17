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

/** Error shape returned by wasm entry points. */
export interface AppError {
  kind: string
  message: string
}

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
