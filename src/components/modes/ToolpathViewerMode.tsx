/**
 * ToolpathViewerMode — G-code Viewer mode component.
 *
 * Provides a sidebar with file loading, stock/tool configuration, and
 * simulation controls alongside a persistent 3-D viewport showing the
 * toolpath centerlines and dexel material-removal mesh.
 *
 * Layout follows the AppShell pattern: Viewport fills the left area,
 * sidebar is fixed-width on the right.
 */

import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { Viewport } from '../../viewport/Viewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { loadGcodeForViewer, simulateGcodeViewer, getSampleGcodePath } from '../../api/gcodeViewer'
import { useViewportStore } from '../../store/viewportStore'
import { useProjectStore } from '../../store/projectStore'
import { checkUnsavedChanges } from '../../lib/unsavedGuard'
import type { GcodeViewerLoadResult } from '../../api/types'

// ── Constants ─────────────────────────────────────────────────────────────────

/** Tool types currently supported by the dexel simulation engine. */
const SUPPORTED_TOOL_TYPES = ['flat_endmill']

// ── Types ─────────────────────────────────────────────────────────────────────

type LoadStatus = 'idle' | 'loading' | 'loaded' | 'error'
type SimulateStatus = 'idle' | 'simulating' | 'done' | 'error'

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Return a change handler that applies the override/revert mechanic for a
 * numeric field.
 *
 * When the user clears a field to empty and a metadata value is present, the
 * override is reverted to `null` so the metadata value shows through again.
 * Otherwise the override is set to whatever the user typed.
 */
function makeFieldHandler(
  setOverride: (v: string | null) => void,
  metaValue: string | undefined,
): (val: string) => void {
  return (val: string) => {
    setOverride(val === '' && metaValue !== undefined ? null : val)
  }
}

// ── Component ─────────────────────────────────────────────────────────────────

