/**
 * Tests for SceneManager (src/viewport/scene.ts).
 *
 * WebGLRenderer and OrbitControls are mocked so the suite runs in jsdom
 * without a real WebGL context.  All other Three.js classes (Scene, Camera,
 * lights, GridHelper, Sphere, …) use their real implementations.
 */

import * as THREE from 'three'
import * as TWEEN from '@tweenjs/tween.js'
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
    object: unknown = null
    enableDamping = false
    enablePan = false
    screenSpacePanning = true
    target = { x: 0, y: 0, z: 0, set: vi.fn(), copy: vi.fn() }
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

// ── Projection toggle ─────────────────────────────────────────────────────────

describe('SceneManager — projection toggle', () => {
  let mgr: SceneManager

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
  })

  afterEach(() => mgr.dispose())

  it('starts in perspective mode', () => {
    expect(mgr.getProjectionMode()).toBe('perspective')
  })

  it('toggles to orthographic on first call', () => {
    mgr.toggleProjection()
    expect(mgr.getProjectionMode()).toBe('orthographic')
  })

  it('toggles back to perspective on second call', () => {
    mgr.toggleProjection()
    mgr.toggleProjection()
    expect(mgr.getProjectionMode()).toBe('perspective')
  })

  it('camera getter returns PerspectiveCamera in perspective mode', () => {
    expect(mgr.camera).toBeInstanceOf(THREE.PerspectiveCamera)
  })

  it('camera getter returns OrthographicCamera in orthographic mode', () => {
    mgr.toggleProjection()
    expect(mgr.camera).toBeInstanceOf(THREE.OrthographicCamera)
  })

  it('orthographic camera gets position from perspective camera on toggle', () => {
    const pCam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    pCam.position.set(10, 20, 30)
    mgr.toggleProjection()
    const oCam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    expect(oCam.position.x).toBeCloseTo(10)
    expect(oCam.position.y).toBeCloseTo(20)
    expect(oCam.position.z).toBeCloseTo(30)
  })

  it('orthographic camera gets up from perspective camera on toggle', () => {
    const pCam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    pCam.up.set(0, 1, 0)
    mgr.toggleProjection()
    const oCam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    expect(oCam.up.x).toBeCloseTo(0)
    expect(oCam.up.y).toBeCloseTo(1)
    expect(oCam.up.z).toBeCloseTo(0)
  })

  it('orthographic frustum half-height matches perspective field of view at current distance', () => {
    const pCam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    const ctl = priv<{ target: THREE.Vector3 }>(mgr, 'controls')
    // controls.target is a plain object stub, treat as origin
    const distance = pCam.position.length()
    const expectedHalfH = Math.tan((pCam.fov / 2) * (Math.PI / 180)) * distance
    mgr.toggleProjection()
    const oCam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    expect(oCam.top).toBeCloseTo(expectedHalfH)
    expect(oCam.bottom).toBeCloseTo(-expectedHalfH)
    void ctl
  })

  it('sets controls.object to the orthographic camera after toggling to ortho', () => {
    const oCam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    const ctl = priv<{ object: unknown }>(mgr, 'controls')
    mgr.toggleProjection()
    expect(ctl.object).toBe(oCam)
  })

  it('sets controls.object to the perspective camera after toggling back', () => {
    const pCam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    const ctl = priv<{ object: unknown }>(mgr, 'controls')
    mgr.toggleProjection()
    mgr.toggleProjection()
    expect(ctl.object).toBe(pCam)
  })

  it('calls controls.update() on toggle', () => {
    const ctl = priv<{ update: ReturnType<typeof vi.fn> }>(mgr, 'controls')
    const callsBefore = ctl.update.mock.calls.length
    mgr.toggleProjection()
    expect(ctl.update.mock.calls.length).toBeGreaterThan(callsBefore)
  })

  it('renderer.render is called with perspective camera in perspective mode', () => {
    const pCam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
    const rdr = priv<{ render: ReturnType<typeof vi.fn> }>(mgr, 'renderer')
    rdr.render.mockClear()
    // Drive a single animation frame manually
    const group = priv<TWEEN.Group>(mgr, '_tweenGroup')
    group.update(performance.now())
    priv<{ update: ReturnType<typeof vi.fn> }>(mgr, 'controls').update()
    mgr['renderer'].render(mgr.scene, mgr.camera)
    const lastCall = rdr.render.mock.calls.at(-1)!
    expect(lastCall[1]).toBe(pCam)
  })

  it('renderer.render is called with orthographic camera in orthographic mode', () => {
    mgr.toggleProjection()
    const oCam = priv<THREE.OrthographicCamera>(mgr, 'orthographicCamera')
    const rdr = priv<{ render: ReturnType<typeof vi.fn> }>(mgr, 'renderer')
    rdr.render.mockClear()
    mgr['renderer'].render(mgr.scene, mgr.camera)
    const lastCall = rdr.render.mock.calls.at(-1)!
    expect(lastCall[1]).toBe(oCam)
  })
})

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

