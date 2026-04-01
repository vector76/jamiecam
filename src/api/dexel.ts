/**
 * Typed wrappers around Tauri's invoke() for dexel simulation IPC commands.
 */

import type { MeshData } from './types'
import { typedInvoke } from './errors'

/**
 * Run the built-in demo simulation on a synthetic 100 × 100 × 20 mm stock block.
 *
 * Produces a two-level stepped pocket without reading or modifying project state,
 * so it can be called on any empty or open project.
 *
 * @param resolution Grid cell size in mm (0.01–5.0). Larger = faster, coarser.
 * @returns MeshData of the demo workpiece after material removal.
 * @throws AppError if resolution is out of range.
 */
export async function getDemoSimulationMesh(resolution: number): Promise<MeshData> {
  return typedInvoke<MeshData>('get_demo_simulation_mesh', { resolution })
}

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
