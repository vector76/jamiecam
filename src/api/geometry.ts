/**
 * Typed wrappers around Tauri's invoke() for geometry IPC commands.
 */

import type { FaceDescriptor } from './types'
import { typedInvoke } from './errors'

/**
 * Retrieve the list of B-rep face descriptors for the currently loaded model.
 * @returns Array of FaceDescriptor with fingerprint, index, centroid, normal, and area.
 * @throws AppError on backend failure or if no model is loaded.
 */
export async function getModelFaces(): Promise<FaceDescriptor[]> {
  return typedInvoke<FaceDescriptor[]>('get_model_faces')
}
