/**
 * Tests for SceneManager (src/viewport/scene.ts).
 *
 * WebGLRenderer and OrbitControls are mocked so the suite runs in jsdom
 * without a real WebGL context.  All other Three.js classes (Scene, Camera,
 * lights, GridHelper, Sphere, …) use their real implementations.
 */

import * as THREE from 'three'
import { SceneManager } from './scene'

// ── Global stubs ─────────────────────────────────────────────────────────────

// requestAnimationFrame / cancelAnimationFrame are stubbed so the animation
// loop never actually fires during tests.
vi.stubGlobal('requestAnimationFrame', vi.fn(() => 1))
vi.stubGlobal('cancelAnimationFrame', vi.fn())

// ResizeObserver stub — jsdom does not implement it.
class MockResizeObserver {
  observe = vi.fn()
  disconnect = vi.fn()
  unobserve = vi.fn()
}
vi.stubGlobal('ResizeObserver', MockResizeObserver)

// ── Module mocks ──────────────────────────────────────────────────────────────

// Partial mock of 'three': replace only WebGLRenderer to avoid the WebGL
// context requirement; every other export remains real.
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

// Mock OrbitControls — the real class requires a DOM event target and a live
// camera object wired to a renderer; a lightweight stub is sufficient here.
vi.mock('three/addons/controls/OrbitControls.js', () => ({
  OrbitControls: class {
    enableDamping = false
    enablePan = false
    screenSpacePanning = true
    target = { set: vi.fn(), copy: vi.fn() }
    update = vi.fn()
    dispose = vi.fn()
  },
}))

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Create a canvas and a container whose client dimensions are 800 × 600. */
function makeElements() {
  const container = document.createElement('div')
  Object.defineProperty(container, 'clientWidth', { get: () => 800, configurable: true })
  Object.defineProperty(container, 'clientHeight', { get: () => 600, configurable: true })
  const canvas = document.createElement('canvas')
  return { canvas, container }
}

/** Access a private field of `obj` by name. */
function priv<T>(obj: SceneManager, key: string): T {
  return (obj as unknown as Record<string, T>)[key]
}

// ── Scene graph ───────────────────────────────────────────────────────────────

describe('SceneManager — scene graph', () => {
  let mgr: SceneManager

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  it('exposes a THREE.Scene', () => {
    expect(mgr.scene).toBeInstanceOf(THREE.Scene)
  })

  it('adds an AmbientLight with intensity 0.4', () => {
    const light = mgr.scene.children.find(
      (c): c is THREE.AmbientLight => c instanceof THREE.AmbientLight,
    )
    expect(light).toBeDefined()
    expect(light!.intensity).toBe(0.4)
  })

  it('adds a key DirectionalLight with intensity 0.8', () => {
    const dirLights = mgr.scene.children.filter(
      (c): c is THREE.DirectionalLight => c instanceof THREE.DirectionalLight,
    )
    expect(dirLights.some((l) => l.intensity === 0.8)).toBe(true)
  })

  it('adds a rim DirectionalLight with intensity 0.3', () => {
    const dirLights = mgr.scene.children.filter(
      (c): c is THREE.DirectionalLight => c instanceof THREE.DirectionalLight,
    )
    expect(dirLights.some((l) => l.intensity === 0.3)).toBe(true)
  })

  it('adds exactly two DirectionalLights', () => {
    const count = mgr.scene.children.filter((c) => c instanceof THREE.DirectionalLight).length
    expect(count).toBe(2)
  })

  it('adds a GridHelper rotated 90° around X (XY plane)', () => {
    const grid = mgr.scene.children.find(
      (c): c is THREE.GridHelper => c instanceof THREE.GridHelper,
    )
    expect(grid).toBeDefined()
    expect(grid!.rotation.x).toBeCloseTo(Math.PI / 2)
  })
})

// ── Cameras ───────────────────────────────────────────────────────────────────

