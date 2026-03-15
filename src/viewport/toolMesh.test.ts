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
})
