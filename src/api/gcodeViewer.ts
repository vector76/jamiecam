/**
 * Typed wrappers around Tauri's invoke() for G-code viewer IPC commands.
 */

import type { GcodeViewerLoadResult, GcodeStockMetadata, MeshData } from './types'
import { typedInvoke } from './errors'

/**
 * Parse a G-code file, extracting metadata and generating viewport geometry.
 *
 * Returns a composite result containing optional stock metadata, tool metadata
 * entries, toolpath centerline geometry, and any non-fatal warnings.
 *
 * @param path Absolute path to the G-code file.
 * @returns Parsed metadata, line geometry, and warnings.
 * @throws AppError if the file cannot be read or is otherwise inaccessible.
 */
export async function loadGcodeForViewer(path: string): Promise<GcodeViewerLoadResult> {
  return typedInvoke<GcodeViewerLoadResult>('load_gcode_for_viewer', { path })
}

/**
 * Run a dexel material-removal simulation on a G-code file.
 *
 * The file is re-parsed on every invocation (stateless design). Stock and tool
 * are supplied by the caller rather than read from the project state.
 *
 * @param path Absolute path to the G-code file.
 * @param stock Box stock definition (origin, width, depth, height).
 * @param toolDiameter Cutting diameter in mm (must be positive).
 * @param toolType Tool geometry type string (currently only `'flat_endmill'`).
 * @param resolution Dexel grid cell size in mm (0.01–5.0). Larger = faster, coarser.
 * @returns MeshData representing the workpiece after material removal.
 * @throws AppError if any parameter is invalid or the file cannot be read.
 */
export async function simulateGcodeViewer(
  path: string,
  stock: Pick<GcodeStockMetadata, 'origin' | 'width' | 'depth' | 'height'>,
  toolDiameter: number,
  toolType: string,
  resolution: number,
): Promise<MeshData> {
  return typedInvoke<MeshData>('simulate_gcode_viewer', {
    path,
    originX: stock.origin.x,
    originY: stock.origin.y,
    originZ: stock.origin.z,
    width: stock.width,
    depth: stock.depth,
    height: stock.height,
    toolDiameter,
    toolType,
    resolution,
  })
}

/**
 * Return the absolute path to the bundled sample G-code file.
 *
 * Uses Tauri's resource directory API to resolve the platform-correct path.
 * The returned path can be passed directly to `loadGcodeForViewer`.
 *
 * @returns Absolute filesystem path to `demo-pocket.nc`.
 * @throws AppError if the resource directory cannot be resolved.
 */
export async function getSampleGcodePath(): Promise<string> {
  return typedInvoke<string>('get_sample_gcode_path', {})
}
