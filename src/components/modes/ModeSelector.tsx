import { useState } from 'react'
import { MODES } from './modeConfig'
import * as api from '../../api/file'
import type { AppError } from '../../api/types'
import { useProjectStore } from '../../store/projectStore'
import { handleOpenProject } from '../../lib/menuActions'
import { Button } from '../ui/button'

export function ModeSelector() {
  const [loading, setLoading] = useState(false)

  async function handleModeClick(id: (typeof MODES)[number]['id']) {
    setLoading(true)
    try {
      const snapshot = await api.newProject(id)
      useProjectStore.getState().setSnapshot(snapshot)
      useProjectStore.getState().bumpLoadGeneration()
    } catch (e: unknown) {
      const err = e as AppError
      useProjectStore.getState().pushNotification(err.message ?? err.kind ?? 'An error occurred')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background p-8 gap-8">
      <h1 className="text-2xl font-semibold text-foreground">Select a Mode</h1>

      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 max-w-4xl w-full">
        {MODES.map((mode) => (
          <button
            key={mode.id}
            type="button"
            aria-label={mode.label}
            disabled={loading}
            onClick={() => handleModeClick(mode.id)}
            className="flex flex-col gap-2 rounded-xl border border-border bg-card px-6 py-6 text-left text-card-foreground shadow-sm transition-colors hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
          >
            <span className="text-sm font-semibold leading-none">{mode.label}</span>
            <span className="text-xs text-muted-foreground">{mode.description}</span>
          </button>
        ))}
      </div>

      <Button variant="outline" disabled={loading} onClick={handleOpenProject}>
        Open Project
      </Button>
    </div>
  )
}
