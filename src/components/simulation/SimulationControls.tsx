import { useViewportStore } from '../../store/viewportStore'
import { extractSimPoints } from '../../viewport/simulationPoints'
import { Button } from '@/components/ui/button'
import { Play, Pause, Square } from 'lucide-react'

export function SimulationControls() {
  const toolpathGeometry = useViewportStore((s) => s.toolpathGeometry)
  const simulationActive = useViewportStore((s) => s.simulationActive)
  const simulationPaused = useViewportStore((s) => s.simulationPaused)
  const simulationPoints = useViewportStore((s) => s.simulationPoints)
  const simulationPointIndex = useViewportStore((s) => s.simulationPointIndex)
  const simulationPlaybackSpeed = useViewportStore((s) => s.simulationPlaybackSpeed)
  const startSimulation = useViewportStore((s) => s.startSimulation)
  const pauseSimulation = useViewportStore((s) => s.pauseSimulation)
  const stopSimulation = useViewportStore((s) => s.stopSimulation)
  const setSimulationPointIndex = useViewportStore((s) => s.setSimulationPointIndex)
  const setSimulationPlaybackSpeed = useViewportStore((s) => s.setSimulationPlaybackSpeed)

  const showPlay = !simulationActive || simulationPaused
  const playLabel = simulationPaused ? 'Resume' : 'Play'

  return (
    <div className="absolute bottom-2 left-2 z-10 flex items-center gap-1">
      {showPlay && (
        <Button
          variant="secondary"
          size="sm"
          onClick={() => startSimulation(extractSimPoints(toolpathGeometry!))}
          disabled={toolpathGeometry === null}
          title={playLabel}
        >
          <Play className="h-3.5 w-3.5" />
        </Button>
      )}
      {simulationActive && !simulationPaused && (
        <Button variant="secondary" size="sm" onClick={pauseSimulation} title="Pause">
          <Pause className="h-3.5 w-3.5" />
        </Button>
      )}
      <Button
        variant="secondary"
        size="sm"
        onClick={stopSimulation}
        disabled={!simulationActive}
        title="Stop"
      >
        <Square className="h-3.5 w-3.5" />
      </Button>
      <input
        type="range"
        min={0}
        max={(simulationPoints?.length ?? 1) - 1}
        value={simulationPointIndex}
        onChange={(e) => setSimulationPointIndex(Number(e.target.value))}
        disabled={!simulationActive}
        className="h-1.5 w-24 accent-primary"
      />
      <select
        value={simulationPlaybackSpeed}
        onChange={(e) => setSimulationPlaybackSpeed(Number(e.target.value))}
        disabled={toolpathGeometry === null}
        className="h-7 rounded-sm border border-border bg-secondary px-1 text-xs text-secondary-foreground"
      >
        <option value={1}>1x</option>
        <option value={5}>5x</option>
        <option value={10}>10x</option>
        <option value={20}>20x</option>
      </select>
    </div>
  )
}
