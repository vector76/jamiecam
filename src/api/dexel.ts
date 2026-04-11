/**
 * Typed wrappers around Tauri's invoke() for dexel simulation IPC commands.
 */

import type { MeshData } from './types'
import { typedInvoke } from './errors'

/**
 * Compute a simulation mesh by applying enabled operation toolpaths to the
 * project stock via the dexel material removal engine.
 *
 * @param resolution Grid cell size in mm (0.01–5.0). Larger = faster, coarser.
 * @param operationIds Specific operation UUIDs to simulate, in order.
 *   Pass null to simulate all enabled operations.
 * @param upToSegment Stop after this many total motion segments. Pass null to apply all.
 * @returns MeshData representing the workpiece after material removal.
 * @throws AppError if no stock is defined, resolution is out of range, or
 *   required operations/tools/toolpaths are missing.
 */
export async function getSimulationMesh(
  resolution: number,
  operationIds?: string[] | null,
  upToSegment?: number | null,
): Promise<MeshData> {
  return typedInvoke<MeshData>('get_simulation_mesh', {
    resolution,
    operationIds: operationIds ?? null,
    upToSegment: upToSegment ?? null,
  })
}
