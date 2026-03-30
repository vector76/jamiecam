/**
 * Zustand store for global tool library state.
 *
 * Keeps the full Tool[] from the global library in memory so UI components
 * can read it without issuing an IPC call on every render.
 */

import { create } from 'zustand'
import type { Tool } from '../api/types'
import { listGlobalTools } from '../api/globalTools'
import { getProjectSnapshot } from '../api/file'
import { useProjectStore } from './projectStore'

interface GlobalToolState {
  /** All tools in the global library. */
  globalTools: Tool[]
  /** Replace the global tools list. */
  setGlobalTools: (tools: Tool[]) => void
  /** Fetch the global tools from the backend and update the store. */
  refreshGlobalTools: () => Promise<void>
}

const EMPTY_GLOBAL_TOOLS: Tool[] = []

export const useGlobalToolStore = create<GlobalToolState>((set) => ({
  globalTools: EMPTY_GLOBAL_TOOLS,
  setGlobalTools: (globalTools) => set({ globalTools }),
  refreshGlobalTools: async () => {
    const tools = await listGlobalTools()
    set({ globalTools: tools })
  },
}))

/** Selector hook: returns the global tool list, or a stable empty array. */
export const useGlobalTools = (): Tool[] =>
  useGlobalToolStore((state) => state.globalTools)

/**
 * Refresh the project snapshot in the project store.
 *
 * Fetches the current snapshot from the backend and sets it in the store.
 * Useful after mutations that change project state.
 */
export async function refreshProjectSnapshot(): Promise<void> {
  const snapshot = await getProjectSnapshot()
  useProjectStore.getState().setSnapshot(snapshot)
}
