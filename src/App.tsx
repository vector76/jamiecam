/**
 * App shell — top-level mounting + mode dispatch.
 *
 * The shell owns two pieces of state that span all modes:
 *   - the active project mode (defaults to Mode 1 on cold start)
 *   - the optional `initialProject` to hydrate the active mode with
 *     when a `.jcam` is opened here at the shell level.
 *
 * Opening a `.jcam` reads its `mode` field, switches the shell to that
 * mode, and remounts the mode component with the unpacked project state
 * via an `instanceKey` bump (so the mode component re-runs its mount
 * effects against the new seed instead of trying to reconcile in place).
 *
 * "New Project" lets the user pick a mode for a fresh project. This is
 * the only way to enter Mode 2 without opening a file.
 */

import { useRef, useState } from 'react'
import { ToolpathViewerMode } from './components/modes/ToolpathViewerMode'
import { Mode2ProfileMode } from './components/modes/Mode2ProfileMode'
import { Button } from './components/ui/button'
import {
  unpackJcamProject,
  JcamFormatError,
  type ProjectMode,
  type ProjectState,
} from './persistence/projectFile'

const DEFAULT_MODE: ProjectMode = 'gcode-viewer'

const MODE_LABELS: Record<ProjectMode, string> = {
  'gcode-viewer': 'G-code Viewer',
  '2d-profile': '2-D Profile',
}

export default function App() {
  const [mode, setMode] = useState<ProjectMode>(DEFAULT_MODE)
  const [initialProject, setInitialProject] = useState<ProjectState | null>(null)
  const [instanceKey, setInstanceKey] = useState(0)
  const [shellError, setShellError] = useState<string | null>(null)
  const projectInputRef = useRef<HTMLInputElement | null>(null)

  function startNew(target: ProjectMode) {
    setShellError(null)
    setInitialProject(null)
    setMode(target)
    setInstanceKey((k) => k + 1)
  }

  function handlePickProject() {
    projectInputRef.current?.click()
  }

  async function handleProjectChosen(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    setShellError(null)
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      const state = unpackJcamProject(bytes)
      setMode(state.mode)
      setInitialProject(state)
      setInstanceKey((k) => k + 1)
    } catch (err) {
      const msg = err instanceof JcamFormatError ? err.message : (err as Error).message
      setShellError(msg || 'Failed to open project file')
    }
  }

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex items-center gap-2 border-b border-border px-3 py-2">
        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          New Project:
        </span>
        {(Object.keys(MODE_LABELS) as ProjectMode[]).map((m) => (
          <Button
            key={m}
            size="sm"
            variant={mode === m ? 'default' : 'secondary'}
            onClick={() => startNew(m)}
          >
            {MODE_LABELS[m]}
          </Button>
        ))}
        <span aria-hidden="true" className="mx-1 h-4 w-px bg-border" />
        <Button size="sm" variant="secondary" onClick={handlePickProject}>
          Open Project…
        </Button>
        {shellError && (
          <p role="alert" className="ml-3 truncate text-xs text-destructive">
            {shellError}
          </p>
        )}
        <input
          ref={projectInputRef}
          type="file"
          accept=".jcam"
          onChange={handleProjectChosen}
          className="hidden"
          aria-label="Shell project file"
        />
      </header>
      <div className="flex flex-1 overflow-hidden">
        {mode === 'gcode-viewer' && (
          <ToolpathViewerMode
            key={`gcode-viewer-${instanceKey}`}
            initialProject={initialProject}
          />
        )}
        {mode === '2d-profile' && (
          <Mode2ProfileMode
            key={`2d-profile-${instanceKey}`}
            initialProject={initialProject}
          />
        )}
      </div>
    </div>
  )
}
