import { useViewportStore } from '../../store/viewportStore'
import { extractSimPoints } from '../../viewport/simulationPoints'
import { Button } from '@/components/ui/button'
import { Play, Pause, Square } from 'lucide-react'

export function SimulationControls() {
  const toolpathGeometry = useViewportStore((s) => s.toolpathGeometry)
  const simulationActive = useViewportStore((s) => s.simulationActive)
  const simulationPaused = useViewportStore((s) => s.simulationPaused)
  const simulationProgress = useViewportStore((s) => s.simulationProgress)
  const simulationPlaybackSpeed = useViewportStore((s) => s.simulationPlaybackSpeed)
  const startSimulation = useViewportStore((s) => s.startSimulation)
  const pauseSimulation = useViewportStore((s) => s.pauseSimulation)
  const resumeSimulation = useViewportStore((s) => s.resumeSimulation)
  const stopSimulation = useViewportStore((s) => s.stopSimulation)
  const setSimulationProgress = useViewportStore((s) => s.setSimulationProgress)
  const setSimulationPlaybackSpeed = useViewportStore((s) => s.setSimulationPlaybackSpeed)

  const showPlay = !simulationActive || simulationPaused
  const playLabel = simulationPaused ? 'Resume' : 'Play'

  function handlePlay() {
    if (simulationPaused) {
      resumeSimulation()
    } else {
      startSimulation(extractSimPoints(toolpathGeometry!))
    }
  }

  return (
    <div className="absolute bottom-2 left-2 right-2 z-10 flex items-center gap-1">
      {showPlay && (
        <Button
          variant="secondary"
          size="sm"
          onClick={handlePlay}
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
        max={1}
        step="any"
        value={simulationProgress}
        onChange={(e) => setSimulationProgress(Number(e.target.value))}
        disabled={!simulationActive}
        className="h-1.5 min-w-0 flex-1 accent-primary"
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
