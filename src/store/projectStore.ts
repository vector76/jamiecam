/**
 * Zustand store for active project state.
 *
 * Holds the most-recently-fetched ProjectSnapshot so UI components can read
 * project metadata (name, model path, checksum) without issuing an IPC call
 * on every render.
 */

import { create } from 'zustand'
import type { ProjectSnapshot } from '../api/types'

interface ProjectState {
  /** Most-recently-fetched project snapshot, or null before the first fetch. */
  snapshot: ProjectSnapshot | null
  /** Replace the current snapshot (pass null to clear). */
  setSnapshot: (s: ProjectSnapshot | null) => void
}

export const useProjectStore = create<ProjectState>((set) => ({
  snapshot: null,
  setSnapshot: (snapshot) => set({ snapshot }),
}))

/**
 * Selector hook: returns the loaded model's absolute path, or null.
 *
 * Re-renders the component only when modelPath changes.
 */
export const useModelPath = (): string | null =>
  useProjectStore((state) => state.snapshot?.modelPath ?? null)

/**
 * Selector hook: returns the loaded model's SHA-256 checksum, or null.
 *
 * Re-renders the component only when modelChecksum changes.
 */
export const useModelChecksum = (): string | null =>
  useProjectStore((state) => state.snapshot?.modelChecksum ?? null)
