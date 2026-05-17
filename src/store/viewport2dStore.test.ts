import {
  IDENTITY_TRANSFORM,
  computePan,
  computeZoomAt,
  computeZoomToFit,
  screenToWorld,
  useViewport2DStore,
  worldToScreen,
  type Extent2D,
  type Transform2D,
} from './viewport2dStore'

beforeEach(() => {
  useViewport2DStore.getState().reset()
})

// ── Pure helpers ────────────────────────────────────────────────────────────

describe('viewport2dStore — worldToScreen / screenToWorld', () => {
  it('identity transform maps world to screen unchanged', () => {
    expect(worldToScreen(IDENTITY_TRANSFORM, 12, -7)).toEqual({ x: 12, y: -7 })
  })

  it('applies scale then translation', () => {
    const t: Transform2D = { tx: 100, ty: 50, scale: 2 }
    expect(worldToScreen(t, 5, 3)).toEqual({ x: 110, y: 56 })
  })

  it('screenToWorld is the inverse of worldToScreen', () => {
    const t: Transform2D = { tx: 25, ty: -40, scale: 1.5 }
    const s = worldToScreen(t, 8, 12)
    const w = screenToWorld(t, s.x, s.y)
    expect(w.x).toBeCloseTo(8, 9)
    expect(w.y).toBeCloseTo(12, 9)
  })

  it('round-trips a variety of points', () => {
    const t: Transform2D = { tx: -17.5, ty: 33.25, scale: 0.42 }
    for (const [x, y] of [
      [0, 0],
      [1, 1],
      [-100, 250],
      [3.14159, -2.71828],
    ]) {
      const s = worldToScreen(t, x, y)
      const back = screenToWorld(t, s.x, s.y)
      expect(back.x).toBeCloseTo(x, 9)
      expect(back.y).toBeCloseTo(y, 9)
    }
  })
})

describe('viewport2dStore — computePan', () => {
  it('shifts the translation by the screen-space delta', () => {
    const t: Transform2D = { tx: 10, ty: 20, scale: 3 }
    expect(computePan(t, 5, -8)).toEqual({ tx: 15, ty: 12, scale: 3 })
  })

  it('leaves scale unchanged', () => {
    const t: Transform2D = { tx: 0, ty: 0, scale: 2.5 }
    expect(computePan(t, 100, -50).scale).toBe(2.5)
  })
})

describe('viewport2dStore — computeZoomAt', () => {
  it('keeps the world point under the pivot fixed', () => {
    const t: Transform2D = { tx: 30, ty: 40, scale: 2 }
    const sx = 200
    const sy = 150
    const worldBefore = screenToWorld(t, sx, sy)
    const t2 = computeZoomAt(t, 1.5, sx, sy)
    const worldAfter = screenToWorld(t2, sx, sy)
    expect(worldAfter.x).toBeCloseTo(worldBefore.x, 9)
    expect(worldAfter.y).toBeCloseTo(worldBefore.y, 9)
  })

  it('multiplies scale by the factor', () => {
    const t: Transform2D = { tx: 0, ty: 0, scale: 2 }
    expect(computeZoomAt(t, 1.5, 0, 0).scale).toBeCloseTo(3, 9)
  })

  it('zoom-in followed by inverse zoom-out at the same pivot is a no-op', () => {
    const t: Transform2D = { tx: 12, ty: -7, scale: 0.8 }
    const sx = 320
    const sy = 240
    const t2 = computeZoomAt(t, 2, sx, sy)
    const t3 = computeZoomAt(t2, 0.5, sx, sy)
    expect(t3.scale).toBeCloseTo(t.scale, 9)
    expect(t3.tx).toBeCloseTo(t.tx, 9)
    expect(t3.ty).toBeCloseTo(t.ty, 9)
  })
})

describe('viewport2dStore — computeZoomToFit', () => {
  const view = { width: 400, height: 300 }

  it('centres the extent in the viewport', () => {
    const extent: Extent2D = { minX: 0, minY: 0, maxX: 100, maxY: 100 }
    const t = computeZoomToFit(extent, view, 0)
    const centre = worldToScreen(t, 50, 50)
    expect(centre.x).toBeCloseTo(200, 6)
    expect(centre.y).toBeCloseTo(150, 6)
  })

  it('with padding=0, scale fills the shorter viewport dimension exactly', () => {
    // 100×100 extent in a 400×300 view → fit limited by height.
    const extent: Extent2D = { minX: 0, minY: 0, maxX: 100, maxY: 100 }
    const t = computeZoomToFit(extent, view, 0)
    expect(t.scale).toBeCloseTo(3, 6) // 300 / 100
  })

  it('off-origin extents still centre correctly', () => {
    const extent: Extent2D = { minX: 1000, minY: -500, maxX: 1200, maxY: -300 }
    const t = computeZoomToFit(extent, view, 0)
    const centre = worldToScreen(t, 1100, -400)
    expect(centre.x).toBeCloseTo(200, 6)
    expect(centre.y).toBeCloseTo(150, 6)
  })

  it('respects the padding fraction', () => {
    const extent: Extent2D = { minX: 0, minY: 0, maxX: 100, maxY: 100 }
    const t0 = computeZoomToFit(extent, view, 0)
    const tPad = computeZoomToFit(extent, view, 0.1)
    expect(tPad.scale).toBeLessThan(t0.scale)
    expect(tPad.scale).toBeCloseTo(t0.scale * 0.9, 6)
  })

  it('handles a degenerate (zero-area) extent without dividing by zero', () => {
    const extent: Extent2D = { minX: 5, minY: 5, maxX: 5, maxY: 5 }
    const t = computeZoomToFit(extent, view, 0)
    expect(Number.isFinite(t.scale)).toBe(true)
    expect(Number.isFinite(t.tx)).toBe(true)
    expect(Number.isFinite(t.ty)).toBe(true)
  })
})

