/**
 * Tests for the useSimulationLoop hook.
 *
 * Covers scene membership (add/remove on start/stop), RAF lifecycle
 * (start on play, cancel on pause, restart on resume), and scrub resyncing.
 */

import { renderHook, act } from '@testing-library/react'
import { useRef } from 'react'
import * as THREE from 'three'
import { useSimulationLoop } from './useSimulationLoop'
import { useViewportStore } from '../store/viewportStore'
import type { SimPoint } from './simulationPoints'

// ── RAF control ───────────────────────────────────────────────────────────────

let pendingRaf: ((ts: number) => void) | null = null
const mockRaf = vi.fn((cb: (ts: number) => void) => {
  pendingRaf = cb
  return 1
})
const mockCancelRaf = vi.fn()

vi.stubGlobal('requestAnimationFrame', mockRaf)
vi.stubGlobal('cancelAnimationFrame', mockCancelRaf)

/** Advance one RAF frame at the given timestamp. */
function tickRaf(ts = 16): void {
  const cb = pendingRaf
  pendingRaf = null
  cb?.(ts)
}

// ── Mocks ─────────────────────────────────────────────────────────────────────

vi.mock('./toolMesh', () => ({
  createToolMesh: vi.fn(() => new THREE.Group()),
  positionToolMesh: vi.fn(),
}))

vi.mock('./simulationHighlight', () => ({
  createHighlightIndicator: vi.fn(() => new THREE.Mesh()),
  positionHighlight: vi.fn(),
}))

// ── Helpers ───────────────────────────────────────────────────────────────────

const SIM_POINTS: SimPoint[] = [
  { x: 0, y: 0, z: 0, moveType: 0 },
  { x: 10, y: 0, z: 0, moveType: 0 },
  { x: 20, y: 0, z: 0, moveType: 0 },
]

function makeScene() {
  return new THREE.Scene()
}

/** Render the hook with a fresh scene. Returns the scene for inspection. */
function setup() {
  const scene = makeScene()
  const mgr = { scene } as unknown as import('./scene').SceneManager

  const { unmount } = renderHook(() => {
    const mgrRef = useRef(mgr)
    useSimulationLoop(mgrRef)
  })

  return { scene, unmount }
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  mockRaf.mockClear()
  mockCancelRaf.mockClear()
  pendingRaf = null
  useViewportStore.setState({
    simulationActive: false,
    simulationPaused: false,
    simulationPoints: null,
    simulationPointIndex: 0,
    simulationPlaybackSpeed: 10,
  })
})

// ── Scene membership ──────────────────────────────────────────────────────────

describe('useSimulationLoop — scene membership', () => {
  it('adds tool mesh and highlight to scene when simulation becomes active', async () => {
    const { scene } = setup()
    expect(scene.children.length).toBe(0)

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    expect(scene.children.length).toBe(2)
  })

  it('removes tool mesh and highlight from scene when simulation stops', async () => {
    const { scene } = setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    await act(async () => {
      useViewportStore.getState().stopSimulation()
    })

    expect(scene.children.length).toBe(0)
  })

  it('does not add objects when simulation starts with empty points', async () => {
    const { scene } = setup()

    await act(async () => {
      useViewportStore.getState().startSimulation([])
    })

    expect(scene.children.length).toBe(0)
  })
})

// ── RAF lifecycle ─────────────────────────────────────────────────────────────

describe('useSimulationLoop — RAF lifecycle', () => {
  it('requests an animation frame when simulation becomes active and not paused', async () => {
    setup()
    expect(mockRaf).not.toHaveBeenCalled()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    expect(mockRaf).toHaveBeenCalled()
  })

  it('cancels the animation frame when simulation is paused', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    await act(async () => {
      useViewportStore.getState().pauseSimulation()
    })

    expect(mockCancelRaf).toHaveBeenCalled()
  })

  it('requests a new animation frame when simulation is resumed', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    const rafCallsAfterStart = mockRaf.mock.calls.length

    await act(async () => {
      useViewportStore.getState().pauseSimulation()
    })

    await act(async () => {
      useViewportStore.getState().resumeSimulation()
    })

    expect(mockRaf.mock.calls.length).toBeGreaterThan(rafCallsAfterStart)
  })

  it('cancels any pending RAF when simulation stops', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    mockCancelRaf.mockClear()

    await act(async () => {
      useViewportStore.getState().stopSimulation()
    })

    expect(mockCancelRaf).toHaveBeenCalled()
  })

  it('cancels RAF on unmount', async () => {
    const { unmount } = setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    mockCancelRaf.mockClear()
    unmount()

    expect(mockCancelRaf).toHaveBeenCalled()
  })
})

// ── RAF loop advancing ────────────────────────────────────────────────────────

describe('useSimulationLoop — loop advances simulationPointIndex', () => {
  it('updates simulationPointIndex after a frame with sufficient elapsed time', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    // SIM_POINTS span 20 mm total. At 50 mm/s × speed 10 = 500 mm/s.
    // First frame (ts=0): sets prevTimestamp, accumulates nothing.
    await act(async () => { tickRaf(0) })
    // Second frame (ts=15 ms): deltaMs=15 → dist = 15/1000 * 50 * 10 = 7.5 mm.
    // 7.5 mm is between cumDist[0]=0 and cumDist[1]=10 → idx = 1 (nearest), not last point.
    await act(async () => { tickRaf(15) })

    const idx = useViewportStore.getState().simulationPointIndex
    expect(idx).toBeGreaterThan(0)
  })

  it('does not accumulate distance on the very first frame', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    // Only one frame — should not advance past index 0
    await act(async () => { tickRaf(100) })

    const idx = useViewportStore.getState().simulationPointIndex
    expect(idx).toBe(0)
  })
})

// ── Scrub resyncing ───────────────────────────────────────────────────────────

describe('useSimulationLoop — scrub resyncing', () => {
  it('resyncs accumulatedDist when simulationPointIndex is changed externally', async () => {
    setup()

    await act(async () => {
      useViewportStore.getState().startSimulation(SIM_POINTS)
    })

    // Pause before any frame fires so accumulatedDist stays at 0.
    await act(async () => {
      useViewportStore.getState().pauseSimulation()
    })

    // Externally scrub to point index 1 (cumDist = 10 mm, midpoint).
    await act(async () => {
      useViewportStore.getState().setSimulationPointIndex(1)
    })

    // Resume and fire one frame (prevTimestamp=null → no delta, but position is
    // computed from the resynced accumulatedDist of 10 mm → idx = 1).
    await act(async () => {
      useViewportStore.getState().resumeSimulation()
    })
    await act(async () => { tickRaf(500) })

    // Without resync, accumulatedDist would still be 0 → idx = 0.
    // With resync, accumulatedDist = 10 mm → idx = 1.
    const idx = useViewportStore.getState().simulationPointIndex
    expect(idx).toBeGreaterThanOrEqual(1)
  })
})
