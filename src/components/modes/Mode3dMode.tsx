/**
 * Mode3dMode — 3D Surface mode (Mode 4).
 *
 * Initial slice: heightmap import only. User picks a PNG/TIFF grayscale image;
 * the Rust backend tessellates it into a displaced plane which is rendered in
 * the shared Viewport. Physical footprint and Z range are hardcoded for v1.
 */

import { useEffect, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { Viewport } from '../../viewport/Viewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { loadHeightmap } from '../../api/heightmap'
import { useViewportStore } from '../../store/viewportStore'
import { useProjectStore } from '../../store/projectStore'
import { checkUnsavedChanges } from '../../lib/unsavedGuard'

type LoadStatus = 'idle' | 'loading' | 'loaded' | 'error'

export function Mode3dMode() {
  const [filePath, setFilePath] = useState<string | null>(null)
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle')
  const [loadError, setLoadError] = useState<string | null>(null)

  // Clear any stale viewport state on mount and when unmounting.
  useEffect(() => {
    useViewportStore.getState().setMeshData(null)
    useViewportStore.getState().setToolpathGeometry(null)
    useViewportStore.getState().clearSimulationMesh()
    return () => {
      useViewportStore.getState().setMeshData(null)
    }
  }, [])

  async function loadFromPath(path: string) {
    setLoadStatus('loading')
    setLoadError(null)
    try {
      const mesh = await loadHeightmap(path)
      useViewportStore.getState().setMeshData(mesh)
      setFilePath(path)
      setLoadStatus('loaded')
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to load heightmap')
    }
  }

  async function handlePickFile() {
    try {
      const result = await open({
        filters: [{ name: 'Heightmap image', extensions: ['png', 'tif', 'tiff'] }],
        multiple: false,
      })
      if (typeof result !== 'string') return
      await loadFromPath(result)
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      setLoadStatus('error')
      setLoadError(err.message ?? err.kind ?? 'Failed to open file dialog')
    }
  }

  async function handleBack() {
    const safe = await checkUnsavedChanges()
    if (!safe) return
    useProjectStore.getState().returnToSelector()
  }

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <div className="flex flex-1 overflow-hidden">
        <Viewport className="flex-1" />
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">
            <div className="border-b border-border px-3 py-2">
              <Button size="sm" variant="ghost" onClick={handleBack}>
                ← Back
              </Button>
            </div>

            <SidebarSection title="Heightmap">
              <div className="flex flex-col gap-2">
                <Button size="sm" className="w-full" onClick={handlePickFile}>
                  Open Heightmap…
                </Button>
                <p className="text-xs text-muted-foreground">
                  PNG or TIFF grayscale. Footprint 100×100 mm, Z range 10 mm (fixed in this build).
                </p>
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
              </div>
            </SidebarSection>
          </ScrollArea>
        </aside>
      </div>
    </div>
  )
}
