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
import * as TWEEN from '@tweenjs/tween.js'
import type { DisplayMode } from '../store/viewportStore'

export class SceneManager {
  /** The Three.js scene.  Viewport.tsx adds/removes model meshes here. */
  readonly scene: THREE.Scene

  private renderer: THREE.WebGLRenderer
  private perspectiveCamera: THREE.PerspectiveCamera
  private orthographicCamera: THREE.OrthographicCamera
  private controls: OrbitControls
  private frameId: number | null = null
  private resizeObserver: ResizeObserver
  private toolpathGroup: THREE.Group
  private _tweenGroup: TWEEN.Group
  private _activeTween: TWEEN.Tween<any> | null = null
  private _projectionMode: 'perspective' | 'orthographic' = 'perspective'
  private _modelMesh: THREE.Mesh | null = null
  private _edgeOverlay: THREE.LineSegments | null = null

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

    this.toolpathGroup = new THREE.Group()
    this.toolpathGroup.name = 'ToolpathGroup'
    this.scene.add(this.toolpathGroup)

    this._tweenGroup = new TWEEN.Group()

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

    const halfH = this.orthographicCamera.top
    this.orthographicCamera.left = -halfH * aspect
    this.orthographicCamera.right = halfH * aspect
    this.orthographicCamera.updateProjectionMatrix()

