/**
 * Zustand store for the Mode 2 (2D Profile Cuts) Canvas2D viewport.
 *
 * Holds:
 * - `extent`: the world-space bounding box of drawn content (or null).
 * - `transform`: the world→screen affine (translate + uniform scale; no
 *   rotation, no skew, no Y inversion — world Y matches canvas Y).
 * - `layerVisibility`: per-layer on/off flags, defaulting to visible.
 *
 * The pan / zoomAt / zoomToFit helpers are exported as pure functions so
 * they can be unit-tested without a DOM, and the store actions wrap them.
 */

import { create } from 'zustand'

export interface Extent2D {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

/**
 * World→screen affine. Screen point = (world * scale) + (tx, ty).
 * Units: `scale` is screen-pixels-per-world-unit, `tx`/`ty` are screen
 * pixels.
 */
export interface Transform2D {
  tx: number
  ty: number
  scale: number
}

export interface ViewSize {
  width: number
  height: number
}

export const IDENTITY_TRANSFORM: Transform2D = { tx: 0, ty: 0, scale: 1 }

// ── Pure helpers ────────────────────────────────────────────────────────────

export function worldToScreen(t: Transform2D, x: number, y: number): { x: number; y: number } {
  return { x: x * t.scale + t.tx, y: y * t.scale + t.ty }
}

export function screenToWorld(t: Transform2D, x: number, y: number): { x: number; y: number } {
  return { x: (x - t.tx) / t.scale, y: (y - t.ty) / t.scale }
}

export function computePan(t: Transform2D, dxScreen: number, dyScreen: number): Transform2D {
  return { tx: t.tx + dxScreen, ty: t.ty + dyScreen, scale: t.scale }
}

/**
 * Zoom around the screen-space pivot (sx, sy): scale is multiplied by
 * `factor` and the translation is adjusted so the world point under the
 * pivot stays under the pivot.
 */
export function computeZoomAt(t: Transform2D, factor: number, sx: number, sy: number): Transform2D {
  const world = screenToWorld(t, sx, sy)
  const newScale = t.scale * factor
  return {
    scale: newScale,
    tx: sx - world.x * newScale,
    ty: sy - world.y * newScale,
  }
}

/**
 * Fit `extent` inside `view`, centred, with an optional padding fraction
 * (e.g. 0.05 leaves a 5 % border on every side). Degenerate extents fall
 * back to scale 1 so callers don't divide by zero.
 */
export function computeZoomToFit(
  extent: Extent2D,
  view: ViewSize,
  padding: number = 0.05,
): Transform2D {
  const worldW = extent.maxX - extent.minX
  const worldH = extent.maxY - extent.minY
  const effW = view.width * (1 - padding)
  const effH = view.height * (1 - padding)
  let scale: number
  if (worldW <= 0 && worldH <= 0) {
    scale = 1
  } else if (worldW <= 0) {
    scale = effH / worldH
  } else if (worldH <= 0) {
    scale = effW / worldW
  } else {
    scale = Math.min(effW / worldW, effH / worldH)
  }
  const cx = (extent.minX + extent.maxX) / 2
  const cy = (extent.minY + extent.maxY) / 2
  return {
    scale,
    tx: view.width / 2 - cx * scale,
    ty: view.height / 2 - cy * scale,
  }
}

// ── Store ───────────────────────────────────────────────────────────────────

interface Viewport2DState {
  extent: Extent2D | null
  transform: Transform2D
  layerVisibility: Record<string, boolean>
  setExtent: (extent: Extent2D | null) => void
  setTransform: (t: Transform2D) => void
  setLayerVisible: (layer: string, visible: boolean) => void
  isLayerVisible: (layer: string) => boolean
  pan: (dxScreen: number, dyScreen: number) => void
  zoomAt: (factor: number, screenX: number, screenY: number) => void
  zoomToFit: (extent: Extent2D, view: ViewSize, padding?: number) => void
  reset: () => void
}

export const useViewport2DStore = create<Viewport2DState>((set, get) => ({
  extent: null,
  transform: IDENTITY_TRANSFORM,
  layerVisibility: {},
  setExtent: (extent) => set({ extent }),
  setTransform: (transform) => set({ transform }),
  setLayerVisible: (layer, visible) =>
    set((s) => ({ layerVisibility: { ...s.layerVisibility, [layer]: visible } })),
  isLayerVisible: (layer) => get().layerVisibility[layer] !== false,
  pan: (dx, dy) => set((s) => ({ transform: computePan(s.transform, dx, dy) })),
  zoomAt: (factor, sx, sy) =>
    set((s) => ({ transform: computeZoomAt(s.transform, factor, sx, sy) })),
  zoomToFit: (extent, view, padding) =>
    set({ transform: computeZoomToFit(extent, view, padding) }),
  reset: () => set({ extent: null, transform: IDENTITY_TRANSFORM, layerVisibility: {} }),
}))
