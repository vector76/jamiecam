/**
 * Zustand store for active project state.
 *
 * Holds the most-recently-fetched ProjectSnapshot so UI components can read
 * project metadata (name, model path, checksum) without issuing an IPC call
 * on every render.
 */

import { create } from 'zustand'
import type { Mode, OperationSummary, ProjectSnapshot, StockDefinition, ToolSummary, WorkCoordinateSystem } from '../api/types'
import { useViewportStore } from './viewportStore'

export type UnsavedChoice = 'save' | 'discard' | 'cancel'

interface ProjectState {
  /** Most-recently-fetched project snapshot, or null before the first fetch. */
  snapshot: ProjectSnapshot | null
  /** Replace the current snapshot (pass null to clear). */
  setSnapshot: (s: ProjectSnapshot | null) => void
  /** Monotonic counter bumped when a new/different project is loaded (not on save). */
  projectLoadGeneration: number
  /** Bump the load generation so mode components re-initialise. */
  bumpLoadGeneration: () => void
  /** Active notification messages shown to the user. */
  notifications: string[]
  /** Append a notification message. */
  pushNotification: (message: string) => void
  /** Remove notification at the given index. */
  dismissNotification: (index: number) => void
  /** The currently-selected operation ID, or null if none. */
  selectedOperationId: string | null
  /** Set the currently-selected operation ID. */
  setSelectedOperationId: (id: string | null) => void
  /** Whether the unsaved-changes dialog is currently open. */
  unsavedDialogOpen: boolean
  /** Resolve callback for the pending unsaved-changes dialog, or null. */
  unsavedDialogResolve: ((choice: UnsavedChoice) => void) | null
  /** Open the unsaved-changes dialog and return the user's choice. */
  showUnsavedDialog: () => Promise<UnsavedChoice>
  /** Resolve the unsaved-changes dialog with the given choice. */
  resolveUnsavedDialog: (choice: UnsavedChoice) => void
  /** Return to the mode selector: clears the snapshot and viewport state. */
  returnToSelector: () => void
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  snapshot: null,
  setSnapshot: (snapshot) => set({ snapshot }),
  projectLoadGeneration: 0,
  bumpLoadGeneration: () => set((s) => ({ projectLoadGeneration: s.projectLoadGeneration + 1 })),
  notifications: [],
  pushNotification: (message) => set((s) => ({ notifications: [...s.notifications, message] })),
  dismissNotification: (index) => set((s) => ({ notifications: s.notifications.filter((_, i) => i !== index) })),
  selectedOperationId: null,
  setSelectedOperationId: (id) => set({ selectedOperationId: id }),
  unsavedDialogOpen: false,
  unsavedDialogResolve: null,
  showUnsavedDialog: () =>
    new Promise<UnsavedChoice>((resolve) => {
      set({ unsavedDialogOpen: true, unsavedDialogResolve: resolve })
    }),
  resolveUnsavedDialog: (choice) => {
    const { unsavedDialogResolve } = get()
    if (unsavedDialogResolve) unsavedDialogResolve(choice)
    set({ unsavedDialogOpen: false, unsavedDialogResolve: null })
  },
  returnToSelector: () => {
    set({ snapshot: null })
    const vp = useViewportStore.getState()
    vp.setMeshData(null)
    vp.setToolpathGeometry(null)
    vp.setSimulationMeshData(null)
  },
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
const EMPTY_WCS: WorkCoordinateSystem[] = []

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
 * Selector hook: returns the WCS list, or an empty array.
 *
 * Re-renders the component only when the wcs array reference changes.
 */
export const useWcs = (): WorkCoordinateSystem[] =>
  useProjectStore((state) => state.snapshot?.wcs ?? EMPTY_WCS)

/**
 * Selector hook: returns the active notification messages array.
 *
 * Re-renders the component only when the notifications array reference changes.
 */
export const useNotifications = (): string[] =>
  useProjectStore((state) => state.notifications)

/**
 * Selector hook: returns the currently-selected operation ID, or null.
 *
 * Re-renders the component only when selectedOperationId changes.
 */
export const useSelectedOperationId = (): string | null =>
  useProjectStore((state) => state.selectedOperationId)

/**
 * Selector hook: returns the pushNotification action.
 *
 * Stable reference — Zustand actions never change identity.
 */
export const usePushNotification = (): ((message: string) => void) =>
  useProjectStore((state) => state.pushNotification)

/**
 * Selector hook: returns the project's file path on disk, or null.
 *
 * Re-renders the component only when filePath changes.
 */
export const useFilePath = (): string | null =>
  useProjectStore((state) => state.snapshot?.filePath ?? null)

/**
 * Selector hook: returns whether the project has unsaved changes.
 *
 * Re-renders the component only when dirty changes. Defaults to false
 * when no snapshot is available.
 */
export const useDirty = (): boolean =>
  useProjectStore((state) => state.snapshot?.dirty ?? false)

/**
 * Selector hook: returns the project load generation counter.
 *
 * Bumped only when a new or different project is loaded (not on save).
 * Mode components use this to re-initialise when the underlying project changes.
 */
export const useProjectLoadGeneration = (): number =>
  useProjectStore((state) => state.projectLoadGeneration)

/**
 * Selector hook: returns the current top-level view.
 *
 * Returns `'selector'` when no project is open, otherwise returns the
 * project's active mode. The return type is `'selector' | Mode` (not
 * `string`) so callers can use exhaustive narrowing.
 */
export const useCurrentView = (): 'selector' | Mode =>
  useProjectStore((state) => {
    const s = state.snapshot
    if (!s || !s.projectIsOpen) return 'selector'
    return s.mode
  })