describe('SceneManager — cameras', () => {
  let mgr: SceneManager

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  it('perspective camera has Z as up vector', () => {
    const cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    expect(cam.up.x).toBe(0)
    expect(cam.up.y).toBe(0)
    expect(cam.up.z).toBe(1)
  })

  it('orthographic camera has Z as up vector', () => {
    const cam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    expect(cam.up.x).toBe(0)
    expect(cam.up.y).toBe(0)
    expect(cam.up.z).toBe(1)
  })

  it('perspective camera FOV is 45°', () => {
    const cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    expect(cam.fov).toBe(45)
  })

  it('perspective near/far planes span 0.1 – 10000', () => {
    const cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    expect(cam.near).toBe(0.1)
    expect(cam.far).toBe(10000)
  })
})

// ── OrbitControls ─────────────────────────────────────────────────────────────

describe('SceneManager — OrbitControls', () => {
  let mgr: SceneManager

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  function controls(m: SceneManager) {
    return priv<{
      enableDamping: boolean
      enablePan: boolean
      screenSpacePanning: boolean
    }>(m, 'controls')
  }

  it('enables damping', () => {
    expect(controls(mgr).enableDamping).toBe(true)
  })

  it('sets screenSpacePanning to false', () => {
    expect(controls(mgr).screenSpacePanning).toBe(false)
  })

  it('enables pan', () => {
    expect(controls(mgr).enablePan).toBe(true)
  })
})

// ── dispose ───────────────────────────────────────────────────────────────────

describe('SceneManager — dispose', () => {
  it('cancels the animation frame', () => {
    const { canvas, container } = makeElements()
    const mgr = new SceneManager(canvas, container)
    const spy = vi.mocked(cancelAnimationFrame)
    spy.mockClear()
    mgr.dispose()
    expect(spy).toHaveBeenCalled()
  })

  it('disconnects the ResizeObserver', () => {
    const { canvas, container } = makeElements()
    const mgr = new SceneManager(canvas, container)
    const ro = priv<{ disconnect: ReturnType<typeof vi.fn> }>(mgr, 'resizeObserver')
    mgr.dispose()
    expect(ro.disconnect).toHaveBeenCalled()
  })

  it('calls controls.dispose()', () => {
    const { canvas, container } = makeElements()
    const mgr = new SceneManager(canvas, container)
    const ctl = priv<{ dispose: ReturnType<typeof vi.fn> }>(mgr, 'controls')
    mgr.dispose()
    expect(ctl.dispose).toHaveBeenCalled()
  })

  it('calls renderer.dispose()', () => {
    const { canvas, container } = makeElements()
    const mgr = new SceneManager(canvas, container)
    const rdr = priv<{ dispose: ReturnType<typeof vi.fn> }>(mgr, 'renderer')
    mgr.dispose()
    expect(rdr.dispose).toHaveBeenCalled()
  })

  it('is safe to call twice (idempotent)', () => {
    const { canvas, container } = makeElements()
    const mgr = new SceneManager(canvas, container)
    expect(() => {
      mgr.dispose()
      mgr.dispose()
    }).not.toThrow()
  })
})

// ── frameModel ────────────────────────────────────────────────────────────────

describe('SceneManager — frameModel', () => {
  let mgr: SceneManager

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  it('positions the camera outside the bounding sphere', () => {
    const cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    const center = new THREE.Vector3(0, 0, 0)
    const radius = 100
    mgr.frameModel(new THREE.Sphere(center, radius))
    expect(cam.position.distanceTo(center)).toBeGreaterThan(radius)
  })

  it('correctly frames a non-origin bounding sphere', () => {
    const cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    const center = new THREE.Vector3(10, 20, 30)
    const radius = 50
    mgr.frameModel(new THREE.Sphere(center, radius))
    expect(cam.position.distanceTo(center)).toBeGreaterThan(radius)
  })

  it('calls controls.update after framing', () => {
    const ctl = priv<{ update: ReturnType<typeof vi.fn> }>(mgr, 'controls')
    const callsBefore = ctl.update.mock.calls.length
    mgr.frameModel(new THREE.Sphere(new THREE.Vector3(0, 0, 0), 100))
    expect(ctl.update.mock.calls.length).toBeGreaterThan(callsBefore)
  })

  it('copies the sphere center to the orbit target', () => {
    const ctl = priv<{ target: { copy: ReturnType<typeof vi.fn> } }>(mgr, 'controls')
    const center = new THREE.Vector3(5, 10, 15)
    mgr.frameModel(new THREE.Sphere(center, 30))
    expect(ctl.target.copy).toHaveBeenCalledWith(center)
  })
})