export function ToolpathViewerMode() {
  // ── File state ─────────────────────────────────────────────────────────────
  const [filePath, setFilePath] = useState<string | null>(null)
  const [loadResult, setLoadResult] = useState<GcodeViewerLoadResult | null>(null)
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle')
  const [loadError, setLoadError] = useState<string | null>(null)

  // ── Stock field overrides ──────────────────────────────────────────────────
  // null  = "not overridden — use metadata value"
  // string = user's explicit entry (overrides metadata)
  const [widthOverride, setWidthOverride] = useState<string | null>(null)
  const [depthOverride, setDepthOverride] = useState<string | null>(null)
  const [heightOverride, setHeightOverride] = useState<string | null>(null)
  const [originXOverride, setOriginXOverride] = useState<string | null>(null)
  const [originYOverride, setOriginYOverride] = useState<string | null>(null)
  const [originZOverride, setOriginZOverride] = useState<string | null>(null)

  // ── Tool field overrides ───────────────────────────────────────────────────
  // toolTypeOverride starts as null (no file loaded); after load it is either
  // the supported type string or '' (empty, if metadata type is unsupported).
  const [toolTypeOverride, setToolTypeOverride] = useState<string | null>(null)
  const [toolDiameterOverride, setToolDiameterOverride] = useState<string | null>(null)

  // ── Simulation state ───────────────────────────────────────────────────────
  const [resolution, setResolution] = useState<number>(0.5)
  const [simulateStatus, setSimulateStatus] = useState<SimulateStatus>('idle')
  const [simulateError, setSimulateError] = useState<string | null>(null)

  // ── Viewport lifecycle ─────────────────────────────────────────────────────
  // Clear any stale viewport state on mount; restore blank canvas on unmount.
  useEffect(() => {
    useViewportStore.getState().setToolpathGeometry(null)
    useViewportStore.getState().clearSimulationMesh()
    return () => {
      useViewportStore.getState().setToolpathGeometry(null)
      useViewportStore.getState().clearSimulationMesh()
    }
  }, [])

  // ── Derived metadata values ────────────────────────────────────────────────
  const metaStock = loadResult?.stock
  const metaW = metaStock?.width.toString()
  const metaD = metaStock?.depth.toString()
  const metaH = metaStock?.height.toString()
  const metaOX = metaStock?.origin.x.toString()
  const metaOY = metaStock?.origin.y.toString()
  const metaOZ = metaStock?.origin.z.toString()

  const firstTool = loadResult?.tools[0]
  const metaToolDiameter = firstTool?.diameter.toString()

  // ── Effective values (override ?? metadata ?? fallback) ───────────────────
  const effectiveWidth = widthOverride ?? metaW ?? ''
  const effectiveDepth = depthOverride ?? metaD ?? ''
  const effectiveHeight = heightOverride ?? metaH ?? ''
  const effectiveOriginX = originXOverride ?? metaOX ?? '0'
  const effectiveOriginY = originYOverride ?? metaOY ?? '0'
  const effectiveOriginZ = originZOverride ?? metaOZ ?? '0'

  // toolTypeOverride is null before first load; after load it's always a string.
  // When null, derive from metadata (only if supported); otherwise use override.
  const effectiveToolType =
    toolTypeOverride !== null
      ? toolTypeOverride
      : firstTool && SUPPORTED_TOOL_TYPES.includes(firstTool.toolType)
        ? firstTool.toolType
        : ''
  const effectiveToolDiameter = toolDiameterOverride ?? metaToolDiameter ?? ''

  // ── Simulate enabled guard ─────────────────────────────────────────────────
  const parsedWidth = parseFloat(effectiveWidth)
  const parsedDepth = parseFloat(effectiveDepth)
  const parsedHeight = parseFloat(effectiveHeight)
  const parsedDiameter = parseFloat(effectiveToolDiameter)

  const canSimulate =
    filePath !== null &&
    !isNaN(parsedWidth) && parsedWidth > 0 &&
    !isNaN(parsedDepth) && parsedDepth > 0 &&
    !isNaN(parsedHeight) && parsedHeight > 0 &&
    effectiveToolType !== '' &&
    !isNaN(parsedDiameter) && parsedDiameter > 0

  // ── Load logic ─────────────────────────────────────────────────────────────

  async function loadFile(path: string) {
    setLoadStatus('loading')
    setLoadError(null)
    setSimulateError(null)
    try {
      const result = await loadGcodeForViewer(path)
      setFilePath(path)
      setLoadResult(result)
      setLoadStatus('loaded')

      // Reset all stock overrides — metadata shows through via null.
      setWidthOverride(null)
      setDepthOverride(null)
      setHeightOverride(null)
      setOriginXOverride(null)
      setOriginYOverride(null)
      setOriginZOverride(null)

      // Tool: pre-populate with metadata type only if it is in the supported list.
      // Unsupported types are left blank; the user must select a supported type.
      const tool = result.tools[0]
      setToolTypeOverride(
        tool && SUPPORTED_TOOL_TYPES.includes(tool.toolType) ? tool.toolType : '',
      )
      setToolDiameterOverride(null)

      // Update viewport: show toolpath centerlines, clear any prior simulation mesh.
      useViewportStore.getState().setToolpathGeometry(result.lineGeometry)
      useViewportStore.getState().clearSimulationMesh()
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to load file')
    }
  }

  async function handlePickFile() {
    try {
      const result = await open({
        filters: [{ name: 'G-code', extensions: ['nc', 'gcode', 'tap'] }],
      })
      if (!result) return
      await loadFile(result as string)
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to open file dialog')
    }
  }

  async function handleLoadSample() {
    setLoadStatus('loading')
    setLoadError(null)
    try {
      const path = await getSampleGcodePath()
      await loadFile(path)
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to get sample path')
      setSimulateError(null)
    }
  }

  // ── Simulate logic ─────────────────────────────────────────────────────────

  async function handleSimulate() {
    if (!filePath || !canSimulate) return
    setSimulateStatus('simulating')
    setSimulateError(null)
    try {
      const ox = parseFloat(effectiveOriginX)
      const oy = parseFloat(effectiveOriginY)
      const oz = parseFloat(effectiveOriginZ)
      const mesh = await simulateGcodeViewer(
        filePath,
        {
          origin: {
            x: isNaN(ox) ? 0 : ox,
            y: isNaN(oy) ? 0 : oy,
            z: isNaN(oz) ? 0 : oz,
          },
          width: parsedWidth,
          depth: parsedDepth,
          height: parsedHeight,
        },
        parsedDiameter,
        effectiveToolType,
        resolution,
      )
      useViewportStore.getState().setSimulationMeshData(mesh)
      setSimulateStatus('done')
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setSimulateStatus('error')
      setSimulateError(err.message ?? err.kind ?? 'Simulation failed')
    }
  }

  // ── Back button ────────────────────────────────────────────────────────────

  async function handleBack() {
    const safe = await checkUnsavedChanges()
    if (!safe) return
    useProjectStore.getState().returnToSelector()
  }

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <div className="flex flex-1 overflow-hidden">
        <Viewport className="flex-1" />
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">

            {/* Back button */}
            <div className="border-b border-border px-3 py-2">
              <Button size="sm" variant="ghost" onClick={handleBack}>
                ← Back
              </Button>
            </div>

            {/* File panel */}
            <SidebarSection title="File">
              <div className="flex flex-col gap-2">
                <Button size="sm" className="w-full" onClick={handlePickFile}>
                  Open File…
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  className="w-full"
                  onClick={handleLoadSample}
                >
                  Load Sample
                </Button>
                {filePath && (
                  <p
                    className="truncate text-xs text-muted-foreground"
                    title={filePath}
                  >
                    {filePath.split(/[\\/]/).pop()}
                  </p>
                )}
                {loadStatus === 'loading' && (
                  <p className="text-xs text-muted-foreground">Loading…</p>
                )}
                {loadError && (
                  <p className="text-xs text-destructive" role="alert">
                    {loadError}
                  </p>
                )}
                {loadResult && loadResult.warnings.length > 0 && (
                  <ul className="mt-1 space-y-1">
                    {loadResult.warnings.map((w, i) => (
                      <li key={i} className="text-xs text-yellow-600">
                        ⚠ {w.message}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </SidebarSection>

            {/* Stock panel */}
            <SidebarSection title="Stock">
              <div className="flex flex-col gap-2">
                <div className="grid grid-cols-3 gap-1">
                  {(
                    [
                      { label: 'Width', meta: metaW, override: widthOverride, set: setWidthOverride },
                      { label: 'Depth', meta: metaD, override: depthOverride, set: setDepthOverride },
                      { label: 'Height', meta: metaH, override: heightOverride, set: setHeightOverride },
                    ] as const
                  ).map(({ label, meta, override, set }) => (
                    <div key={label} className="flex flex-col gap-0.5">
                      <label className="text-xs text-muted-foreground">{label}</label>
                      <input
                        type="number"
                        aria-label={label}
                        value={override ?? meta ?? ''}
                        onChange={(e) => makeFieldHandler(set, meta)(e.target.value)}
                        className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                        placeholder="—"
                        step="0.1"
                        min="0"
                      />
                    </div>
                  ))}
                </div>
                <div className="grid grid-cols-3 gap-1">
                  {(
                    [
                      { label: 'Origin X', meta: metaOX, override: originXOverride, set: setOriginXOverride },
                      { label: 'Origin Y', meta: metaOY, override: originYOverride, set: setOriginYOverride },
                      { label: 'Origin Z', meta: metaOZ, override: originZOverride, set: setOriginZOverride },
                    ] as const
                  ).map(({ label, meta, override, set }) => (
                    <div key={label} className="flex flex-col gap-0.5">
                      <label className="text-xs text-muted-foreground">{label}</label>
                      <input
                        type="number"
                        aria-label={label}
                        value={override ?? meta ?? ''}
                        onChange={(e) => makeFieldHandler(set, meta)(e.target.value)}
                        className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                        placeholder="0"
                        step="0.1"
                      />
                    </div>
                  ))}
                </div>
              </div>
            </SidebarSection>

            {/* Tool panel */}
            <SidebarSection title="Tool">
              <div className="flex flex-col gap-2">
                <div className="flex flex-col gap-0.5">
                  <label
                    className="text-xs text-muted-foreground"
                    htmlFor="tool-type-select"
                  >
                    Tool Type
                  </label>
                  <select
                    id="tool-type-select"
                    value={effectiveToolType}
                    onChange={(e) => setToolTypeOverride(e.target.value)}
                    className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                  >
                    <option value="">— Select —</option>
                    <option value="flat_endmill">Flat Endmill</option>
                  </select>
                </div>
                <div className="flex flex-col gap-0.5">
                  <label
                    className="text-xs text-muted-foreground"
                    htmlFor="tool-diameter-input"
                  >
                    Diameter (mm)
                  </label>
                  <input
                    id="tool-diameter-input"
                    type="number"
                    aria-label="Diameter"
                    value={effectiveToolDiameter}
                    onChange={(e) =>
                      makeFieldHandler(setToolDiameterOverride, metaToolDiameter)(e.target.value)
                    }
                    className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                    placeholder="—"
                    step="0.1"
                    min="0"
                  />
                </div>
              </div>
            </SidebarSection>

            {/* Simulation controls */}
            <SidebarSection title="Simulation">
              <div className="flex flex-col gap-2">
                <div className="flex items-center gap-2">
                  <label
                    className="shrink-0 text-xs text-muted-foreground"
                    htmlFor="resolution-slider"
                  >
                    Resolution
                  </label>
                  <input
                    id="resolution-slider"
                    type="range"
                    min={0.1}
                    max={2.0}
                    step={0.1}
                    value={resolution}
                    onChange={(e) => setResolution(parseFloat(e.target.value))}
                    className="flex-1"
                    aria-label="Resolution"
                  />
                  <span className="w-10 shrink-0 text-right text-xs text-muted-foreground">
                    {resolution.toFixed(1)} mm
                  </span>
                </div>
                <Button
                  size="sm"
                  className="w-full"
                  onClick={handleSimulate}
                  disabled={!canSimulate || simulateStatus === 'simulating'}
                  aria-busy={simulateStatus === 'simulating'}
                >
                  {simulateStatus === 'simulating' ? 'Simulating…' : 'Simulate'}
                </Button>
                {simulateError && (
                  <p className="text-xs text-destructive" role="alert">
                    {simulateError}
                  </p>
                )}
              </div>
            </SidebarSection>

          </ScrollArea>
        </aside>
      </div>
    </div>
  )
}
