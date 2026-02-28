/**
 * Typed wrappers around Tauri's invoke() for toolpath and G-code IPC commands.
 */

import type { PostProcessorMeta, ExportParams } from './types'
import { typedInvoke } from './errors'

/**
 * List all built-in post-processor definitions.
 * @returns Array of PostProcessorMeta (id, name, description).
 * @throws AppError on unexpected backend failure.
 */
export async function listPostProcessors(): Promise<PostProcessorMeta[]> {
  return typedInvoke<PostProcessorMeta[]>('list_post_processors')
}

/**
 * Generate a G-code preview for the given operation using the specified post-processor.
 * @param operationId UUID string of the operation whose toolpath to preview.
 * @param postProcessorId Builtin post-processor ID (e.g. "fanuc-0i", "linuxcnc").
 * @returns G-code string.
 * @throws AppError (kind "NotFound") if no toolpath has been computed for the operation.
 */
export async function getGcodePreview(operationId: string, postProcessorId: string): Promise<string> {
  return typedInvoke<string>('get_gcode_preview', { operationId, postProcessorId })
}

/**
 * Export G-code for the specified operations to a file on disk.
 * @param params Export configuration including operation IDs, post-processor, and output path.
 * @throws AppError on post-processor error, missing toolpath, or I/O failure.
 */
export async function exportGcode(params: ExportParams): Promise<void> {
  return typedInvoke<void>('export_gcode', { params })
}
