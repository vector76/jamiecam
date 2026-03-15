import { useViewportStore } from '../../store/viewportStore'
import { extractSimPoints } from '../../viewport/simulationPoints'

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
    <div style={{ position: 'absolute', bottom: 8, left: 8, display: 'flex', gap: 4, alignItems: 'center' }}>
      {showPlay && (
        <button
          onClick={() => startSimulation(extractSimPoints(toolpathGeometry!))}
          disabled={toolpathGeometry === null}
          style={{ padding: '2px 8px', fontSize: 12, cursor: 'pointer' }}
        >
          {playLabel}
        </button>
      )}
      {simulationActive && !simulationPaused && (
        <button
          onClick={pauseSimulation}
          style={{ padding: '2px 8px', fontSize: 12, cursor: 'pointer' }}
        >
          Pause
        </button>
      )}
      <button
        onClick={stopSimulation}
        disabled={!simulationActive}
        style={{ padding: '2px 8px', fontSize: 12, cursor: 'pointer' }}
      >
        Stop
      </button>
      <input
        type="range"
        min={0}
        max={(simulationPoints?.length ?? 1) - 1}
        value={simulationPointIndex}
        onChange={(e) => setSimulationPointIndex(Number(e.target.value))}
        disabled={!simulationActive}
      />
      <select
        value={simulationPlaybackSpeed}
        onChange={(e) => setSimulationPlaybackSpeed(Number(e.target.value))}
        disabled={toolpathGeometry === null}
        style={{ padding: '2px 4px', fontSize: 12, cursor: 'pointer' }}
      >
        <option value={1}>1×</option>
        <option value={5}>5×</option>
        <option value={10}>10×</option>
        <option value={20}>20×</option>
      </select>
    </div>
  )
}
