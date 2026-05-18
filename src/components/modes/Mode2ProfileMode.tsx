/**
 * Mode2ProfileMode — Phase 4 (2-D Profile Cuts) component.
 *
 * Lays out the Mode 2 surface: Canvas2DViewport as the primary workspace
 * with a right-hand sidebar mirroring Mode 1's structure. The Setup
 * section exposes the active machine setup selector at the top of the
 * sidebar. The File section exposes a single-file SVG/DXF picker that
 * dispatches to `parseSvg` / `parseDxf` based on extension; imported
 * paths populate the Paths section (one row per polyline with a
 * selection checkbox) and render in the viewport using the `artwork`
 * style. The Operation section is the profile-cut form (tool dropdown
 * filtered by the active setup's availability matrix, cut-side toggle,
 * and the depth / feed / spindle numeric inputs). Form state is held
 * locally — persistence into `.jcam` lands in a later bead. Parse
 * failures surface as a red alert block in File; recoverable
 * ParseWarnings show as a yellow inline list, mirroring Mode 1's
 * load-warning pattern.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Canvas2DViewport,
  type Canvas2DDrawAPI,
} from '../../viewport2d/Canvas2DViewport'
import { Viewport } from '../../viewport/Viewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { prewarmWasm, simulateGcodeViewer } from '../../api/gcodeViewer'
import {
  emitGrblGcode,
  generateProfileToolpath,
  parseDxf,
  parseSvg,
} from '../../api/mode2'
import { useViewport2DStore, type Extent2D } from '../../store/viewport2dStore'
import { useViewportStore } from '../../store/viewportStore'
import { WorkingEnvironmentModal } from '../working-env/WorkingEnvironmentModal'
import {
  loadActiveSetupId,
  loadWorkingEnv,
  saveActiveSetupId,
} from '../../persistence/workingEnv'
import type {
  AppError,
  CutSide,
  ParseWarning,
  Polyline,
  ProfileOperationInput,
  Tool,
  ToolpathOutput,
  WorkingEnvironment,
} from '../../api/types'
import type { ProjectState } from '../../persistence/projectFile'

type EngineStatus = 'initializing' | 'ready' | 'failed'
type ImportStatus = 'idle' | 'importing' | 'imported' | 'error'
type GenerateStatus = 'idle' | 'generating' | 'generated' | 'error'
type ExportStatus = 'idle' | 'exporting' | 'error'
type SimStatus = 'idle' | 'simulating' | 'ready' | 'error'
type ViewMode = 'canvas2d' | 'viewport3d'

// Stand-in dexel resolution until Mode 2 grows its own simulation controls.
// Matches Mode 1's DEFAULT_RESOLUTION so the runtime cost is familiar.
const SIM_RESOLUTION_MM = 0.5

interface SampleEntry {
  label: string
  fileName: string
  format: 'svg' | 'dxf'
}

const SAMPLES: readonly SampleEntry[] = [
  { label: 'Star (SVG)', fileName: 'sample-profile.svg', format: 'svg' },
  { label: 'Octagon (DXF)', fileName: 'sample-profile.dxf', format: 'dxf' },
]

function sampleUrl(fileName: string): string {
  return `${import.meta.env.BASE_URL}samples/${fileName}`
}

interface ProfileOperationFormState {
  toolId: string | null
  cutSide: CutSide
  depthTotal: number
  depthPerPass: number
  safeZ: number
  plungeFeed: number
  cutFeed: number
  spindleRpm: number
}

const DEFAULT_OPERATION: ProfileOperationFormState = {
  toolId: null,
  cutSide: 'outside',
  depthTotal: 5,
  depthPerPass: 1,
  safeZ: 5,
  plungeFeed: 200,
  cutFeed: 800,
  spindleRpm: 18000,
}

const EMPTY_ENV: WorkingEnvironment = { setups: [], tools: [], availability: [] }

const CUT_SIDE_OPTIONS: ReadonlyArray<{ value: CutSide; label: string }> = [
  { value: 'outside', label: 'Outside' },
  { value: 'inside', label: 'Inside' },
  { value: 'onLine', label: 'On Line' },
]

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
  const [env, setEnv] = useState<WorkingEnvironment>(EMPTY_ENV)
  const [activeSetupId, setActiveSetupId] = useState<string | null>(null)

  const [operation, setOperation] = useState<ProfileOperationFormState>(DEFAULT_OPERATION)

  const [generateStatus, setGenerateStatus] = useState<GenerateStatus>('idle')
  const [generateError, setGenerateError] = useState<string | null>(null)
  // The last successful planner output. Kept in component state so that
  // downstream actions (Simulate, Export — landed in later beads) can
  // reuse it without re-running the planner. Cleared whenever a fresh
  // import lands or a regenerate fails, since the prior result would
  // then refer to artwork or parameters the UI no longer shows.
  const [toolpath, setToolpath] = useState<ToolpathOutput | null>(null)

  const [exportStatus, setExportStatus] = useState<ExportStatus>('idle')
  const [exportError, setExportError] = useState<string | null>(null)

  const [simStatus, setSimStatus] = useState<SimStatus>('idle')
  const [simError, setSimError] = useState<string | null>(null)
  const [view, setView] = useState<ViewMode>('canvas2d')

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

  const refreshWorkingEnv = useCallback(async () => {
    const [loadedEnv, loadedActive] = await Promise.all([
      loadWorkingEnv(),
      loadActiveSetupId(),
    ])
    setEnv(loadedEnv)
    // Validate the persisted active id against the loaded setups — a
    // stale id (e.g. left behind by a multi-window deletion) would render
    // a controlled <select> with no matching <option>. Fall back to the
    // first setup so the Operation form stays usable on first run; the
    // user can override via the selector.
    const validActive =
      loadedActive !== null && loadedEnv.setups.some((s) => s.id === loadedActive)
        ? loadedActive
        : null
    setActiveSetupId(validActive ?? loadedEnv.setups[0]?.id ?? null)
  }, [])

  useEffect(() => {
    void refreshWorkingEnv()
  }, [refreshWorkingEnv])

  // `view` is a dep so the redraw fires after a switch back to Canvas2D —
  // the new draw API is committed on remount, but none of the other deps
  // (paths/toolpath/transform) would have changed, so without this the
  // canvas would come back blank.
  useEffect(() => {
    if (view !== 'canvas2d') return
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
    if (toolpath && toolpath.length > 0) {
      // Each motion's `to` is absolute machine XY; segments are drawn
      // pairwise from the previous endpoint. The very first motion has
      // no predecessor — there is no "where the spindle was" to draw
      // from — so we only seed `prev` and start emitting segments from
      // the second motion onward. Linear moves render as toolpath
      // strokes; rapids render as rapid strokes.
      let prev: readonly [number, number] | null = null
      for (const m of toolpath) {
        const xy = [m.to[0], m.to[1]] as const
        if (prev) {
          const style = m.kind === 'rapid' ? 'rapid' : 'toolpath'
          api.polyline([prev, xy], style)
        }
        prev = xy
      }
    }
  }, [paths, toolpath, transform, view])

  const availableTools = useMemo<Tool[]>(() => {
    if (activeSetupId === null) return []
    const toolIds = new Set(
      env.availability
        .filter((p) => p.setupId === activeSetupId)
        .map((p) => p.toolId),
    )
    return env.tools.filter((t) => toolIds.has(t.id))
  }, [env, activeSetupId])

  // Keep the operation's selected tool consistent with what's currently
  // available: clear it when the choice disappears, default it to the
  // first available tool when nothing is picked.
  useEffect(() => {
    setOperation((prev) => {
      if (prev.toolId !== null && availableTools.some((t) => t.id === prev.toolId)) {
        return prev
      }
      const next = availableTools[0]?.id ?? null
      if (prev.toolId === next) return prev
      return { ...prev, toolId: next }
    })
  }, [availableTools])

  function handlePickFile() {
    fileInputRef.current?.click()
  }

  async function importBytes(name: string, format: 'svg' | 'dxf', bytes: Uint8Array) {
    setImportStatus('importing')
    setImportError(null)
    setWarnings([])
    // A new import invalidates any previously generated toolpath: the
    // motions reference boundaries that are about to be replaced.
    setToolpath(null)
    setGenerateStatus('idle')
    setGenerateError(null)
    setExportStatus('idle')
    setExportError(null)
    // A fresh import also retires any prior simulation result — the mesh
    // was carved against artwork that no longer exists. Drop back to the
    // 2-D workspace so the user sees their new paths immediately.
    useViewportStore.getState().clearSimulationMesh()
    setSimStatus('idle')
    setSimError(null)
    setView('canvas2d')
    try {
      const result =
        format === 'svg' ? await parseSvg(bytes) : await parseDxf(bytes)
      // A successful parse proves the engine works — clear any stale
      // failure indicator left over from a transient prewarm error.
      setEngineStatus('ready')
      setEngineError(null)
      setSourceFileName(name)
      setPaths(result.paths)
      setSelected(result.paths.map(() => true))
      setWarnings(result.warnings)
      setImportStatus('imported')
      useViewport2DStore.getState().setExtent(computeExtent(result.paths))
    } catch (e) {
      setImportStatus('error')
      setImportError(formatAppErrorMessage(e))
    }
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
    const bytes = new Uint8Array(await file.arrayBuffer())
    await importBytes(file.name, format, bytes)
  }

  async function handleSampleChosen(event: React.ChangeEvent<HTMLSelectElement>) {
    const value = event.target.value
    event.target.value = ''
    if (!value) return
    const sample = SAMPLES.find((s) => s.fileName === value)
    if (!sample) return
    setImportStatus('importing')
    setImportError(null)
    setWarnings([])
    try {
      const response = await fetch(sampleUrl(sample.fileName))
      if (!response.ok) {
        throw new Error(`Sample fetch failed: ${response.status}`)
      }
      const bytes = new Uint8Array(await response.arrayBuffer())
      await importBytes(sample.fileName, sample.format, bytes)
    } catch (e) {
      setImportStatus('error')
      setImportError((e as Error).message ?? 'Failed to load sample')
    }
  }

  function togglePath(i: number) {
    setSelected((prev) => prev.map((v, idx) => (idx === i ? !v : v)))
  }

  async function handleActiveSetupChanged(event: React.ChangeEvent<HTMLSelectElement>) {
    const next = event.target.value || null
    setActiveSetupId(next)
    await saveActiveSetupId(next)
  }

  function handleToolChanged(event: React.ChangeEvent<HTMLSelectElement>) {
    const next = event.target.value || null
    setOperation((prev) => ({ ...prev, toolId: next }))
  }

  function handleCutSideChanged(side: CutSide) {
    setOperation((prev) => ({ ...prev, cutSide: side }))
  }

  function handleNumberChanged(
    key: Exclude<keyof ProfileOperationFormState, 'toolId' | 'cutSide'>,
  ) {
    return (event: React.ChangeEvent<HTMLInputElement>) => {
      const parsed = Number(event.target.value)
      if (!Number.isFinite(parsed)) return
      setOperation((prev) => ({ ...prev, [key]: parsed }))
    }
  }

  async function handleWorkingEnvClose() {
    setWorkingEnvOpen(false)
    await refreshWorkingEnv()
  }

  const selectedBoundaries = useMemo<Polyline[]>(
    () => paths.filter((_, i) => selected[i]),
    [paths, selected],
  )

  const selectedTool = useMemo<Tool | null>(
    () =>
      operation.toolId === null
        ? null
        : env.tools.find((t) => t.id === operation.toolId) ?? null,
    [env.tools, operation.toolId],
  )

  const activeSetup = useMemo(
    () =>
      activeSetupId === null
        ? null
        : env.setups.find((s) => s.id === activeSetupId) ?? null,
    [env.setups, activeSetupId],
  )

  // Export needs the toolpath itself plus a tool (for the @TOOL header)
  // and stock dimensions (for the @STOCK header). Mode 2 doesn't yet
  // carry a separate stock model, so the active setup's workspace box
  // stands in — that's the volume the planner is implicitly aimed at.
  const exportBlocked =
    toolpath === null || selectedTool === null || activeSetup === null

  // The button only makes sense once we have something to plan with —
  // at least one selected path and a tool resolved from the active
  // setup's availability. Anything missing is reported below the button
  // rather than baked into the disabled state alone, so the user knows
  // *why* generation is unavailable.
  const generateBlockingReason: string | null =
    selectedBoundaries.length === 0
      ? 'Select at least one path to generate a toolpath.'
      : selectedTool === null
        ? 'Select a tool before generating.'
        : null

  async function handleGenerate() {
    if (generateBlockingReason !== null || selectedTool === null) return
    setGenerateStatus('generating')
    setGenerateError(null)
    // A regenerate retires the prior simulation mesh — it was carved
    // against motions about to be replaced. Flip back to 2D so the user
    // sees the new toolpath overlay rather than staring at an empty 3-D
    // scene; they can hit Simulate again if they want a fresh mesh.
    useViewportStore.getState().clearSimulationMesh()
    setSimStatus('idle')
    setSimError(null)
    setView('canvas2d')
    const input: ProfileOperationInput = {
      boundaries: selectedBoundaries,
      tool: selectedTool,
      cutSide: operation.cutSide,
      depthTotal: operation.depthTotal,
      depthPerPass: operation.depthPerPass,
      safeZ: operation.safeZ,
      plungeFeed: operation.plungeFeed,
      cutFeed: operation.cutFeed,
      spindleRpm: operation.spindleRpm,
    }
    try {
      const result = await generateProfileToolpath(input)
      setToolpath(result)
      setGenerateStatus('generated')
    } catch (e) {
      setToolpath(null)
      setGenerateStatus('error')
      setGenerateError(formatAppErrorMessage(e))
    }
  }

  // The Simulate pipeline needs the same three pieces as Export plus a
  // dexel grid resolution. Mode 2 has no stock model yet, so the active
  // setup's workspace stands in — same compromise the Export header makes.
  const simulateBlocked =
    toolpath === null || selectedTool === null || activeSetup === null

  async function handleSimulate() {
    if (toolpath === null || selectedTool === null || activeSetup === null) return
    setSimStatus('simulating')
    setSimError(null)
    try {
      const gcode = await emitGrblGcode(toolpath, selectedTool, activeSetup.workspace)
      const mesh = await simulateGcodeViewer(gcode, {
        stock: activeSetup.workspace,
        toolDiameter: selectedTool.diameter,
        resolution: SIM_RESOLUTION_MM,
      })
      useViewportStore.getState().setSimulationMeshData(mesh)
      setSimStatus('ready')
      setView('viewport3d')
    } catch (e) {
      // Preserve any mesh from a prior successful sim. If this is the
      // first attempt the store is already empty; if the user re-ran
      // Simulate from the 3-D preview and it failed, clearing here would
      // strand them on an empty 3-D scene with only a sidebar error to
      // explain it. The last good result is the more useful default.
      setSimStatus('error')
      setSimError(formatAppErrorMessage(e))
    }
  }

  // Make sure we don't leak this mode's simulation mesh into another
  // mode's viewport — the store is process-wide.
  useEffect(() => {
    return () => {
      useViewportStore.getState().clearSimulationMesh()
    }
  }, [])

  async function handleExport() {
    if (toolpath === null || selectedTool === null || activeSetup === null) return
    setExportStatus('exporting')
    setExportError(null)
    try {
      const gcode = await emitGrblGcode(toolpath, selectedTool, activeSetup.workspace)
      triggerTextDownload(gcode, gcodeFileName(sourceFileName))
      setExportStatus('idle')
    } catch (e) {
      setExportStatus('error')
      setExportError(formatAppErrorMessage(e))
    }
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
        {view === 'canvas2d' ? (
          <Canvas2DViewport ref={drawApiRef} className="flex-1" />
        ) : (
          <Viewport className="flex-1" />
        )}
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">

            <SidebarSection title="Setup">
              <div className="flex flex-col gap-2">
                {env.setups.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    No machine setups configured.
                  </p>
                ) : (
                  <select
                    aria-label="Active machine setup"
                    value={activeSetupId ?? ''}
                    onChange={handleActiveSetupChanged}
                    className="h-8 w-full rounded border border-border bg-secondary px-2 text-xs text-secondary-foreground"
                  >
                    <option value="" disabled>
                      Choose setup…
                    </option>
                    {env.setups.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                )}
                <Button
                  size="sm"
                  variant="secondary"
                  className="w-full"
                  onClick={() => setWorkingEnvOpen(true)}
                >
                  Working Environment…
                </Button>
              </div>
            </SidebarSection>

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
                <select
                  aria-label="Load Sample"
                  value=""
                  onChange={handleSampleChosen}
                  className="h-8 w-full rounded border border-border bg-secondary px-2 text-xs text-secondary-foreground"
                >
                  <option value="" disabled>
                    Load Sample…
                  </option>
                  {SAMPLES.map((s) => (
                    <option key={s.fileName} value={s.fileName}>
                      {s.label}
                    </option>
                  ))}
                </select>
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

            <SidebarSection title="Operation">
              <div className="flex flex-col gap-3">
                <div className="flex flex-col gap-1 text-xs">
                  <span className="text-muted-foreground">Tool</span>
                  {availableTools.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {activeSetupId === null
                        ? 'Choose an active setup to see its tools.'
                        : 'No tools available for this setup.'}
                    </p>
                  ) : (
                    <select
                      aria-label="Tool"
                      value={operation.toolId ?? ''}
                      onChange={handleToolChanged}
                      className="h-8 w-full rounded border border-border bg-secondary px-2 text-xs text-secondary-foreground"
                    >
                      <option value="" disabled>
                        Choose tool…
                      </option>
                      {availableTools.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.name}
                        </option>
                      ))}
                    </select>
                  )}
                </div>

                <fieldset className="flex flex-col gap-1 text-xs">
                  <legend className="text-muted-foreground">Cut side</legend>
                  <div className="flex gap-3">
                    {CUT_SIDE_OPTIONS.map((opt) => (
                      <label key={opt.value} className="flex items-center gap-1">
                        <input
                          type="radio"
                          name="cut-side"
                          value={opt.value}
                          checked={operation.cutSide === opt.value}
                          onChange={() => handleCutSideChanged(opt.value)}
                        />
                        {opt.label}
                      </label>
                    ))}
                  </div>
                </fieldset>

                <NumberField
                  label="Depth total (mm)"
                  ariaLabel="Depth total"
                  value={operation.depthTotal}
                  step={0.1}
                  min={0}
                  onChange={handleNumberChanged('depthTotal')}
                />
                <NumberField
                  label="Depth per pass (mm)"
                  ariaLabel="Depth per pass"
                  value={operation.depthPerPass}
                  step={0.1}
                  min={0}
                  onChange={handleNumberChanged('depthPerPass')}
                />
                <NumberField
                  label="Safe Z (mm)"
                  ariaLabel="Safe Z"
                  value={operation.safeZ}
                  step={0.1}
                  onChange={handleNumberChanged('safeZ')}
                />
                <NumberField
                  label="Plunge feed (mm/min)"
                  ariaLabel="Plunge feed"
                  value={operation.plungeFeed}
                  step={10}
                  min={0}
                  onChange={handleNumberChanged('plungeFeed')}
                />
                <NumberField
                  label="Cut feed (mm/min)"
                  ariaLabel="Cut feed"
                  value={operation.cutFeed}
                  step={10}
                  min={0}
                  onChange={handleNumberChanged('cutFeed')}
                />
                <NumberField
                  label="Spindle RPM"
                  ariaLabel="Spindle RPM"
                  value={operation.spindleRpm}
                  step={100}
                  min={0}
                  onChange={handleNumberChanged('spindleRpm')}
                />
              </div>
            </SidebarSection>

            <SidebarSection title="Generate">
              <div className="flex flex-col gap-2">
                <Button
                  size="sm"
                  className="w-full"
                  aria-label="Generate toolpath"
                  onClick={() => void handleGenerate()}
                  disabled={
                    generateBlockingReason !== null ||
                    generateStatus === 'generating' ||
                    simStatus === 'simulating'
                  }
                >
                  {generateStatus === 'generating' ? 'Generating…' : 'Generate'}
                </Button>
                {generateBlockingReason !== null && (
                  <p className="text-xs text-muted-foreground">
                    {generateBlockingReason}
                  </p>
                )}
                {generateStatus === 'generated' && toolpath !== null && (
                  <p className="text-xs text-muted-foreground" role="status">
                    Generated {toolpath.length} moves
                  </p>
                )}
                {generateError && (
                  <p className="text-xs text-destructive" role="alert">
                    {generateError}
                  </p>
                )}
              </div>
            </SidebarSection>

            <SidebarSection title="Simulate">
              <div className="flex flex-col gap-2">
                <Button
                  size="sm"
                  className="w-full"
                  aria-label="Simulate toolpath"
                  onClick={() => void handleSimulate()}
                  disabled={simulateBlocked || simStatus === 'simulating'}
                >
                  {simStatus === 'simulating' ? 'Simulating…' : 'Simulate'}
                </Button>
                {view === 'viewport3d' && (
                  <Button
                    size="sm"
                    variant="secondary"
                    className="w-full"
                    aria-label="Back to 2D"
                    onClick={() => setView('canvas2d')}
                  >
                    Back to 2D
                  </Button>
                )}
                {toolpath === null && (
                  <p className="text-xs text-muted-foreground">
                    Generate a toolpath before simulating.
                  </p>
                )}
                {simStatus === 'ready' && (
                  <p className="text-xs text-muted-foreground" role="status">
                    Simulation complete.
                  </p>
                )}
                {simError && (
                  <p className="text-xs text-destructive" role="alert">
                    {simError}
                  </p>
                )}
              </div>
            </SidebarSection>

            <SidebarSection title="Export">
              <div className="flex flex-col gap-2">
                <Button
                  size="sm"
                  className="w-full"
                  aria-label="Export G-code"
                  onClick={() => void handleExport()}
                  disabled={exportBlocked || exportStatus === 'exporting'}
                >
                  {exportStatus === 'exporting' ? 'Exporting…' : 'Export G-code'}
                </Button>
                {toolpath === null && (
                  <p className="text-xs text-muted-foreground">
                    Generate a toolpath before exporting.
                  </p>
                )}
                {exportError && (
                  <p className="text-xs text-destructive" role="alert">
                    {exportError}
                  </p>
                )}
              </div>
            </SidebarSection>

          </ScrollArea>
        </aside>
      </div>
      <WorkingEnvironmentModal
        open={workingEnvOpen}
        onClose={() => void handleWorkingEnvClose()}
      />
    </div>
  )
}

interface NumberFieldProps {
  label: string
  ariaLabel: string
  value: number
  step?: number
  min?: number
  onChange: (event: React.ChangeEvent<HTMLInputElement>) => void
}

function NumberField({ label, ariaLabel, value, step, min, onChange }: NumberFieldProps) {
  return (
    <label className="flex flex-col gap-1 text-xs">
      <span className="text-muted-foreground">{label}</span>
      <input
        type="number"
        aria-label={ariaLabel}
        value={value}
        step={step}
        min={min}
        onChange={onChange}
        className="h-8 w-full rounded border border-border bg-background px-2 text-xs"
      />
    </label>
  )
}

function triggerTextDownload(text: string, fileName: string): void {
  const blob = new Blob([text], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = fileName
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

function gcodeFileName(sourceName: string | null): string {
  if (!sourceName) return 'toolpath.nc'
  const dot = sourceName.lastIndexOf('.')
  const stem = dot > 0 ? sourceName.slice(0, dot) : sourceName
  return `${stem}.nc`
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

function formatAppErrorMessage(err: unknown): string {
  if (isAppError(err)) {
    if (err.kind === 'ParseFailure') {
      const detail = err.message
      const lineSuffix = detail.line !== null ? ` (line ${detail.line})` : ''
      return `${detail.source}: ${detail.message}${lineSuffix}`
    }
    if (err.kind === 'MissingSetup' || err.kind === 'MissingTool') {
      return `${err.kind}: ${err.message.id}`
    }
    if (typeof err.message === 'string') return err.message
    return err.kind
  }
  if (err instanceof Error) return err.message
  return 'Unknown error'
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
