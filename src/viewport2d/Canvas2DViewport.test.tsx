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
import { useViewport2DStore } from '../store/viewport2dStore'

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

/**
 * jsdom does not implement PointerEvent. The handlers under test only
 * read `pointerId`, `clientX`, and `clientY`, so we synthesise a
 * MouseEvent of the right type and attach a `pointerId` property.
 */
function pointer(
  type: 'pointerdown' | 'pointermove' | 'pointerup' | 'pointercancel',
  init: { pointerId?: number; clientX?: number; clientY?: number },
): Event {
  const e = new MouseEvent(type, {
    bubbles: true,
    clientX: init.clientX ?? 0,
    clientY: init.clientY ?? 0,
  })
  Object.defineProperty(e, 'pointerId', { value: init.pointerId ?? 1, configurable: true })
  return e
}

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
  useViewport2DStore.getState().reset()
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

  it('passes through world coordinates unchanged under the identity transform', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      ref.current!.polyline([[123.5, -7.25], [200, 0]], 'artwork')
    })
    expect(mockCtx.moveTo).toHaveBeenCalledWith(123.5, -7.25)
    expect(mockCtx.lineTo).toHaveBeenCalledWith(200, 0)
  })

  it('applies the store transform when drawing', () => {
    const ref = createRef<Canvas2DDrawAPI>()
    render(<Canvas2DViewport ref={ref} />)
    act(() => {
      useViewport2DStore.getState().setTransform({ tx: 50, ty: -10, scale: 2 })
    })
    act(() => {
      ref.current!.polyline([[10, 5], [20, 0]], 'artwork')
    })
    // world (10,5) → (10*2+50, 5*2-10) = (70, 0)
    expect(mockCtx.moveTo).toHaveBeenCalledWith(70, 0)
    // world (20,0) → (20*2+50, 0*2-10) = (90, -10)
    expect(mockCtx.lineTo).toHaveBeenCalledWith(90, -10)
  })
})

// ── Pan / zoom integration ───────────────────────────────────────────────────

describe('Canvas2DViewport — pan/zoom interactions', () => {
  it('drag updates the store transform translation by the cumulative screen delta', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement

    act(() => {
      canvas.dispatchEvent(pointer('pointerdown', { clientX: 100, clientY: 100 }))
      canvas.dispatchEvent(pointer('pointermove', { clientX: 140, clientY: 75 }))
      canvas.dispatchEvent(pointer('pointerup', { clientX: 140, clientY: 75 }))
    })

    const t = useViewport2DStore.getState().transform
    expect(t.tx).toBe(40)
    expect(t.ty).toBe(-25)
    expect(t.scale).toBe(1)
  })

  it('pointer move without a prior pointerdown does not pan', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    act(() => {
      canvas.dispatchEvent(pointer('pointermove', { clientX: 200, clientY: 200 }))
    })
    expect(useViewport2DStore.getState().transform).toEqual({ tx: 0, ty: 0, scale: 1 })
  })

  it('pointer move after pointerup stops panning', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement
    act(() => {
      canvas.dispatchEvent(pointer('pointerdown', { clientX: 0, clientY: 0 }))
      canvas.dispatchEvent(pointer('pointermove', { clientX: 10, clientY: 10 }))
      canvas.dispatchEvent(pointer('pointerup', { clientX: 10, clientY: 10 }))
      canvas.dispatchEvent(pointer('pointermove', { clientX: 50, clientY: 50 }))
    })
    expect(useViewport2DStore.getState().transform.tx).toBe(10)
    expect(useViewport2DStore.getState().transform.ty).toBe(10)
  })

  it('wheel up zooms in at the cursor', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement

    act(() => {
      canvas.dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, clientX: 200, clientY: 150, bubbles: true, cancelable: true }),
      )
    })

    const t = useViewport2DStore.getState().transform
    expect(t.scale).toBeCloseTo(1.1, 9)
    // Pivot at (200, 150) on identity transform → world (200, 150).
    // After zooming to scale 1.1, world point must still land at (200, 150):
    // tx = 200 - 200 * 1.1 = -20, ty = 150 - 150 * 1.1 = -15.
    expect(t.tx).toBeCloseTo(-20, 9)
    expect(t.ty).toBeCloseTo(-15, 9)
  })

  it('wheel down zooms out at the cursor', () => {
    const { container } = render(<Canvas2DViewport />)
    const canvas = container.querySelector('canvas') as HTMLCanvasElement

    act(() => {
      canvas.dispatchEvent(
        new WheelEvent('wheel', { deltaY: 100, clientX: 0, clientY: 0, bubbles: true, cancelable: true }),
      )
    })

    const t = useViewport2DStore.getState().transform
    expect(t.scale).toBeCloseTo(1 / 1.1, 9)
  })
})
