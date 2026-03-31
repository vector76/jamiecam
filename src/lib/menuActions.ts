/**
 * Shared menu action handlers.
 *
 * Standalone async functions that access Zustand stores via getState() so
 * they can be called from both React components and non-React contexts
 * (native menu event listeners).
 *
 * Errors are surfaced through the project store's notification system.
 */

import { open, save } from '@tauri-apps/plugin-dialog'
import * as api from '../api/file'
import { getToolpathGeometry } from '../api/toolpath'
import { useProjectStore } from '../store/projectStore'
import { useViewportStore } from '../store/viewportStore'
import { updateWindowTitle } from './windowTitle'
import { checkUnsavedChanges } from './unsavedGuard'
import type { AppError } from '../api/types'

function notify(e: unknown): void {
  const err = e as AppError
  const message = err.message ?? err.kind ?? 'An error occurred'
  useProjectStore.getState().pushNotification(message)
}

export async function handleOpenModel(): Promise<void> {
  const path = await open({
    filters: [{ name: 'CAD Files', extensions: ['step', 'stp', 'stl'] }],
  })
  if (!path) return
  try {
    const meshData = await api.openModel(path)
    const snapshot = await api.getProjectSnapshot()
    useViewportStore.getState().setMeshData(meshData)
    useProjectStore.getState().setSnapshot(snapshot)
    await updateWindowTitle(snapshot)
  } catch (e: unknown) {
    notify(e)
  }
}

export async function handleNewProject(): Promise<void> {
  const proceed = await checkUnsavedChanges()
  if (!proceed) return
  try {
    const snapshot = await api.newProject()
    useProjectStore.getState().setSnapshot(snapshot)
    useViewportStore.getState().setMeshData(null)
    await updateWindowTitle(snapshot)
  } catch (e: unknown) {
    notify(e)
  }
}

export async function handleSaveAs(): Promise<void> {
  const path = await save({
    filters: [{ name: 'JamieCam Project', extensions: ['jcam'] }],
  })
  if (!path) return
  try {
    await api.saveProject(path)
  } catch (e: unknown) {
    notify(e)
  }
}

export async function handleSave(): Promise<void> {
  const snapshot = useProjectStore.getState().snapshot
  if (snapshot?.filePath) {
    try {
      await api.saveProjectCurrent()
    } catch (e: unknown) {
      notify(e)
    }
  } else {
    await handleSaveAs()
  }
}

export async function handleOpenProject(): Promise<void> {
  const proceed = await checkUnsavedChanges()
  if (!proceed) return
  const path = await open({
    filters: [{ name: 'JamieCam Project', extensions: ['jcam'] }],
  })
  if (!path) return
  try {
    const snapshot = await api.loadProject(path)
    useProjectStore.getState().setSnapshot(snapshot)
    if (snapshot.modelPath) {
      const meshData = await api.openModel(snapshot.modelPath)
      useViewportStore.getState().setMeshData(meshData)
    } else {
      useViewportStore.getState().setMeshData(null)
    }
    for (const op of snapshot.operations) {
      if (!op.needsRecalculate) {
        try {
          const geometry = await getToolpathGeometry(op.id)
          useViewportStore.getState().setToolpathGeometry(geometry)
        } catch {
          // Non-fatal: toolpath may not be available; leave viewport as-is.
        }
      }
    }
    await updateWindowTitle(snapshot)
  } catch (e: unknown) {
    notify(e)
  }
}

/**
 * Menu-action dispatch table: maps native menu item IDs to handler functions.
 * Used by the bootstrap listener for `menu:action` events.
 */
export const menuActionDispatch: Record<string, () => Promise<void>> = {
  'new-project': handleNewProject,
  'open-project': handleOpenProject,
  'open-model': handleOpenModel,
  'save': handleSave,
  'save-as': handleSaveAs,
}
