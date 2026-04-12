import * as THREE from 'three'
import type { InterpolatedPoint } from './simulationPoints'

const FLUTE_MATERIAL = new THREE.MeshStandardMaterial({ color: 0x4488ff, roughness: 0.3 })
const SHANK_MATERIAL = new THREE.MeshStandardMaterial({ color: 0x3366cc, roughness: 0.3 })

export function createToolMesh(diameter: number, cuttingLength: number): THREE.Group {
  const group = new THREE.Group()

  const flute = new THREE.Mesh(
    new THREE.CylinderGeometry(diameter / 2, diameter / 2, cuttingLength, 16),
    FLUTE_MATERIAL,
  )
  flute.position.y = cuttingLength / 2
  group.add(flute)

  const shank = new THREE.Mesh(
    new THREE.CylinderGeometry(diameter / 4, diameter / 4, cuttingLength * 2, 16),
    SHANK_MATERIAL,
  )
  shank.position.y = 2 * cuttingLength
  group.add(shank)

  // CylinderGeometry is Y-aligned; rotate so the tool points along Z (Z-up scene)
  group.rotation.x = Math.PI / 2

  return group
}

export function positionToolMesh(mesh: THREE.Group, point: InterpolatedPoint): void {
  mesh.position.set(point.x, point.y, point.z)
}
