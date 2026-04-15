/**
 * Typed wrappers around Tauri's invoke() for 3D-mode heightmap IPC commands.
 */

import type { MeshData } from './types'
import { typedInvoke } from './errors'

/**
 * Load a heightmap image (PNG or TIFF grayscale) from disk and return a
 * tessellated plane mesh with per-pixel Z displacement.
 *
 * The initial implementation hardcodes a 100×100 mm footprint and a 10 mm Z
 * range; a future slice will expose these as user-controlled parameters.
 *
 * @param path Absolute path to the image file.
 * @returns Tessellated mesh ready for direct use by the Viewport.
 * @throws AppError if the file is missing, undecodable, or smaller than 2×2.
 */
export async function loadHeightmap(path: string): Promise<MeshData> {
  return typedInvoke<MeshData>('load_heightmap', { path })
}
