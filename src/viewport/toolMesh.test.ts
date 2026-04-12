import * as THREE from 'three'
import { createToolMesh } from './toolMesh'

describe('createToolMesh', () => {
  it('returns a THREE.Group with at least one child', () => {
    const mesh = createToolMesh(6, 10)
    expect(mesh).toBeInstanceOf(THREE.Group)
    expect(mesh.children.length).toBeGreaterThanOrEqual(1)
  })

  it('has non-zero bounding box size', () => {
    const mesh = createToolMesh(6, 10)
    const box = new THREE.Box3().setFromObject(mesh)
    const size = new THREE.Vector3()
    box.getSize(size)
    expect(size.x).toBeGreaterThan(0)
    expect(size.y).toBeGreaterThan(0)
    expect(size.z).toBeGreaterThan(0)
  })

  it('is oriented along Z axis (Z-up scene convention)', () => {
    const diameter = 6
    const cuttingLength = 10
    const mesh = createToolMesh(diameter, cuttingLength)
    const box = new THREE.Box3().setFromObject(mesh)
    const size = new THREE.Vector3()
    box.getSize(size)
    // The tool should be tallest along Z, not Y
    expect(size.z).toBeGreaterThan(size.x)
    expect(size.z).toBeGreaterThan(size.y)
  })

  it('has tool tip at group origin (z ≈ 0) and extends upward', () => {
    const mesh = createToolMesh(6, 10)
    const box = new THREE.Box3().setFromObject(mesh)
    // Tip should be near z = 0 (the group position)
    expect(box.min.z).toBeCloseTo(0, 0)
    // Tool body extends upward
    expect(box.max.z).toBeGreaterThan(0)
  })
})
