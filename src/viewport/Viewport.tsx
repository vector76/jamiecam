/**
 * Viewport — the 3-D canvas component.
 *
 * Mounts a Three.js SceneManager into a div, subscribes to the viewport
 * store for mesh data updates, and disposes the scene on unmount.
 */

import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import { SceneManager } from './scene'
import { buildModelMesh } from './modelMesh'
import { createAxisTriad } from './controls'
import { useViewportStore } from '../store/viewportStore'
import type { DisplayMode } from '../store/viewportStore'
import { SimulationControls } from '../components/simulation/SimulationControls'
import { useSimulationLoop } from './useSimulationLoop'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Ruler, Grab, Eraser } from 'lucide-react'

interface ViewportProps {
  className?: string
}

export function Viewport({ className }: ViewportProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const mgrRef = useRef<SceneManager | null>(null)
  const modelGroupRef = useRef<THREE.Group | null>(null)
  const highlightMeshRef = useRef<THREE.Mesh | null>(null)

  useSimulationLoop(mgrRef)

  const meshData = useViewportStore((state) => state.meshData)
  const simulationMeshData = useViewportStore((state) => state.simulationMeshData)
  const displayedMesh = simulationMeshData ?? meshData
  const toolpathGeometry = useViewportStore((state) => state.toolpathGeometry)
  const selectionMode = useViewportStore((state) => state.selectionMode)
  const hoveredFaceIdx = useViewportStore((state) => state.hoveredFaceIdx)
  const selectedFaceFingerprints = useViewportStore((state) => state.selectedFaceFingerprints)
  const faceDescriptors = useViewportStore((state) => state.faceDescriptors)
  const projectionMode = useViewportStore((state) => state.projectionMode)
  const setProjectionMode = useViewportStore((state) => state.setProjectionMode)
  const displayMode = useViewportStore((state) => state.displayMode)
  const setDisplayMode = useViewportStore((state) => state.setDisplayMode)
  const setHoveredFaceIdx = useViewportStore((state) => state.setHoveredFaceIdx)
  const toggleFaceSelection = useViewportStore((state) => state.toggleFaceSelection)
  const measurementMode = useViewportStore((state) => state.measurementMode)
  const setMeasurementMode = useViewportStore((state) => state.setMeasurementMode)
  const setSelectionMode = useViewportStore((state) => state.setSelectionMode)
  const clearMeasurements = useViewportStore((state) => state.clearMeasurements)
  const measurements = useViewportStore((state) => state.measurements)
  const measurementPoints = useViewportStore((state) => state.measurementPoints)

  // Mutable refs to avoid stale closures in mount-registered event handlers.
  const selectionModeRef = useRef(selectionMode)
  const projectionModeRef = useRef(projectionMode)
  const hoveredFaceIdxRef = useRef(hoveredFaceIdx)
  const faceDescriptorsRef = useRef(faceDescriptors)
  const meshDataRef = useRef(displayedMesh)

  useEffect(() => { selectionModeRef.current = selectionMode }, [selectionMode])
  useEffect(() => { projectionModeRef.current = projectionMode }, [projectionMode])
  useEffect(() => { hoveredFaceIdxRef.current = hoveredFaceIdx }, [hoveredFaceIdx])
  useEffect(() => { faceDescriptorsRef.current = faceDescriptors }, [faceDescriptors])
  useEffect(() => { meshDataRef.current = displayedMesh }, [displayedMesh])

  // ── Mount / unmount ────────────────────────────────────────────────────────
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const canvas = document.createElement('canvas')
    container.appendChild(canvas)

    const mgr = new SceneManager(canvas, container)
    mgrRef.current = mgr

    const triad = createAxisTriad()
    mgr.scene.add(triad)

    const raycaster = new THREE.Raycaster()
    const pointer = new THREE.Vector2()

    function onMouseMove(event: MouseEvent) {
      if (!selectionModeRef.current) return
      if (!container) return
      const md = meshDataRef.current
      if (!md) return
      const currentMgr = mgrRef.current
      if (!currentMgr) return
      const group = modelGroupRef.current
      if (!group) return

      const rect = container.getBoundingClientRect()
      pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1
      pointer.y = -((event.clientY - rect.top) / rect.height) * 2 + 1

      raycaster.setFromCamera(pointer, currentMgr.camera)
      const meshes: THREE.Object3D[] = []
      group.traverse((obj) => { if (obj instanceof THREE.Mesh) meshes.push(obj) })
      const intersects = raycaster.intersectObjects(meshes, false)

      if (intersects.length > 0) {
        const hit = intersects[0]
        const triIdx = hit.faceIndex!
        const fg = md.faceGroups
        let foundFaceIdx: number | null = null
        for (let i = 0; i < fg.length; i++) {
          if (triIdx >= fg[i].startTriangle && triIdx < fg[i].startTriangle + fg[i].triangleCount) {
            foundFaceIdx = i
            break
          }
        }
        setHoveredFaceIdx(foundFaceIdx)
      } else {
        setHoveredFaceIdx(null)
      }
    }

    function onClick() {
      if (!selectionModeRef.current) return
      const hovered = hoveredFaceIdxRef.current
      if (hovered === null) return
      const desc = faceDescriptorsRef.current.find((d) => d.faceIdx === hovered)
      if (desc) {
        toggleFaceSelection(desc.fingerprint)
      }
    }

    function onPointerDown(e: PointerEvent) {
      const mode = useViewportStore.getState().measurementMode
      if (mode === 'off') return
      const currentMgr = mgrRef.current
      if (!currentMgr) return
      const rect = canvas.getBoundingClientRect()
      const x = (e.clientX - rect.left) / rect.width * 2 - 1
      const y = -((e.clientY - rect.top) / rect.height) * 2 + 1
      const rc = new THREE.Raycaster()
      rc.setFromCamera(new THREE.Vector2(x, y), currentMgr.getActiveCamera())
      const mesh = currentMgr.getModelMesh()
      const hits = rc.intersectObjects(mesh ? [mesh] : [], false)
      if (hits.length > 0) {
        const hit = hits[0]
        useViewportStore.getState().addMeasurementPoint([hit.point.x, hit.point.y, hit.point.z])
      }
      e.stopPropagation()
    }

    function onKeyDown(event: KeyboardEvent) {
      const tag = document.activeElement?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
      switch (event.key.toUpperCase()) {
        case 'T': mgrRef.current?.snapTop(); break
        case 'F': mgrRef.current?.snapFront(); break
        case 'R': mgrRef.current?.snapRight(); break
        case 'I': mgrRef.current?.snapIsometric(); break
        case 'P': setProjectionMode(projectionModeRef.current === 'perspective' ? 'orthographic' : 'perspective'); break
        case 'ESCAPE': {
          const currentMode = useViewportStore.getState().measurementMode
          if (currentMode !== 'off') setMeasurementMode('off')
          break
        }
      }
    }

    canvas.addEventListener('pointerdown', onPointerDown)
    container.addEventListener('mousemove', onMouseMove)
    container.addEventListener('click', onClick)
    window.addEventListener('keydown', onKeyDown)

    return () => {
      canvas.removeEventListener('pointerdown', onPointerDown)
      container.removeEventListener('mousemove', onMouseMove)
      container.removeEventListener('click', onClick)
      window.removeEventListener('keydown', onKeyDown)
      mgrRef.current = null
      mgr.dispose()
      if (container.contains(canvas)) container.removeChild(canvas)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // ── Toolpath update ────────────────────────────────────────────────────────
  useEffect(() => {
    const mgr = mgrRef.current
    if (!mgr) return
    mgr.setToolpathData(toolpathGeometry)
  }, [toolpathGeometry])

  // ── Mesh update ────────────────────────────────────────────────────────────
  useEffect(() => {
    const mgr = mgrRef.current
    if (!mgr) return

    // Remove previous model group from the scene.
    if (modelGroupRef.current) {
      mgr.scene.remove(modelGroupRef.current)
      modelGroupRef.current = null
      mgr.setModelMesh(null)
    }

    if (displayedMesh) {
      const { mesh, boundingSphere } = buildModelMesh(displayedMesh)
      const group = new THREE.Group()
      group.name = 'ModelGroup'
      group.add(mesh)
      mgr.scene.add(group)
      modelGroupRef.current = group
      mgr.setModelMesh(mesh)
      mgr.setDisplayMode(displayMode)
      mgr.frameModel(boundingSphere)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [displayedMesh])

  // ── Selection mode effect ──────────────────────────────────────────────────
  useEffect(() => {
    const mgr = mgrRef.current

    // Always tear down any existing overlay first (handles meshData swap during
    // selection mode, or exiting selection mode).  Remove shared position/normal
    // attributes before disposing so their GPU buffers aren't freed — they're
    // still owned by the model mesh geometry.
    if (highlightMeshRef.current) {
      const geo = highlightMeshRef.current.geometry
      geo.deleteAttribute('position')
      geo.deleteAttribute('normal')
      geo.dispose()
      ;(highlightMeshRef.current.material as THREE.Material).dispose()
      mgr?.scene.remove(highlightMeshRef.current)
      highlightMeshRef.current = null
    }

    if (selectionMode && displayedMesh) {
      mgr?.setOrbitEnabled(false)
      const modelMesh = modelGroupRef.current?.children.find(
        (c) => c instanceof THREE.Mesh,
      ) as THREE.Mesh | undefined
      if (modelMesh && mgr) {
        const overlayGeo = new THREE.BufferGeometry()
        overlayGeo.setAttribute('position', modelMesh.geometry.getAttribute('position'))
        overlayGeo.setAttribute('normal', modelMesh.geometry.getAttribute('normal'))
        const mat = new THREE.MeshBasicMaterial({
          transparent: true,
          opacity: 0.4,
          depthTest: false,
          vertexColors: true,
        })
        const overlay = new THREE.Mesh(overlayGeo, mat)
        overlay.renderOrder = 1
        mgr.scene.add(overlay)
        highlightMeshRef.current = overlay
      }
    } else {
      mgr?.setOrbitEnabled(true)
      setHoveredFaceIdx(null)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectionMode, displayedMesh])

  // ── Projection mode sync ───────────────────────────────────────────────────
  useEffect(() => {
    const mgr = mgrRef.current
    if (mgr && mgr.getProjectionMode() !== projectionMode) {
      mgr.toggleProjection()
    }
  }, [projectionMode])

  // ── Display mode sync ──────────────────────────────────────────────────────
  useEffect(() => {
    mgrRef.current?.setDisplayMode(displayMode)
  }, [displayMode])

  // ── Measurement labels sync ────────────────────────────────────────────────
  useEffect(() => {
    mgrRef.current?.updateMeasurementLabels(measurements)
  }, [measurements])

  // ── Measurement points sync ────────────────────────────────────────────────
  useEffect(() => {
    mgrRef.current?.updateMeasurementPoints(measurementPoints)
  }, [measurementPoints])

  // ── Measurement / selection mutual exclusivity ─────────────────────────────
  useEffect(() => {
    if (measurementMode !== 'off') setSelectionMode(false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [measurementMode])

  useEffect(() => {
    if (selectionMode) setMeasurementMode('off')
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectionMode])

  // ── Highlight rebuild effect ───────────────────────────────────────────────
  useEffect(() => {
    const overlay = highlightMeshRef.current
    if (!selectionMode || !overlay || !meshData) return

    const { faceGroups, indices } = meshData
    const vertexCount = meshData.vertices.length / 3
    const hoveredGroup = hoveredFaceIdx !== null ? faceGroups[hoveredFaceIdx] : null

    const selectedFaceIndices = new Set(
      faceDescriptors
        .filter((d) => selectedFaceFingerprints.includes(d.fingerprint))
        .map((d) => d.faceIdx),
    )

    // Build index array: hovered face triangles first, then selected.
    const indexParts: number[] = []
    if (hoveredGroup) {
      for (let t = hoveredGroup.startTriangle; t < hoveredGroup.startTriangle + hoveredGroup.triangleCount; t++) {
        indexParts.push(indices[t * 3], indices[t * 3 + 1], indices[t * 3 + 2])
      }
    }
    for (const fi of selectedFaceIndices) {
      if (fi === hoveredFaceIdx) continue
      const fg = faceGroups[fi]
      if (!fg) continue
      for (let t = fg.startTriangle; t < fg.startTriangle + fg.triangleCount; t++) {
        indexParts.push(indices[t * 3], indices[t * 3 + 1], indices[t * 3 + 2])
      }
    }

    // Build per-vertex color array for the full geometry.
    const colors = new Float32Array(vertexCount * 3)
    // Selected faces: blue (0, 0.4, 1) — set first so hovered can override.
    for (const fi of selectedFaceIndices) {
      const fg = faceGroups[fi]
      if (!fg) continue
      for (let t = fg.startTriangle; t < fg.startTriangle + fg.triangleCount; t++) {
        for (let k = 0; k < 3; k++) {
          const vi = indices[t * 3 + k]
          colors[vi * 3] = 0; colors[vi * 3 + 1] = 0.4; colors[vi * 3 + 2] = 1
        }
      }
    }
    // Hovered face: yellow (1, 1, 0) — overrides selected color on shared vertices.
    if (hoveredGroup) {
      for (let t = hoveredGroup.startTriangle; t < hoveredGroup.startTriangle + hoveredGroup.triangleCount; t++) {
        for (let k = 0; k < 3; k++) {
          const vi = indices[t * 3 + k]
          colors[vi * 3] = 1; colors[vi * 3 + 1] = 1; colors[vi * 3 + 2] = 0
        }
      }
    }

    overlay.geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(indexParts), 1))
    overlay.geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  }, [selectionMode, hoveredFaceIdx, selectedFaceFingerprints, faceDescriptors, meshData])

  return (
    <div className={cn('relative h-full w-full', className)}>
      <div ref={containerRef} className="h-full w-full" />
      <div className="absolute left-2 top-2 z-10 flex items-center gap-1">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => setProjectionMode(projectionMode === 'perspective' ? 'orthographic' : 'perspective')}
        >
          {projectionMode === 'perspective' ? 'Persp' : 'Ortho'}
        </Button>
        <select
          aria-label="Display mode"
          value={displayMode}
          onChange={(e) => setDisplayMode(e.target.value as DisplayMode)}
          className="h-7 rounded-sm border border-border bg-secondary px-1.5 text-xs text-secondary-foreground"
        >
          <option value="shaded">Shaded</option>
          <option value="shaded-edges">Shaded + Edges</option>
          <option value="wireframe">Wireframe</option>
          <option value="transparent">Transparent</option>
        </select>
        <Button
          variant={measurementMode === 'distance' ? 'default' : 'secondary'}
          size="sm"
          title="Distance measurement"
          onClick={() => setMeasurementMode('distance')}
        >
          <Ruler className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant={measurementMode === 'angle' ? 'default' : 'secondary'}
          size="sm"
          title="Angle measurement"
          onClick={() => setMeasurementMode('angle')}
        >
          <Grab className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="secondary"
          size="sm"
          title="Clear measurements"
          onClick={() => { clearMeasurements(); setMeasurementMode('off') }}
        >
          <Eraser className="h-3.5 w-3.5" />
        </Button>
      </div>
      <SimulationControls />
    </div>
  )
}
