/**
 * Typed wrappers around Tauri's invoke() for operation CRUD IPC commands.
 */

import type { Operation, OperationInput } from './types'
import { typedInvoke } from './errors'

/**
 * Add a new operation to the project.
 *
 * The tool referenced by `input.toolId` must exist in the project tool library.
 *
 * @param input Operation fields including type, params, and tool ID.
 * @returns The created Operation with its server-generated ID.
 * @throws AppError if the tool ID is not found or input is invalid.
 */
export async function addOperation(input: OperationInput): Promise<Operation> {
  return typedInvoke<Operation>('add_operation', { input })
}

/**
 * Replace all fields of an existing operation.
 *
 * @param id UUID string of the operation to update.
 * @param input Replacement operation fields.
 * @returns The updated Operation.
 * @throws AppError if the operation or tool ID is not found.
 */
export async function editOperation(id: string, input: OperationInput): Promise<Operation> {
  return typedInvoke<Operation>('edit_operation', { id, input })
}

/**
 * Remove an operation from the project.
 *
 * @param id UUID string of the operation to remove.
 * @throws AppError if the operation ID is not found.
 */
export async function deleteOperation(id: string): Promise<void> {
  return typedInvoke<void>('delete_operation', { id })
}

/**
 * Reorder the project's operation list.
 *
 * `ids` must contain exactly the same set of UUIDs as the current operation
 * list — no additions or deletions.
 *
 * @param ids Complete ordered list of operation UUID strings.
 * @throws AppError if the ID count or any ID does not match the current list.
 */
export async function reorderOperations(ids: string[]): Promise<void> {
  return typedInvoke<void>('reorder_operations', { ids })
}

/**
 * Return all operations in the project, in program order.
 *
 * @returns Array of Operation objects.
 * @throws AppError on unexpected backend failure.
 */
export async function listOperations(): Promise<Operation[]> {
  return typedInvoke<Operation[]>('list_operations')
}
