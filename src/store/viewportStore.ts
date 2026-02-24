/**
 * Zustand store for 3-D viewport state.
 *
 * Holds the tessellated mesh currently displayed, the orbit camera target,
 * zoom level, and display mode. All viewport UI components derive their
 * rendering parameters from this store.
 */

import { create } from 'zustand'
import type { MeshData } from '../api/types'

interface ViewportState {
  /** Tessellated mesh currently loaded into the viewport, or null. */
  meshData: MeshData | null
  /** World-space point the orbit camera rotates around (x, y, z). */
  orbitTarget: [number, number, number]
  /** Camera zoom level (1 = default). */
  zoom: number
  /**
   * Shading mode for the mesh.
   * Phase 0: only 'Shaded' is supported; additional modes added later.
   */
  displayMode: 'Shaded'
  /** Replace the displayed mesh (pass null to clear the viewport). */
  setMeshData: (m: MeshData | null) => void
  /** Move the orbit camera target to (x, y, z). */
  setOrbitTarget: (x: number, y: number, z: number) => void
  /** Set the camera zoom level. */
  setZoom: (z: number) => void
}

export const useViewportStore = create<ViewportState>((set) => ({
  meshData: null,
  orbitTarget: [0, 0, 0],
  zoom: 1,
  displayMode: 'Shaded',
  setMeshData: (meshData) => set({ meshData }),
  setOrbitTarget: (x, y, z) => set({ orbitTarget: [x, y, z] }),
  setZoom: (zoom) => set({ zoom }),
}))
