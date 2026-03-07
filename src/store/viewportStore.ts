/**
 * Zustand store for 3-D viewport state.
 *
 * Holds the tessellated mesh currently displayed, the orbit camera target,
 * zoom level, and display mode. All viewport UI components derive their
 * rendering parameters from this store.
 */

import { create } from 'zustand'
import type { MeshData, LineGeometryData, FaceDescriptor } from '../api/types'

interface ViewportState {
  /** Tessellated mesh currently loaded into the viewport, or null. */
  meshData: MeshData | null
  /** Toolpath line geometry currently loaded into the viewport, or null. */
  toolpathGeometry: LineGeometryData | null
  /** World-space point the orbit camera rotates around (x, y, z). */
  orbitTarget: [number, number, number]
  /** Camera zoom level (1 = default). */
  zoom: number
  /**
   * Shading mode for the mesh.
   * Phase 0: only 'Shaded' is supported; additional modes added later.
   */
  displayMode: 'Shaded'
  /** Whether face-selection mode is active. */
  selectionMode: boolean
  /** Index of the face currently under the cursor, or null. */
  hoveredFaceIdx: number | null
  /** Fingerprints of all currently selected faces. */
  selectedFaceFingerprints: string[]
  /** Face descriptors for the currently loaded model. */
  faceDescriptors: FaceDescriptor[]
  /** Replace the displayed mesh (pass null to clear the viewport). */
  setMeshData: (m: MeshData | null) => void
  /** Replace the displayed toolpath geometry (pass null to clear). */
  setToolpathGeometry: (g: LineGeometryData | null) => void
  /** Move the orbit camera target to (x, y, z). */
  setOrbitTarget: (x: number, y: number, z: number) => void
  /** Set the camera zoom level. */
  setZoom: (z: number) => void
  /** Enable or disable face-selection mode. Disabling clears hover and descriptors. */
  setSelectionMode: (mode: boolean) => void
  /** Update the hovered face index. */
  setHoveredFaceIdx: (idx: number | null) => void
  /** Add or remove a face fingerprint from the selection. */
  toggleFaceSelection: (fingerprint: string) => void
  /** Clear all selected face fingerprints. */
  clearFaceSelection: () => void
  /** Replace the face descriptor list. */
  setFaceDescriptors: (descriptors: FaceDescriptor[]) => void
}

export const useViewportStore = create<ViewportState>((set) => ({
  meshData: null,
  toolpathGeometry: null,
  orbitTarget: [0, 0, 0],
  zoom: 1,
  displayMode: 'Shaded',
  selectionMode: false,
  hoveredFaceIdx: null,
  selectedFaceFingerprints: [],
  faceDescriptors: [],
  setMeshData: (meshData) => set({ meshData }),
  setToolpathGeometry: (toolpathGeometry) => set({ toolpathGeometry }),
  setOrbitTarget: (x, y, z) => set({ orbitTarget: [x, y, z] }),
  setZoom: (zoom) => set({ zoom }),
  setSelectionMode: (mode) => set(() => mode
    ? { selectionMode: true }
    : { selectionMode: false, hoveredFaceIdx: null, faceDescriptors: [] }
  ),
  setHoveredFaceIdx: (idx) => set({ hoveredFaceIdx: idx }),
  toggleFaceSelection: (fp) => set((s) =>
    s.selectedFaceFingerprints.includes(fp)
      ? { selectedFaceFingerprints: s.selectedFaceFingerprints.filter(f => f !== fp) }
      : { selectedFaceFingerprints: [...s.selectedFaceFingerprints, fp] }
  ),
  clearFaceSelection: () => set({ selectedFaceFingerprints: [] }),
  setFaceDescriptors: (faceDescriptors) => set({ faceDescriptors }),
}))
