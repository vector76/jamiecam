/**
 * Typed wrappers around Tauri's invoke() for all JamieCam IPC commands.
 *
 * Each function:
 *  - Calls the matching Rust command via invoke().
 *  - On failure, narrows the rejected value to AppError and re-throws it so
 *    callers always receive a typed error object.
 */

import type { MeshData, ProjectSnapshot } from './types'
import { typedInvoke } from './errors'

/**
 * Open a 3D model file, tessellate it, and store it in the active project.
 *
 * @param path Absolute path to the model file (.step, .iges, or .stl).
 * @returns Tessellated MeshData ready for the viewport.
 * @throws AppError on import failure or if the path is not found.
 */
export async function openModel(path: string): Promise<MeshData> {
  return typedInvoke<MeshData>('open_model', { path })
}

/**
 * Reset the active project to a fresh default state.
 *
 * @returns A ProjectSnapshot reflecting the new empty project.
 * @throws AppError on unexpected backend failure.
 */
export async function newProject(): Promise<ProjectSnapshot> {
  return typedInvoke<ProjectSnapshot>('new_project')
}

/**
 * Save the active project to a .jcam file.
 *
 * @param path Absolute path for the output file.
 * @throws AppError if the file cannot be written.
 */
export async function saveProject(path: string): Promise<void> {
  return typedInvoke<void>('save_project', { path })
}

/**
 * Load a .jcam project file and replace the active project.
 *
 * @param path Absolute path to the .jcam file.
 * @returns A ProjectSnapshot reflecting the loaded project.
 * @throws AppError if the file is missing or has an unsupported schema version.
 */
export async function loadProject(path: string): Promise<ProjectSnapshot> {
  return typedInvoke<ProjectSnapshot>('load_project', { path })
}

/**
 * Return a lightweight snapshot of the current project state.
 *
 * Safe to call at any time; acquires a read lock on the backend project state.
 *
 * @returns The current ProjectSnapshot.
 * @throws AppError on unexpected backend failure.
 */
export async function getProjectSnapshot(): Promise<ProjectSnapshot> {
  return typedInvoke<ProjectSnapshot>('get_project_snapshot')
}
