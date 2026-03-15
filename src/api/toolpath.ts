/**
 * Typed wrappers around Tauri's invoke() for toolpath and G-code IPC commands.
 */

import type { PostProcessorMeta, ExportParams, ToolpathStats, LineGeometryData, ToolpathProgressEvent, GougeCheckResult } from './types'
import { typedInvoke } from './errors'
import { listen } from '@tauri-apps/api/event'

/**
 * Calculate the toolpath for the given operation and return summary statistics.
 * @param operationId UUID string of the operation to calculate.
 * @returns ToolpathStats with point count, pass count, and total path length.
 * @throws AppError on backend failure.
 */
export async function calculateToolpath(operationId: string): Promise<ToolpathStats> {
  return typedInvoke<ToolpathStats>('calculate_toolpath', { operationId })
}

/**
 * Retrieve the line geometry data for a previously-calculated toolpath.
 * @param operationId UUID string of the operation whose toolpath to retrieve.
 * @returns LineGeometryData containing positions, colours, and move types.
 * @throws AppError (kind "NotFound") if no toolpath has been computed for the operation.
 */
export async function getToolpathGeometry(operationId: string): Promise<LineGeometryData> {
  return typedInvoke<LineGeometryData>('get_toolpath_geometry', { operationId })
}

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

/**
 * Check the cached toolpath for gouge violations against the model surface.
 * @param operationId UUID string of the operation whose toolpath to check.
 * @returns GougeCheckResult with any violations found.
 * @throws AppError (kind "NotFound") if no toolpath has been computed or no model loaded.
 * @throws AppError (kind "InvalidInput") if the operation is not a surface finishing type.
 */
export async function checkGouge(operationId: string): Promise<GougeCheckResult> {
  return typedInvoke<GougeCheckResult>('check_gouge', { operationId })
}

/**
 * Auto-lift gouging points in the cached toolpath so they no longer violate.
 * @param operationId UUID string of the operation whose toolpath to correct.
 * @returns Number of points that were corrected.
 * @throws AppError (kind "NotFound") if no toolpath has been computed or no model loaded.
 * @throws AppError (kind "InvalidInput") if the operation is not a surface finishing type.
 */
export async function autoLift(operationId: string): Promise<number> {
  return typedInvoke<number>('auto_lift', { operationId })
}

/**
 * Subscribe to toolpath calculation progress events from the backend.
 * @param handler Callback invoked with each ToolpathProgressEvent.
 * @returns Unsubscribe function — call it to stop listening.
 */
export async function listenToolpathProgress(
  handler: (event: ToolpathProgressEvent) => void
): Promise<() => void> {
  return listen<ToolpathProgressEvent>('toolpath:progress', (e) => handler(e.payload))
}
