/**
 * Typed wrappers around Tauri's invoke() for all JamieCam IPC commands.
 *
 * Each function:
 *  - Calls the matching Rust command via invoke().
 *  - On failure, narrows the rejected value to AppError and re-throws it so
 *    callers always receive a typed error object.
 */

import { invoke } from '@tauri-apps/api/core'
import type { AppError, MeshData, ProjectSnapshot } from './types'

/**
 * Narrow an unknown rejection value to a typed AppError.
 *
 * Tauri serialises Rust errors as `{ kind, message }` objects. Any other
 * shape (e.g. a network error during development) is wrapped in an
 * AppError with kind "Unknown".
 */
function toAppError(e: unknown): AppError {
  if (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    typeof (e as Record<string, unknown>).kind === 'string'
  ) {
    return e as AppError
  }
  return { kind: 'Unknown', message: String(e) }
}

/**
 * Open a 3D model file, tessellate it, and store it in the active project.
 *
 * @param path Absolute path to the model file (.step, .iges, or .stl).
 * @returns Tessellated MeshData ready for the viewport.
 * @throws AppError on import failure or if the path is not found.
 */
export async function openModel(path: string): Promise<MeshData> {
  try {
    return await invoke<MeshData>('open_model', { path })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Reset the active project to a fresh default state.
 *
 * @returns A ProjectSnapshot reflecting the new empty project.
 * @throws AppError on unexpected backend failure.
 */
export async function newProject(): Promise<ProjectSnapshot> {
  try {
    return await invoke<ProjectSnapshot>('new_project')
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Save the active project to a .jcam file.
 *
 * @param path Absolute path for the output file.
 * @throws AppError if the file cannot be written.
 */
export async function saveProject(path: string): Promise<void> {
  try {
    await invoke<void>('save_project', { path })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Load a .jcam project file and replace the active project.
 *
 * @param path Absolute path to the .jcam file.
 * @returns A ProjectSnapshot reflecting the loaded project.
 * @throws AppError if the file is missing or has an unsupported schema version.
 */
export async function loadProject(path: string): Promise<ProjectSnapshot> {
  try {
    return await invoke<ProjectSnapshot>('load_project', { path })
  } catch (e) {
    throw toAppError(e)
  }
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
  try {
    return await invoke<ProjectSnapshot>('get_project_snapshot')
  } catch (e) {
    throw toAppError(e)
  }
}
