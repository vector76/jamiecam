import './index.css'
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { ToolEditorWindow } from './components/tools/ToolEditorWindow'
import { useProjectStore } from './store/projectStore'
import { useGlobalToolStore } from './store/globalToolStore'
import type { ProjectSnapshot } from './api/types'
import { getProjectSnapshot } from './api/file'
import { listGlobalTools } from './api/globalTools'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { updateWindowTitle } from './lib/windowTitle'
import { menuActionDispatch } from './lib/menuActions'
import { checkUnsavedChanges } from './lib/unsavedGuard'

/**
 * Detect the current Tauri window label, falling back to 'main'
 * when running outside of Tauri (tests, browser dev mode).
 */
export function getWindowLabel(): string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (window as any).__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? 'main'
  } catch {
    return 'main'
  }
}

/**
 * Bootstrap the main application window: register backend event listeners.
 *
 * Does not call getProjectSnapshot() — initial state is loaded by the selector
 * or by the project-open flow.
 */
export async function bootstrapApp(): Promise<void> {
  const useMock = import.meta.env.VITE_MOCK_API === 'true'

  if (!useMock) {
    // Register a listener for backend-initiated project state changes.
    // Skip the update if snapshot is null (user is on the selector screen)
    // so no backend event can accidentally flip the UI back to a mode view.
    await listen<ProjectSnapshot>('project:modified', (event) => {
      const { snapshot, setSnapshot } = useProjectStore.getState()
      if (snapshot === null) return
      setSnapshot(event.payload)
      updateWindowTitle(event.payload)
    })

    // Dispatch native menu actions to the shared handler functions.
    await listen<string>('menu:action', (event) => {
      const handler = menuActionDispatch[event.payload]
      if (handler) handler()
    })

    // Intercept window close to guard unsaved changes.
    const currentWindow = getCurrentWindow()
    await currentWindow.onCloseRequested(async (event) => {
      const proceed = await checkUnsavedChanges()
      if (!proceed) {
        event.preventDefault()
      }
    })
  }
}

/**
 * Bootstrap the tool editor window: fetch global tools and project state,
 * then register event listeners.
 */
export async function bootstrapToolEditor(): Promise<void> {
  const setSnapshot = useProjectStore.getState().setSnapshot
  const setGlobalTools = useGlobalToolStore.getState().setGlobalTools

  const [snapshot, globalTools] = await Promise.all([
    getProjectSnapshot(),
    listGlobalTools(),
  ])

  setSnapshot(snapshot)
  setGlobalTools(globalTools)

  await listen<ProjectSnapshot>('project:modified', (event) => {
    setSnapshot(event.payload)
  })
}

// ── Entry point ──────────────────────────────────────────────────────────────

const label = getWindowLabel()
const isToolEditor = label === 'tool-editor'

// Run the appropriate bootstrap for this window.
const init = isToolEditor
  ? bootstrapToolEditor()
  : bootstrapApp()

init.catch(console.error)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {isToolEditor ? <ToolEditorWindow /> : <App />}
  </React.StrictMode>,
)
