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
import { getToolpathGeometry } from '../../api/toolpath'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import { Button } from '@/components/ui/button'
import { FolderOpen, FilePlus, Save, FolderInput, X, AlertTriangle } from 'lucide-react'
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
  const setToolpathGeometry = useViewportStore((s) => s.setToolpathGeometry)

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
      for (const op of snapshot.operations) {
        if (!op.needsRecalculate) {
          try {
            const geometry = await getToolpathGeometry(op.id)
            setToolpathGeometry(geometry)
          } catch {
            // Non-fatal: toolpath may not be available; leave viewport as-is.
          }
        }
      }
      await updateWindowTitle(snapshot)
    } catch (e: unknown) {
      showError(e)
    }
  }

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div className="flex items-center gap-1.5 border-b border-border bg-card px-2 py-1">
      {errorMsg && (
        <div role="alert" className="flex items-center gap-1.5 rounded-sm bg-destructive/10 px-2 py-1 text-xs text-destructive">
          <AlertTriangle className="h-3.5 w-3.5" />
          <span>{errorMsg}</span>
          <button onClick={dismissError} aria-label="Dismiss error" className="ml-1 rounded-sm p-0.5 hover:bg-destructive/20">
            <X className="h-3 w-3" />
          </button>
        </div>
      )}
      <Button variant="ghost" size="sm" onClick={() => void handleOpenModel()}>
        <FolderOpen className="mr-1 h-3.5 w-3.5" />
        Open Model
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleNewProject()}>
        <FilePlus className="mr-1 h-3.5 w-3.5" />
        New Project
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleSaveProject()}>
        <Save className="mr-1 h-3.5 w-3.5" />
        Save Project
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleOpenProject()}>
        <FolderInput className="mr-1 h-3.5 w-3.5" />
        Open Project
      </Button>
    </div>
  )
}
