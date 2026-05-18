/**
 * Mode2ProfileMode — Phase 4 (2-D Profile Cuts) component.
 *
 * Lays out the Mode 2 surface: Canvas2DViewport as the primary workspace
 * with a right-hand sidebar mirroring Mode 1's structure. The File
 * section exposes a single-file SVG/DXF picker that dispatches to
 * `parseSvg` / `parseDxf` based on extension; imported paths populate
 * the Paths section (one row per polyline with a selection checkbox)
 * and render in the viewport using the `artwork` style. Parse failures
 * surface as a red alert block in File; recoverable ParseWarnings show
 * as a yellow inline list, mirroring Mode 1's load-warning pattern.
 */

import { useEffect, useRef, useState } from 'react'
import {
  Canvas2DViewport,
  type Canvas2DDrawAPI,
} from '../../viewport2d/Canvas2DViewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { prewarmWasm } from '../../api/gcodeViewer'
import { parseDxf, parseSvg } from '../../api/mode2'
import { useViewport2DStore, type Extent2D } from '../../store/viewport2dStore'
import { WorkingEnvironmentModal } from '../working-env/WorkingEnvironmentModal'
import type { AppError, ParseWarning, Polyline } from '../../api/types'
import type { ProjectState } from '../../persistence/projectFile'

type EngineStatus = 'initializing' | 'ready' | 'failed'
type ImportStatus = 'idle' | 'importing' | 'imported' | 'error'

interface Mode2ProfileModeProps {
  /**
   * Optional project to hydrate from on mount. Accepted for shape
   * parity with Mode 1 — actual hydration of Mode 2 payloads lands in
   * a later bead, so the prop is currently unused.
   */
  initialProject?: ProjectState | null
}

