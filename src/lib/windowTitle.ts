import { getCurrentWindow } from '@tauri-apps/api/window'
import type { ProjectSnapshot } from '../api/types'

/**
 * Compute the window title from the current project state.
 *
 * Format: "<filename>[*] — JamieCam"
 * Uses "Untitled" when no file path is set. Appends * when dirty.
 */
export function computeWindowTitle(filePath: string | null, dirty: boolean): string {
  let name: string
  if (filePath) {
    // Handle both Unix (/) and Windows (\) path separators.
    const segments = filePath.split(/[/\\]/)
    name = segments[segments.length - 1]
  } else {
    name = 'Untitled'
  }
  return `${name}${dirty ? '*' : ''} \u2014 JamieCam`
}

/**
 * Update the Tauri window title based on the current project snapshot.
 * Silently ignores errors when not running inside a Tauri webview.
 */
export async function updateWindowTitle(snapshot: ProjectSnapshot): Promise<void> {
  const title = computeWindowTitle(snapshot.filePath, snapshot.dirty)
  try {
    await getCurrentWindow().setTitle(title)
  } catch {
    // Not running inside Tauri (tests, browser dev mode) — safe to ignore.
  }
}
