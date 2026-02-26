/**
 * Mock API: stub implementations of all IPC commands.
 *
 * Selected at runtime when the VITE_MOCK_API environment variable is 'true'.
 * All functions return hardcoded fixture data without contacting the Rust
 * backend, enabling frontend development and testing in environments where
 * Tauri is not available.
 *
 * The function signatures are identical to the real API modules so either
 * module can be selected transparently.
 */

import type {
  MeshData,
  Operation,
  OperationInput,
  ProjectSnapshot,
  StockDefinition,
  Tool,
  ToolInput,
  WorkCoordinateSystem,
} from './types'

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
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
}

// ── File / project commands ───────────────────────────────────────────────────

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

// ── Tool commands ─────────────────────────────────────────────────────────────

/** Mock: returns a stub Tool with a placeholder ID. */
export async function addTool(_input: ToolInput): Promise<Tool> {
  return {
    id: '00000000-0000-0000-0000-000000000000',
    name: _input.name,
    type: _input.type,
    material: _input.material,
    diameter: _input.diameter,
    fluteCount: _input.fluteCount,
  }
}

/** Mock: returns a stub Tool with the given ID. */
export async function editTool(id: string, _input: ToolInput): Promise<Tool> {
  return {
    id,
    name: _input.name,
    type: _input.type,
    material: _input.material,
    diameter: _input.diameter,
    fluteCount: _input.fluteCount,
  }
}

/** Mock: no-op delete (always succeeds). */
export async function deleteTool(_id: string): Promise<void> {
  // no-op
}

/** Mock: returns an empty tool list. */
export async function listTools(): Promise<Tool[]> {
  return []
}

// ── Stock / WCS commands ──────────────────────────────────────────────────────

/** Mock: no-op set stock (always succeeds). */
export async function setStock(_stock: StockDefinition | null): Promise<void> {
  // no-op
}

/** Mock: returns null (no stock set). */
export async function getStock(): Promise<StockDefinition | null> {
  return null
}

/** Mock: no-op set WCS (always succeeds). */
export async function setWcs(_wcs: WorkCoordinateSystem[]): Promise<void> {
  // no-op
}

/** Mock: returns an empty WCS list. */
export async function getWcs(): Promise<WorkCoordinateSystem[]> {
  return []
}

// ── Operation commands ────────────────────────────────────────────────────────

/** Mock: returns a stub Operation with a placeholder ID. */
export async function addOperation(_input: OperationInput): Promise<Operation> {
  return {
    id: '00000000-0000-0000-0000-000000000000',
    name: _input.name,
    enabled: _input.enabled ?? true,
    toolId: _input.toolId,
    type: _input.type,
    params: _input.params,
  }
}

/** Mock: returns a stub Operation with the given ID. */
export async function editOperation(id: string, _input: OperationInput): Promise<Operation> {
  return {
    id,
    name: _input.name,
    enabled: _input.enabled ?? true,
    toolId: _input.toolId,
    type: _input.type,
    params: _input.params,
  }
}

/** Mock: no-op delete (always succeeds). */
export async function deleteOperation(_id: string): Promise<void> {
  // no-op
}

/** Mock: no-op reorder (always succeeds). */
export async function reorderOperations(_ids: string[]): Promise<void> {
  // no-op
}

/** Mock: returns an empty operation list. */
export async function listOperations(): Promise<Operation[]> {
  return []
}
