import { useState } from 'react'
import { getDemoSimulationMesh, getSimulationMesh } from '../../api/dexel'
import { useViewportStore } from '../../store/viewportStore'
import { useProjectStore } from '../../store/projectStore'
import { Button } from '@/components/ui/button'

const RESOLUTION_OPTIONS: Array<{ label: string; value: number }> = [
  { label: '0.5 mm (fast)', value: 0.5 },
  { label: '0.1 mm', value: 0.1 },
  { label: '0.05 mm (fine)', value: 0.05 },
]

type LoadingState = 'idle' | 'simulating' | 'demo'

export function MaterialRemovalPanel() {
  const [loadingState, setLoadingState] = useState<LoadingState>('idle')
  const [resolution, setResolution] = useState(0.5)

  const simulationMeshData = useViewportStore((s) => s.simulationMeshData)
  const setSimulationMeshData = useViewportStore((s) => s.setSimulationMeshData)
  const clearSimulationMesh = useViewportStore((s) => s.clearSimulationMesh)
  const pushNotification = useProjectStore((s) => s.pushNotification)

  const busy = loadingState !== 'idle'

  async function runSimulation(fn: () => Promise<void>) {
    try {
      await fn()
    } catch (e: unknown) {
      const err = e as { message?: string; kind?: string }
      pushNotification(err.message ?? err.kind ?? 'Simulation failed')
    } finally {
      setLoadingState('idle')
    }
  }

  function handleSimulate() {
    setLoadingState('simulating')
    runSimulation(async () => {
      const mesh = await getSimulationMesh(resolution)
      setSimulationMeshData(mesh)
    })
  }

  function handleDemo() {
    setLoadingState('demo')
    runSimulation(async () => {
      const mesh = await getDemoSimulationMesh(resolution)
      setSimulationMeshData(mesh)
    })
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <label className="text-xs text-muted-foreground" htmlFor="sim-resolution">
          Resolution
        </label>
        <select
          id="sim-resolution"
          value={resolution}
          onChange={(e) => setResolution(Number(e.target.value))}
          className="h-7 flex-1 rounded-sm border border-border bg-background px-1 text-xs"
        >
          {RESOLUTION_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
      <div className="flex gap-2">
        <Button
          size="sm"
          className="flex-1"
          onClick={handleSimulate}
          disabled={busy}
          aria-busy={loadingState === 'simulating'}
        >
          {loadingState === 'simulating' ? 'Simulating…' : 'Simulate'}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          onClick={handleDemo}
          disabled={busy}
          aria-busy={loadingState === 'demo'}
          title="Run built-in demo: 100×100×20 mm block with a two-level stepped pocket"
        >
          {loadingState === 'demo' ? 'Loading…' : 'Demo'}
        </Button>
      </div>
      {simulationMeshData !== null && (
        <div className="flex items-center justify-between">
          <p className="text-xs text-muted-foreground">Showing simulated workpiece</p>
          <Button size="sm" variant="ghost" className="h-5 px-1 text-xs" onClick={clearSimulationMesh}>
            Clear
          </Button>
        </div>
      )}
    </div>
  )
}
