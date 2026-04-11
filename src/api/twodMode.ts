/**
 * Typed wrappers around Tauri's invoke() for 2D Profiling mode IPC commands.
 */

import { typedInvoke } from './errors'
import type { LineGeometryData, ToolpathStats } from './types'

// ── TypeScript types ──────────────────────────────────────────────────────────

export interface BoundingBox2d {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

export interface CurveSummary {
  id: string
  isClosed: boolean
  bbox: BoundingBox2d
}

export interface Load2dFileResult {
  curves: CurveSummary[]
  /** Full point arrays keyed by curve UUID string. */
  curvePoints: Record<string, number[][]>
  unitSystem: 'mm' | 'inches'
  warnings: string[]
}

export interface Get2dCurvesResult {
  curves: CurveSummary[]
  /** Full point arrays keyed by curve UUID string. */
  curvePoints: Record<string, number[][]>
  unitSystem: 'mm' | 'inches'
}

export interface Generate2dResult {
  /** Generated G-code text. */
  gcode: string
  /** Merged line geometry for viewport rendering. */
  lineGeometry: LineGeometryData
  /** Non-fatal validation warnings (e.g. top_of_cut at/below stock top). */
  warnings: string[]
  /** Aggregated toolpath statistics across all operations. */
  stats: ToolpathStats
}

// ── API functions ─────────────────────────────────────────────────────────────

/**
 * Parse a 2D artwork file (SVG or DXF), store it as the project's active 2D
 * artwork, and return curve summaries with full point data.
 *
 * @param path Absolute path to the SVG or DXF file.
 * @param unitSystemHint Required for SVG files (`'mm'` or `'inches'`); pass
 *   `null` for DXF files (unit system is read from `$INSUNITS`).
 * @returns Parsed curve summaries, point data, detected unit system, and
 *   any non-fatal import warnings.
 * @throws AppError if the file cannot be read, the extension is unsupported,
 *   or `unitSystemHint` is absent for an SVG file.
 */
export async function loadTwodFile(
  path: string,
  unitSystemHint: 'mm' | 'inches' | null,
): Promise<Load2dFileResult> {
  return typedInvoke<Load2dFileResult>('load_2d_file', {
    path,
    unitSystemHint,
  })
}

/**
 * Return curve summaries and point data for the currently loaded 2D artwork.
 *
 * Returns `null` when no artwork has been loaded into the project yet.
 *
 * @returns Curve summaries with point data and unit system, or `null`.
 * @throws AppError if the project lock cannot be acquired.
 */
export async function getTwodCurves(): Promise<Get2dCurvesResult | null> {
  return typedInvoke<Get2dCurvesResult | null>('get_2d_curves', {})
}

// ── Safe height ───────────────────────────────────────────────────────────────

/**
 * Set the safe height for 2D Profiling mode rapid moves.
 *
 * @param height Z height in mm, or `null` to clear.
 * @throws AppError if the project lock cannot be acquired.
 */
export async function setSafeHeight(height: number | null): Promise<void> {
  return typedInvoke<void>('set_safe_height', { height })
}

/**
 * Return the current safe height for 2D Profiling mode, or `null` if unset.
 *
 * @throws AppError if the project lock cannot be acquired.
 */
export async function getSafeHeight(): Promise<number | null> {
  return typedInvoke<number | null>('get_safe_height', {})
}

// ── Artwork origin ────────────────────────────────────────────────────────────

/**
 * Set the artwork origin offset for 2D Profiling mode geometry.
 *
 * @param x X offset in artwork units.
 * @param y Y offset in artwork units.
 * @throws AppError if the project lock cannot be acquired.
 */
export async function setArtworkOrigin(x: number, y: number): Promise<void> {
  return typedInvoke<void>('set_artwork_origin', { x, y })
}

/**
 * Return the current artwork origin as `[x, y]`.
 *
 * @throws AppError if the project lock cannot be acquired.
 */
export async function getArtworkOrigin(): Promise<[number, number]> {
  return typedInvoke<[number, number]>('get_artwork_origin', {})
}

// ── G-code generation ─────────────────────────────────────────────────────────

/**
 * Generate G-code for all enabled 2D Profile operations in the current project.
 *
 * @param postProcessorId The ID of the built-in post-processor to use
 *   (e.g. `'fanuc-0i'`, `'grbl'`).
 * @returns G-code text, merged line geometry, validation warnings, and stats.
 * @throws AppError if no artwork is loaded, a curve reference is invalid,
 *   more than one tool is used, or the post-processor cannot be loaded.
 */
export async function generate2dGcode(
  postProcessorId: string,
): Promise<Generate2dResult> {
  return typedInvoke<Generate2dResult>('generate_2d_gcode', { postProcessorId })
}
