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

export function Viewport() {
  const containerRef = useRef<HTMLDivElement>(null)
  const mgrRef = useRef<SceneManager | null>(null)
  const modelGroupRef = useRef<THREE.Group | null>(null)

  const meshData = useViewportStore((state) => state.meshData)

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

    return () => {
      mgrRef.current = null
      mgr.dispose()
      if (container.contains(canvas)) container.removeChild(canvas)
    }
  }, [])

  // ── Mesh update ────────────────────────────────────────────────────────────
  useEffect(() => {
    const mgr = mgrRef.current
    if (!mgr) return

    // Remove previous model group from the scene.
    if (modelGroupRef.current) {
      mgr.scene.remove(modelGroupRef.current)
      modelGroupRef.current = null
    }

    if (meshData) {
      const { mesh, boundingSphere } = buildModelMesh(meshData)
      const group = new THREE.Group()
      group.name = 'ModelGroup'
      group.add(mesh)
      mgr.scene.add(group)
      modelGroupRef.current = group
      mgr.frameModel(boundingSphere)
    }
  }, [meshData])

  return <div ref={containerRef} style={{ width: '100%', height: '100%' }} />
}
