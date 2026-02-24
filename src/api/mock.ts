/**
 * Mock API: stub implementations of all IPC commands.
 *
 * Selected at runtime when the VITE_MOCK_API environment variable is 'true'.
 * All functions return hardcoded fixture data without contacting the Rust
 * backend, enabling frontend development and testing in environments where
 * Tauri is not available.
 *
 * The function signatures are identical to src/api/file.ts so either module
 * can be selected transparently.
 */

import type { MeshData, ProjectSnapshot } from './types'

const EMPTY_MESH: MeshData = {
  vertices: [],
  normals: [],
  indices: [],
}

const DEFAULT_SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: '',
  modifiedAt: '',
}

/** Mock: returns an empty MeshData without invoking the backend. */
export async function openModel(_path: string): Promise<MeshData> {
  return { ...EMPTY_MESH }
}

/** Mock: returns a default ProjectSnapshot without invoking the backend. */
export async function newProject(): Promise<ProjectSnapshot> {
  return { ...DEFAULT_SNAPSHOT }
}

/** Mock: no-op save (always succeeds). */
export async function saveProject(_path: string): Promise<void> {
  // no-op
}

/** Mock: returns a default ProjectSnapshot without reading any file. */
export async function loadProject(_path: string): Promise<ProjectSnapshot> {
  return { ...DEFAULT_SNAPSHOT }
}

/** Mock: returns a default ProjectSnapshot. */
export async function getProjectSnapshot(): Promise<ProjectSnapshot> {
  return { ...DEFAULT_SNAPSHOT }
}
