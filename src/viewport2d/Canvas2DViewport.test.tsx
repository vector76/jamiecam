/**
 * Tests for Canvas2DViewport — the Mode 2 (2D Profile Cuts) viewport shell.
 *
 * jsdom does not implement a real CanvasRenderingContext2D, so we mock
 * HTMLCanvasElement.prototype.getContext to return a recording spy
 * object. Container clientWidth / clientHeight are also defined via
 * Object.defineProperty since jsdom reports zero by default.
 */

import { act, render } from '@testing-library/react'
import { createRef } from 'react'
import { Canvas2DViewport, type Canvas2DDrawAPI, CANVAS_2D_STYLES } from './Canvas2DViewport'

// ── Global stubs ─────────────────────────────────────────────────────────────

class MockResizeObserver {
  static instances: MockResizeObserver[] = []
  callback: ResizeObserverCallback
  observe = vi.fn()
  disconnect = vi.fn()
  unobserve = vi.fn()
  constructor(cb: ResizeObserverCallback) {
    this.callback = cb
    MockResizeObserver.instances.push(this)
  }
}
vi.stubGlobal('ResizeObserver', MockResizeObserver)

// ── Helpers ──────────────────────────────────────────────────────────────────

function makeMockCtx() {
  return {
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    stroke: vi.fn(),
    fill: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    strokeStyle: '',
    fillStyle: '',
    lineWidth: 0,
  }
}

type MockCtx = ReturnType<typeof makeMockCtx>

let mockCtx: MockCtx
let originalGetContext: typeof HTMLCanvasElement.prototype.getContext

beforeEach(() => {
  MockResizeObserver.instances = []
  mockCtx = makeMockCtx()
  originalGetContext = HTMLCanvasElement.prototype.getContext
  HTMLCanvasElement.prototype.getContext = vi.fn(
    () => mockCtx as unknown as CanvasRenderingContext2D,
  ) as unknown as typeof HTMLCanvasElement.prototype.getContext
  vi.stubGlobal('devicePixelRatio', 2)
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
    configurable: true,
    value: 400,
  })
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
    configurable: true,
    value: 300,
  })
})

afterEach(() => {
  HTMLCanvasElement.prototype.getContext = originalGetContext
})

// ── Tests ────────────────────────────────────────────────────────────────────

describe('Canvas2DViewport — mount', () => {
  it('renders a canvas element', () => {
    const { container } = render(<Canvas2DViewport />)
    expect(container.querySelector('canvas')).toBeInTheDocument()
  })

  it('sizes the canvas backing store by clientWidth * DPR and clientHeight * DPR', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    expect(canvas.width).toBe(800)
    expect(canvas.height).toBe(600)
  })

  it('sets canvas CSS size to client dimensions in px', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    expect(canvas.style.width).toBe('400px')
    expect(canvas.style.height).toBe('300px')
  })

  it('applies a DPR-scaled identity transform to the 2D context', () => {
    render(<Canvas2DViewport />)
    expect(mockCtx.setTransform).toHaveBeenCalledWith(2, 0, 0, 2, 0, 0)
  })

  it('observes the container with a ResizeObserver', () => {
    render(<Canvas2DViewport />)
    expect(MockResizeObserver.instances.length).toBeGreaterThan(0)
    expect(MockResizeObserver.instances[0].observe).toHaveBeenCalled()
  })

  it('disconnects the ResizeObserver on unmount', () => {
    const { unmount } = render(<Canvas2DViewport />)
    const observer = MockResizeObserver.instances.at(-1)!
    unmount()
    expect(observer.disconnect).toHaveBeenCalled()
  })

  it('re-applies sizing to the new client dimensions when the ResizeObserver fires', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    const observer = MockResizeObserver.instances.at(-1)!

    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, value: 200 })
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, value: 100 })

    act(() => {
      observer.callback([], observer as unknown as ResizeObserver)
    })

    expect(canvas.width).toBe(400)
    expect(canvas.height).toBe(200)
    expect(canvas.style.width).toBe('200px')
    expect(canvas.style.height).toBe('100px')
    // setTransform fires once on mount, once on the resize.
    expect(mockCtx.setTransform).toHaveBeenCalledTimes(2)
  })
})

