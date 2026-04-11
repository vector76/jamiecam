/**
 * Tests for Canvas2D.tsx — interactive 2D canvas component.
 *
 * useCanvas2dStore is mocked so tests run in jsdom without a real Zustand
 * subscription cycle. The canvas 2D context is also mocked since jsdom does
 * not implement it.
 */

import { render, fireEvent } from '@testing-library/react'
import { Canvas2D } from './Canvas2D'

// ── Hoisted mocks (available inside vi.mock factories) ────────────────────────

const mockFns = vi.hoisted(() => ({
  setSelectedCurveId: vi.fn(),
  setPanOffset: vi.fn(),
  setZoom: vi.fn(),
}))

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../../store/canvas2dStore', () => {
  // Fixed store state: panOffset=(200,200), zoom=1.0, canvasHeight=400 from
  // the ResizeObserver mock gives a simple 1:1 coordinate mapping:
  //   worldToScreen(wx, wy) = (wx+200, 400-(wy+200))
  const storeState = {
    panOffset: { x: 200, y: 200 },
    zoom: 1.0,
    selectedCurveId: null as string | null,
    setPanOffset: mockFns.setPanOffset,
    setZoom: mockFns.setZoom,
    setSelectedCurveId: mockFns.setSelectedCurveId,
    resetView: vi.fn(),
  }
  const useCanvas2dStore = Object.assign(vi.fn(() => storeState), {
    getState: () => storeState,
  })
  return { useCanvas2dStore }
})

// ── Canvas 2D context mock ────────────────────────────────────────────────────

const mockCtx = {
  clearRect: vi.fn(),
  fillRect: vi.fn(),
  strokeRect: vi.fn(),
  beginPath: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  closePath: vi.fn(),
  stroke: vi.fn(),
  setLineDash: vi.fn(),
  fillStyle: '',
  strokeStyle: '',
  lineWidth: 1,
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()

  // jsdom doesn't implement getContext — return a no-op mock
  HTMLCanvasElement.prototype.getContext = vi.fn().mockReturnValue(mockCtx) as never

  // Fire the ResizeObserver callback synchronously so React effects that depend
  // on container size settle before assertions.
  vi.stubGlobal(
    'ResizeObserver',
    vi.fn().mockImplementation((cb: ResizeObserverCallback) => ({
      observe: vi.fn().mockImplementation(() => {
        cb(
          [{ contentRect: { width: 400, height: 400 } } as ResizeObserverEntry],
          {} as ResizeObserver,
        )
      }),
      disconnect: vi.fn(),
    })),
  )
})

afterEach(() => {
  vi.unstubAllGlobals()
})

// ── Fixtures ──────────────────────────────────────────────────────────────────

const CLOSED_CURVE = {
  id: 'curve-1',
  isClosed: true,
  bbox: { minX: 0, minY: 0, maxX: 50, maxY: 50 },
}

// Points form a 50×50 square; with the mocked store state they map to screen:
//   (0,0)→(200,200)  (50,0)→(250,200)  (50,50)→(250,150)  (0,50)→(200,150)
const CURVE_POINTS: Map<string, number[][]> = new Map([
  ['curve-1', [[0, 0], [50, 0], [50, 50], [0, 50]]],
])

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Canvas2D — rendering', () => {
  it('renders a canvas element', () => {
    const { container } = render(
      <Canvas2D
        curves={[]}
        fullCurvePoints={new Map()}
        artworkOffset={[0, 0]}
        stockDims={null}
        assignedCurveIds={new Set()}
        onCurveSelect={vi.fn()}
        onArtworkOriginChange={vi.fn()}
      />,
    )
    expect(container.querySelector('canvas')).not.toBeNull()
  })
})

describe('Canvas2D — click selection', () => {
  it('calls onCurveSelect(null) when clicking empty area', () => {
    const onCurveSelect = vi.fn()
    const { container } = render(
      <Canvas2D
        curves={[]}
        fullCurvePoints={new Map()}
        artworkOffset={[0, 0]}
        stockDims={null}
        assignedCurveIds={new Set()}
        onCurveSelect={onCurveSelect}
        onArtworkOriginChange={vi.fn()}
      />,
    )
    const canvas = container.querySelector('canvas')!
    vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      width: 400,
      height: 400,
      right: 400,
      bottom: 400,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect)

    fireEvent.click(canvas, { clientX: 10, clientY: 10 })

    expect(onCurveSelect).toHaveBeenCalledWith(null)
  })

  it('calls setSelectedCurveId with correct ID when clicking near a closed curve', () => {
    const onCurveSelect = vi.fn()
    const { container } = render(
      <Canvas2D
        curves={[CLOSED_CURVE]}
        fullCurvePoints={CURVE_POINTS}
        artworkOffset={[0, 0]}
        stockDims={null}
        assignedCurveIds={new Set()}
        onCurveSelect={onCurveSelect}
        onArtworkOriginChange={vi.fn()}
      />,
    )
    const canvas = container.querySelector('canvas')!
    vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      width: 400,
      height: 400,
      right: 400,
      bottom: 400,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect)

    // Bottom edge of the curve runs from screen (200,200) to (250,200).
    // Clicking at the midpoint (225,200) has distance 0 from the edge.
    fireEvent.click(canvas, { clientX: 225, clientY: 200 })

    expect(mockFns.setSelectedCurveId).toHaveBeenCalledWith('curve-1')
    expect(onCurveSelect).toHaveBeenCalledWith('curve-1')
  })
})
