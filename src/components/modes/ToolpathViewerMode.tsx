/**
 * ToolpathViewerMode — G-code Viewer mode component (web build).
 *
 * Provides a sidebar for loading a `.nc` file (via browser file picker or
 * a bundled sample) alongside a persistent 3-D viewport showing the parsed
 * toolpath centerlines.
 */

import { useState, useEffect, useRef } from 'react'
import { Viewport } from '../../viewport/Viewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { loadGcodeForViewer } from '../../api/gcodeViewer'
import { useViewportStore } from '../../store/viewportStore'
import type { GcodeViewerLoadResult } from '../../api/types'

const SAMPLE_URL = `${import.meta.env.BASE_URL}samples/demo-pocket.nc`

type LoadStatus = 'idle' | 'loading' | 'loaded' | 'error'

export function ToolpathViewerMode() {
  const fileInputRef = useRef<HTMLInputElement | null>(null)

  const [fileName, setFileName] = useState<string | null>(null)
  const [loadResult, setLoadResult] = useState<GcodeViewerLoadResult | null>(null)
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle')
  const [loadError, setLoadError] = useState<string | null>(null)

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
      setLoadResult(result)
      setLoadStatus('loaded')
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

  const metaStock = loadResult?.stock
  const firstTool = loadResult?.tools[0]

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

          </ScrollArea>
        </aside>
      </div>
    </div>
  )
}
