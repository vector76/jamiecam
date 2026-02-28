/**
 * Zustand store for active project state.
 *
 * Holds the most-recently-fetched ProjectSnapshot so UI components can read
 * project metadata (name, model path, checksum) without issuing an IPC call
 * on every render.
 */

import { create } from 'zustand'
import type { OperationSummary, ProjectSnapshot, StockDefinition, ToolSummary } from '../api/types'

interface ProjectState {
  /** Most-recently-fetched project snapshot, or null before the first fetch. */
  snapshot: ProjectSnapshot | null
  /** Replace the current snapshot (pass null to clear). */
  setSnapshot: (s: ProjectSnapshot | null) => void
  /** Active notification messages shown to the user. */
  notifications: string[]
  /** Append a notification message. */
  pushNotification: (message: string) => void
  /** Remove notification at the given index. */
  dismissNotification: (index: number) => void
}

export const useProjectStore = create<ProjectState>((set) => ({
  snapshot: null,
  setSnapshot: (snapshot) => set({ snapshot }),
  notifications: [],
  pushNotification: (message) => set((s) => ({ notifications: [...s.notifications, message] })),
  dismissNotification: (index) => set((s) => ({ notifications: s.notifications.filter((_, i) => i !== index) })),
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

// Stable empty arrays used as fallback defaults so that selectors returning
// arrays don't create a new reference on every call (which would cause Zustand
// to see a changed value and trigger an infinite re-render loop).
const EMPTY_OPERATIONS: OperationSummary[] = []
const EMPTY_TOOLS: ToolSummary[] = []

/**
 * Selector hook: returns the operation summary list, or an empty array.
 *
 * Re-renders the component only when the operations array reference changes.
 */
export const useOperations = (): OperationSummary[] =>
  useProjectStore((state) => state.snapshot?.operations ?? EMPTY_OPERATIONS)

/**
 * Selector hook: returns the tool summary list, or an empty array.
 *
 * Re-renders the component only when the tools array reference changes.
 */
export const useTools = (): ToolSummary[] =>
  useProjectStore((state) => state.snapshot?.tools ?? EMPTY_TOOLS)

/**
 * Selector hook: returns the stock definition, or null if not set.
 *
 * Re-renders the component only when the stock value changes.
 */
export const useStock = (): StockDefinition | null =>
  useProjectStore((state) => state.snapshot?.stock ?? null)

/**
 * Selector hook: returns the active notification messages array.
 *
 * Re-renders the component only when the notifications array reference changes.
 */
export const useNotifications = (): string[] =>
  useProjectStore((state) => state.notifications)
