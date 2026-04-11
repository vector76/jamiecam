import { create } from 'zustand'

const ZOOM_MIN = 0.05
const ZOOM_MAX = 50.0

interface Canvas2dState {
  panOffset: { x: number; y: number }
  zoom: number
  selectedCurveId: string | null
  setPanOffset: (offset: { x: number; y: number }) => void
  setZoom: (zoom: number) => void
  setSelectedCurveId: (id: string | null) => void
  resetView: () => void
}

export const useCanvas2dStore = create<Canvas2dState>((set) => ({
  panOffset: { x: 0, y: 0 },
  zoom: 1.0,
  selectedCurveId: null,
  setPanOffset: (offset) => set({ panOffset: offset }),
  setZoom: (zoom) => set({ zoom: Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom)) }),
  setSelectedCurveId: (id) => set({ selectedCurveId: id }),
  resetView: () => set({ panOffset: { x: 0, y: 0 }, zoom: 1.0, selectedCurveId: null }),
}))