    this.renderer.setSize(w, h)
  }

  private _activeCamera(): THREE.PerspectiveCamera | THREE.OrthographicCamera {
    return this._projectionMode === 'perspective'
      ? this.perspectiveCamera
      : this.orthographicCamera
  }

  private _animate(): void {
    this.frameId = requestAnimationFrame(() => this._animate())
    this._tweenGroup.update(performance.now())
    this.controls.update()
    this.renderer.render(this.scene, this._activeCamera())
  }

  // ── Public API ───────────────────────────────────────────────────────────

  /** Expose the active camera for external raycasting. */
  get camera(): THREE.PerspectiveCamera | THREE.OrthographicCamera {
    return this._activeCamera()
  }

  /** Return the current projection mode. */
  getProjectionMode(): 'perspective' | 'orthographic' {
    return this._projectionMode
  }

  /**
   * Toggle between perspective and orthographic projection.
   * Synchronises the incoming camera's position and up vector from the
   * outgoing camera, sizes the orthographic frustum to match the current
   * perspective view distance, then hands control over to the new camera.
   */
  toggleProjection(): void {
    const outgoing =
      this._projectionMode === 'perspective'
        ? this.perspectiveCamera
        : this.orthographicCamera
    const incoming =
      this._projectionMode === 'perspective'
        ? this.orthographicCamera
        : this.perspectiveCamera

    incoming.position.copy(outgoing.position)
    incoming.up.copy(outgoing.up)

    if (this._projectionMode === 'perspective') {
      // Switching perspective → orthographic: fit frustum to current view.
      const distance = this.perspectiveCamera.position.distanceTo(
        this.controls.target,
      )
      const halfH =
        Math.tan((this.perspectiveCamera.fov / 2) * (Math.PI / 180)) * distance
      const aspect =
        this.renderer.domElement.clientWidth /
        Math.max(this.renderer.domElement.clientHeight, 1)
      this.orthographicCamera.left = -halfH * aspect
      this.orthographicCamera.right = halfH * aspect
      this.orthographicCamera.top = halfH
      this.orthographicCamera.bottom = -halfH
      this.orthographicCamera.updateProjectionMatrix()
    }

    this._projectionMode =
      this._projectionMode === 'perspective' ? 'orthographic' : 'perspective'

    this.controls.object = this._activeCamera() as THREE.Camera
    this.controls.update()
  }

  /** Enable or disable orbit controls (e.g. during face-selection mode). */
  setOrbitEnabled(enabled: boolean): void {
    this.controls.enabled = enabled
  }

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
    this._activeTween?.stop()
    this._activeTween = null
    if (this._edgeOverlay !== null) {
      this._edgeOverlay.geometry.dispose()
      ;(this._edgeOverlay.material as THREE.Material).dispose()
      this._edgeOverlay = null
    }
    this.resizeObserver.disconnect()
    this.controls.dispose()
    this.renderer.dispose()
  }

  /**
   * Animate the perspective camera to a new view direction, preserving the
   * current orbit distance from the target so the model stays the same
   * apparent size.  Any in-flight animation is cancelled before starting.
   *
   * @param position  Unit-vector direction (will be normalised internally).
   * @param up        Desired camera up vector.
   */
  snapToView(position: THREE.Vector3, up: THREE.Vector3): void {
    const orbitDistance = this.perspectiveCamera.position.distanceTo(
      this.controls.target,
    )
    const targetPos = position.clone().normalize().multiplyScalar(orbitDistance)

    const ε = 0.001
    if (
      this.perspectiveCamera.position.distanceTo(targetPos) < ε &&
      this.perspectiveCamera.up.distanceTo(up) < ε
    ) {
      return
    }

    this._activeTween?.stop()
    this._activeTween = null

    const state = {
      x: this.perspectiveCamera.position.x,
      y: this.perspectiveCamera.position.y,
      z: this.perspectiveCamera.position.z,
      ux: this.perspectiveCamera.up.x,
      uy: this.perspectiveCamera.up.y,
      uz: this.perspectiveCamera.up.z,
    }
    this._activeTween = new TWEEN.Tween(state, this._tweenGroup)
      .to(
        {
          x: targetPos.x,
          y: targetPos.y,
          z: targetPos.z,
          ux: up.x,
          uy: up.y,
          uz: up.z,
        },
        300,
      )
      .easing(TWEEN.Easing.Quadratic.InOut)
      .onUpdate(() => {
        this.perspectiveCamera.position.set(state.x, state.y, state.z)
        this.perspectiveCamera.up.set(state.ux, state.uy, state.uz)
        this.controls.target.set(0, 0, 0)
        this.controls.update()
      })
      .onComplete(() => {
        this._activeTween = null
        this.controls.update()
      })
      .start()
  }

  /** Snap camera to top-down view (+Z up, looking down). */
  snapTop(): void {
    this.snapToView(new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, 1, 0))
  }

  /** Snap camera to front view (looking in from -Y). */
  snapFront(): void {
    this.snapToView(new THREE.Vector3(0, -1, 0), new THREE.Vector3(0, 0, 1))
  }

  /** Snap camera to right-side view (looking in from +X). */
  snapRight(): void {
    this.snapToView(new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 0, 1))
  }

  /** Snap camera to isometric view (equal parts X, -Y, Z). */
  snapIsometric(): void {
    this.snapToView(
      new THREE.Vector3(1, -1, 1).normalize(),
      new THREE.Vector3(0, 0, 1),
    )
  }

  /** Store a reference to the model mesh for display-mode changes. */
  setModelMesh(mesh: THREE.Mesh | null): void {
    // Dispose the edge overlay built from the previous mesh — it is now stale.
    if (this._edgeOverlay !== null) {
      this.scene.remove(this._edgeOverlay)
      this._edgeOverlay.geometry.dispose()
      ;(this._edgeOverlay.material as THREE.Material).dispose()
      this._edgeOverlay = null
    }
    this._modelMesh = mesh
  }

  /** Apply a display mode to the current model mesh. No-op if no mesh is loaded. */
  setDisplayMode(mode: DisplayMode): void {
    if (this._modelMesh === null) return
    const material = this._modelMesh.material as THREE.MeshStandardMaterial

    switch (mode) {
      case 'shaded':
        material.wireframe = false
        material.transparent = false
        material.opacity = 1
        material.needsUpdate = true
        if (this._edgeOverlay !== null) this._edgeOverlay.visible = false
        break

      case 'shaded-edges':
        material.wireframe = false
        material.transparent = false
        material.opacity = 1
        material.needsUpdate = true
        if (this._edgeOverlay === null) {
          this._edgeOverlay = new THREE.LineSegments(
            new THREE.EdgesGeometry(this._modelMesh.geometry),
            new THREE.LineBasicMaterial({ color: 0x000000 }),
          )
          this.scene.add(this._edgeOverlay)
        }
        this._edgeOverlay.visible = true
        break

      case 'wireframe':
        material.wireframe = true
        material.transparent = false
        material.opacity = 1
        material.needsUpdate = true
        if (this._edgeOverlay !== null) this._edgeOverlay.visible = false
        break

      case 'transparent':
        material.wireframe = false
        material.transparent = true
        material.opacity = 0.3
        material.needsUpdate = true
        if (this._edgeOverlay !== null) this._edgeOverlay.visible = false
        break
    }
  }

  /**
   * Replace the toolpath line segments displayed in the scene.
   * Disposes the previous geometry before replacing.
   */
  setToolpathLines(lines: THREE.LineSegments | null): void {
    for (const child of [...this.toolpathGroup.children]) {
      if (child instanceof THREE.LineSegments) {
        // Dispose geometry only — the material is a shared module-level
        // singleton owned by toolpathLines.ts and must not be disposed here.
        child.geometry.dispose()
      }
    }
    this.toolpathGroup.clear()
    if (lines !== null) {
      this.toolpathGroup.add(lines)
    }
  }
}
