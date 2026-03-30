/**
 * Typed wrappers around Tauri's invoke() for global tool library IPC commands.
 */

import type { Tool, ToolInput } from './types'
import { typedInvoke } from './errors'

/** Return all tools in the global tool library. */
export async function listGlobalTools(): Promise<Tool[]> {
  return typedInvoke<Tool[]>('list_global_tools')
}

/** Add a new tool to the global tool library. */
export async function addGlobalTool(input: ToolInput): Promise<Tool> {
  return typedInvoke<Tool>('add_global_tool', { input })
}

/** Replace all fields of a tool in the global tool library. */
export async function editGlobalTool(id: string, input: ToolInput): Promise<Tool> {
  return typedInvoke<Tool>('edit_global_tool', { id, input })
}

/** Remove a tool from the global tool library. */
export async function deleteGlobalTool(id: string): Promise<void> {
  return typedInvoke<void>('delete_global_tool', { id })
}

/** Import a tool from the global library into the active project. */
export async function importFromLibrary(id: string): Promise<Tool> {
  return typedInvoke<Tool>('import_from_library', { id })
}

/** Export a project tool to the global library. */
export async function exportToLibrary(id: string): Promise<Tool> {
  return typedInvoke<Tool>('export_to_library', { id })
}

/** Check whether a project is currently open. */
export async function isProjectOpen(): Promise<boolean> {
  return typedInvoke<boolean>('is_project_open')
}
