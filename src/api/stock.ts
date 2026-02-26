/**
 * Typed wrappers around Tauri's invoke() for stock and WCS IPC commands.
 */

import { invoke } from '@tauri-apps/api/core'
import type { StockDefinition, WorkCoordinateSystem } from './types'
import { toAppError } from './errors'

/**
 * Set (or clear) the project stock definition.
 *
 * @param stock The new stock definition, or null to clear it.
 * @throws AppError on unexpected backend failure.
 */
export async function setStock(stock: StockDefinition | null): Promise<void> {
  try {
    await invoke<void>('set_stock', { stock })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Return the current project stock definition.
 *
 * @returns The stock definition, or null if none is set.
 * @throws AppError on unexpected backend failure.
 */
export async function getStock(): Promise<StockDefinition | null> {
  try {
    return await invoke<StockDefinition | null>('get_stock')
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Replace the project's WCS list.
 *
 * @param wcs The complete replacement WCS list.
 * @throws AppError on unexpected backend failure.
 */
export async function setWcs(wcs: WorkCoordinateSystem[]): Promise<void> {
  try {
    await invoke<void>('set_wcs', { wcs })
  } catch (e) {
    throw toAppError(e)
  }
}

/**
 * Return the project's WCS list.
 *
 * @returns Array of WorkCoordinateSystem objects in insertion order.
 * @throws AppError on unexpected backend failure.
 */
export async function getWcs(): Promise<WorkCoordinateSystem[]> {
  try {
    return await invoke<WorkCoordinateSystem[]>('get_wcs')
  } catch (e) {
    throw toAppError(e)
  }
}
