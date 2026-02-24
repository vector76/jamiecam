/**
 * Three.js scene infrastructure for the JamieCam 3-D viewport.
 *
 * SceneManager owns the renderer, cameras, orbit controls, grid, and
 * three-point lighting.  Viewport.tsx mounts the canvas and holds a
 * SceneManager instance for the lifetime of the component.
 *
 * Coordinate convention: Z-up right-handed (matches CNC machines).
 * Both cameras have `up` set to (0, 0, 1); OrbitControls is configured
 * accordingly so pan and orbit behave correctly in Z-up space.
 */

import * as THREE from 'three'
import { OrbitControls } from 'three/addons/controls/OrbitControls.js'

export class SceneManager {
  /** The Three.js scene.  Viewport.tsx adds/removes model meshes here. */
  readonly scene: THREE.Scene

  private renderer: THREE.WebGLRenderer
  private perspectiveCamera: THREE.PerspectiveCamera
  private orthographicCamera: THREE.OrthographicCamera
  private controls: OrbitControls
  private frameId: number | null = null
  private resizeObserver: ResizeObserver

  constructor(canvas: HTMLCanvasElement, container: HTMLElement) {
    this.scene = new THREE.Scene()

    // ── Renderer ──────────────────────────────────────────────────────────
    this.renderer = new THREE.WebGLRenderer({ antialias: true, canvas })
    this.renderer.setPixelRatio(window.devicePixelRatio)
    this.renderer.setSize(container.clientWidth, container.clientHeight)

    // ── Cameras (both Z-up) ───────────────────────────────────────────────
    const aspect = container.clientWidth / Math.max(container.clientHeight, 1)

    this.perspectiveCamera = new THREE.PerspectiveCamera(45, aspect, 0.1, 10000)
    this.perspectiveCamera.position.set(0, -500, 300)
    this.perspectiveCamera.up.set(0, 0, 1)

    // Orthographic frustum sized to match perspective at 500mm distance.
    const frustumHalf = 250
    this.orthographicCamera = new THREE.OrthographicCamera(
      -frustumHalf * aspect,
      frustumHalf * aspect,
      frustumHalf,
      -frustumHalf,
      0.1,
      10000,
    )
    this.orthographicCamera.position.set(0, -500, 300)
    this.orthographicCamera.up.set(0, 0, 1)

    // ── Orbit controls — Z-up (non-default, must be set explicitly) ───────
    this.controls = new OrbitControls(this.perspectiveCamera, this.renderer.domElement)
    this.controls.enableDamping = true
    this.controls.enablePan = true
    this.controls.screenSpacePanning = false // keeps pan on XY plane
    this.controls.target.set(0, 0, 0)

    // ── Grid — XY plane at Z = 0 ──────────────────────────────────────────
    // Three.js GridHelper lies on the XZ plane by default; rotate 90° around
    // X so it lies on XY instead (Z-up convention).
    const grid = new THREE.GridHelper(1000, 100)
    grid.rotation.x = Math.PI / 2
    this.scene.add(grid)

    // ── Three-point lighting (intensities from docs/viewport-design.md) ───
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.4)
    this.scene.add(ambientLight)

    const keyLight = new THREE.DirectionalLight(0xffffff, 0.8)
    keyLight.position.set(1, -1, 2) // upper-right
    this.scene.add(keyLight)

    const rimLight = new THREE.DirectionalLight(0xffffff, 0.3)
    rimLight.position.set(-1, 1, -1) // lower-left
    this.scene.add(rimLight)

    // ── Resize observer ───────────────────────────────────────────────────
    this.resizeObserver = new ResizeObserver(() => this._onResize(container))
    this.resizeObserver.observe(container)

    // ── Animation loop ────────────────────────────────────────────────────
    this._animate()
  }

  // ── Private helpers ──────────────────────────────────────────────────────

  private _onResize(container: HTMLElement): void {
    const w = container.clientWidth
    const h = Math.max(container.clientHeight, 1)
    const aspect = w / h

    this.perspectiveCamera.aspect = aspect
    this.perspectiveCamera.updateProjectionMatrix()

    const frustumHalf = 250
    this.orthographicCamera.left = -frustumHalf * aspect
    this.orthographicCamera.right = frustumHalf * aspect
    this.orthographicCamera.updateProjectionMatrix()

    this.renderer.setSize(w, h)
  }

  private _animate(): void {
    this.frameId = requestAnimationFrame(() => this._animate())
    this.controls.update()
    this.renderer.render(this.scene, this.perspectiveCamera)
  }

  // ── Public API ───────────────────────────────────────────────────────────

  /**
   * Position the perspective camera so that `boundingSphere` fills the
   * viewport with a comfortable margin.  Updates the orbit target to the
   * sphere centre.
   */
  frameModel(boundingSphere: THREE.Sphere): void {
    const { center, radius } = boundingSphere
    const fovRad = this.perspectiveCamera.fov * (Math.PI / 180)
    // Distance at which the sphere exactly fits the vertical FOV, plus 50% margin.
    const distance = (radius / Math.tan(fovRad / 2)) * 1.5
    // Approach from a diagonal direction: slightly behind (-Y) and above (+Z).
    const dir = new THREE.Vector3(0, -1, 0.7).normalize()
    this.perspectiveCamera.position.copy(center).addScaledVector(dir, distance)
    this.controls.target.copy(center)
    this.controls.update()
  }

  /**
   * Tear down the animation loop, resize observer, controls, and renderer.
   * Call this when the host component unmounts.
   */
  dispose(): void {
    if (this.frameId !== null) {
      cancelAnimationFrame(this.frameId)
      this.frameId = null
    }
    this.resizeObserver.disconnect()
    this.controls.dispose()
    this.renderer.dispose()
  }
}
