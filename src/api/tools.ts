/**
 * Typed wrappers around Tauri's invoke() for tool CRUD IPC commands.
 */

import { invoke } from '@tauri-apps/api/core'
import type { Tool, ToolInput } from './types'
import { toAppError } from './errors'

/**
 * Add a new tool to the project tool library.
 *
 * @param input Tool fields (type, material, dimensions, etc.).
 * @returns The created Tool with its server-generated ID.
 * @throws AppError on validation failure or backend error.
 */
export async function addTool(input: ToolInput): Promise<Tool> {
  try {
    return await invoke<Tool>('add_tool', { input })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Replace all fields of an existing tool.
 *
 * @param id UUID string of the tool to update.
 * @param input Replacement tool fields.
 * @returns The updated Tool.
 * @throws AppError if the tool ID is not found.
 */
export async function editTool(id: string, input: ToolInput): Promise<Tool> {
  try {
    return await invoke<Tool>('edit_tool', { id, input })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Remove a tool from the project tool library.
 *
 * @param id UUID string of the tool to remove.
 * @throws AppError if the tool ID is not found.
 */
export async function deleteTool(id: string): Promise<void> {
  try {
    await invoke<void>('delete_tool', { id })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Return all tools in the project tool library.
 *
 * @returns Array of Tool objects in insertion order.
 * @throws AppError on unexpected backend failure.
 */
export async function listTools(): Promise<Tool[]> {
  try {
    return await invoke<Tool[]>('list_tools')
  } catch (e) {
    throw toAppError(e)
  }
}
