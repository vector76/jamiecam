import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { useProjectStore } from './store/projectStore'
import type { ProjectSnapshot } from './api/types'

/**
 * Bootstrap the application: fetch initial project state and register backend
 * event listeners before the first render.
 *
 * Uses dynamic imports so that the real Tauri API and the mock API are not
 * both bundled in production — Vite eliminates the dead branch when
 * VITE_MOCK_API is set at build time.
 */
async function bootstrap(): Promise<void> {
  const setSnapshot = useProjectStore.getState().setSnapshot
  const useMock = import.meta.env.VITE_MOCK_API === 'true'

  // Select real Tauri IPC or mock stubs based on the build-time env var.
  const { getProjectSnapshot } = useMock
    ? await import('./api/mock')
    : await import('./api/file')

  // Populate the project store with the current backend state on startup.
  const snapshot = await getProjectSnapshot()
  setSnapshot(snapshot)

  if (!useMock) {
    // Register a listener for backend-initiated project state changes.
    // Phase 0: this event is never emitted by the backend; the listener is
    // in place so future phases can push updates without changing main.tsx.
    const { listen } = await import('@tauri-apps/api/event')
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
