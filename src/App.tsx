/**
 * App shell — top-level mounting + mode dispatch.
 *
 * The shell owns three pieces of state that span all modes:
 *   - the active project mode (defaults to Mode 1 on cold start)
 *   - the optional `initialProject` to hydrate the active mode with
 *     when a `.jcam` is opened here at the shell level
 *   - the mode-agnostic Recents list and the current savable
 *     ProjectState (so Save Project lives in the shell, not the mode)
 *
 * Modes accept two new props:
 *   - `initialProject`        — hydration seed (read-once on mount)
 *   - `onProjectStateChange`  — fires whenever the mode's savable state
 *                               changes (or back to `null` if there's
 *                               nothing to save). The shell uses this
 *                               to drive Save Project + Recents.
 *
 * Opening a `.jcam` reads its `mode` field, switches the shell to that
 * mode, and remounts the mode component with the unpacked project state
 * via an `instanceKey` bump (so the mode component re-runs its mount
 * effects against the new seed instead of trying to reconcile in place).
 *
 * "New Project" lets the user pick a mode for a fresh project. This is
 * the only way to enter Mode 2 without opening a file.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { ToolpathViewerMode } from './components/modes/ToolpathViewerMode'
import { Mode2ProfileMode } from './components/modes/Mode2ProfileMode'
import { Button } from './components/ui/button'
import {
  packJcamProject,
  unpackJcamProject,
  JcamFormatError,
  type ProjectMode,
  type ProjectState,
} from './persistence/projectFile'
import {
  listRecents,
  upsertRecent,
  type RecentRecord,
} from './persistence/recents'

const DEFAULT_MODE: ProjectMode = 'gcode-viewer'

const MODE_LABELS: Record<ProjectMode, string> = {
  'gcode-viewer': 'G-code Viewer',
  '2d-profile': '2-D Profile',
}

/** Short badge label rendered next to each Recent entry. */
const MODE_BADGES: Record<ProjectMode, string> = {
  'gcode-viewer': 'GC',
  '2d-profile': '2D',
}

function jcamFileName(sourceName: string): string {
  const dot = sourceName.lastIndexOf('.')
  const stem = dot > 0 ? sourceName.slice(0, dot) : sourceName
  return `${stem}.jcam`
}

function triggerDownload(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = fileName
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

export default function App() {
  const [mode, setMode] = useState<ProjectMode>(DEFAULT_MODE)
  const [initialProject, setInitialProject] = useState<ProjectState | null>(null)
  const [instanceKey, setInstanceKey] = useState(0)
  const [shellError, setShellError] = useState<string | null>(null)
  const [currentProjectState, setCurrentProjectState] = useState<ProjectState | null>(null)
  const [recents, setRecents] = useState<RecentRecord[]>([])
  const projectInputRef = useRef<HTMLInputElement | null>(null)

  const refreshRecents = useCallback(async () => {
    try {
      setRecents(await listRecents())
    } catch {
      // IndexedDB unavailable (private-mode Firefox, locked-down browser);
      // silently degrade — recents list just stays empty.
      setRecents([])
    }
  }, [])

  useEffect(() => {
    void refreshRecents()
  }, [refreshRecents])

  // Fires whenever the active mode's savable state changes. A non-null
  // value gets immediately persisted to Recents so the file is recoverable
  // even if the user closes the tab without hitting Save Project.
  const handleProjectStateChange = useCallback(
    (state: ProjectState | null) => {
      setCurrentProjectState(state)
      if (state !== null) {
        void (async () => {
          await upsertRecent(state)
          await refreshRecents()
        })()
      }
    },
    [refreshRecents],
  )

  function startNew(target: ProjectMode) {
    setShellError(null)
    setInitialProject(null)
    setCurrentProjectState(null)
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
      setCurrentProjectState(null)
      setInstanceKey((k) => k + 1)
    } catch (err) {
      const msg = err instanceof JcamFormatError ? err.message : (err as Error).message
      setShellError(msg || 'Failed to open project file')
    }
  }

  function handleRestoreRecent(record: RecentRecord) {
    setShellError(null)
    setMode(record.state.mode)
    setInitialProject(record.state)
    setCurrentProjectState(null)
    setInstanceKey((k) => k + 1)
  }

  function handleSaveProject() {
    if (currentProjectState === null) return
    const bytes = packJcamProject(currentProjectState)
    const blob = new Blob([new Uint8Array(bytes)], { type: 'application/zip' })
    triggerDownload(blob, jcamFileName(currentProjectState.fileName))
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
        <Button
          size="sm"
          variant="secondary"
          onClick={handleSaveProject}
          disabled={currentProjectState === null}
        >
          Save Project
        </Button>
        {recents.length > 0 && (
          <div className="flex items-center gap-1 overflow-x-auto">
            <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Recent:
            </span>
            <ul
              role="list"
              aria-label="Recent projects"
              className="flex items-center gap-1"
            >
              {recents.map((r) => (
                <li key={r.fileName} className="flex items-center">
                  <button
                    type="button"
                    onClick={() => handleRestoreRecent(r)}
                    className="flex items-center gap-1 rounded border border-border bg-secondary px-2 py-1 text-xs text-secondary-foreground hover:bg-accent"
                    title={`${r.fileName}\nLast opened ${new Date(r.savedAt).toLocaleString()}`}
                    aria-label={r.fileName}
                  >
                    <span
                      data-testid="recent-mode-badge"
                      title={MODE_LABELS[r.state.mode]}
                      className="rounded bg-muted px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground"
                    >
                      {MODE_BADGES[r.state.mode]}
                    </span>
                    <span className="truncate">{r.fileName}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
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
            onProjectStateChange={handleProjectStateChange}
          />
        )}
        {mode === '2d-profile' && (
          <Mode2ProfileMode
            key={`2d-profile-${instanceKey}`}
            initialProject={initialProject}
            onProjectStateChange={handleProjectStateChange}
          />
        )}
      </div>
    </div>
  )
}