describe('Canvas2DViewport — drawing API', () => {
  it('polyline calls beginPath, moveTo, lineTo per point, then stroke', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[0, 0], [10, 0], [10, 10]], 'artwork')
    })
    expect(mockCtx.beginPath).toHaveBeenCalled()
    expect(mockCtx.moveTo).toHaveBeenCalledWith(0, 0)
    expect(mockCtx.lineTo).toHaveBeenNthCalledWith(1, 10, 0)
    expect(mockCtx.lineTo).toHaveBeenNthCalledWith(2, 10, 10)
    expect(mockCtx.stroke).toHaveBeenCalled()
    expect(mockCtx.closePath).not.toHaveBeenCalled()
  })

  it('polyline with zero points is a no-op', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([], 'artwork')
    })
    expect(mockCtx.beginPath).not.toHaveBeenCalled()
    expect(mockCtx.stroke).not.toHaveBeenCalled()
  })

  it('polygon calls closePath before stroking', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polygon([[0, 0], [10, 0], [10, 10]], 'artwork')
    })
    expect(mockCtx.beginPath).toHaveBeenCalled()
    expect(mockCtx.moveTo).toHaveBeenCalledWith(0, 0)
    expect(mockCtx.lineTo).toHaveBeenNthCalledWith(1, 10, 0)
    expect(mockCtx.lineTo).toHaveBeenNthCalledWith(2, 10, 10)
    expect(mockCtx.closePath).toHaveBeenCalled()
    expect(mockCtx.stroke).toHaveBeenCalled()
  })

  it('artwork style sets stroke color and line width from CANVAS_2D_STYLES', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[0, 0], [1, 1]], 'artwork')
    })
    expect(mockCtx.strokeStyle).toBe(CANVAS_2D_STYLES.artwork.stroke)
    expect(mockCtx.lineWidth).toBe(CANVAS_2D_STYLES.artwork.lineWidth)
  })

  it('toolpath style sets stroke color and line width from CANVAS_2D_STYLES', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[0, 0], [1, 1]], 'toolpath')
    })
    expect(mockCtx.strokeStyle).toBe(CANVAS_2D_STYLES.toolpath.stroke)
    expect(mockCtx.lineWidth).toBe(CANVAS_2D_STYLES.toolpath.lineWidth)
  })

  it('rapid style sets stroke color and line width from CANVAS_2D_STYLES', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[0, 0], [1, 1]], 'rapid')
    })
    expect(mockCtx.strokeStyle).toBe(CANVAS_2D_STYLES.rapid.stroke)
    expect(mockCtx.lineWidth).toBe(CANVAS_2D_STYLES.rapid.lineWidth)
  })

  it('clear() calls clearRect over the full backing store ignoring the world transform', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    const { container } = render(<Canvas2DViewport ref={ref} />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    act(() => {
      ref.current!.clear()
    })
    expect(mockCtx.save).toHaveBeenCalled()
    expect(mockCtx.setTransform).toHaveBeenLastCalledWith(1, 0, 0, 1, 0, 0)
    expect(mockCtx.clearRect).toHaveBeenCalledWith(0, 0, canvas.width, canvas.height)
    expect(mockCtx.restore).toHaveBeenCalled()
  })

  it('passes through world coordinates unchanged (identity for now)', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[123.5, -7.25], [200, 0]], 'artwork')
    })
    expect(mockCtx.moveTo).toHaveBeenCalledWith(123.5, -7.25)
    expect(mockCtx.lineTo).toHaveBeenCalledWith(200, 0)
  })
})
