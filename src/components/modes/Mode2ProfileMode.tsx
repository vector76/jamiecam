/**
 * Mode2ProfileMode — Phase 4 (2-D Profile Cuts) component shell.
 *
 * Lays out the Mode 2 surface: Canvas2DViewport as the primary workspace
 * with a right-hand sidebar mirroring Mode 1's structure. Individual
 * sidebar sections (File, Paths, Operation, Generate, Simulate, Export)
 * are intentionally empty for now — they get fleshed out in later beads.
 *
 * The engine-init lifecycle mirrors Mode 1: prewarm the shared wasm
 * module on mount and surface its status (initializing / ready / failed)
 * inside the File section, since loading SVG/DXF inputs will need it.
 */

import { useEffect, useState } from 'react'
import { Canvas2DViewport } from '../../viewport2d/Canvas2DViewport'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { prewarmWasm } from '../../api/gcodeViewer'
import type { ProjectState } from '../../persistence/projectFile'

type EngineStatus = 'initializing' | 'ready' | 'failed'

interface Mode2ProfileModeProps {
  /**
   * Optional project to hydrate from on mount. Accepted for shape
   * parity with Mode 1 — actual hydration of Mode 2 payloads lands in
   * a later bead, so the prop is currently unused.
   */
  initialProject?: ProjectState | null
}

export function Mode2ProfileMode(_props: Mode2ProfileModeProps = {}) {
  const [engineStatus, setEngineStatus] = useState<EngineStatus>('initializing')
  const [engineError, setEngineError] = useState<string | null>(null)

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

  return (
    <div
      data-testid="mode2-root"
      className="flex h-full flex-1 flex-col bg-background text-foreground"
    >
      <div className="flex flex-1 overflow-hidden">
        <Canvas2DViewport className="flex-1" />
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
                <p className="text-xs text-muted-foreground">
                  SVG / DXF import — coming soon.
                </p>
              </div>
            </SidebarSection>

            <SidebarSection title="Paths">
              <p className="text-xs text-muted-foreground">
                Path selection — coming soon.
              </p>
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
    </div>
  )
}
