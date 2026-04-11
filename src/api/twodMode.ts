/**
 * Typed wrappers around Tauri's invoke() for 2D Profiling mode IPC commands.
 */

import { typedInvoke } from './errors'

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
