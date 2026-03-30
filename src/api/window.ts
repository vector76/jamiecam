/**
 * Window management utilities for opening secondary Tauri windows.
 */

import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

/**
 * Open the tool editor in a secondary window, or focus it if already open.
 *
 * The window renders the same frontend bundle (url: '/') and uses the
 * label 'tool-editor' so main.tsx routes it to the ToolEditorWindow component.
 */
export async function openToolEditor(): Promise<void> {
  const existing = await WebviewWindow.getByLabel('tool-editor')
  if (existing) {
    await existing.setFocus()
    return
  }

  const win = new WebviewWindow('tool-editor', {
    url: '/',
    title: 'Tool Editor',
    width: 900,
    height: 650,
  })

  win.once('tauri://error', (e) => {
    console.error('Failed to create tool editor window:', e)
  })
}
