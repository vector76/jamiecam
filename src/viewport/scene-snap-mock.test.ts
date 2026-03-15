/**
 * Mock-based tests for SceneManager snap methods.
 *
 * These tests use a mocked @tweenjs/tween.js to assert directly that:
 *  - TWEEN.Tween constructor is called (a tween is initiated)
 *  - The .to() argument passes the camera to the correct direction
 *  - snapToView is a no-op when the camera is already at the target
 *
 * The parallel real-TWEEN test suite (scene.test.ts) drives animation to
 * completion and verifies final camera positions.  This file verifies the
 * tween setup arguments.
 */

import * as THREE from 'three'
import * as TWEEN from '@tweenjs/tween.js'
import { SceneManager } from './scene'

// ── Global stubs ─────────────────────────────────────────────────────────────

vi.stubGlobal('requestAnimationFrame', vi.fn(() => 1))
vi.stubGlobal('cancelAnimationFrame', vi.fn())

class MockResizeObserver {
  observe = vi.fn()
  disconnect = vi.fn()
  unobserve = vi.fn()
}
vi.stubGlobal('ResizeObserver', MockResizeObserver)

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('three', async (importOriginal) => {
  const actual = await importOriginal<typeof import('three')>()

  class MockWebGLRenderer {
    domElement = document.createElement('canvas')
    setPixelRatio = vi.fn()
    setSize = vi.fn()
    render = vi.fn()
    dispose = vi.fn()
  }

  return { ...actual, WebGLRenderer: vi.fn(() => new MockWebGLRenderer()) }
})

vi.mock('three/addons/controls/OrbitControls.js', () => ({
  OrbitControls: class {
    object: unknown = null
    enableDamping = false
    enablePan = false
    screenSpacePanning = true
    target = { x: 0, y: 0, z: 0, set: vi.fn(), copy: vi.fn(), distanceTo: vi.fn(() => 500) }
    update = vi.fn()
    dispose = vi.fn()
    addEventListener = vi.fn()
  },
}))

vi.mock('@tweenjs/tween.js', () => ({
  Group: vi.fn(() => ({ update: vi.fn(), add: vi.fn() })),
  Tween: vi.fn(() => ({
    to: vi.fn().mockReturnThis(),
    duration: vi.fn().mockReturnThis(),
    easing: vi.fn().mockReturnThis(),
    onUpdate: vi.fn().mockReturnThis(),
    onComplete: vi.fn().mockReturnThis(),
    start: vi.fn().mockReturnThis(),
    stop: vi.fn().mockReturnThis(),
  })),
  Easing: { Quadratic: { InOut: vi.fn() } },
}))

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeElements() {
  const container = document.createElement('div')
  Object.defineProperty(container, 'clientWidth', { get: () => 800, configurable: true })
  Object.defineProperty(container, 'clientHeight', { get: () => 600, configurable: true })
  const canvas = document.createElement('canvas')
  return { canvas, container }
}

/** Return the most recently created mock Tween instance. */
function latestTweenInstance() {
  const TweenMock = TWEEN.Tween as ReturnType<typeof vi.fn>
  const last = TweenMock.mock.results.at(-1)
  if (!last || last.type !== 'return') throw new Error('Tween not yet constructed')
  return last.value as {
    to: ReturnType<typeof vi.fn>
    stop: ReturnType<typeof vi.fn>
    start: ReturnType<typeof vi.fn>
  }
}

// ── Snap direction argument tests ─────────────────────────────────────────────

describe('SceneManager — snap methods (mock Tween)', () => {
  let mgr: SceneManager

  beforeEach(() => {
    vi.mocked(TWEEN.Tween).mockClear()
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  it('snapTop: initiates a tween toward +Z', () => {
    mgr.snapTop()
    expect(TWEEN.Tween).toHaveBeenCalled()
    const { to } = latestTweenInstance()
    expect(to).toHaveBeenCalledWith(
      expect.objectContaining({
        z: expect.any(Number),
      }),
      300,
    )
    const [[args]] = to.mock.calls as [[{ x: number; y: number; z: number }]]
    expect(args.z).toBeGreaterThan(0)
    expect(Math.abs(args.x)).toBeCloseTo(0, 3)
    expect(Math.abs(args.y)).toBeCloseTo(0, 3)
  })

  it('snapFront: initiates a tween toward -Y', () => {
    mgr.snapFront()
    expect(TWEEN.Tween).toHaveBeenCalled()
    const { to } = latestTweenInstance()
    expect(to).toHaveBeenCalledWith(expect.objectContaining({ y: expect.any(Number) }), 300)
    const [[args]] = to.mock.calls as [[{ x: number; y: number; z: number }]]
    expect(args.y).toBeLessThan(0)
    expect(Math.abs(args.x)).toBeCloseTo(0, 3)
    expect(Math.abs(args.z)).toBeCloseTo(0, 3)
  })

  it('snapRight: initiates a tween toward +X', () => {
    mgr.snapRight()
    expect(TWEEN.Tween).toHaveBeenCalled()
    const { to } = latestTweenInstance()
    expect(to).toHaveBeenCalledWith(expect.objectContaining({ x: expect.any(Number) }), 300)
    const [[args]] = to.mock.calls as [[{ x: number; y: number; z: number }]]
    expect(args.x).toBeGreaterThan(0)
    expect(Math.abs(args.y)).toBeCloseTo(0, 3)
    expect(Math.abs(args.z)).toBeCloseTo(0, 3)
  })

  it('snapIsometric: initiates a tween with x > 0, y < 0, z > 0', () => {
    mgr.snapIsometric()
    expect(TWEEN.Tween).toHaveBeenCalled()
    const { to } = latestTweenInstance()
    expect(to).toHaveBeenCalledWith(
      expect.objectContaining({ x: expect.any(Number), y: expect.any(Number), z: expect.any(Number) }),
      300,
    )
    const [[args]] = to.mock.calls as [[{ x: number; y: number; z: number }]]
    expect(args.x).toBeGreaterThan(0)
    expect(args.y).toBeLessThan(0)
    expect(args.z).toBeGreaterThan(0)
  })

  it('is a no-op (no Tween created) when camera is already at the target', () => {
    // snapTop targets (0, 0, 1) * orbitDistance, up (0, 1, 0).
    // Position the perspective camera there manually so the early-return fires.
    const perspectiveCamera = (mgr as any).perspectiveCamera as THREE.PerspectiveCamera
    const orbitDistance = perspectiveCamera.position.length()
    perspectiveCamera.position.set(0, 0, orbitDistance)
    perspectiveCamera.up.set(0, 1, 0)

    vi.mocked(TWEEN.Tween).mockClear()
    mgr.snapTop()
    expect(TWEEN.Tween).not.toHaveBeenCalled()
  })
})
