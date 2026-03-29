import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { useProjectStore } from './store/projectStore'
import type { ProjectSnapshot } from './api/types'
import { getProjectSnapshot } from './api/file'
import { listen } from '@tauri-apps/api/event'

/**
 * Bootstrap the application: fetch initial project state and register backend
 * event listeners before the first render.
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
    // Phase 0: this event is never emitted by the backend; the listener is
    // in place so future phases can push updates without changing main.tsx.
    await listen<ProjectSnapshot>('project:modified', (event) => {
      setSnapshot(event.payload)
    })
  }
}

// Run bootstrap; errors are logged but do not prevent the UI from rendering.
bootstrap().catch(console.error)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