// ── setDisplayMode ────────────────────────────────────────────────────────────

describe('SceneManager — setDisplayMode', () => {
  let mgr: SceneManager
  let material: THREE.MeshStandardMaterial

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
    material = new THREE.MeshStandardMaterial()
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), material)
    mgr.setModelMesh(mesh)
  })

  afterEach(() => mgr.dispose())

  it('shaded: resets wireframe and transparent after switching from other modes', () => {
    mgr.setDisplayMode('wireframe')
    mgr.setDisplayMode('shaded')
    expect(material.wireframe).toBe(false)
    mgr.setDisplayMode('transparent')
    mgr.setDisplayMode('shaded')
    expect(material.transparent).toBe(false)
  })

  it('shaded: edge overlay is hidden after switching from shaded-edges', () => {
    mgr.setDisplayMode('shaded-edges')
    mgr.setDisplayMode('shaded')
    // shaded-edges created the overlay; shaded must hide it (not destroy it).
    const overlay = (mgr as any)._edgeOverlay as THREE.LineSegments
    expect(overlay).not.toBeNull()
    expect(overlay.visible).toBe(false)
  })

  it('wireframe: wireframe is true', () => {
    mgr.setDisplayMode('wireframe')
    expect(material.wireframe).toBe(true)
  })

  it('transparent: transparent is true, opacity ≈ 0.3', () => {
    mgr.setDisplayMode('transparent')
    expect(material.transparent).toBe(true)
    expect(material.opacity).toBeCloseTo(0.3)
  })

  it('shaded-edges: wireframe is false, edge overlay is not null and visible', () => {
    mgr.setDisplayMode('shaded-edges')
    expect(material.wireframe).toBe(false)
    const overlay = (mgr as any)._edgeOverlay as THREE.LineSegments | null
    expect(overlay).not.toBeNull()
    expect(overlay!.visible).toBe(true)
  })

  it('shaded-edges: overlay is reused on second call', () => {
    mgr.setDisplayMode('shaded-edges')
    const first = (mgr as any)._edgeOverlay
    mgr.setDisplayMode('shaded-edges')
    const second = (mgr as any)._edgeOverlay
    expect(second).toBe(first)
  })

  it('does not throw when no mesh is loaded', () => {
    mgr.setModelMesh(null)
    expect(() => mgr.setDisplayMode('shaded')).not.toThrow()
  })
})

// ── snapToView / snap* ────────────────────────────────────────────────────────

