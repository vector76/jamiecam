import * as THREE from 'three'
import type { SimPoint } from './simulationPoints'

const HIGHLIGHT_MATERIAL = new THREE.MeshStandardMaterial({
  color: 0xffaa00,
  emissive: 0xffaa00,
  emissiveIntensity: 0.8,
})

export function createHighlightIndicator(): THREE.Mesh {
  return new THREE.Mesh(new THREE.SphereGeometry(0.5, 8, 8), HIGHLIGHT_MATERIAL)
}

export function positionHighlight(indicator: THREE.Mesh, point: SimPoint): void {
  indicator.position.set(point.x, point.y, point.z)
}
