import { worldToScreen, screenToWorld, autoFitTransform } from './coordTransform'
import type { CurveSummary } from '../../../api/twodMode'

// ── helpers ───────────────────────────────────────────────────────────────────

function makeCurve(minX: number, minY: number, maxX: number, maxY: number): CurveSummary {
  return { id: 'test', isClosed: true, bbox: { minX, minY, maxX, maxY } }
}

// ── worldToScreen / screenToWorld round-trip ──────────────────────────────────

describe('coordTransform — round-trip', () => {
  const PAN = { x: 50, y: 100 }
  const ZOOM = 2.0
  const H = 600

  it('screenToWorld(worldToScreen(wx, wy)) returns original point', () => {
    const wx = 10
    const wy = 20
    const screen = worldToScreen(wx, wy, PAN, ZOOM, H)
    const world = screenToWorld(screen.x, screen.y, PAN, ZOOM, H)
    expect(world.x).toBeCloseTo(wx, 10)
    expect(world.y).toBeCloseTo(wy, 10)
  })

  it('round-trip works with negative world coordinates', () => {
    const wx = -5
    const wy = -15
    const screen = worldToScreen(wx, wy, PAN, ZOOM, H)
    const world = screenToWorld(screen.x, screen.y, PAN, ZOOM, H)
    expect(world.x).toBeCloseTo(wx, 10)
    expect(world.y).toBeCloseTo(wy, 10)
  })

  it('round-trip works with zero pan offset and zoom=1', () => {
    const pan = { x: 0, y: 0 }
    const zoom = 1.0
    const wx = 7
    const wy = 3
    const screen = worldToScreen(wx, wy, pan, zoom, H)
    const world = screenToWorld(screen.x, screen.y, pan, zoom, H)
    expect(world.x).toBeCloseTo(wx, 10)
    expect(world.y).toBeCloseTo(wy, 10)
  })

  it('round-trip works with sub-unity zoom', () => {
    const zoom = 0.1
    const wx = 100
    const wy = 200
    const screen = worldToScreen(wx, wy, PAN, zoom, H)
    const world = screenToWorld(screen.x, screen.y, PAN, zoom, H)
    expect(world.x).toBeCloseTo(wx, 8)
    expect(world.y).toBeCloseTo(wy, 8)
  })
})

describe('coordTransform — worldToScreen Y inversion', () => {
  it('world Y=0 maps to screenY = canvasHeight - panOffset.y', () => {
    const pan = { x: 0, y: 0 }
    const { y } = worldToScreen(0, 0, pan, 1.0, 500)
    expect(y).toBe(500)
  })

  it('larger world Y produces smaller screenY (Y-axis is inverted)', () => {
    const pan = { x: 0, y: 0 }
    const y1 = worldToScreen(0, 10, pan, 1.0, 500).y
    const y2 = worldToScreen(0, 20, pan, 1.0, 500).y
    expect(y2).toBeLessThan(y1)
  })
})

// ── autoFitTransform ──────────────────────────────────────────────────────────

describe('coordTransform — autoFitTransform', () => {
  it('all bbox corners are within canvas bounds with default padding', () => {
    const curves: CurveSummary[] = [
      makeCurve(0, 0, 100, 50),
      makeCurve(20, 10, 80, 40),
    ]
    const W = 800
    const H = 600
    const { panOffset, zoom } = autoFitTransform(curves, null, W, H)

    // Check all four corners of the combined bbox fit within [0, W] × [0, H].
    const corners: [number, number][] = [
      [0, 0],
      [100, 0],
      [0, 50],
      [100, 50],
    ]
    for (const [wx, wy] of corners) {
      const { x, y } = worldToScreen(wx, wy, panOffset, zoom, H)
      expect(x).toBeGreaterThanOrEqual(0)
      expect(x).toBeLessThanOrEqual(W)
      expect(y).toBeGreaterThanOrEqual(0)
      expect(y).toBeLessThanOrEqual(H)
    }
  })

  it('zoom is positive', () => {
    const curves = [makeCurve(0, 0, 50, 50)]
    const { zoom } = autoFitTransform(curves, null, 400, 400)
    expect(zoom).toBeGreaterThan(0)
  })

  it('content is approximately centred in the canvas', () => {
    // Single square bbox from (0,0) to (100,100) — centre should map near canvas centre.
    const curves = [makeCurve(0, 0, 100, 100)]
    const W = 800
    const H = 600
    const { panOffset, zoom } = autoFitTransform(curves, null, W, H)

    const centre = worldToScreen(50, 50, panOffset, zoom, H)
    expect(centre.x).toBeCloseTo(W / 2, 5)
    expect(centre.y).toBeCloseTo(H / 2, 5)
  })

  it('includes stock dimensions in the fit when provided', () => {
    // Curves entirely inside stock; stock should be fully visible too.
    const curves = [makeCurve(10, 10, 90, 90)]
    const stock = { width: 200, depth: 150 }
    const W = 800
    const H = 600
    const { panOffset, zoom } = autoFitTransform(curves, stock, W, H)

    // Stock corners (0,0)→(200,150) must be within canvas.
    const stockCorners: [number, number][] = [
      [0, 0],
      [200, 0],
      [0, 150],
      [200, 150],
    ]
    for (const [wx, wy] of stockCorners) {
      const { x, y } = worldToScreen(wx, wy, panOffset, zoom, H)
      expect(x).toBeGreaterThanOrEqual(0)
      expect(x).toBeLessThanOrEqual(W)
      expect(y).toBeGreaterThanOrEqual(0)
      expect(y).toBeLessThanOrEqual(H)
    }
  })

  it('returns default transform when curves array is empty and no stock', () => {
    const { panOffset, zoom } = autoFitTransform([], null, 800, 600)
    expect(panOffset).toEqual({ x: 0, y: 0 })
    expect(zoom).toBe(1.0)
  })
})