export function Mode2ProfileMode(_props: Mode2ProfileModeProps = {}) {
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const drawApiRef = useRef<Canvas2DDrawAPI | null>(null)

  const [engineStatus, setEngineStatus] = useState<EngineStatus>('initializing')
  const [engineError, setEngineError] = useState<string | null>(null)

  const [sourceFileName, setSourceFileName] = useState<string | null>(null)
  const [importStatus, setImportStatus] = useState<ImportStatus>('idle')
  const [importError, setImportError] = useState<string | null>(null)
  const [paths, setPaths] = useState<Polyline[]>([])
  const [selected, setSelected] = useState<boolean[]>([])
  const [warnings, setWarnings] = useState<ParseWarning[]>([])

  const [workingEnvOpen, setWorkingEnvOpen] = useState(false)

  // Subscribe to the viewport transform so pan/zoom re-runs the redraw
  // effect below — Canvas2DViewport's imperative API does not redraw on
  // its own, the consumer owns the draw loop.
  const transform = useViewport2DStore((s) => s.transform)

  useEffect(() => {
    let cancelled = false
    prewarmWasm().then(
      () => {
        if (!cancelled) setEngineStatus('ready')
      },
      (err: { message?: string; kind?: string }) => {
        if (cancelled) return
        setEngineStatus('failed')
        setEngineError(err.message ?? err.kind ?? 'Failed to initialize engine')
      },
    )
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    const api = drawApiRef.current
    if (!api) return
    api.clear()
    for (const path of paths) {
      if (path.points.length === 0) continue
      const pts: ReadonlyArray<readonly [number, number]> = path.points.map(
        (p) => [p.x, p.y] as const,
      )
      if (path.closed) api.polygon(pts, 'artwork')
      else api.polyline(pts, 'artwork')
    }
  }, [paths, transform])

  function handlePickFile() {
    fileInputRef.current?.click()
  }

  async function handleFileChosen(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    const format = detectFormat(file.name)
    if (!format) {
      setImportStatus('error')
      setImportError(`Unsupported file type: ${file.name}`)
      setWarnings([])
      return
    }
    setImportStatus('importing')
    setImportError(null)
    setWarnings([])
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      const result =
        format === 'svg' ? await parseSvg(bytes) : await parseDxf(bytes)
      // A successful parse proves the engine works — clear any stale
      // failure indicator left over from a transient prewarm error.
      setEngineStatus('ready')
      setEngineError(null)
      setSourceFileName(file.name)
      setPaths(result.paths)
      setSelected(result.paths.map(() => true))
      setWarnings(result.warnings)
      setImportStatus('imported')
      useViewport2DStore.getState().setExtent(computeExtent(result.paths))
    } catch (e) {
      setImportStatus('error')
      setImportError(formatParseError(e))
    }
  }

  function togglePath(i: number) {
    setSelected((prev) => prev.map((v, idx) => (idx === i ? !v : v)))
  }

  return (
    <div
      data-testid="mode2-root"
      className="flex h-full flex-1 flex-col bg-background text-foreground"
    >
      <input
        ref={fileInputRef}
        type="file"
        accept=".svg,.dxf"
        onChange={handleFileChosen}
        className="hidden"
        aria-label="SVG or DXF artwork file"
      />
      <div className="flex flex-1 overflow-hidden">
        <Canvas2DViewport ref={drawApiRef} className="flex-1" />
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">

            <SidebarSection title="File">
              <div className="flex flex-col gap-2">
                {engineStatus === 'initializing' && (
                  <p className="text-xs text-muted-foreground" role="status">
                    Initializing engine…
                  </p>
                )}
                {engineStatus === 'failed' && (
                  <p className="text-xs text-destructive" role="alert">
                    Engine failed to load: {engineError}
                  </p>
                )}
                <Button size="sm" className="w-full" onClick={handlePickFile}>
                  Open SVG / DXF…
                </Button>
                {sourceFileName && (
                  <p
                    className="truncate text-xs text-muted-foreground"
                    title={sourceFileName}
                  >
                    {sourceFileName}
                  </p>
                )}
                {importStatus === 'importing' && (
                  <p className="text-xs text-muted-foreground">Importing…</p>
                )}
                {importError && (
                  <p className="text-xs text-destructive" role="alert">
                    {importError}
                  </p>
                )}
                {warnings.length > 0 && (
                  <ul className="mt-1 space-y-1">
                    {warnings.map((w, i) => (
                      <li key={i} className="text-xs text-yellow-600">
                        ⚠ {w.message}
                        {w.line !== null && ` (line ${w.line})`}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </SidebarSection>

            <SidebarSection title="Paths">
              {paths.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No paths imported.
                </p>
              ) : (
                <ul className="flex flex-col gap-1">
                  {paths.map((path, i) => (
                    <li key={i}>
                      <label className="flex cursor-pointer items-center gap-2 rounded px-1 py-0.5 text-xs hover:bg-accent">
                        <input
                          type="checkbox"
                          checked={selected[i] ?? false}
                          onChange={() => togglePath(i)}
                          aria-label={`Path ${i + 1}`}
                        />
                        <span className="flex-1 truncate">Path {i + 1}</span>
                        <span className="text-muted-foreground">
                          {path.closed ? 'closed' : 'open'} · {path.points.length} pts
                        </span>
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </SidebarSection>

            <SidebarSection title="Machine">
              <Button
                size="sm"
                variant="secondary"
                className="w-full"
                onClick={() => setWorkingEnvOpen(true)}
              >
                Working Environment…
              </Button>
            </SidebarSection>

            <SidebarSection title="Operation">
              <p className="text-xs text-muted-foreground">
                Profile operation settings — coming soon.
              </p>
            </SidebarSection>

            <SidebarSection title="Generate">
              <p className="text-xs text-muted-foreground">
                Toolpath generation — coming soon.
              </p>
            </SidebarSection>

            <SidebarSection title="Simulate">
              <p className="text-xs text-muted-foreground">
                Material-removal simulation — coming soon.
              </p>
            </SidebarSection>

            <SidebarSection title="Export">
              <p className="text-xs text-muted-foreground">
                G-code export — coming soon.
              </p>
            </SidebarSection>

          </ScrollArea>
        </aside>
      </div>
      <WorkingEnvironmentModal
        open={workingEnvOpen}
        onClose={() => setWorkingEnvOpen(false)}
      />
    </div>
  )
}

function detectFormat(name: string): 'svg' | 'dxf' | null {
  const lower = name.toLowerCase()
  if (lower.endsWith('.svg')) return 'svg'
  if (lower.endsWith('.dxf')) return 'dxf'
  return null
}

function isAppError(err: unknown): err is AppError {
  return (
    typeof err === 'object' && err !== null && 'kind' in err && 'message' in err
  )
}

function formatParseError(err: unknown): string {
  if (isAppError(err)) {
    if (err.kind === 'ParseFailure') {
      const detail = err.message
      const lineSuffix = detail.line !== null ? ` (line ${detail.line})` : ''
      return `${detail.source}: ${detail.message}${lineSuffix}`
    }
    if (typeof err.message === 'string') return err.message
    return err.kind
  }
  if (err instanceof Error) return err.message
  return 'Failed to import file'
}

function computeExtent(paths: Polyline[]): Extent2D | null {
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  for (const path of paths) {
    for (const p of path.points) {
      if (p.x < minX) minX = p.x
      if (p.y < minY) minY = p.y
      if (p.x > maxX) maxX = p.x
      if (p.y > maxY) maxY = p.y
    }
  }
  if (!Number.isFinite(minX)) return null
  return { minX, minY, maxX, maxY }
}
