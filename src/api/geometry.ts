/**
 * Typed wrappers around Tauri's invoke() for geometry IPC commands.
 */

import type { FaceDescriptor, HoleDescriptor } from './types'
import { typedInvoke } from './errors'

/**
 * Retrieve the list of B-rep face descriptors for the currently loaded model.
 * @returns Array of FaceDescriptor with fingerprint, index, centroid, normal, and area.
 * @throws AppError on backend failure or if no model is loaded.
 */
export async function getModelFaces(): Promise<FaceDescriptor[]> {
  return typedInvoke<FaceDescriptor[]>('get_model_faces')
}

/**
 * Detect cylindrical holes in the currently loaded model.
 * @returns Array of HoleDescriptor with centre, radius, depth, and through-hole flag.
 * @throws AppError on backend failure or if no model is loaded.
 */
export async function detectHoles(): Promise<HoleDescriptor[]> {
  return typedInvoke<HoleDescriptor[]>('detect_holes')
}
