/**
 * Toolbar — primary file operation buttons.
 *
 * Provides Open Model, New Project, Save Project, and Open Project actions.
 * Each action calls a Tauri native dialog for path selection then delegates
 * to the IPC API layer.  Errors are surfaced as a dismissible banner; they
 * are never silently swallowed.
 */

import { useState } from 'react'
import { open, save } from '@tauri-apps/plugin-dialog'
import * as api from '../../api/file'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import type { AppError, ProjectSnapshot } from '../../api/types'

// ── Window title helper ────────────────────────────────────────────────────────

async function updateWindowTitle(snapshot: ProjectSnapshot): Promise<void> {
  const filename = snapshot.modelPath?.split('/').pop() ?? snapshot.modelPath
  const title = snapshot.projectName || filename || 'JamieCam'
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTitle(title)
  } catch {
    // Not running inside Tauri (tests, browser dev mode) — safe to ignore.
  }
}

// ── Component ─────────────────────────────────────────────────────────────────

export function Toolbar() {
  const [errorMsg, setErrorMsg] = useState<string | null>(null)

  const setSnapshot = useProjectStore((s) => s.setSnapshot)
  const setMeshData = useViewportStore((s) => s.setMeshData)

  function dismissError() {
    setErrorMsg(null)
  }

  function showError(e: unknown) {
    const err = e as AppError
    setErrorMsg(err.message ?? err.kind ?? 'An error occurred')
  }

  // ── Open Model ───────────────────────────────────────────────────────────

  async function handleOpenModel() {
    const path = await open({
      filters: [{ name: 'CAD Files', extensions: ['step', 'stp', 'stl'] }],
    })
    if (!path) return
    try {
      const meshData = await api.openModel(path)
      const snapshot = await api.getProjectSnapshot()
      setMeshData(meshData)
      setSnapshot(snapshot)
      await updateWindowTitle(snapshot)
    } catch (e: unknown) {
      showError(e)
    }
  }

  // ── New Project ──────────────────────────────────────────────────────────

  async function handleNewProject() {
    try {
      const snapshot = await api.newProject()
      setSnapshot(snapshot)
      setMeshData(null)
      await updateWindowTitle(snapshot)
    } catch (e: unknown) {
      showError(e)
    }
  }

  // ── Save Project ─────────────────────────────────────────────────────────

  async function handleSaveProject() {
    const path = await save({
      filters: [{ name: 'JamieCam Project', extensions: ['jcam'] }],
    })
    if (!path) return
    try {
      await api.saveProject(path)
    } catch (e: unknown) {
      showError(e)
    }
  }

  // ── Open Project ─────────────────────────────────────────────────────────

  async function handleOpenProject() {
    const path = await open({
      filters: [{ name: 'JamieCam Project', extensions: ['jcam'] }],
    })
    if (!path) return
    try {
      const snapshot = await api.loadProject(path)
      setSnapshot(snapshot)
      if (snapshot.modelPath) {
        const meshData = await api.openModel(snapshot.modelPath)
        setMeshData(meshData)
      } else {
        setMeshData(null)
      }
      await updateWindowTitle(snapshot)
    } catch (e: unknown) {
      showError(e)
    }
  }

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', padding: '0.25rem 0.5rem', borderBottom: '1px solid #ccc' }}>
      {errorMsg && (
        <div role="alert" style={{ color: 'red', marginRight: '0.5rem' }}>
          <span>{errorMsg}</span>
          <button onClick={dismissError} aria-label="Dismiss error" style={{ marginLeft: '0.25rem' }}>
            ✕
          </button>
        </div>
      )}
      <button onClick={() => void handleOpenModel()}>Open Model</button>
      <button onClick={() => void handleNewProject()}>New Project</button>
      <button onClick={() => void handleSaveProject()}>Save Project</button>
      <button onClick={() => void handleOpenProject()}>Open Project</button>
    </div>
  )
}