// ── Store actions ───────────────────────────────────────────────────────────

describe('viewport2dStore — initial state', () => {
  it('starts with identity transform', () => {
    expect(useViewport2DStore.getState().transform).toEqual(IDENTITY_TRANSFORM)
  })

  it('starts with null extent', () => {
    expect(useViewport2DStore.getState().extent).toBeNull()
  })

  it('starts with empty layerVisibility', () => {
    expect(useViewport2DStore.getState().layerVisibility).toEqual({})
  })

  it('layers default to visible when not in the map', () => {
    expect(useViewport2DStore.getState().isLayerVisible('artwork')).toBe(true)
  })
})

describe('viewport2dStore — setExtent / setTransform', () => {
  it('setExtent stores the bounds', () => {
    const e: Extent2D = { minX: -1, minY: -2, maxX: 3, maxY: 4 }
    useViewport2DStore.getState().setExtent(e)
    expect(useViewport2DStore.getState().extent).toEqual(e)
  })

  it('setTransform replaces the current transform', () => {
    const t: Transform2D = { tx: 9, ty: 8, scale: 7 }
    useViewport2DStore.getState().setTransform(t)
    expect(useViewport2DStore.getState().transform).toEqual(t)
  })
})

describe('viewport2dStore — layer visibility', () => {
  it('setLayerVisible(false) hides a layer', () => {
    useViewport2DStore.getState().setLayerVisible('artwork', false)
    expect(useViewport2DStore.getState().isLayerVisible('artwork')).toBe(false)
  })

  it('setLayerVisible(true) shows a previously hidden layer', () => {
    useViewport2DStore.getState().setLayerVisible('artwork', false)
    useViewport2DStore.getState().setLayerVisible('artwork', true)
    expect(useViewport2DStore.getState().isLayerVisible('artwork')).toBe(true)
  })

  it('per-layer visibility is independent', () => {
    useViewport2DStore.getState().setLayerVisible('artwork', false)
    useViewport2DStore.getState().setLayerVisible('toolpath', true)
    const s = useViewport2DStore.getState()
    expect(s.isLayerVisible('artwork')).toBe(false)
    expect(s.isLayerVisible('toolpath')).toBe(true)
    expect(s.isLayerVisible('rapid')).toBe(true) // default
  })
})

describe('viewport2dStore — pan / zoomAt / zoomToFit actions', () => {
  it('pan shifts the transform translation by the screen delta', () => {
    useViewport2DStore.getState().pan(10, -5)
    expect(useViewport2DStore.getState().transform).toEqual({ tx: 10, ty: -5, scale: 1 })
  })

  it('zoomAt keeps the world point under the pivot fixed', () => {
    const sx = 100
    const sy = 80
    const worldBefore = screenToWorld(useViewport2DStore.getState().transform, sx, sy)
    useViewport2DStore.getState().zoomAt(2, sx, sy)
    const worldAfter = screenToWorld(useViewport2DStore.getState().transform, sx, sy)
    expect(worldAfter.x).toBeCloseTo(worldBefore.x, 9)
    expect(worldAfter.y).toBeCloseTo(worldBefore.y, 9)
    expect(useViewport2DStore.getState().transform.scale).toBeCloseTo(2, 9)
  })

  it('zoomToFit centres the extent in the viewport', () => {
    const extent: Extent2D = { minX: 0, minY: 0, maxX: 100, maxY: 100 }
    useViewport2DStore.getState().zoomToFit(extent, { width: 400, height: 300 }, 0)
    const t = useViewport2DStore.getState().transform
    const centre = worldToScreen(t, 50, 50)
    expect(centre.x).toBeCloseTo(200, 6)
    expect(centre.y).toBeCloseTo(150, 6)
  })

  it('reset returns the store to initial state', () => {
    useViewport2DStore.getState().pan(50, 50)
    useViewport2DStore.getState().setExtent({ minX: 0, minY: 0, maxX: 1, maxY: 1 })
    useViewport2DStore.getState().setLayerVisible('artwork', false)
    useViewport2DStore.getState().reset()
    const s = useViewport2DStore.getState()
    expect(s.transform).toEqual(IDENTITY_TRANSFORM)
    expect(s.extent).toBeNull()
    expect(s.layerVisibility).toEqual({})
  })
})
