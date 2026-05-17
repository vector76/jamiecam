/**
 * ToolpathViewerMode — G-code Viewer mode component (web build).
 *
 * Provides a sidebar for loading a `.nc` file (via browser file picker or
 * a bundled sample) alongside a persistent 3-D viewport showing the parsed
 * toolpath centerlines. Also exposes a Simulate action that runs the dexel
 * material-removal engine and renders the resulting workpiece mesh.
 */

import { useState, useEffect, useRef } from 'react'
import { Viewport } from '../../viewport/Viewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { loadGcodeForViewer, simulateGcodeViewer } from '../../api/gcodeViewer'
import { useViewportStore } from '../../store/viewportStore'
import type { GcodeViewerLoadResult, SimulateGcodeViewerParams } from '../../api/types'

const SAMPLE_URL = `${import.meta.env.BASE_URL}samples/demo-pocket.nc`

type LoadStatus = 'idle' | 'loading' | 'loaded' | 'error'
type SimStatus = 'idle' | 'simulating' | 'ready' | 'error'

interface SimForm {
  originX: string
  originY: string
  originZ: string
  width: string
  depth: string
  height: string
  toolDiameter: string
  resolution: string
}

const DEFAULT_RESOLUTION = '0.5'
const DEFAULT_FORM: SimForm = {
  originX: '0',
  originY: '0',
  originZ: '0',
  width: '',
  depth: '',
  height: '',
  toolDiameter: '',
  resolution: DEFAULT_RESOLUTION,
}

/**
 * Build a fresh sim form from a newly-loaded file's metadata. Stock and
 * tool fields are wiped when the new file lacks the corresponding header
 * so the user can't accidentally run a simulation against stale dimensions
 * from a previous file. Resolution is treated as a sticky user preference.
 */
function formFromMetadata(result: GcodeViewerLoadResult, prev: SimForm): SimForm {
  const stock = result.stock
  const tool = result.tools[0]
  return {
    originX: stock ? String(stock.origin.x) : '0',
    originY: stock ? String(stock.origin.y) : '0',
    originZ: stock ? String(stock.origin.z) : '0',
    width: stock ? String(stock.width) : '',
    depth: stock ? String(stock.depth) : '',
    height: stock ? String(stock.height) : '',
    toolDiameter: tool ? String(tool.diameter) : '',
    resolution: prev.resolution || DEFAULT_RESOLUTION,
  }
}

type ParamsResult =
  | { ok: true; value: SimulateGcodeViewerParams }
  | { ok: false; error: string }

function paramsFromForm(form: SimForm): ParamsResult {
  const positive = (raw: string, label: string): number | string => {
    const n = Number(raw)
    if (!Number.isFinite(n) || n <= 0) return `Enter a positive ${label}.`
    return n
  }
  const width = positive(form.width, 'width')
  const depth = positive(form.depth, 'depth')
  const height = positive(form.height, 'height')
  const toolDiameter = positive(form.toolDiameter, 'tool diameter')
  const resolution = positive(form.resolution, 'resolution')

  for (const v of [width, depth, height, toolDiameter, resolution]) {
    if (typeof v === 'string') return { ok: false, error: v }
  }

  const originX = Number(form.originX)
  const originY = Number(form.originY)
  const originZ = Number(form.originZ)
  if (![originX, originY, originZ].every(Number.isFinite)) {
    return { ok: false, error: 'Origin must be three numbers.' }
  }

  return {
    ok: true,
    value: {
      stock: {
        origin: { x: originX, y: originY, z: originZ },
        width: width as number,
        depth: depth as number,
        height: height as number,
      },
      toolDiameter: toolDiameter as number,
      resolution: resolution as number,
    },
  }
}

