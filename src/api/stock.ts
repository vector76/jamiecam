/**
 * Typed wrappers around Tauri's invoke() for stock and WCS IPC commands.
 */

import type { StockDefinition, WorkCoordinateSystem } from './types'
import { typedInvoke } from './errors'

/**
 * Set (or clear) the project stock definition.
 *
 * @param stock The new stock definition, or null to clear it.
 * @throws AppError on unexpected backend failure.
 */
export async function setStock(stock: StockDefinition | null): Promise<void> {
  return typedInvoke<void>('set_stock', { stock })
}

/**
 * Return the current project stock definition.
 *
 * @returns The stock definition, or null if none is set.
 * @throws AppError on unexpected backend failure.
 */
export async function getStock(): Promise<StockDefinition | null> {
  return typedInvoke<StockDefinition | null>('get_stock')
}

/**
 * Replace the project's WCS list.
 *
 * @param wcs The complete replacement WCS list.
 * @throws AppError on unexpected backend failure.
 */
export async function setWcs(wcs: WorkCoordinateSystem[]): Promise<void> {
  return typedInvoke<void>('set_wcs', { wcs })
}

/**
 * Return the project's WCS list.
 *
 * @returns Array of WorkCoordinateSystem objects in insertion order.
 * @throws AppError on unexpected backend failure.
 */
export async function getWcs(): Promise<WorkCoordinateSystem[]> {
  return typedInvoke<WorkCoordinateSystem[]>('get_wcs')
}