describe('SceneManager — snapToView', () => {
  let mgr: SceneManager
  let cam: THREE.PerspectiveCamera

  beforeEach(() => {
    const { canvas, container } = makeElements()
    mgr = new SceneManager(canvas, container)
    cam = priv<THREE.PerspectiveCamera>(mgr, 'perspectiveCamera')
  })

  afterEach(() => mgr.dispose())

  /** Drive the tween group 400 ms into the future to complete any active tween. */
  function driveToCompletion() {
    const group = priv<TWEEN.Group>(mgr, '_tweenGroup')
    group.update(performance.now() + 400)
  }

  it('snapFront sets _activeTween while animating', () => {
    mgr.snapFront()
    expect(priv<TWEEN.Tween<any> | null>(mgr, '_activeTween')).not.toBeNull()
  })

  it('snapFront positions camera on the -Y axis after completion', () => {
    mgr.snapFront()
    driveToCompletion()
    expect(cam.position.x).toBeCloseTo(0, 3)
    expect(cam.position.z).toBeCloseTo(0, 3)
    expect(cam.position.y).toBeLessThan(0)
  })

  it('snapTop positions camera on the +Z axis after completion', () => {
    mgr.snapTop()
    driveToCompletion()
    expect(cam.position.x).toBeCloseTo(0, 3)
    expect(cam.position.y).toBeCloseTo(0, 3)
    expect(cam.position.z).toBeGreaterThan(0)
  })

  it('snapRight positions camera on the +X axis after completion', () => {
    mgr.snapRight()
    driveToCompletion()
    expect(cam.position.y).toBeCloseTo(0, 3)
    expect(cam.position.z).toBeCloseTo(0, 3)
    expect(cam.position.x).toBeGreaterThan(0)
  })

  it('snapIsometric positions camera at equal |X|, |Y|, |Z| components after completion', () => {
    mgr.snapIsometric()
    driveToCompletion()
    expect(cam.position.x).toBeCloseTo(Math.abs(cam.position.y), 3)
    expect(cam.position.x).toBeCloseTo(cam.position.z, 3)
    expect(cam.position.y).toBeLessThan(0)
  })

  it('preserves the orbit distance', () => {
    // Initial camera at (0,-500,300); mock controls.target at origin.
    const expectedDist = new THREE.Vector3(0, -500, 300).length()
    mgr.snapFront()
    driveToCompletion()
    expect(cam.position.length()).toBeCloseTo(expectedDist, 1)
  })

  it('clears _activeTween after completion', () => {
    mgr.snapFront()
    driveToCompletion()
    expect(priv<TWEEN.Tween<any> | null>(mgr, '_activeTween')).toBeNull()
  })

  it('is a no-op if camera is already at the target view', () => {
    mgr.snapFront()
    driveToCompletion()
    // A second snapFront should hit the early-return path.
    mgr.snapFront()
    expect(priv<TWEEN.Tween<any> | null>(mgr, '_activeTween')).toBeNull()
  })

  it('cancels an in-flight tween when a new snap is requested', () => {
    mgr.snapFront()
    const firstTween = priv<TWEEN.Tween<any>>(mgr, '_activeTween')!
    const stopSpy = vi.spyOn(firstTween, 'stop')
    mgr.snapRight()
    expect(stopSpy).toHaveBeenCalled()
    expect(priv<TWEEN.Tween<any> | null>(mgr, '_activeTween')).not.toBe(firstTween)
  })

  it('dispose() stops and clears an in-flight tween', () => {
    mgr.snapFront()
    const activeTween = priv<TWEEN.Tween<any>>(mgr, '_activeTween')!
    const stopSpy = vi.spyOn(activeTween, 'stop')
    mgr.dispose()
    expect(stopSpy).toHaveBeenCalled()
    expect(priv<TWEEN.Tween<any> | null>(mgr, '_activeTween')).toBeNull()
  })

  it('snapFront sets camera up to (0,0,1) after completion', () => {
    mgr.snapFront()
    driveToCompletion()
    expect(cam.up.x).toBeCloseTo(0, 5)
    expect(cam.up.y).toBeCloseTo(0, 5)
    expect(cam.up.z).toBeCloseTo(1, 5)
  })

  it('snapTop sets camera up to (0,1,0) after completion', () => {
    mgr.snapTop()
    driveToCompletion()
    expect(cam.up.x).toBeCloseTo(0, 5)
    expect(cam.up.y).toBeCloseTo(1, 5)
    expect(cam.up.z).toBeCloseTo(0, 5)
  })
})
