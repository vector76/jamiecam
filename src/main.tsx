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
 * Bootstrap the main application window: fetch initial project state and
 * register backend event listeners before the first render.
 */
async function bootstrap(): Promise<void> {
  const setSnapshot = useProjectStore.getState().setSnapshot
  const useMock = import.meta.env.VITE_MOCK_API === 'true'

  // Select real Tauri IPC or mock stubs based on the build-time env var.
  // The mock import stays dynamic so it is only bundled when VITE_MOCK_API is set.
  const { getProjectSnapshot: getSnapshot } = useMock
    ? await import('./api/mock')
    : { getProjectSnapshot }

  // Populate the project store with the current backend state on startup.
  const snapshot = await getSnapshot()
  setSnapshot(snapshot)

  if (!useMock) {
    // Register a listener for backend-initiated project state changes.
    await listen<ProjectSnapshot>('project:modified', (event) => {
      setSnapshot(event.payload)
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
  : bootstrap()

init.catch(console.error)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {isToolEditor ? <ToolEditorWindow /> : <App />}
  </React.StrictMode>,
)
