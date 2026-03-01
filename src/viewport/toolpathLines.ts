/**
 * Builds a Three.js LineSegments object from toolpath line geometry data.
 *
 * Each pair of consecutive vertices defines a line segment. Positions and
 * colours are passed as interleaved flat arrays matching the backend layout.
 */

import * as THREE from 'three'
import type { LineGeometryData } from '../api/types'

/** Shared material for all toolpath line segments — per-vertex colour driven. */
const TOOLPATH_MATERIAL = new THREE.LineBasicMaterial({ vertexColors: true })

/**
 * Build a `THREE.LineSegments` from `LineGeometryData`.
 *
 * Returns null if `data` is null or contains no positions.
 */
export function buildToolpathLines(data: LineGeometryData | null): THREE.LineSegments | null {
  if (data === null || data.positions.length === 0) {
    return null
  }

  const geometry = new THREE.BufferGeometry()

  geometry.setAttribute(
    'position',
    new THREE.BufferAttribute(new Float32Array(data.positions), 3),
  )
  geometry.setAttribute(
    'color',
    new THREE.BufferAttribute(new Float32Array(data.colours), 3),
  )

  return new THREE.LineSegments(geometry, TOOLPATH_MATERIAL)
}