export function ToolpathViewerMode() {
  const fileInputRef = useRef<HTMLInputElement | null>(null)

  const [fileName, setFileName] = useState<string | null>(null)
  const [gcodeContent, setGcodeContent] = useState<string | null>(null)
  const [loadResult, setLoadResult] = useState<GcodeViewerLoadResult | null>(null)
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle')
  const [loadError, setLoadError] = useState<string | null>(null)

  const [simForm, setSimForm] = useState<SimForm>(DEFAULT_FORM)
  const [simStatus, setSimStatus] = useState<SimStatus>('idle')
  const [simError, setSimError] = useState<string | null>(null)

  useEffect(() => {
    useViewportStore.getState().setToolpathGeometry(null)
    useViewportStore.getState().clearSimulationMesh()
    return () => {
      useViewportStore.getState().setToolpathGeometry(null)
      useViewportStore.getState().clearSimulationMesh()
    }
  }, [])

  async function loadFromText(name: string, content: string) {
    setLoadStatus('loading')
    setLoadError(null)
    try {
      const result = await loadGcodeForViewer(content)
      setFileName(name)
      setGcodeContent(content)
      setLoadResult(result)
      setLoadStatus('loaded')
      setSimForm((prev) => formFromMetadata(result, prev))
      setSimStatus('idle')
      setSimError(null)
      useViewportStore.getState().setToolpathGeometry(result.lineGeometry)
      useViewportStore.getState().clearSimulationMesh()
    } catch (e) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to load file')
    }
  }

  function handlePickFile() {
    fileInputRef.current?.click()
  }

  async function handleFileChosen(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    const content = await file.text()
    await loadFromText(file.name, content)
  }

  async function handleLoadSample() {
    setLoadStatus('loading')
    setLoadError(null)
    try {
      const response = await fetch(SAMPLE_URL)
      if (!response.ok) {
        throw new Error(`Sample fetch failed: ${response.status}`)
      }
      const content = await response.text()
      await loadFromText('demo-pocket.nc', content)
    } catch (e) {
      setLoadStatus('error')
      setLoadError((e as Error).message ?? 'Failed to load sample')
    }
  }

  async function handleSimulate() {
    if (!gcodeContent) return
    const built = paramsFromForm(simForm)
    if (!built.ok) {
      setSimStatus('error')
      setSimError(built.error)
      return
    }
    setSimStatus('simulating')
    setSimError(null)
    try {
      const mesh = await simulateGcodeViewer(gcodeContent, built.value)
      useViewportStore.getState().setSimulationMeshData(mesh)
      setSimStatus('ready')
    } catch (e) {
      const err = e as { message?: string; kind?: string }
      setSimStatus('error')
      setSimError(err.message ?? err.kind ?? 'Simulation failed')
    }
  }

  function setField<K extends keyof SimForm>(key: K, value: string) {
    setSimForm((prev) => ({ ...prev, [key]: value }))
  }

  const metaStock = loadResult?.stock
  const firstTool = loadResult?.tools[0]
  const canSimulate = gcodeContent !== null && simStatus !== 'simulating'

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <input
        ref={fileInputRef}
        type="file"
        accept=".nc,.gcode,.tap"
        onChange={handleFileChosen}
        className="hidden"
        aria-label="G-code file"
      />

      <div className="flex flex-1 overflow-hidden">
        <Viewport className="flex-1" />
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">

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
                {fileName && (
                  <p className="truncate text-xs text-muted-foreground" title={fileName}>
                    {fileName}
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

            {metaStock && (
              <SidebarSection title="Stock">
                <dl className="grid grid-cols-2 gap-x-2 gap-y-1 text-xs">
                  <dt className="text-muted-foreground">Type</dt>
                  <dd>{metaStock.stockType}</dd>
                  <dt className="text-muted-foreground">Width</dt>
                  <dd>{metaStock.width}</dd>
                  <dt className="text-muted-foreground">Depth</dt>
                  <dd>{metaStock.depth}</dd>
                  <dt className="text-muted-foreground">Height</dt>
                  <dd>{metaStock.height}</dd>
                  <dt className="text-muted-foreground">Origin</dt>
                  <dd>
                    {metaStock.origin.x}, {metaStock.origin.y}, {metaStock.origin.z}
                  </dd>
                </dl>
              </SidebarSection>
            )}

            {firstTool && (
              <SidebarSection title="Tool">
                <dl className="grid grid-cols-2 gap-x-2 gap-y-1 text-xs">
                  <dt className="text-muted-foreground">Number</dt>
                  <dd>{firstTool.number}</dd>
                  <dt className="text-muted-foreground">Type</dt>
                  <dd>{firstTool.toolType}</dd>
                  <dt className="text-muted-foreground">Diameter</dt>
                  <dd>{firstTool.diameter}</dd>
                  {firstTool.flutes !== null && (
                    <>
                      <dt className="text-muted-foreground">Flutes</dt>
                      <dd>{firstTool.flutes}</dd>
                    </>
                  )}
                  {firstTool.material && (
                    <>
                      <dt className="text-muted-foreground">Material</dt>
                      <dd>{firstTool.material}</dd>
                    </>
                  )}
                </dl>
              </SidebarSection>
            )}

            {loadResult && (
              <SidebarSection title="Simulation">
                <div className="grid grid-cols-2 gap-x-2 gap-y-1 text-xs">
                  <SimNumberInput
                    label="Width"
                    value={simForm.width}
                    onChange={(v) => setField('width', v)}
                  />
                  <SimNumberInput
                    label="Depth"
                    value={simForm.depth}
                    onChange={(v) => setField('depth', v)}
                  />
                  <SimNumberInput
                    label="Height"
                    value={simForm.height}
                    onChange={(v) => setField('height', v)}
                  />
                  <SimNumberInput
                    label="Origin X"
                    value={simForm.originX}
                    onChange={(v) => setField('originX', v)}
                  />
                  <SimNumberInput
                    label="Origin Y"
                    value={simForm.originY}
                    onChange={(v) => setField('originY', v)}
                  />
                  <SimNumberInput
                    label="Origin Z"
                    value={simForm.originZ}
                    onChange={(v) => setField('originZ', v)}
                  />
                  <SimNumberInput
                    label="Tool Ø"
                    value={simForm.toolDiameter}
                    onChange={(v) => setField('toolDiameter', v)}
                  />
                  <SimNumberInput
                    label="Resolution"
                    value={simForm.resolution}
                    onChange={(v) => setField('resolution', v)}
                  />
                </div>
                <Button
                  size="sm"
                  className="mt-3 w-full"
                  onClick={handleSimulate}
                  disabled={!canSimulate}
                >
                  {simStatus === 'simulating' ? 'Simulating…' : 'Simulate'}
                </Button>
                {simStatus === 'ready' && (
                  <p className="mt-1 text-xs text-muted-foreground">
                    Simulation complete.
                  </p>
                )}
                {simError && (
                  <p className="mt-1 text-xs text-destructive" role="alert">
                    {simError}
                  </p>
                )}
              </SidebarSection>
            )}

          </ScrollArea>
        </aside>
      </div>
    </div>
  )
}

function SimNumberInput({
  label,
  value,
  onChange,
}: {
  label: string
  value: string
  onChange: (v: string) => void
}) {
  return (
    <>
      <label className="self-center text-muted-foreground">{label}</label>
      <input
        type="text"
        inputMode="decimal"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        aria-label={label}
        className="rounded border border-border bg-background px-1 py-0.5 text-xs"
      />
    </>
  )
}
