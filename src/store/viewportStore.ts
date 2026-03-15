/**
 * Zustand store for 3-D viewport state.
 *
 * Holds the tessellated mesh currently displayed, the orbit camera target,
 * zoom level, and display mode. All viewport UI components derive their
 * rendering parameters from this store.
 */

import { create } from 'zustand'
import type { MeshData, LineGeometryData, FaceDescriptor } from '../api/types'
import type { SimPoint } from '../viewport/simulationPoints'
import { distanceBetweenPoints, angleBetweenThreePoints } from '../viewport/measurementMath'

export type DisplayMode = 'shaded' | 'shaded-edges' | 'wireframe' | 'transparent'

export interface Measurement {
  points: [number, number, number][]
  value: number
  label: string
  anchor: [number, number, number]
}

interface ViewportState {
  /** Tessellated mesh currently loaded into the viewport, or null. */
  meshData: MeshData | null
  /** Toolpath line geometry currently loaded into the viewport, or null. */
  toolpathGeometry: LineGeometryData | null
  /** World-space point the orbit camera rotates around (x, y, z). */
  orbitTarget: [number, number, number]
  /** Camera zoom level (1 = default). */
  zoom: number
  /** Shading mode for the mesh. */
  displayMode: DisplayMode
  /** Camera projection mode. */
  projectionMode: 'perspective' | 'orthographic'
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
  /** Set the display mode. */
  setDisplayMode: (mode: DisplayMode) => void
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
  /** Set the camera projection mode. */
  setProjectionMode: (mode: 'perspective' | 'orthographic') => void
  /** Whether simulation is active (playing or paused). */
  simulationActive: boolean
  /** Whether simulation is paused (simulationActive must also be true). */
  simulationPaused: boolean
  /** Current point index along the simulation path. */
  simulationPointIndex: number
  /** Playback speed multiplier (default 10.0 = 10× real feed rate). */
  simulationPlaybackSpeed: number
  /** Simulation points set by startSimulation, cleared by stopSimulation. */
  simulationPoints: SimPoint[] | null
  startSimulation: (points: SimPoint[]) => void
  pauseSimulation: () => void
  resumeSimulation: () => void
  stopSimulation: () => void
  setSimulationPointIndex: (idx: number) => void
  setSimulationPlaybackSpeed: (speed: number) => void
  /** Current measurement mode. */
  measurementMode: 'off' | 'distance' | 'angle'
  /** Points collected so far for the in-progress measurement. */
  measurementPoints: [number, number, number][]
  /** All completed measurements. */
  measurements: Measurement[]
  setMeasurementMode: (mode: 'off' | 'distance' | 'angle') => void
  addMeasurementPoint: (point: [number, number, number]) => void
  clearMeasurements: () => void
  removeMeasurement: (index: number) => void
}

export const useViewportStore = create<ViewportState>((set) => ({
  meshData: null,
  toolpathGeometry: null,
  orbitTarget: [0, 0, 0],
  zoom: 1,
  displayMode: 'shaded',
  projectionMode: 'perspective',
  selectionMode: false,
  hoveredFaceIdx: null,
  selectedFaceFingerprints: [],
  faceDescriptors: [],
  setMeshData: (meshData) => set({ meshData }),
  setToolpathGeometry: (toolpathGeometry) => set({ toolpathGeometry }),
  setOrbitTarget: (x, y, z) => set({ orbitTarget: [x, y, z] }),
  setZoom: (zoom) => set({ zoom }),
  setDisplayMode: (displayMode) => set({ displayMode }),
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
  setProjectionMode: (projectionMode) => set({ projectionMode }),
  simulationActive: false,
  simulationPaused: false,
  simulationPointIndex: 0,
  simulationPlaybackSpeed: 10.0,
  simulationPoints: null,
  startSimulation: (points) => set({ simulationActive: true, simulationPaused: false, simulationPointIndex: 0, simulationPoints: points }),
  pauseSimulation: () => set({ simulationPaused: true }),
  resumeSimulation: () => set({ simulationPaused: false }),
  stopSimulation: () => set({ simulationActive: false, simulationPaused: false, simulationPointIndex: 0, simulationPoints: null }),
  setSimulationPointIndex: (idx) => set({ simulationPointIndex: idx }),
  setSimulationPlaybackSpeed: (speed) => set({ simulationPlaybackSpeed: speed }),
  measurementMode: 'off',
  measurementPoints: [],
  measurements: [],
  setMeasurementMode: (mode) => set({ measurementMode: mode, measurementPoints: [] }),
  addMeasurementPoint: (point) => set((s) => {
    const pts = s.measurementPoints
    if (s.measurementMode === 'distance' && pts.length === 1) {
      const p1 = pts[0]
      const p2 = point
      const distance = distanceBetweenPoints(p1, p2)
      const anchor: [number, number, number] = [
        (p1[0] + p2[0]) / 2,
        (p1[1] + p2[1]) / 2,
        (p1[2] + p2[2]) / 2,
      ]
      return {
        measurements: [...s.measurements, { points: [p1, p2], value: distance, label: `${distance.toFixed(1)} mm`, anchor }],
        measurementPoints: [],
      }
    }
    if (s.measurementMode === 'angle' && pts.length === 2) {
      const p1 = pts[0]
      const vertex = pts[1]
      const p3 = point
      const angle = angleBetweenThreePoints(p1, vertex, p3)
      return {
        measurements: [...s.measurements, { points: [p1, vertex, p3], value: angle, label: `${angle.toFixed(1)}°`, anchor: vertex }],
        measurementPoints: [],
      }
    }
    return { measurementPoints: [...pts, point] }
  }),
  clearMeasurements: () => set({ measurements: [], measurementPoints: [] }),
  removeMeasurement: (index) => set((s) => ({
    measurements: s.measurements.filter((_, i) => i !== index),
  })),
}))
