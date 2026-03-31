/**
 * Guard function that checks for unsaved changes before destructive actions.
 *
 * Call `checkUnsavedChanges()` before any action that would discard the
 * current project state (New Project, Open Project, etc.). Returns true if
 * it is safe to proceed, false if the action should be aborted.
 */

import { save } from '@tauri-apps/plugin-dialog'
import { saveProject, saveProjectCurrent } from '../api/file'
import { useProjectStore } from '../store/projectStore'

export async function checkUnsavedChanges(): Promise<boolean> {
  const { snapshot, showUnsavedDialog, pushNotification } =
    useProjectStore.getState()

  const dirty = snapshot?.dirty ?? false
  if (!dirty) return true

  const choice = await showUnsavedDialog()

  if (choice === 'cancel') return false

  if (choice === 'discard') return true

  // choice === 'save'
  try {
    if (snapshot?.filePath) {
      await saveProjectCurrent()
    } else {
      const path = await save({
        filters: [{ name: 'JamieCam Project', extensions: ['jcam'] }],
      })
      if (!path) return false
      await saveProject(path)
    }
    return true
  } catch {
    pushNotification('Failed to save project.')
    return false
  }
}
