import { render, screen, fireEvent, act } from '@testing-library/react'
import { SimulationControls } from './SimulationControls'
import { useViewportStore } from '../../store/viewportStore'
import type { LineGeometryData } from '../../api/types'
import type { SimPoint } from '../../viewport/simulationPoints'

vi.mock('../../viewport/simulationPoints', () => ({
  extractSimPoints: vi.fn((_data: LineGeometryData): SimPoint[] => [
    { x: 0, y: 0, z: 0, moveType: 0 },
    { x: 1, y: 0, z: 0, moveType: 0 },
  ]),
}))

const { extractSimPoints } = await import('../../viewport/simulationPoints')

const TOOLPATH: LineGeometryData = {
  positions: [0, 0, 0, 1, 0, 0],
  types: [0],
  colours: [1, 1, 1, 1, 1, 1],
}

function defaultState() {
  return {
    toolpathGeometry: null as LineGeometryData | null,
    simulationActive: false,
    simulationPaused: false,
    simulationPoints: null as SimPoint[] | null,
    simulationProgress: 0,
    simulationPlaybackSpeed: 10,
  }
}

beforeEach(() => {
  useViewportStore.setState(defaultState())
  vi.mocked(extractSimPoints).mockClear()
})

describe('SimulationControls — disabled with no toolpath', () => {
  it('Play button is disabled when toolpathGeometry is null', () => {
    render(<SimulationControls />)
    expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled()
  })

  it('speed control is disabled when toolpathGeometry is null', () => {
    render(<SimulationControls />)
    expect(screen.getByRole('combobox')).toBeDisabled()
  })
})

describe('SimulationControls — Play starts simulation', () => {
  it('clicking Play calls startSimulation with extracted points', async () => {
    const startSimulation = vi.fn()
    useViewportStore.setState({ toolpathGeometry: TOOLPATH, startSimulation } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Play' }))
    })

    expect(extractSimPoints).toHaveBeenCalledWith(TOOLPATH)
    expect(startSimulation).toHaveBeenCalledWith([
      { x: 0, y: 0, z: 0, moveType: 0 },
      { x: 1, y: 0, z: 0, moveType: 0 },
    ])
  })
})

describe('SimulationControls — Resume from paused', () => {
  it('clicking Play while paused calls resumeSimulation instead of startSimulation', async () => {
    const resumeSimulation = vi.fn()
    const startSimulation = vi.fn()
    useViewportStore.setState({
      toolpathGeometry: TOOLPATH,
      simulationActive: true,
      simulationPaused: true,
      resumeSimulation,
      startSimulation,
    } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Resume' }))
    })

    expect(resumeSimulation).toHaveBeenCalledOnce()
    expect(startSimulation).not.toHaveBeenCalled()
  })
})

describe('SimulationControls — Pause appears when playing', () => {
  it('shows Pause button when simulationActive=true, simulationPaused=false', () => {
    useViewportStore.setState({ simulationActive: true, simulationPaused: false })
    render(<SimulationControls />)
    expect(screen.getByRole('button', { name: 'Pause' })).toBeInTheDocument()
  })

  it('clicking Pause calls pauseSimulation', async () => {
    const pauseSimulation = vi.fn()
    useViewportStore.setState({ simulationActive: true, simulationPaused: false, pauseSimulation } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Pause' }))
    })

    expect(pauseSimulation).toHaveBeenCalledOnce()
  })
})

describe('SimulationControls — Stop resets', () => {
  it('clicking Stop calls stopSimulation', async () => {
    const stopSimulation = vi.fn()
    useViewportStore.setState({ simulationActive: true, stopSimulation } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Stop' }))
    })

    expect(stopSimulation).toHaveBeenCalledOnce()
  })
})

describe('SimulationControls — Scrub slider', () => {
  it('changing slider calls setSimulationProgress with new value', async () => {
    const setSimulationProgress = vi.fn()
    useViewportStore.setState({
      simulationActive: true,
      setSimulationProgress,
    } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.change(screen.getByRole('slider'), { target: { value: '0.75' } })
    })

    expect(setSimulationProgress).toHaveBeenCalledWith(0.75)
  })
})

describe('SimulationControls — Speed control', () => {
  it('changing speed select calls setSimulationPlaybackSpeed with numeric value', async () => {
    const setSimulationPlaybackSpeed = vi.fn()
    useViewportStore.setState({ toolpathGeometry: TOOLPATH, setSimulationPlaybackSpeed } as never)

    render(<SimulationControls />)
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox'), { target: { value: '20' } })
    })

    expect(setSimulationPlaybackSpeed).toHaveBeenCalledWith(20)
  })
})
